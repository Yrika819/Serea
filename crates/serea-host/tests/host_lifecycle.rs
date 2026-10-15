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

#[cfg(feature = "synthetic-test-hooks")]
fn start_at_scheduler_barrier(directory: &Path, barrier_dir: &Path, phase: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["run", "--synthetic", "--data-dir"])
        .arg(directory)
        .env("SEREA_SCHEDULER_TEST_BARRIER", phase)
        .env("SEREA_SCHEDULER_TEST_BARRIER_DIR", barrier_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("launch Host at Scheduler crash barrier")
}

#[cfg(feature = "synthetic-test-hooks")]
fn wait_for_barrier(child: &mut Child, barrier_dir: &Path, phase: &str) {
    let marker = barrier_dir.join(format!("{phase}.reached"));
    let deadline = monotonic_now() + Duration::from_secs(30);
    while !marker.exists() {
        if let Some(status) = child.try_wait().expect("check barrier child") {
            panic!("Host exited before {phase} barrier: {status}");
        }
        assert!(monotonic_now() < deadline, "{phase} barrier deadline");
        std::thread::park_timeout(Duration::from_millis(5));
    }
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

#[cfg(unix)]
#[test]
fn unix_sigterm_requests_bounded_cooperative_host_shutdown() {
    let directory = std::env::temp_dir().join(format!(
        "serea-host-sigterm-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create disposable signal test directory");
    let mut child = start(&directory);
    let mut output = wait_ready(&mut child);
    let started = monotonic_now();
    let signal_status = Command::new("kill")
        .args(["-TERM", &child.id().to_string()])
        .status()
        .expect("send POSIX SIGTERM");
    assert!(signal_status.success());
    let deadline = monotonic_now() + Duration::from_secs(5);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll SIGTERM child") {
            break status;
        }
        assert!(monotonic_now() < deadline, "SIGTERM shutdown deadline");
        std::thread::park_timeout(Duration::from_millis(10));
    };
    assert!(status.success(), "Host did not shut down cooperatively");
    assert!(started.elapsed() < Duration::from_secs(5));
    let mut tail = String::new();
    std::io::Read::read_to_string(&mut output, &mut tail).expect("read shutdown diagnostics");
    assert!(tail.contains("\"event\":\"host_stopped\""));

    let clock = serea_testkit::TestClock::at_epoch();
    let store = serea_storage::Store::open(&directory.join("core.sqlite"), &clock)
        .expect("reopen SQLite after signal shutdown");
    assert_eq!(store.schema_version().expect("read schema version"), 8);
    store
        .checkpoint_for_close()
        .expect("checkpoint signal shutdown DB");
    drop(store);
    std::fs::remove_dir_all(&directory).expect("clean signal test directory");
}

#[cfg(windows)]
#[test]
fn windows_console_ctrl_c_and_ctrl_break_shutdown_reopens_sqlite() {
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    use windows_sys::Win32::System::Console::{
        AttachConsole, CTRL_BREAK_EVENT, CTRL_C_EVENT, FreeConsole, GenerateConsoleCtrlEvent,
    };

    // CTRL_C_EVENT cannot be addressed to one process group. Give the Host
    // its own console so group 0 targets only this disposable child. For
    // CTRL_BREAK_EVENT, use a dedicated process group and address its PID.
    for (label, event, new_group) in [
        ("ctrl-c", CTRL_C_EVENT, false),
        ("ctrl-break", CTRL_BREAK_EVENT, true),
    ] {
        const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
        const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
        let directory = std::env::temp_dir().join(format!(
            "serea-host-windows-{label}-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).expect("create disposable Windows Host directory");
        let flags = CREATE_NEW_CONSOLE
            | if new_group {
                CREATE_NEW_PROCESS_GROUP
            } else {
                0
            };
        let mut child = Command::new(env!("CARGO_BIN_EXE_serea-host"))
            .args(["run", "--synthetic", "--data-dir"])
            .arg(&directory)
            .creation_flags(flags)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("start Host with isolated console");
        let mut output = wait_ready(&mut child);

        // Console control events are broadcast within a console, so attach
        // temporarily to the isolated child console. Restore the parent's
        // console association after delivery when one exists.
        unsafe {
            let _ = FreeConsole();
            assert_ne!(AttachConsole(child.id()), 0, "attach to Host console");
            let group = if new_group { child.id() } else { 0 };
            assert_ne!(
                GenerateConsoleCtrlEvent(event, group),
                0,
                "deliver real Windows {label} control event"
            );
            let _ = FreeConsole();
            // ATTACH_PARENT_PROCESS = (DWORD)-1. A service runner may have no
            // console, so restoration is best-effort and not a test condition.
            let _ = AttachConsole(u32::MAX);
        }

        let deadline = monotonic_now() + Duration::from_secs(15);
        let status = loop {
            if let Some(status) = child.try_wait().expect("poll Windows Host shutdown") {
                break status;
            }
            assert!(monotonic_now() < deadline, "bounded {label} shutdown");
            std::thread::park_timeout(Duration::from_millis(20));
        };
        assert!(
            status.success(),
            "{label} should request cooperative shutdown"
        );
        let mut tail = String::new();
        output
            .read_to_string(&mut tail)
            .expect("read final Windows lifecycle diagnostics");
        assert!(tail.contains("\"event\":\"host_stopped\""));

        let clock = serea_testkit::TestClock::at_epoch();
        let store = serea_storage::Store::open(&directory.join("core.sqlite"), &clock)
            .expect("reopen SQLite after Windows console event");
        assert_eq!(store.schema_version().unwrap(), 8);
        store
            .checkpoint_for_close()
            .expect("checkpoint Windows Host DB");
        drop(store);
        std::fs::remove_dir_all(&directory).expect("clean Windows console test directory");
    }
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
        let expected_event = if scenario == "success" {
            "synthetic_model_completed"
        } else {
            "synthetic_model_failed"
        };
        let repetitions = if scenario == "success" { 2 } else { 1 };
        for _ in 0..repetitions {
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
            assert!(String::from_utf8_lossy(&output.stdout).contains(expected_event));
        }

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
        assert_eq!(
            kinds
                .iter()
                .filter(|kind| **kind == serea_protocol::EventKind::ModelCalled)
                .count(),
            repetitions,
            "separate Host process lifetimes must persist distinct RequestIds"
        );
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

#[cfg(feature = "synthetic-test-hooks")]
#[test]
fn actual_host_model_policy_and_accounting_scenarios_survive_database_reopen() {
    use serea_storage::{ModelAttemptState, ReplayItem, Store};

    for scenario in [
        "cancelled",
        "cancelled-after-intent",
        "expired",
        "identity-mismatch",
        "usage-bound",
        "spend-bound",
        "paid-success",
        "timeout",
    ] {
        let directory = std::env::temp_dir().join(format!(
            "serea-host-model-policy-{scenario}-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).expect("create disposable scenario directory");
        let output = Command::new(env!("CARGO_BIN_EXE_serea-host"))
            .args(["model-smoke", "--synthetic", "--data-dir"])
            .arg(&directory)
            .args(["--scenario", scenario])
            .output()
            .expect("run compiled synthetic Host scenario");
        assert!(
            output.status.success(),
            "Host {scenario} scenario failed: stdout={} stderr={}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );

        let clock = serea_testkit::TestClock::at_epoch();
        let store = Store::open(&directory.join("core.sqlite"), &clock)
            .expect("reopen Host scenario database");
        let events = store
            .replay_events(None, None, 128)
            .expect("read persisted model scenario events")
            .items
            .into_iter()
            .filter_map(|item| match item {
                ReplayItem::Event { event } => Some(*event),
                _ => None,
            })
            .collect::<Vec<_>>();
        let called = events
            .iter()
            .filter(|event| event.kind == serea_protocol::EventKind::ModelCalled)
            .collect::<Vec<_>>();
        let request_id = called
            .first()
            .and_then(|event| event.payload.get("request_id"))
            .and_then(serde_json::Value::as_str)
            .map(|value| serea_protocol::RequestId::new(value.to_owned()).unwrap());

        match scenario {
            "cancelled" | "expired" | "spend-bound" => {
                assert!(called.is_empty(), "{scenario} must refuse before intent");
                assert!(
                    store
                        .list_unfinished_model_call_attempts()
                        .unwrap()
                        .is_empty()
                );
                if scenario == "spend-bound" {
                    let bound = events
                        .iter()
                        .find(|event| event.kind == serea_protocol::EventKind::BoundExceeded)
                        .expect("daily spend refusal audit");
                    assert_eq!(bound.payload["limit"], 1);
                    assert!(bound.payload["observed"].as_u64().unwrap() > 1);
                }
            }
            "cancelled-after-intent" => {
                assert_eq!(called.len(), 1);
                let id = request_id.clone().expect("intent has durable RequestId");
                let attempt = store.get_model_call_attempt(&id).unwrap().unwrap();
                assert_eq!(attempt.state, ModelAttemptState::Failed);
                assert!(attempt.reserved_cost_usd_micros.get() > 0);
                assert_eq!(attempt.actual_cost_usd_micros.unwrap().get(), 0);
                assert!(store.model_usage_for_request(&id).unwrap().is_none());
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| event.kind == serea_protocol::EventKind::ModelFailed)
                        .count(),
                    1
                );
            }
            "identity-mismatch" | "usage-bound" => {
                assert_eq!(called.len(), 1);
                let id = request_id.clone().expect("intent has durable RequestId");
                let attempt = store.get_model_call_attempt(&id).unwrap().unwrap();
                assert_eq!(attempt.state, ModelAttemptState::Failed);
                assert_eq!(attempt.reserved_cost_usd_micros.get(), 0);
                assert_eq!(attempt.actual_cost_usd_micros, None);
                assert!(store.model_usage_for_request(&id).unwrap().is_none());
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| event.kind == serea_protocol::EventKind::ModelFailed)
                        .count(),
                    1
                );
            }
            "paid-success" => {
                assert_eq!(called.len(), 1);
                let id = request_id.clone().expect("intent has durable RequestId");
                let attempt = store.get_model_call_attempt(&id).unwrap().unwrap();
                assert_eq!(attempt.state, ModelAttemptState::Completed);
                assert_eq!(attempt.reserved_cost_usd_micros.get(), 131_080);
                assert_eq!(attempt.actual_cost_usd_micros.unwrap().get(), 2);
                let usage = store.model_usage_for_request(&id).unwrap().unwrap();
                assert_eq!(usage.input_tokens.get(), 1);
                assert_eq!(usage.output_tokens.get(), 1);
                assert_eq!(usage.cost_usd_micros.get(), 2);
            }
            "timeout" => {
                assert_eq!(called.len(), 1);
                let id = request_id.clone().expect("intent has durable RequestId");
                let attempt = store.get_model_call_attempt(&id).unwrap().unwrap();
                assert_eq!(attempt.state, ModelAttemptState::Ambiguous);
                assert_eq!(attempt.reserved_cost_usd_micros.get(), 131_080);
                assert_eq!(attempt.actual_cost_usd_micros, None);
                assert!(store.model_usage_for_request(&id).unwrap().is_none());
                assert_eq!(
                    events
                        .iter()
                        .filter(|event| event.kind == serea_protocol::EventKind::ModelFailed)
                        .count(),
                    1
                );
            }
            _ => unreachable!(),
        }
        if let Some(id) = request_id.as_ref() {
            let id_text = id.as_str();
            for event in events.iter().filter(|event| {
                matches!(
                    event.kind,
                    serea_protocol::EventKind::ModelFailed
                        | serea_protocol::EventKind::ModelCompleted
                )
            }) {
                assert_eq!(
                    event
                        .payload
                        .get("request_id")
                        .and_then(serde_json::Value::as_str),
                    Some(id_text),
                    "terminal event must preserve the intent RequestId"
                );
                let called_event = called[0];
                assert!(
                    called_event.seq < event.seq,
                    "audit order must be intent then outcome"
                );
            }
        }
        let called_count = called.len();
        let failed_count = events
            .iter()
            .filter(|event| event.kind == serea_protocol::EventKind::ModelFailed)
            .count();
        let completed_count = events
            .iter()
            .filter(|event| event.kind == serea_protocol::EventKind::ModelCompleted)
            .count();
        let original_request = request_id.clone();
        store
            .checkpoint_for_close()
            .expect("checkpoint policy scenario DB");
        drop(store);

        // A fresh normal Host performs startup reconciliation but must never
        // auto-dispatch or create a replacement RequestId for this probe.
        let mut restarted = start(&directory);
        let output = wait_ready(&mut restarted);
        stop(&directory, restarted, output);
        let reopened = Store::open(&directory.join("core.sqlite"), &clock)
            .expect("reopen policy DB after normal Host restart");
        let after_restart = reopened
            .replay_events(None, None, 128)
            .expect("replay events after normal Host restart");
        let after_called = after_restart
            .items
            .iter()
            .filter(|item| matches!(item, ReplayItem::Event { event } if event.kind == serea_protocol::EventKind::ModelCalled))
            .count();
        let after_failed = after_restart
            .items
            .iter()
            .filter(|item| matches!(item, ReplayItem::Event { event } if event.kind == serea_protocol::EventKind::ModelFailed))
            .count();
        let after_completed = after_restart
            .items
            .iter()
            .filter(|item| matches!(item, ReplayItem::Event { event } if event.kind == serea_protocol::EventKind::ModelCompleted))
            .count();
        assert_eq!(
            after_called, called_count,
            "{scenario} redispatched after restart"
        );
        assert_eq!(
            after_failed, failed_count,
            "{scenario} duplicated terminal failure"
        );
        assert_eq!(
            after_completed, completed_count,
            "{scenario} duplicated completion"
        );
        if let Some(id) = original_request {
            assert!(reopened.get_model_call_attempt(&id).unwrap().is_some());
        }
        reopened
            .checkpoint_for_close()
            .expect("checkpoint reopened DB");
        drop(reopened);
        std::fs::remove_dir_all(&directory).expect("clean model scenario directory");
    }
}

#[cfg(feature = "synthetic-test-hooks")]
#[test]
fn model_accounting_is_bound_to_a_task_created_by_the_scheduler_task_engine() {
    use serea_storage::{ReplayItem, Store};

    let directory = std::env::temp_dir().join(format!(
        "serea-host-task-model-accounting-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create disposable Task model directory");
    let seeded = Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["scheduler-seed", "--synthetic", "--data-dir"])
        .arg(&directory)
        .args(["--count", "3"])
        .output()
        .expect("seed schedule through the Scheduler API");
    assert!(seeded.status.success());

    let mut host = start(&directory);
    let output = wait_ready(&mut host);
    stop(&directory, host, output);

    let clock = serea_testkit::TestClock::at_epoch();
    let store = Store::open(&directory.join("core.sqlite"), &clock)
        .expect("open TaskEngine-created schedule database");
    let events = store
        .replay_events(None, None, 256)
        .expect("read TaskEngine audit")
        .items
        .into_iter()
        .filter_map(|item| match item {
            ReplayItem::Event { event } => Some(*event),
            _ => None,
        })
        .collect::<Vec<_>>();
    let mut task_ids = events
        .iter()
        .filter(|event| event.kind == serea_protocol::EventKind::TaskCreated)
        .map(|event| {
            event
                .correlation_id
                .clone()
                .expect("TaskCreated Task identity")
        })
        .collect::<Vec<_>>();
    task_ids.sort_by(|a, b| a.as_str().cmp(b.as_str()));
    assert_eq!(task_ids.len(), 3, "Scheduler/TaskEngine creates all Tasks");
    let paid_task_id = task_ids[0].clone();
    let bounded_task_id = task_ids[1].clone();
    let token_task_id = task_ids[2].clone();
    for task_id in [&paid_task_id, &bounded_task_id, &token_task_id] {
        assert_eq!(
            store.load_task(task_id).expect("load Task").task.kind,
            serea_protocol::TaskKind::Scheduled
        );
        assert_eq!(store.task_model_call_limit(task_id).unwrap(), 12);
    }

    // The explicit synthetic command accepts a Task id only so this opt-in
    // test path can exercise the Router's existing durable task accounting.
    // The id is the occurrence Task's TaskCreated correlation, never a fixture row.
    for index in 0..12 {
        let result = Command::new(env!("CARGO_BIN_EXE_serea-host"))
            .args(["model-smoke", "--synthetic", "--data-dir"])
            .arg(&directory)
            .args([
                "--scenario",
                "success",
                "--task-id",
                bounded_task_id.as_str(),
            ])
            .output()
            .expect("execute explicit fake Router call correlated to Task");
        assert!(
            result.status.success(),
            "Task model call {index} failed: stdout={} stderr={}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
    }

    let token_usage = Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["model-smoke", "--synthetic", "--data-dir"])
        .arg(&directory)
        .args([
            "--scenario",
            "token-usage",
            "--task-id",
            token_task_id.as_str(),
        ])
        .output()
        .expect("persist bounded synthetic token usage");
    assert!(
        token_usage.status.success(),
        "token usage dispatch failed: {}",
        String::from_utf8_lossy(&token_usage.stderr)
    );
    let token_bound = Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["model-smoke", "--synthetic", "--data-dir"])
        .arg(&directory)
        .args([
            "--scenario",
            "token-probe",
            "--task-id",
            token_task_id.as_str(),
        ])
        .output()
        .expect("probe Task token budget after threshold crossing");
    assert!(
        token_bound.status.success(),
        "token ceiling must refuse through Router: {}",
        String::from_utf8_lossy(&token_bound.stderr)
    );

    let paid_result = Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["model-smoke", "--synthetic", "--data-dir"])
        .arg(&directory)
        .args([
            "--scenario",
            "paid-success",
            "--task-id",
            paid_task_id.as_str(),
        ])
        .output()
        .expect("execute paid fake Router call correlated to Task");
    assert!(
        paid_result.status.success(),
        "paid Task model call failed: {}",
        String::from_utf8_lossy(&paid_result.stderr)
    );

    let bounded = Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["model-smoke", "--synthetic", "--data-dir"])
        .arg(&directory)
        .args([
            "--scenario",
            "budget-probe",
            "--task-id",
            bounded_task_id.as_str(),
        ])
        .output()
        .expect("probe TaskEngine model-call budget");
    assert!(
        bounded.status.success(),
        "exhausted Task budget must refuse through Router: {}",
        String::from_utf8_lossy(&bounded.stderr)
    );

    let reopened = Store::open(&directory.join("core.sqlite"), &clock)
        .expect("reopen Task-correlated model database");
    assert_eq!(
        reopened.task_model_call_count(&bounded_task_id).unwrap(),
        12
    );
    assert_eq!(
        reopened.task_model_turn_count(&bounded_task_id).unwrap(),
        12
    );
    assert_eq!(
        reopened
            .task_model_token_usage(&bounded_task_id)
            .unwrap()
            .get(),
        24
    );
    assert_eq!(reopened.task_model_call_count(&paid_task_id).unwrap(), 1);
    assert_eq!(
        reopened
            .task_model_token_usage(&paid_task_id)
            .unwrap()
            .get(),
        2
    );
    assert_eq!(reopened.task_model_call_count(&token_task_id).unwrap(), 1);
    assert_eq!(
        reopened
            .task_model_token_usage(&token_task_id)
            .unwrap()
            .get(),
        131_072
    );
    let final_events = reopened
        .replay_events(None, None, 256)
        .expect("inspect correlated Model audit")
        .items
        .into_iter()
        .filter_map(|item| match item {
            ReplayItem::Event { event } => Some(*event),
            _ => None,
        })
        .collect::<Vec<_>>();
    let called = final_events
        .iter()
        .filter(|event| event.kind == serea_protocol::EventKind::ModelCalled)
        .collect::<Vec<_>>();
    let completed = final_events
        .iter()
        .filter(|event| event.kind == serea_protocol::EventKind::ModelCompleted)
        .collect::<Vec<_>>();
    let bound = final_events
        .iter()
        .filter(|event| event.kind == serea_protocol::EventKind::ModelBudgetExhausted)
        .collect::<Vec<_>>();
    assert_eq!(called.len(), 14);
    assert_eq!(completed.len(), 14);
    assert_eq!(bound.len(), 1);
    let token_completion = completed
        .iter()
        .find(|event| event.correlation_id.as_ref() == Some(&token_task_id))
        .expect("token-threshold response has one durable ModelCompleted event");
    assert_eq!(token_completion.payload["input_tokens"], 131_071);
    assert_eq!(token_completion.payload["output_tokens"], 1);
    assert_eq!(bound[0].payload["limit"], 12);
    assert_eq!(bound[0].payload["observed"], 12);
    assert!(called.iter().all(|event| {
        event.correlation_id.as_ref() == Some(&paid_task_id)
            || event.correlation_id.as_ref() == Some(&bounded_task_id)
            || event.correlation_id.as_ref() == Some(&token_task_id)
    }));
    let request_ids = called
        .iter()
        .map(|event| event.payload["request_id"].as_str().unwrap())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(request_ids.len(), 14);
    let token_bound_audit = final_events
        .iter()
        .find(|event| {
            event.kind == serea_protocol::EventKind::BoundExceeded
                && event.correlation_id.as_ref() == Some(&token_task_id)
        })
        .expect("token-bound refusal is attributed to its Task");
    assert_eq!(
        token_bound_audit.payload["bound_name"],
        "max_task_total_tokens"
    );
    let paid = called
        .iter()
        .find(|event| {
            let id = event.payload["request_id"].as_str().unwrap();
            reopened
                .get_model_call_attempt(&serea_protocol::RequestId::new(id.to_owned()).unwrap())
                .unwrap()
                .is_some_and(|attempt| {
                    attempt
                        .actual_cost_usd_micros
                        .is_some_and(|cost| cost.get() > 0)
                })
        })
        .expect("one paid fake result has durable settlement");
    let paid_id =
        serea_protocol::RequestId::new(paid.payload["request_id"].as_str().unwrap().to_owned())
            .unwrap();
    let paid_attempt = reopened.get_model_call_attempt(&paid_id).unwrap().unwrap();
    assert_eq!(paid_attempt.task_id.as_ref(), Some(&paid_task_id));
    assert_eq!(paid_attempt.reserved_cost_usd_micros.get(), 131_080);
    assert_eq!(paid_attempt.actual_cost_usd_micros.unwrap().get(), 2);
    let usage = reopened.model_usage_for_request(&paid_id).unwrap().unwrap();
    assert_eq!(usage.input_tokens.get(), 1);
    assert_eq!(usage.output_tokens.get(), 1);
    assert_eq!(usage.cost_usd_micros.get(), 2);
    reopened.checkpoint_for_close().unwrap();
    drop(reopened);

    // Startup recovery is read-only for completed calls and does not duplicate
    // their terminal audit or settle usage a second time.
    let mut restarted = start(&directory);
    let output = wait_ready(&mut restarted);
    stop(&directory, restarted, output);
    let after_restart =
        Store::open(&directory.join("core.sqlite"), &clock).expect("reopen after Host startup");
    assert_eq!(
        after_restart
            .task_model_call_count(&bounded_task_id)
            .unwrap(),
        12
    );
    assert_eq!(
        after_restart
            .task_model_token_usage(&bounded_task_id)
            .unwrap()
            .get(),
        24
    );
    assert_eq!(
        after_restart.task_model_call_count(&paid_task_id).unwrap(),
        1
    );
    assert_eq!(
        after_restart.task_model_call_count(&token_task_id).unwrap(),
        1
    );
    assert_eq!(
        after_restart
            .task_model_token_usage(&paid_task_id)
            .unwrap()
            .get(),
        2
    );
    assert_eq!(
        after_restart
            .task_model_token_usage(&token_task_id)
            .unwrap()
            .get(),
        131_072
    );
    assert_eq!(
        after_restart
            .replay_events(None, None, 256)
            .unwrap()
            .items
            .iter()
            .filter(|item| matches!(item, ReplayItem::Event { event } if event.kind == serea_protocol::EventKind::ModelCompleted))
            .count(),
        14
    );
    after_restart.checkpoint_for_close().unwrap();
    drop(after_restart);
    std::fs::remove_dir_all(&directory).expect("clean Task model directory");
}

#[cfg(feature = "synthetic-test-hooks")]
#[test]
fn explicit_cancellation_while_fake_provider_is_pending_leaves_ambiguous_intent() {
    use serea_storage::{ModelAttemptState, ReplayItem, Store};

    let directory = std::env::temp_dir().join(format!(
        "serea-host-model-cancel-inflight-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    let barrier_dir = directory.join("barrier");
    std::fs::create_dir_all(&directory).expect("create cancellation directory");
    let mut child = Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["model-smoke", "--synthetic", "--data-dir"])
        .arg(&directory)
        .args([
            "--scenario",
            "cancel-during-provider",
            "--test-barrier",
            "provider",
            "--test-barrier-dir",
        ])
        .arg(&barrier_dir)
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch compiled Host fake cancellation scenario");
    wait_for_barrier(&mut child, &barrier_dir, "provider");

    let database = directory.join("core.sqlite");
    let clock = serea_testkit::TestClock::at_epoch();
    let store = Store::open(&database, &clock).expect("inspect live dispatch intent");
    let before_cancel = store
        .replay_events(None, None, 128)
        .expect("read in-flight intent audit")
        .items
        .into_iter()
        .filter_map(|item| match item {
            ReplayItem::Event { event } => Some(*event),
            _ => None,
        })
        .collect::<Vec<_>>();
    let called = before_cancel
        .iter()
        .find(|event| event.kind == serea_protocol::EventKind::ModelCalled)
        .expect("Router committed intent before entering fake provider");
    let request_id =
        serea_protocol::RequestId::new(called.payload["request_id"].as_str().unwrap().to_owned())
            .unwrap();
    let active = store.get_model_call_attempt(&request_id).unwrap().unwrap();
    assert_eq!(active.state, ModelAttemptState::DispatchIntent);
    let reservation = active.reserved_cost_usd_micros;
    assert!(reservation.get() > 0);
    assert_eq!(
        before_cancel
            .iter()
            .filter(|event| event.kind == serea_protocol::EventKind::ModelCalled)
            .count(),
        1
    );
    store.checkpoint_for_close().unwrap();
    drop(store);

    // This explicit test-control file is observed by the synthetic Host's
    // bounded execution pump, which drops the pending Router future. It is
    // separate from a deadline expiry and from killing the process.
    std::fs::write(barrier_dir.join("cancel.request"), b"cancel")
        .expect("request in-flight cancellation");
    let deadline = monotonic_now() + Duration::from_secs(15);
    let status = loop {
        if let Some(status) = child.try_wait().expect("poll cancelled Host") {
            break status;
        }
        assert!(
            monotonic_now() < deadline,
            "cancelled Host should finish boundedly"
        );
        std::thread::park_timeout(Duration::from_millis(10));
    };
    assert!(
        status.success(),
        "explicit cancellation recovery should complete"
    );
    let reopened = Store::open(&database, &clock).expect("reopen after explicit cancellation");
    let attempt = reopened
        .get_model_call_attempt(&request_id)
        .unwrap()
        .unwrap();
    assert_eq!(attempt.state, ModelAttemptState::Ambiguous);
    assert_eq!(attempt.reserved_cost_usd_micros, reservation);
    assert!(attempt.actual_cost_usd_micros.is_none());
    assert!(
        reopened
            .model_usage_for_request(&request_id)
            .unwrap()
            .is_none()
    );
    let final_events = reopened
        .replay_events(None, None, 128)
        .expect("inspect cancellation recovery audit")
        .items
        .into_iter()
        .filter_map(|item| match item {
            ReplayItem::Event { event } => Some(*event),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        final_events
            .iter()
            .filter(|event| event.kind == serea_protocol::EventKind::ModelCalled)
            .count(),
        1
    );
    assert_eq!(
        final_events
            .iter()
            .filter(|event| event.kind == serea_protocol::EventKind::ModelFailed)
            .count(),
        1
    );
    assert_eq!(
        final_events
            .iter()
            .filter(|event| event.kind == serea_protocol::EventKind::ModelCompleted)
            .count(),
        0
    );
    assert!(
        final_events
            .iter()
            .position(|event| event.kind == serea_protocol::EventKind::ModelCalled)
            < final_events
                .iter()
                .position(|event| event.kind == serea_protocol::EventKind::ModelFailed)
    );
    reopened.checkpoint_for_close().unwrap();
    drop(reopened);
    std::fs::remove_dir_all(&directory).expect("clean cancellation directory");
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

    // Startup recovery commits the due occurrence's Task and audit before it
    // publishes readiness. Kill at that process boundary, before the later
    // scheduler diagnostic/acknowledgement, then inspect SQLite independently.
    let mut first = start(&directory);
    let _output = wait_ready(&mut first);
    first
        .kill()
        .expect("kill Host after durable Scheduler recovery");
    assert!(
        !first.wait().expect("reap killed Scheduler Host").success(),
        "process-kill boundary must terminate the child"
    );

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

#[test]
fn bounded_scheduler_throughput_sample() {
    const SCHEDULES: usize = 32;
    let directory = std::env::temp_dir().join(format!(
        "serea-host-scheduler-throughput-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create Scheduler throughput DB directory");
    let seeded = Command::new(env!("CARGO_BIN_EXE_serea-host"))
        .args(["scheduler-seed", "--synthetic", "--data-dir"])
        .arg(&directory)
        .args(["--count", &SCHEDULES.to_string()])
        .output()
        .expect("seed bounded synthetic schedule workload");
    assert!(seeded.status.success());

    let started = monotonic_now();
    let mut child = start(&directory);
    let output = wait_ready(&mut child);
    let elapsed = started.elapsed();
    let clock = serea_testkit::TestClock::at_epoch();
    let store = serea_storage::Store::open(&directory.join("core.sqlite"), &clock)
        .expect("inspect bounded Scheduler throughput DB");
    let replay = store
        .replay_events(None, None, 256)
        .expect("inspect bounded Scheduler throughput audit");
    let created = replay
        .items
        .iter()
        .filter(|item| matches!(item, serea_storage::ReplayItem::Event { event } if event.kind == serea_protocol::EventKind::TaskCreated))
        .count();
    assert_eq!(
        created, SCHEDULES,
        "all due occurrences should create one Task"
    );
    store
        .checkpoint_for_close()
        .expect("checkpoint throughput inspection");
    drop(store);
    stop(&directory, child, output);
    let throughput = SCHEDULES as f64 / elapsed.as_secs_f64();
    eprintln!(
        "RUNTIME_MEASUREMENT scheduler_tasks={SCHEDULES} startup_to_ready_ms={} tasks_per_second={throughput:.2}",
        elapsed.as_secs_f64() * 1000.0
    );
    std::fs::remove_dir_all(&directory).expect("clean Scheduler throughput directory");
}

#[test]
fn bounded_fake_model_process_throughput_sample() {
    const CALLS: usize = 16;
    let directory = std::env::temp_dir().join(format!(
        "serea-host-model-throughput-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create Model throughput DB directory");
    let started = monotonic_now();
    for _ in 0..CALLS {
        let output = Command::new(env!("CARGO_BIN_EXE_serea-host"))
            .args(["model-smoke", "--synthetic", "--data-dir"])
            .arg(&directory)
            .args(["--scenario", "success"])
            .output()
            .expect("run one bounded fake model request");
        assert!(
            output.status.success(),
            "synthetic model process failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let elapsed = started.elapsed();
    let clock = serea_testkit::TestClock::at_epoch();
    let store = serea_storage::Store::open(&directory.join("core.sqlite"), &clock)
        .expect("inspect bounded Model throughput DB");
    let events = store
        .replay_events(None, None, 256)
        .expect("inspect bounded Model throughput audit");
    let mut called = std::collections::BTreeSet::new();
    let mut completed = std::collections::BTreeSet::new();
    for item in events.items {
        if let serea_storage::ReplayItem::Event { event } = item {
            let destination = match event.kind {
                serea_protocol::EventKind::ModelCalled => Some(&mut called),
                serea_protocol::EventKind::ModelCompleted => Some(&mut completed),
                _ => None,
            };
            if let Some(destination) = destination {
                let request = event
                    .payload
                    .get("request_id")
                    .and_then(serde_json::Value::as_str)
                    .expect("model event RequestId");
                assert!(
                    destination.insert(request.to_owned()),
                    "duplicate RequestId event"
                );
            }
        }
    }
    assert_eq!(called.len(), CALLS);
    assert_eq!(completed, called);
    assert!(
        store
            .list_unfinished_model_call_attempts()
            .unwrap()
            .is_empty()
    );
    store
        .checkpoint_for_close()
        .expect("checkpoint Model throughput inspection");
    drop(store);
    let throughput = CALLS as f64 / elapsed.as_secs_f64();
    eprintln!(
        "RUNTIME_MEASUREMENT fake_model_process_calls={CALLS} elapsed_ms={} calls_per_second={throughput:.2}",
        elapsed.as_secs_f64() * 1000.0
    );
    std::fs::remove_dir_all(&directory).expect("clean Model throughput directory");
}

#[cfg(feature = "synthetic-test-hooks")]
#[test]
fn actual_host_scheduler_crash_barriers_recover_durable_occurrences() {
    use serea_protocol::{EpochMillis, EventKind, ScheduleId};
    use serea_storage::{MissedOccurrencePolicy, Store};

    for phase in [
        "before-admission",
        "after-admission",
        "after-claim",
        "before-task-commit",
        "after-task-commit",
    ] {
        let directory = std::env::temp_dir().join(format!(
            "serea-host-scheduler-barrier-{phase}-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        let barrier_dir = directory.join("barrier");
        std::fs::create_dir_all(&directory).expect("create Scheduler barrier database");
        let seeded = Command::new(env!("CARGO_BIN_EXE_serea-host"))
            .args(["scheduler-seed", "--synthetic", "--data-dir"])
            .arg(&directory)
            .output()
            .expect("seed synthetic schedule through Scheduler API");
        assert!(seeded.status.success(), "seed schedule before crash test");

        let mut child = start_at_scheduler_barrier(&directory, &barrier_dir, phase);
        wait_for_barrier(&mut child, &barrier_dir, phase);
        let database = directory.join("core.sqlite");
        let inspection_clock = serea_testkit::TestClock::at_epoch();
        let store =
            Store::open(&database, &inspection_clock).expect("inspect Scheduler barrier DB");
        let schedule_id = ScheduleId::new("sch_00000000000000000000000041").unwrap();
        let inspection_time = EpochMillis::new(8_000_000_000_000).unwrap();
        let occurrence_rows = store
            .transact(|tx| {
                let due = tx.due_schedule_occurrences(&schedule_id, inspection_time, 16)?;
                let claimed = tx.expired_claimed_schedule_occurrences(inspection_time, 16)?;
                Ok((due, claimed))
            })
            .expect("inspect pending and claimed occurrences");
        let audit = store
            .replay_events(None, None, 256)
            .expect("inspect Scheduler audit");
        let task_created = audit
            .items
            .iter()
            .filter(|item| {
                matches!(item,
            serea_storage::ReplayItem::Event { event } if event.kind == EventKind::TaskCreated)
            })
            .count();
        match phase {
            "before-admission" => {
                assert!(occurrence_rows.0.is_empty());
                assert!(occurrence_rows.1.is_empty());
                assert_eq!(task_created, 0);
            }
            "after-admission" => {
                assert_eq!(occurrence_rows.0.len(), 1);
                assert!(occurrence_rows.1.is_empty());
                assert_eq!(task_created, 0);
            }
            "after-claim" | "before-task-commit" => {
                assert!(occurrence_rows.0.is_empty());
                assert_eq!(occurrence_rows.1.len(), 1);
                assert_eq!(task_created, 0);
            }
            "after-task-commit" => {
                assert_eq!(task_created, 1);
                assert_eq!(occurrence_rows.0.len() + occurrence_rows.1.len(), 0);
            }
            _ => unreachable!(),
        }
        store
            .checkpoint_for_close()
            .expect("checkpoint Scheduler inspection");
        drop(store);
        child
            .kill()
            .expect("kill Host at Scheduler durable boundary");
        assert!(!child.wait().expect("reap killed Scheduler Host").success());

        let mut restart = Command::new(env!("CARGO_BIN_EXE_serea-host"));
        restart
            .args(["run", "--synthetic", "--data-dir"])
            .arg(&directory)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Advance the injected Host clock past both the Scheduler consumer
        // fence and any occurrence claim. This exercises lease recovery
        // without waiting in real time.
        restart.env("SEREA_TEST_CLOCK_NOW_MS", "4000000000000");
        let mut restarted = restart.spawn().expect("restart Host after Scheduler crash");
        let output = wait_ready(&mut restarted);
        stop(&directory, restarted, output);

        let recovered =
            Store::open(&database, &inspection_clock).expect("inspect recovered Scheduler DB");
        let final_audit = recovered
            .replay_events(None, None, 256)
            .expect("replay post-recovery audit");
        let tasks = final_audit
            .items
            .iter()
            .filter(|item| {
                matches!(item,
            serea_storage::ReplayItem::Event { event } if event.kind == EventKind::TaskCreated)
            })
            .count();
        let schedule_attribution = final_audit.items.iter().filter(|item| matches!(item,
            serea_storage::ReplayItem::Event { event } if event.kind == EventKind::ScheduleTaskCreated)).count();
        assert_eq!(tasks, 1, "one durable Task after {phase} recovery");
        assert_eq!(
            schedule_attribution, 1,
            "one occurrence attribution after {phase} recovery"
        );
        assert!(matches!(
            recovered
                .transact(|tx| tx.load_schedule(&schedule_id))
                .unwrap()
                .missed_policy,
            MissedOccurrencePolicy::RunOnce
        ));
        recovered
            .checkpoint_for_close()
            .expect("checkpoint recovered Scheduler DB");
        drop(recovered);
        std::fs::remove_dir_all(&directory).expect("clean Scheduler barrier directory");
    }
}

#[test]
fn fresh_host_startup_reconciles_unresolved_model_dispatch_once() {
    use serea_protocol::{
        CostClass, DataClass, EpochMillis, ModelId, ModelPurpose, ProviderId, RequestId,
    };
    use serea_storage::{
        ModelAttemptRelationKind, ModelAttemptState, ModelCallAttemptDraft, ModelDeploymentClass,
        ModelPriceSnapshot, Store, UsdMicros,
    };

    let directory = std::env::temp_dir().join(format!(
        "serea-host-model-recovery-{}-{}",
        std::process::id(),
        NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&directory).expect("create disposable model recovery directory");
    let database = directory.join("core.sqlite");
    let seed_clock = serea_testkit::TestClock::at_epoch();
    let request_id =
        RequestId::new("req_00000000000000000000000081").expect("valid synthetic request id");
    let store = Store::open(&database, &seed_clock).expect("open recovery database");
    let intent = store
        .reserve_model_call(
            ModelCallAttemptDraft {
                request_id: request_id.clone(),
                task_id: None,
                purpose: ModelPurpose::Chat,
                model_id: ModelId::new("nemotron-3-nano-30b").expect("model id"),
                // This descriptor identifier is resolved exclusively by the
                // explicit in-process MockModelProvider in synthetic mode.
                provider_id: ProviderId::new("ollama").expect("provider id"),
                deployment_class: ModelDeploymentClass::Local,
                data_class: DataClass::Public,
                relation_kind: ModelAttemptRelationKind::None,
                parent_request_id: None,
                fallback_from_model_id: None,
                price: ModelPriceSnapshot::new(CostClass::Free, "synthetic-free-v1", 0, 0),
                max_context_tokens: 4096,
                effective_max_output_tokens: 8,
                dispatch_intent_at: EpochMillis::new(0).expect("epoch"),
            },
            UsdMicros::new(0).expect("zero synthetic spend bound"),
        )
        .expect("persist unresolved dispatch intent");
    let original_reservation = intent.reserved_cost_usd_micros;
    drop(store);

    let mut first = start(&directory);
    let mut output = wait_ready(&mut first);
    let mut line = String::new();
    let started = monotonic_now();
    let mut recovery_reported = false;
    while started.elapsed() < Duration::from_secs(5) {
        line.clear();
        if output
            .read_line(&mut line)
            .expect("read startup diagnostics")
            == 0
        {
            break;
        }
        if line.contains("\"event\":\"model_recovery\"")
            && line.contains("\"ambiguous_attempts\":1")
        {
            recovery_reported = true;
            break;
        }
    }
    assert!(
        recovery_reported,
        "Host did not report startup reconciliation"
    );
    stop(&directory, first, output);

    let clock = serea_testkit::TestClock::at_epoch();
    let store = Store::open(&database, &clock).expect("inspect recovered intent");
    let recovered = store
        .get_model_call_attempt(&request_id)
        .expect("read recovered attempt")
        .expect("original attempt retained");
    assert_eq!(recovered.state, ModelAttemptState::Ambiguous);
    assert_eq!(recovered.request_id, request_id);
    assert_eq!(recovered.reserved_cost_usd_micros, original_reservation);
    assert_eq!(recovered.actual_cost_usd_micros, None);
    assert!(
        store
            .model_usage_for_request(&recovered.request_id)
            .unwrap()
            .is_none()
    );
    let failed_count = || {
        store
            .replay_events(None, None, 128)
            .expect("replay recovery audit")
            .items
            .iter()
            .filter(|item| {
                matches!(item, serea_storage::ReplayItem::Event { event }
                    if event.kind == serea_protocol::EventKind::ModelFailed)
            })
            .count()
    };
    assert_eq!(failed_count(), 1);

    // A second real process startup is idempotent: terminal state and its
    // content-free failure audit record remain singular.
    let mut second = start(&directory);
    let output = wait_ready(&mut second);
    stop(&directory, second, output);
    assert_eq!(failed_count(), 1);
    assert!(
        store
            .list_unfinished_model_call_attempts()
            .unwrap()
            .is_empty()
    );
    store
        .checkpoint_for_close()
        .expect("checkpoint recovery database");
    drop(store);
    std::fs::remove_dir_all(&directory).expect("clean model recovery directory");
}

#[cfg(feature = "synthetic-test-hooks")]
#[test]
fn actual_host_model_dispatch_crash_barriers_recover_without_redispatch() {
    use serea_protocol::RequestId;
    use serea_storage::{ModelAttemptState, Store};

    for phase in ["pre-dispatch", "provider", "after-result", "terminal"] {
        let directory = std::env::temp_dir().join(format!(
            "serea-host-model-barrier-{phase}-{}-{}",
            std::process::id(),
            NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed)
        ));
        let barrier_dir = directory.join("barrier");
        std::fs::create_dir_all(&directory).expect("create disposable model barrier directory");
        let mut child = Command::new(env!("CARGO_BIN_EXE_serea-host"))
            .args(["model-smoke", "--synthetic", "--data-dir"])
            .arg(&directory)
            .args([
                "--scenario",
                "success",
                "--test-barrier",
                phase,
                "--test-barrier-dir",
            ])
            .arg(&barrier_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .expect("launch actual Host model dispatch");

        let marker = barrier_dir.join(format!("{phase}.reached"));
        let deadline = monotonic_now() + Duration::from_secs(30);
        while !marker.exists() {
            if let Some(status) = child.try_wait().expect("check model child") {
                panic!("Host exited before {phase} barrier: {status}");
            }
            assert!(monotonic_now() < deadline, "model {phase} barrier deadline");
            std::thread::park_timeout(Duration::from_millis(5));
        }

        let database = directory.join("core.sqlite");
        let clock = serea_testkit::TestClock::at_epoch();
        let store = Store::open(&database, &clock).expect("inspect barrier durable state");
        let replay = store
            .replay_events(None, None, 128)
            .expect("read dispatch audit at barrier");
        let called_events = replay
            .items
            .iter()
            .filter(|item| {
                matches!(item, serea_storage::ReplayItem::Event { event }
                if event.kind == serea_protocol::EventKind::ModelCalled)
            })
            .count();
        assert_eq!(called_events, usize::from(phase != "pre-dispatch"));
        let attempts = store
            .list_unfinished_model_call_attempts()
            .expect("inspect active dispatch intent");

        let request_id = match phase {
            "pre-dispatch" => {
                assert!(attempts.is_empty());
                None
            }
            "provider" | "after-result" => {
                assert_eq!(attempts.len(), 1);
                assert_eq!(attempts[0].state, ModelAttemptState::DispatchIntent);
                Some(attempts[0].request_id.clone())
            }
            "terminal" => {
                assert!(
                    attempts.is_empty(),
                    "terminal barrier is after durable completion"
                );
                let completed = replay.items.iter().find_map(|item| match item {
                    serea_storage::ReplayItem::Event { event }
                        if event.kind == serea_protocol::EventKind::ModelCompleted =>
                    {
                        event
                            .payload
                            .get("request_id")
                            .and_then(serde_json::Value::as_str)
                            .and_then(|id| RequestId::new(id.to_owned()).ok())
                    }
                    _ => None,
                });
                // Model event metadata uses its canonical correlation field;
                // inspect attempts by their durable event identity below.
                Some(completed.expect("completed durable request identity"))
            }
            _ => unreachable!(),
        };
        store
            .checkpoint_for_close()
            .expect("checkpoint inspection connection");
        drop(store);

        child
            .kill()
            .expect("kill Host at deterministic model barrier");
        assert!(!child.wait().expect("reap killed model Host").success());

        let mut restarted = start(&directory);
        let output = wait_ready(&mut restarted);
        stop(&directory, restarted, output);

        let reopened =
            Store::open(&database, &clock).expect("reopen model database after recovery");
        if let Some(request_id) = request_id {
            let attempt = reopened
                .get_model_call_attempt(&request_id)
                .expect("read attempt after process restart")
                .expect("original request id remains durable");
            if matches!(phase, "provider" | "after-result") {
                assert_eq!(attempt.state, ModelAttemptState::Ambiguous);
                assert_eq!(attempt.actual_cost_usd_micros, None);
                assert!(
                    reopened
                        .model_usage_for_request(&request_id)
                        .unwrap()
                        .is_none()
                );
            } else {
                assert_eq!(attempt.state, ModelAttemptState::Completed);
                assert!(
                    reopened
                        .model_usage_for_request(&request_id)
                        .unwrap()
                        .is_some()
                );
            }
        } else {
            assert!(
                reopened
                    .list_unfinished_model_call_attempts()
                    .unwrap()
                    .is_empty()
            );
        }
        let after = reopened
            .replay_events(None, None, 128)
            .expect("inspect recovered audit");
        let called_after = after.items.iter().filter(|item| matches!(item,
            serea_storage::ReplayItem::Event { event } if event.kind == serea_protocol::EventKind::ModelCalled)).count();
        let terminal_after = after
            .items
            .iter()
            .filter(|item| {
                matches!(item,
            serea_storage::ReplayItem::Event { event } if matches!(event.kind,
                serea_protocol::EventKind::ModelFailed | serea_protocol::EventKind::ModelCompleted))
            })
            .count();
        let expected_dispatches = usize::from(phase != "pre-dispatch");
        assert_eq!(
            called_after, expected_dispatches,
            "recovery never creates a new dispatch"
        );
        assert_eq!(
            terminal_after, expected_dispatches,
            "terminal audit remains singular"
        );
        reopened
            .checkpoint_for_close()
            .expect("checkpoint reopened model database");
        drop(reopened);
        std::fs::remove_dir_all(&directory).expect("clean model barrier directory");
    }
}
