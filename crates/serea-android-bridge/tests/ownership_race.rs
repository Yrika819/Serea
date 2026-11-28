//! Host-only coverage of the bridge's process-local ownership transition.
use serea_android_bridge::{BridgeError, close, get_status, initialize, schema_version};
use std::{
    sync::{Arc, Barrier},
    thread,
};

#[test]
fn concurrent_initialize_has_one_owner_and_stale_handle_cannot_close_successor() {
    // A fresh, private directory per run, distinct from every Android database.
    let mut entropy = [0_u8; 8];
    getrandom::fill(&mut entropy).unwrap();
    let dir = std::env::temp_dir().join(format!(
        "serea-bridge-owner-race-{}-{}",
        std::process::id(),
        u64::from_ne_bytes(entropy)
    ));
    let mut builder = std::fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&dir).unwrap();
    let path = dir.join("synthetic.sqlite").to_str().unwrap().to_owned();
    let barrier = Arc::new(Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let barrier = Arc::clone(&barrier);
            let path = path.clone();
            thread::spawn(move || {
                barrier.wait();
                initialize(path)
            })
        })
        .collect();
    barrier.wait();
    let results: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    let handles: Vec<_> = results
        .iter()
        .filter_map(|result| result.as_ref().ok().copied())
        .collect();
    assert_eq!(
        handles.len(),
        1,
        "exactly one caller must acquire ownership"
    );
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(BridgeError::AlreadyInitialized))
            .count(),
        1
    );
    let first = handles[0];
    assert_eq!(get_status(first), Ok(true));
    close(first).unwrap();

    let second = initialize(path).unwrap();
    assert_ne!(first, second);
    assert_eq!(close(first), Err(BridgeError::InvalidHandle));
    assert_eq!(get_status(first), Err(BridgeError::InvalidHandle));
    assert_eq!(schema_version(first), Err(BridgeError::InvalidHandle));
    assert_eq!(get_status(second), Ok(true));
    assert!(schema_version(second).unwrap() > 0);
    close(second).unwrap();
    // Retain this synthetic fixture with the test evidence.
}
