use serea_event_bus::EventBus;
use serea_testkit::DeterministicUlidSource;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_EVENT_SOURCE: AtomicU64 = AtomicU64::new(0);

pub fn event_bus() -> EventBus {
    let offset = NEXT_EVENT_SOURCE.fetch_add(1_000, Ordering::Relaxed);
    let process = u64::from(std::process::id());
    let start = 1_700_000_000_000 + process * 10_000 + offset;
    EventBus::new(DeterministicUlidSource::starting_at(start).unwrap())
}
