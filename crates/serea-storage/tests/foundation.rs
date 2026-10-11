use serea_protocol::{Clock, EpochMillis, ProtocolError};
use serea_storage::{CheckpointOutcome, Store};
use std::sync::atomic::{AtomicU64, Ordering};

struct Fixed;
impl Clock for Fixed {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(-1)
    }
}
static NEXT: AtomicU64 = AtomicU64::new(0);

#[test]
fn file_migration_reopen_and_retryable_checkpoint() {
    let dir = std::env::temp_dir().join(format!(
        "serea-p2c-foundation-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&dir).unwrap();
    let path = dir.join("store.sqlite");
    {
        let store = Store::open(&path, &Fixed).unwrap();
        assert_eq!(store.schema_version().unwrap(), 5);
        assert_eq!(
            store.checkpoint_for_close().unwrap(),
            CheckpointOutcome::Complete
        );
    }
    let store = Store::open(&path, &Fixed).unwrap();
    assert_eq!(store.schema_version().unwrap(), 5);
    store.verify_integrity().unwrap();
    store.transact(|_tx| Ok(())).unwrap();
    drop(store);
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn memory_profile_is_explicitly_not_durable() {
    let store = Store::open_in_memory(&Fixed).unwrap();
    assert_eq!(store.schema_version().unwrap(), 5);
    assert_eq!(
        store.checkpoint_for_close().unwrap(),
        CheckpointOutcome::NotApplicable
    );
}
