use serea_protocol::SereaEvent;

pub use serea_protocol::{EventPredicateError, EventPredicateV1};

/// Provenance resolved from durable Scheduler occurrence mappings. Runtime
/// callers must derive this value from committed state, never event JSON.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventCausality {
    Independent,
    SchedulerOccurrence,
}

/// Applies exact EventKind matching after the host causal-root check.
pub fn matches_host_event(
    predicate: &EventPredicateV1,
    event: &SereaEvent,
    causality: EventCausality,
) -> bool {
    causality == EventCausality::Independent && predicate.matches_event_kind(event.kind)
}
