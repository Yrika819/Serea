//! Experimental, synthetic-only Android Core bootstrap. Not a product API.
use serea_core::Core;
use serea_event_bus::EventBus;
use serea_protocol::{
    Clock, EpochMillis, ProtocolError, TimestampMs, UlidSource, UlidValue, ValueField,
    ValueRejection,
};
use std::{
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
        let ms = SystemTime::now()
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
    current: Option<(u64, Core)>,
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
    state.current = Some((handle, core));
    Ok(handle)
}
#[uniffi::export]
pub fn get_status(handle: u64) -> Result<bool, BridgeError> {
    let state = STATE.lock().map_err(|_| BridgeError::Internal)?;
    if state.current.as_ref().is_some_and(|(h, _)| *h == handle) {
        Ok(true)
    } else {
        Err(BridgeError::InvalidHandle)
    }
}
#[uniffi::export]
pub fn schema_version(handle: u64) -> Result<u32, BridgeError> {
    let state = STATE.lock().map_err(|_| BridgeError::Internal)?;
    let (_, core) = state
        .current
        .as_ref()
        .filter(|(h, _)| *h == handle)
        .ok_or(BridgeError::InvalidHandle)?;
    core.schema_version()
        .map_err(|_| BridgeError::StorageUnavailable)
}
#[uniffi::export]
pub fn close(handle: u64) -> Result<(), BridgeError> {
    let mut state = STATE.lock().map_err(|_| BridgeError::Internal)?;
    if !state.current.as_ref().is_some_and(|(h, _)| *h == handle) {
        return Err(BridgeError::InvalidHandle);
    }
    let (_, core) = state.current.take().ok_or(BridgeError::Internal)?;
    core.shutdown()
        .map_err(|_| BridgeError::StorageUnavailable)?;
    Ok(())
}
