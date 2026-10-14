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
