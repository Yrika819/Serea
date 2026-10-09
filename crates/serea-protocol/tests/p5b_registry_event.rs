use serea_protocol::EventKind;

#[test]
fn capability_registry_changed_is_a_supported_event_kind() {
    assert!(EventKind::WIRE_NAMES.contains(&"CAPABILITY_REGISTRY_CHANGED"));
}
