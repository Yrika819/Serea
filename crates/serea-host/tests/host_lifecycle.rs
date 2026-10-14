#![cfg(feature = "synthetic")]

use std::io::{BufRead, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

fn monotonic_now() -> Instant {
    // Test-harness deadlines only; this does not supply application time.
    #[allow(clippy::disallowed_methods)]
    Instant::now()
}

fn start(directory: &Path) -> Child {
    Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["run", "--synthetic", "--data-dir"])
        .arg(directory)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch synthetic host")
}

fn wait_ready(child: &mut Child) -> BufReader<std::process::ChildStdout> {
    let stdout = child.stdout.take().expect("captured stdout");
    let mut reader = BufReader::new(stdout);
    let mut line = String::new();
    let started = monotonic_now();
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "host startup deadline"
        );
        line.clear();
        let bytes = reader.read_line(&mut line).expect("host output");
        assert_ne!(
            bytes,
            0,
            "host exited before readiness: {:?}; stderr={}",
            child.try_wait(),
            {
                use std::io::Read;
                let mut stderr = String::new();
                if let Some(mut stream) = child.stderr.take() {
                    let _ = stream.read_to_string(&mut stderr);
                }
                stderr
            }
        );
        if line.contains("\"event\":\"host_ready\"") {
            return reader;
        }
    }
}

fn wait_scheduled_task(reader: &mut BufReader<std::process::ChildStdout>) {
    let started = monotonic_now();
    let mut line = String::new();
    loop {
        assert!(
            started.elapsed() < Duration::from_secs(30),
            "Host did not report scheduled task processing"
        );
        line.clear();
        let bytes = reader.read_line(&mut line).expect("read Host diagnostics");
        assert_ne!(bytes, 0, "Host exited before scheduled processing");
        if (line.contains("scheduler_recovery") || line.contains("scheduler_wake"))
            && line.contains("calendar_tasks_created_or_reused\":1")
        {
            return;
        }
    }
}

fn stop(directory: &Path, mut child: Child, _stdout: BufReader<std::process::ChildStdout>) {
    let status = Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["stop", "--synthetic", "--data-dir"])
        .arg(directory)
        .status()
        .expect("request graceful stop");
    assert!(status.success());
    let deadline = monotonic_now() + Duration::from_secs(15);
    loop {
        if let Some(status) = child.try_wait().expect("wait for host") {
            assert!(status.success());
            return;
        }
        assert!(monotonic_now() < deadline, "host shutdown deadline");
        std::thread::sleep(Duration::from_millis(25));
    }
}

#[test]
fn long_running_host_is_single_instance_and_restarts_from_durable_schema() {
    let directory = std::env::temp_dir().join(format!(
        "serea-host-lifecycle-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create disposable data directory");

    let mut first = start(&directory);
    let first_stdout = wait_ready(&mut first);
    let second = Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["run", "--synthetic", "--data-dir"])
        .arg(&directory)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("probe single-instance protection");
    assert!(!second.success());
    stop(&directory, first, first_stdout);
    assert!(directory.join("host.lock").exists());

    let mut crashed_run = start(&directory);
    let _crashed_stdout = wait_ready(&mut crashed_run);
    crashed_run.kill().expect("terminate host process");
    assert!(!crashed_run.wait().expect("reap terminated host").success());

    let mut recovered_run = start(&directory);
    let recovered_stdout = wait_ready(&mut recovered_run);
    stop(&directory, recovered_run, recovered_stdout);
    assert!(directory.join("core.sqlite").exists());
    std::fs::remove_dir_all(&directory).expect("clean disposable data directory");
}

#[test]
fn explicit_model_smoke_runs_fake_success_and_failure_through_host_router() {
    let directory = std::env::temp_dir().join(format!(
        "serea-host-model-smoke-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create disposable model data directory");
    for scenario in ["success", "failure"] {
        let scenario_dir = directory.join(scenario);
        std::fs::create_dir_all(&scenario_dir).expect("create scenario database directory");
        let output = Command::new(env!("CARGO_BIN_EXE_serea-host"))
            .args(["model-smoke", "--synthetic", "--data-dir"])
            .arg(&scenario_dir)
            .args(["--scenario", scenario])
            .output()
            .expect("run actual synthetic Host model command");
        assert!(
            output.status.success(),
            "synthetic model Host {scenario} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let expected_event = if scenario == "success" {
            "synthetic_model_completed"
        } else {
            "synthetic_model_failed"
        };
        assert!(String::from_utf8_lossy(&output.stdout).contains(expected_event));

        let clock = serea_testkit::TestClock::at_epoch();
        let store = serea_storage::Store::open(&scenario_dir.join("core.sqlite"), &clock)
            .expect("reopen durable model database");
        assert!(
            store
                .list_unfinished_model_call_attempts()
                .expect("inspect durable model attempts")
                .is_empty()
        );
        let audit = store
            .replay_events(None, None, 256)
            .expect("inspect durable model audit");
        let kinds: Vec<_> = audit
            .items
            .iter()
            .filter_map(|item| match item {
                serea_storage::ReplayItem::Event { event } => Some(event.kind),
                _ => None,
            })
            .collect();
        assert!(kinds.contains(&serea_protocol::EventKind::ModelCalled));
        assert!(kinds.contains(&if scenario == "success" {
            serea_protocol::EventKind::ModelCompleted
        } else {
            serea_protocol::EventKind::ModelFailed
        }));
        store
            .checkpoint_for_close()
            .expect("close reopened database cleanly");
        // Windows does not allow deletion while SQLite still owns a file handle.
        drop(store);
    }
    std::fs::remove_dir_all(&directory).expect("clean disposable model directory");
}

#[test]
fn compiled_host_creates_one_durable_task_for_a_due_calendar_occurrence() {
    let directory = std::env::temp_dir().join(format!(
        "serea-host-scheduler-e2e-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create disposable Scheduler data directory");
    let seeded = Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["scheduler-seed", "--synthetic", "--data-dir"])
        .arg(&directory)
        .output()
        .expect("seed synthetic schedule through Scheduler API");
    assert!(
        seeded.status.success(),
        "schedule seed failed: stdout={} stderr={}",
        String::from_utf8_lossy(&seeded.stdout),
        String::from_utf8_lossy(&seeded.stderr)
    );

    let mut first = start(&directory);
    let mut output = wait_ready(&mut first);
    wait_scheduled_task(&mut output);
    stop(&directory, first, output);

    let clock = serea_testkit::TestClock::at_epoch();
    let store = serea_storage::Store::open(&directory.join("core.sqlite"), &clock)
        .expect("inspect durable scheduled task database");
    let audit = store
        .replay_events(None, None, 256)
        .expect("replay Scheduler audit");
    let events: Vec<_> = audit
        .items
        .iter()
        .filter_map(|item| match item {
            serea_storage::ReplayItem::Event { event } => Some(event.as_ref()),
            _ => None,
        })
        .collect();
    let task_events: Vec<_> = events
        .iter()
        .copied()
        .filter(|event| event.kind == serea_protocol::EventKind::TaskCreated)
        .collect();
    let scheduled_events: Vec<_> = events
        .iter()
        .copied()
        .filter(|event| event.kind == serea_protocol::EventKind::ScheduleTaskCreated)
        .collect();
    assert_eq!(task_events.len(), 1);
    assert_eq!(scheduled_events.len(), 1);
    assert!(task_events[0].seq.get() < scheduled_events[0].seq.get());
    let task_id = task_events[0]
        .correlation_id
        .clone()
        .expect("TaskCreated correlation identity");
    assert_eq!(scheduled_events[0].correlation_id.as_ref(), Some(&task_id));
    assert_eq!(
        scheduled_events[0]
            .payload
            .get("task_id")
            .and_then(serde_json::Value::as_str),
        Some(task_id.as_str())
    );
    assert_eq!(
        scheduled_events[0]
            .payload
            .get("schedule_id")
            .and_then(serde_json::Value::as_str),
        Some("sch_00000000000000000000000041")
    );
    let occurrence_key = scheduled_events[0]
        .payload
        .get("occurrence_key")
        .and_then(serde_json::Value::as_str)
        .expect("durable occurrence identity");
    let task = store.load_task(&task_id).expect("load scheduled task");
    assert_eq!(task.task.kind, serea_protocol::TaskKind::Scheduled);
    assert_eq!(task.task.origin.kind.as_str(), "SCHEDULED");
    let provenance = store
        .schedule_task_provenance(&task_id)
        .expect("read occurrence provenance")
        .expect("scheduled task provenance");
    assert_eq!(provenance.occurrence_key, occurrence_key);
    let generation = store
        .get_task_registry_generation(&task_id)
        .expect("read Task registry generation")
        .expect("Scheduler contract pins generation");
    assert!(generation > 0);
    assert_eq!(
        store
            .current_registry_generation()
            .expect("read active synthetic generation")
            .expect("synthetic generation is active")
            .generation_id(),
        generation
    );

    // A fresh Host startup repeats recovery/wakeup against the same database.
    let mut restarted = start(&directory);
    let restarted_output = wait_ready(&mut restarted);
    stop(&directory, restarted, restarted_output);
    let audit_after_restart = store
        .replay_events(None, None, 256)
        .expect("replay audit after Host restart");
    let task_created_after_restart = audit_after_restart
        .items
        .iter()
        .filter(|item| {
            matches!(
                item,
                serea_storage::ReplayItem::Event { event }
                    if event.kind == serea_protocol::EventKind::TaskCreated
            )
        })
        .count();
    assert_eq!(task_created_after_restart, 1);
    store
        .checkpoint_for_close()
        .expect("checkpoint Scheduler database");
    drop(store);
    std::fs::remove_dir_all(&directory).expect("clean Scheduler data directory");
}
