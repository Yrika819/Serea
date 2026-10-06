//! Durable Event Bus boundary. Storage owns sequence allocation and commit;
//! this crate exposes the event-specific call without exposing SQL.
#![forbid(unsafe_code)]

use serea_protocol::{EpochMillis, SereaEvent};
use serea_storage::{StoreError, Tx};

/// Event Bus append authority scoped to the caller's existing transaction.
pub struct EventBus;

impl EventBus {
    /// Appends an immutable event in the caller's transaction. Task Engine and
    /// Scheduler callers compose this with their state mutation before commit.
    pub fn append(
        tx: &mut Tx<'_>,
        event: SereaEvent,
        retention_at: Option<EpochMillis>,
    ) -> Result<SereaEvent, StoreError> {
        tx.append_event(event, retention_at)
    }
}
