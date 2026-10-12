#![cfg(feature = "p7-gate-c")]

use fs2::FileExt as _;
use serea_storage::p7_gate_c::LedgerFileGuard;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);

#[test]
fn ledger_file_lock_child_entry() {
    let Ok(path) = std::env::var("SEREA_P7_LOCK_CHILD_PATH") else {
        return;
    };
    let _guard = LedgerFileGuard::acquire(path).unwrap();
    println!("LEDGER_LOCKED");
    std::io::stdout().flush().unwrap();
    std::thread::sleep(std::time::Duration::from_secs(60));
}

#[test]
fn ledger_lock_is_exclusive_across_processes_and_released_on_termination() {
    let root = std::env::temp_dir().join(format!(
        "serea-gate-c-lock-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&root).unwrap();
    let ledger_path = root.join("ledger.synthetic");
    let lock_path = root.join("ledger.synthetic.lock");
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("ledger_file_lock_child_entry")
        .arg("--nocapture")
        .env("SEREA_P7_LOCK_CHILD_PATH", &ledger_path)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert_ne!(
            stdout.read_line(&mut line).unwrap(),
            0,
            "lock child exited early"
        );
        if line.contains("LEDGER_LOCKED") {
            break;
        }
    }
    let probe = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .unwrap();
    assert!(probe.try_lock_exclusive().is_err());
    child.kill().unwrap();
    child.wait().unwrap();
    probe.try_lock_exclusive().unwrap();
    fs::remove_dir_all(root).unwrap();
}
