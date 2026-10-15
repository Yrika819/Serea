use serea_core::Core;
use serea_event_bus::EventBus;
use serea_storage::Store;
use serea_testkit::{DeterministicUlidSource, TestClock};

#[test]
fn normal_core_startup_does_not_install_a_registry_generation() {
    let path = std::env::temp_dir().join(format!(
        "serea-core-startup-no-registry-{}.sqlite",
        std::process::id()
    ));
    let clock = TestClock::at_epoch();
    let events = EventBus::new(DeterministicUlidSource::new());

    Core::open(&path, &clock, events)
        .expect("Core opens the canonical store")
        .shutdown()
        .expect("Core shuts down cleanly");

    let store = Store::open(&path, &clock).expect("the SQLite database reopens");
    assert_eq!(store.current_registry_generation().unwrap(), None);
    drop(store);
    std::fs::remove_file(path).expect("the closed database is removable");
}
