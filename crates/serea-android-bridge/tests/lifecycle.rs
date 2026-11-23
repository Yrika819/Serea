use serea_android_bridge::{BridgeError, close, get_status, initialize, schema_version};

#[test]
fn lifecycle_and_invalid_handles() {
    let path =
        std::env::temp_dir().join(format!("serea-bridge-{}-lifecycle.db", std::process::id()));
    let path = path.to_str().unwrap().to_owned();
    let h = initialize(path.clone()).unwrap();
    assert!(get_status(h).unwrap());
    assert!(schema_version(h).unwrap() > 0);
    assert_eq!(
        initialize(path.clone()),
        Err(BridgeError::AlreadyInitialized)
    );
    close(h).unwrap();
    assert_eq!(close(h), Err(BridgeError::InvalidHandle));
    assert_eq!(get_status(h), Err(BridgeError::InvalidHandle));
    assert_eq!(schema_version(u64::MAX), Err(BridgeError::InvalidHandle));
    let h2 = initialize(path).unwrap();
    close(h2).unwrap();
    assert_eq!(
        initialize("/dev/null/serea-synthetic.db".to_owned()),
        Err(BridgeError::StorageUnavailable)
    );
}
