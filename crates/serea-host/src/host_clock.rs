//! The only production wall-clock adapter, owned by the executable boundary.

use std::time::{SystemTime, UNIX_EPOCH};

use serea_protocol::{Clock, EpochMillis, ProtocolError, ValueField, ValueRejection};

/// OS wall clock injected into Core by the executable Host.
pub(super) struct WallClock;

impl Clock for WallClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        #[cfg(feature = "synthetic-test-hooks")]
        if let Some(value) = std::env::var_os("SEREA_TEST_CLOCK_NOW_MS") {
            let millis = value.to_string_lossy().parse::<i64>().map_err(|_| {
                ProtocolError::MalformedValue {
                    field: ValueField::Timestamp,
                    reason: ValueRejection::OutOfRange,
                }
            })?;
            return EpochMillis::new(millis);
        }
        // Core and Scheduler only see the injected Clock port. This is the
        // deliberately narrow process-boundary source of runtime time.
        #[allow(clippy::disallowed_methods)]
        let time = SystemTime::now();
        let millis = time
            .duration_since(UNIX_EPOCH)
            .map_err(|_| ProtocolError::MalformedValue {
                field: ValueField::Timestamp,
                reason: ValueRejection::OutOfRange,
            })?
            .as_millis();
        EpochMillis::new(i64::try_from(millis).unwrap_or(i64::MAX))
    }
}
