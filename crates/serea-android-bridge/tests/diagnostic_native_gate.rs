#![cfg(feature = "s1b-diagnostic")]

use serea_android_bridge::{
    close, diagnostic_arm, diagnostic_release, diagnostic_wait_entered, get_status, initialize,
};
use std::{
    sync::{Mutex, mpsc},
    time::Duration,
};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn gate_wait_timeout_does_not_leave_gate_armed() {
    let _serial = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    assert!(diagnostic_release().is_err());
    diagnostic_arm().unwrap();
    assert!(diagnostic_arm().is_err());
    assert!(diagnostic_wait_entered().is_err());
    diagnostic_arm().expect("timed-out arm must be cleared");
    assert!(diagnostic_wait_entered().is_err());
}

#[test]
fn native_status_gate_defers_close_until_native_returns() {
    let _serial = TEST_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let root = std::env::temp_dir().join(format!("serea-native-gate-{}", std::process::id()));
    std::fs::create_dir(&root).expect("fresh synthetic test directory");
    let handle = initialize(root.join("synthetic.db").to_str().unwrap().to_owned()).unwrap();
    diagnostic_arm().unwrap();
    let worker = std::thread::spawn(move || get_status(handle));
    diagnostic_wait_entered().unwrap();
    let (sent, received) = mpsc::channel();
    let closer = std::thread::spawn(move || {
        sent.send(close(handle)).unwrap();
    });
    assert!(received.recv_timeout(Duration::from_millis(100)).is_err());
    diagnostic_release().unwrap();
    assert!(worker.join().unwrap().unwrap());
    assert!(
        received
            .recv_timeout(Duration::from_secs(10))
            .unwrap()
            .is_ok()
    );
    closer.join().unwrap();
    assert!(get_status(handle).is_err());
}
