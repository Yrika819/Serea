use serea_core::Core;
use serea_event_bus::EventBus;
use serea_testkit::{DeterministicUlidSource, TestClock};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_DB: AtomicUsize = AtomicUsize::new(0);

#[test]
fn bounded_scheduler_wakes_release_the_consumer_fence_for_the_next_wake() {
    let path = std::env::temp_dir().join(format!(
        "serea-core-scheduler-wake-{}-{}.sqlite",
        std::process::id(),
        NEXT_DB.fetch_add(1, Ordering::Relaxed)
    ));
    let clock = TestClock::at_epoch();
    let now = clock.now_ms().unwrap();
    let events = EventBus::new(DeterministicUlidSource::new());
    let mut core = Core::open(&path, &clock, events).unwrap();

    core.recover(now, "scheduler-wake-test").unwrap();
    assert_eq!(
        core.process_scheduler_wake(now, "scheduler-wake-test")
            .unwrap(),
        (0, 0)
    );
    assert_eq!(
        core.process_scheduler_wake(now, "scheduler-wake-test")
            .unwrap(),
        (0, 0)
    );
    core.shutdown().unwrap();
    std::fs::remove_file(path).unwrap();
}
