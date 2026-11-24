//! Experimental, synthetic-only Android Core bootstrap. Not a product API.
use serea_core::Core;
use serea_event_bus::EventBus;
use serea_protocol::{
    Clock, EpochMillis, ProtocolError, TimestampMs, UlidSource, UlidValue, ValueField,
    ValueRejection,
};
use std::{
    fs::{File, OpenOptions},
    path::Path,
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

uniffi::setup_scaffolding!();

#[derive(Debug, Clone, PartialEq, Eq, uniffi::Error)]
pub enum BridgeError {
    AlreadyInitialized,
    InvalidHandle,
    StorageUnavailable,
    RecoveryFailed,
    Internal,
}
impl std::fmt::Display for BridgeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for BridgeError {}

struct WallClock;
impl Clock for WallClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        let invalid = || ProtocolError::MalformedValue {
            field: ValueField::Timestamp,
            reason: ValueRejection::OutOfRange,
        };
        // The isolated Android process boundary supplies time through Core's Clock port.
        #[allow(clippy::disallowed_methods)]
        let now = SystemTime::now();
        let ms = now
            .duration_since(UNIX_EPOCH)
            .map_err(|_| invalid())?
            .as_millis();
        EpochMillis::new(i64::try_from(ms).map_err(|_| invalid())?)
    }
}
struct Ids {
    time: TimestampMs,
    entropy: [u8; 10],
}
impl UlidSource for Ids {
    fn next_ulid(&mut self) -> UlidValue {
        let value = UlidValue::new(self.time, self.entropy);
        let mut carry = true;
        for byte in self.entropy.iter_mut().rev() {
            if !carry {
                break;
            }
            let (n, c) = byte.overflowing_add(1);
            *byte = n;
            carry = c;
        }
        assert!(!carry, "identifier source exhausted");
        self.time = self.time.checked_next().unwrap_or(self.time);
        value
    }
}
struct State {
    next: u64,
    current: Option<(u64, Core, File)>,
}
static STATE: Mutex<State> = Mutex::new(State {
    next: 1,
    current: None,
});

#[uniffi::export]
pub fn initialize(path: String) -> Result<u64, BridgeError> {
    let mut state = STATE.lock().map_err(|_| BridgeError::Internal)?;
    if state.current.is_some() {
        return Err(BridgeError::AlreadyInitialized);
    }
    // Reserve the sidecar namespace so no bridge may open a lock as a DB.
    const LOCK_SUFFIX: &str = ".serea-owner-lock";
    let reserved = |name: &str| {
        [LOCK_SUFFIX, "-wal", "-shm", "-journal"]
            .iter()
            .any(|suffix| name.ends_with(suffix))
    };
    if reserved(&path) {
        return Err(BridgeError::StorageUnavailable);
    }
    // SQLite must be free to manage its own locks. Use a persistent sidecar
    // next to the canonical database path; never remove it on close or failure.
    // The DB directory must be private to cooperating callers: unlinking or
    // replacing either file while owned would defeat path-based coordination.
    let db = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|_| BridgeError::StorageUnavailable)?;
    let metadata = db.metadata().map_err(|_| BridgeError::StorageUnavailable)?;
    if !metadata.is_file() {
        return Err(BridgeError::StorageUnavailable);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(BridgeError::StorageUnavailable);
        }
    }
    let canonical = std::fs::canonicalize(&path).map_err(|_| BridgeError::StorageUnavailable)?;
    if reserved(&canonical.as_os_str().to_string_lossy()) {
        return Err(BridgeError::StorageUnavailable);
    }
    let mut lock_path = canonical.into_os_string();
    lock_path.push(LOCK_SUFFIX);
    let ownership = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(lock_path)
        .map_err(|_| BridgeError::StorageUnavailable)?;
    ownership
        .try_lock()
        .map_err(|_| BridgeError::StorageUnavailable)?;
    let now = WallClock.now_ms().map_err(|_| BridgeError::Internal)?;
    let time = TimestampMs::new(u64::try_from(now.get()).map_err(|_| BridgeError::Internal)?)
        .map_err(|_| BridgeError::Internal)?;
    let mut entropy = [0; 10];
    getrandom::fill(&mut entropy).map_err(|_| BridgeError::Internal)?;
    let mut core = Core::open(
        Path::new(&path),
        &WallClock,
        EventBus::new(Ids { time, entropy }),
    )
    .map_err(|_| BridgeError::StorageUnavailable)?;
    core.recover(now, "android-synthetic")
        .map_err(|_| BridgeError::RecoveryFailed)?;
    let handle = state.next;
    state.next = handle.checked_add(1).ok_or(BridgeError::Internal)?;
    state.current = Some((handle, core, ownership));
    Ok(handle)
}
#[uniffi::export]
pub fn get_status(handle: u64) -> Result<bool, BridgeError> {
    let state = STATE.lock().map_err(|_| BridgeError::Internal)?;
    if state.current.as_ref().is_some_and(|(h, _, _)| *h == handle) {
        Ok(true)
    } else {
        Err(BridgeError::InvalidHandle)
    }
}
#[uniffi::export]
pub fn schema_version(handle: u64) -> Result<u32, BridgeError> {
    let state = STATE.lock().map_err(|_| BridgeError::Internal)?;
    let (_, core, _) = state
        .current
        .as_ref()
        .filter(|(h, _, _)| *h == handle)
        .ok_or(BridgeError::InvalidHandle)?;
    core.schema_version()
        .map_err(|_| BridgeError::StorageUnavailable)
}
#[uniffi::export]
pub fn close(handle: u64) -> Result<(), BridgeError> {
    let mut state = STATE.lock().map_err(|_| BridgeError::Internal)?;
    if state.current.as_ref().is_none_or(|(h, _, _)| *h != handle) {
        return Err(BridgeError::InvalidHandle);
    }
    let (_, core, _ownership) = state.current.take().ok_or(BridgeError::Internal)?;
    core.shutdown()
        .map_err(|_| BridgeError::StorageUnavailable)?;
    Ok(())
}
