#[path = "common/mod.rs"]
mod common;

use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serea_capability::{
    CapabilityManifestV1, CapabilitySchemaCatalogV1, HostEligibility, ManifestEntryV1,
    ProviderRegistry, ResolveError, descriptor_semantic_digest,
};
use serea_event_bus::EventBus;
use serea_protocol::provider::{CapabilityProvider, ProviderContext};
use serea_protocol::{
    ActionResult, Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId,
    CostClass, DataClass, DescriptorDescription, DescriptorTitle, EpochMillis, IdempotencySupport,
    ImplementationId, JsonSchemaRef, ProviderHealth, ProviderId, ReplaySafety, RiskClass,
    RootRequirement, SemVer, SideEffectClass,
};
use serea_storage::Store;
use serea_testkit::DeterministicUlidSource;

const PREFIX: &str = "https://serea.local/schemas/";

struct ScriptedProvider {
    id: ProviderId,
    descriptors: Mutex<Vec<CapabilityDescriptor>>,
    health: Mutex<ProviderHealth>,
    health_calls: AtomicUsize,
    caps_calls: AtomicUsize,
    invoke_calls: AtomicUsize,
}

struct WakingProvider {
    id: ProviderId,
    descriptors: Vec<CapabilityDescriptor>,
    health_calls: AtomicUsize,
    caps_calls: AtomicUsize,
    ready: std::sync::atomic::AtomicBool,
}

#[async_trait::async_trait]
impl CapabilityProvider for WakingProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }
    fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        self.caps_calls.fetch_add(1, Ordering::SeqCst);
        self.descriptors.clone()
    }
    async fn invoke(
        &self,
        _request: &serea_protocol::ActionRequest,
        _ctx: &ProviderContext,
    ) -> Result<ActionResult, serea_protocol::ActionError> {
        panic!("snapshot construction must never invoke a provider")
    }
    async fn health(&self) -> ProviderHealth {
        self.health_calls.fetch_add(1, Ordering::SeqCst);
        let mut pending_once = true;
        std::future::poll_fn(|cx| {
            if pending_once {
                pending_once = false;
                let waker = cx.waker().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    waker.wake();
                });
                std::task::Poll::Pending
            } else if self.ready.load(Ordering::SeqCst) {
                std::task::Poll::Ready(ProviderHealth::Ready)
            } else {
                std::task::Poll::Ready(ProviderHealth::Degraded)
            }
        })
        .await
    }
}

impl ScriptedProvider {
    fn new(id: &str, descriptors: Vec<CapabilityDescriptor>, health: ProviderHealth) -> Self {
        Self {
            id: ProviderId::new(id).unwrap(),
            descriptors: Mutex::new(descriptors),
            health: Mutex::new(health),
            health_calls: AtomicUsize::new(0),
            caps_calls: AtomicUsize::new(0),
            invoke_calls: AtomicUsize::new(0),
        }
    }
    fn set_health(&self, h: ProviderHealth) {
        *self.health.lock().unwrap() = h;
    }
}

#[async_trait::async_trait]
impl CapabilityProvider for ScriptedProvider {
    fn provider_id(&self) -> ProviderId {
        self.id.clone()
    }
    fn capabilities(&self) -> Vec<CapabilityDescriptor> {
        self.caps_calls.fetch_add(1, Ordering::SeqCst);
        self.descriptors.lock().unwrap().clone()
    }
    async fn invoke(
        &self,
        _request: &serea_protocol::ActionRequest,
        _ctx: &ProviderContext,
    ) -> Result<ActionResult, serea_protocol::ActionError> {
        self.invoke_calls.fetch_add(1, Ordering::SeqCst);
        panic!("invoke must never be called");
    }
    async fn health(&self) -> ProviderHealth {
        self.health_calls.fetch_add(1, Ordering::SeqCst);
        *self.health.lock().unwrap()
    }
}

fn catalog() -> CapabilitySchemaCatalogV1 {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}input.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"name":{"type":"string","maxLength":32}},"required":["name"]}"#
            .to_string(),
    );
    docs.insert(
        format!("{PREFIX}output.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"ok":{"type":"boolean"}}}"#
            .to_string(),
    );
    CapabilitySchemaCatalogV1::build(docs).unwrap()
}

fn descriptor(
    id: &str,
    version: &str,
    provider: &str,
    impl_id: Option<&str>,
) -> CapabilityDescriptor {
    CapabilityDescriptor::new(CapabilityDescriptorDraft {
        id: CapabilityId::new(id).unwrap(),
        version: SemVer::new(version).unwrap(),
        title: DescriptorTitle::new("T").unwrap(),
        description: DescriptorDescription::new("D").unwrap(),
        provider_id: ProviderId::new(provider).unwrap(),
        implementation_id: impl_id.map(|s| ImplementationId::new(s).unwrap()),
        input_schema: JsonSchemaRef::new(format!("{PREFIX}input.json")).unwrap(),
        output_schema: JsonSchemaRef::new(format!("{PREFIX}output.json")).unwrap(),
        side_effect_class: SideEffectClass::None,
        risk_class: RiskClass::Observe,
        required_authorization: Authorization::None,
        replay_safety: ReplaySafety::Idempotent,
        data_class: DataClass::Personal,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::None,
        max_duration_ms: 5_000,
        cost_class: CostClass::Free,
        experimental: false,
    })
    .unwrap()
}

fn entry(
    d: &CapabilityDescriptor,
    priority: u32,
    catalog: &CapabilitySchemaCatalogV1,
) -> ManifestEntryV1 {
    ManifestEntryV1 {
        descriptor: d.clone(),
        candidate_priority: priority,
        descriptor_digest: descriptor_semantic_digest(d, catalog).unwrap(),
        input_schema_digest: catalog
            .document_digest(d.input_schema().as_str())
            .unwrap()
            .clone(),
        output_schema_digest: catalog
            .document_digest(d.output_schema().as_str())
            .unwrap()
            .clone(),
    }
}

struct FixedClock;
impl serea_protocol::Clock for FixedClock {
    fn now_ms(&self) -> Result<serea_protocol::EpochMillis, serea_protocol::ProtocolError> {
        serea_protocol::EpochMillis::new(1_796_000_000_000)
    }
}

fn store() -> Store {
    Store::open_in_memory(&FixedClock).unwrap()
}

fn eligibility_for(
    descriptor: &CapabilityDescriptor,
    catalog: &CapabilitySchemaCatalogV1,
    eligible: bool,
) -> HostEligibility {
    let mut map = HashMap::new();
    map.insert(
        descriptor_semantic_digest(descriptor, catalog).unwrap(),
        eligible,
    );
    HostEligibility::new(map)
}

fn manifest_two() -> (CapabilityManifestV1, CapabilitySchemaCatalogV1) {
    let c = catalog();
    let d1 = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let d2 = descriptor("gmail.messages.send", "1.0.0", "gmail", None);
    let m = CapabilityManifestV1::build(
        vec![entry(&d1, 0, &c), entry(&d2, 0, &c)],
        vec![
            (d1.id().clone(), d1.version().clone()),
            (d2.id().clone(), d2.version().clone()),
        ],
        c.clone(),
    )
    .unwrap();
    (m, c)
}

#[test]
fn eligible_default_candidate_selected() {
    let (m, c) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![cal.clone()],
        ProviderHealth::Ready,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let elig = eligibility_for(&cal, &c, true);
    let snap = common::build_snapshot(m, &registry, &store(), elig).unwrap();
    let res = snap
        .resolve(&CapabilityId::new("calendar.events.read").unwrap())
        .unwrap();
    assert_eq!(res.provider_id.as_str(), "calendar");
}

#[test]
fn first_candidate_ineligible_second_selected() {
    let c = catalog();
    let d_local = descriptor("calendar.events.read", "1.0.0", "calendar", Some("local"));
    let d_backup = descriptor("calendar.events.read", "1.0.0", "calendar", Some("backup"));
    let m = CapabilityManifestV1::build(
        vec![entry(&d_local, 0, &c), entry(&d_backup, 1, &c)],
        vec![(d_local.id().clone(), d_local.version().clone())],
        c.clone(),
    )
    .unwrap();
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![d_local.clone(), d_backup.clone()],
        ProviderHealth::Ready,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let mut elig = HashMap::new();
    elig.insert(descriptor_semantic_digest(&d_local, &c).unwrap(), false);
    elig.insert(descriptor_semantic_digest(&d_backup, &c).unwrap(), true);
    let snap = common::build_snapshot(m, &registry, &store(), HostEligibility::new(elig)).unwrap();
    let res = snap
        .resolve(&CapabilityId::new("calendar.events.read").unwrap())
        .unwrap();
    assert_eq!(res.implementation_id.unwrap().as_str(), "backup");
}

#[test]
fn all_ineligible_unavailable() {
    let (m, c) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![cal.clone()],
        ProviderHealth::Ready,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let elig = eligibility_for(&cal, &c, false);
    let snap = common::build_snapshot(m, &registry, &store(), elig).unwrap();
    let err = snap
        .resolve(&CapabilityId::new("calendar.events.read").unwrap())
        .unwrap_err();
    assert!(matches!(err, ResolveError::Unavailable { .. }));
}

#[test]
fn missing_eligibility_unavailable() {
    let (m, _c) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![cal],
        ProviderHealth::Ready,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let elig = HostEligibility::new(HashMap::new());
    let snap = common::build_snapshot(m, &registry, &store(), elig).unwrap();
    let err = snap
        .resolve(&CapabilityId::new("calendar.events.read").unwrap())
        .unwrap_err();
    assert!(matches!(err, ResolveError::Unavailable { .. }));
}

#[test]
fn unknown_capability() {
    let (m, _c) = manifest_two();
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![],
        ProviderHealth::Ready,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let snap = common::build_snapshot(m, &registry, &store(), HostEligibility::new(HashMap::new()))
        .unwrap();
    let err = snap
        .resolve(&CapabilityId::new("calendar.events.delete").unwrap())
        .unwrap_err();
    assert!(matches!(err, ResolveError::Unknown { .. }));
}

#[test]
fn ready_available_degraded_unavailable() {
    let (m, c) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![cal.clone()],
        ProviderHealth::Degraded,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let elig = eligibility_for(&cal, &c, true);
    let snap = common::build_snapshot(m, &registry, &store(), elig).unwrap();
    let err = snap
        .resolve(&CapabilityId::new("calendar.events.read").unwrap())
        .unwrap_err();
    assert!(matches!(err, ResolveError::Unavailable { .. }));
}

#[test]
fn one_health_call_per_provider_per_snapshot() {
    let (m, _c) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let gmail = descriptor("gmail.messages.send", "1.0.0", "gmail", None);
    let p1 = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![cal],
        ProviderHealth::Ready,
    ));
    let p2 = Arc::new(ScriptedProvider::new(
        "gmail",
        vec![gmail],
        ProviderHealth::Ready,
    ));
    let c1 = p1.clone();
    let c2 = p2.clone();
    let registry = ProviderRegistry::build(vec![p1, p2]).unwrap();
    let _snap =
        common::build_snapshot(m, &registry, &store(), HostEligibility::new(HashMap::new()))
            .unwrap();
    assert_eq!(c1.health_calls.load(Ordering::SeqCst), 1);
    assert_eq!(c2.health_calls.load(Ordering::SeqCst), 1);
    assert_eq!(c1.caps_calls.load(Ordering::SeqCst), 1);
    assert_eq!(c2.caps_calls.load(Ordering::SeqCst), 1);
}

#[test]
fn pending_health_future_wakes_snapshot_builder_and_snapshot_stays_frozen() {
    let (manifest, catalog) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let provider = Arc::new(WakingProvider {
        id: ProviderId::new("calendar").unwrap(),
        descriptors: vec![cal.clone()],
        health_calls: AtomicUsize::new(0),
        caps_calls: AtomicUsize::new(0),
        ready: std::sync::atomic::AtomicBool::new(true),
    });
    let registry = ProviderRegistry::build(vec![provider.clone()]).unwrap();
    let eligibility = eligibility_for(&cal, &catalog, true);
    let store = store();
    let first = common::build_snapshot(manifest.clone(), &registry, &store, eligibility.clone())
        .expect("woken health future completes");
    let id = CapabilityId::new("calendar.events.read").unwrap();
    assert!(first.resolve(&id).is_ok());
    assert_eq!(provider.health_calls.load(Ordering::SeqCst), 1);
    assert_eq!(provider.caps_calls.load(Ordering::SeqCst), 1);

    provider.ready.store(false, Ordering::SeqCst);
    assert!(first.resolve(&id).is_ok(), "existing snapshot is immutable");
    let second = common::build_snapshot(manifest, &registry, &store, eligibility)
        .expect("next snapshot samples changed health");
    assert!(matches!(
        second.resolve(&id),
        Err(ResolveError::Unavailable { .. })
    ));
    assert_eq!(provider.health_calls.load(Ordering::SeqCst), 2);
    assert_eq!(provider.caps_calls.load(Ordering::SeqCst), 2);
}

#[test]
fn health_change_between_snapshots() {
    let (m, c) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![cal.clone()],
        ProviderHealth::Ready,
    ));
    let pc = p.clone();
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let elig = || eligibility_for(&cal, &c, true);
    let snap1 = common::build_snapshot(m.clone(), &registry, &store(), elig()).unwrap();
    assert!(
        snap1
            .resolve(&CapabilityId::new("calendar.events.read").unwrap())
            .is_ok()
    );
    pc.set_health(ProviderHealth::Degraded);
    let snap2 = common::build_snapshot(m, &registry, &store(), elig()).unwrap();
    assert!(matches!(
        snap2.resolve(&CapabilityId::new("calendar.events.read").unwrap()),
        Err(ResolveError::Unavailable { .. })
    ));
}

#[test]
fn zero_invoke_across_availability() {
    let (m, c) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![cal.clone()],
        ProviderHealth::Ready,
    ));
    let pc = p.clone();
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let elig = eligibility_for(&cal, &c, true);
    let snap = common::build_snapshot(m, &registry, &store(), elig).unwrap();
    let _ = snap.resolve(&CapabilityId::new("calendar.events.read").unwrap());
    assert_eq!(pc.invoke_calls.load(Ordering::SeqCst), 0);
}

#[test]
fn overlay_disabled_unavailable() {
    let (m, c) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![cal.clone()],
        ProviderHealth::Ready,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let store = store();
    let events = EventBus::new(DeterministicUlidSource::new());
    let id = CapabilityId::new("calendar.events.read").unwrap();
    serea_capability::CapabilityRegistry::set_overlay(
        &store,
        &events,
        id.clone(),
        0,
        serea_capability::CapabilityOverlayState::Disabled,
        false,
        serea_protocol::EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    let elig = eligibility_for(&cal, &c, true);
    let snap = common::build_snapshot(m, &registry, &store, elig).unwrap();
    assert!(matches!(
        snap.resolve(&id),
        Err(ResolveError::Unavailable { .. })
    ));
}

#[test]
fn overlay_removed_unavailable() {
    let (m, c) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![cal.clone()],
        ProviderHealth::Ready,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let store = store();
    let events = EventBus::new(DeterministicUlidSource::new());
    let id = CapabilityId::new("calendar.events.read").unwrap();
    serea_capability::CapabilityRegistry::set_overlay(
        &store,
        &events,
        id.clone(),
        0,
        serea_capability::CapabilityOverlayState::Removed,
        false,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    let elig = eligibility_for(&cal, &c, true);
    let snap = common::build_snapshot(m, &registry, &store, elig).unwrap();
    assert!(matches!(
        snap.resolve(&id),
        Err(ResolveError::Unavailable { .. })
    ));
}

#[test]
fn optional_root_candidate_order_follows_manifest_not_name() {
    let c = catalog();
    // The higher-priority candidate is named "aaa-rootless" and the lower one
    // "zzz-rooted": eligibility must decide, never the spelling.
    let rootless = descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        Some("aaa-rootless"),
    );
    let rooted = descriptor(
        "calendar.events.read",
        "1.0.0",
        "calendar",
        Some("zzz-rooted"),
    );
    let with_root = |d: &serea_protocol::CapabilityDescriptor, requirement| {
        let mut draft = CapabilityDescriptorDraft::from(d.clone());
        draft.root_requirement = requirement;
        CapabilityDescriptor::new(draft).unwrap()
    };
    let rootless = with_root(&rootless, RootRequirement::OptionalRoot);
    let rooted = with_root(&rooted, RootRequirement::RequiresRoot);
    let m = CapabilityManifestV1::build(
        vec![entry(&rootless, 0, &c), entry(&rooted, 1, &c)],
        vec![(rootless.id().clone(), rootless.version().clone())],
        c.clone(),
    )
    .unwrap();
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![rootless.clone(), rooted.clone()],
        ProviderHealth::Ready,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    // both candidates eligible: manifest priority decides, so the rootless
    // name wins even though it sorts earlier alphabetically
    let both = HashMap::from([
        (descriptor_semantic_digest(&rootless, &c).unwrap(), true),
        (descriptor_semantic_digest(&rooted, &c).unwrap(), true),
    ]);
    let snap =
        common::build_snapshot(m.clone(), &registry, &store(), HostEligibility::new(both)).unwrap();
    let res = snap.resolve(rootless.id()).unwrap();
    assert_eq!(
        res.implementation_id.as_ref().unwrap().as_str(),
        "aaa-rootless"
    );
    // rootless device: only the rooted candidate is eligible, and the host
    // eligibility fact — not the implementation name — moves the binding
    let only_rooted = HashMap::from([
        (descriptor_semantic_digest(&rootless, &c).unwrap(), false),
        (descriptor_semantic_digest(&rooted, &c).unwrap(), true),
    ]);
    let snap = common::build_snapshot(
        m.clone(),
        &registry,
        &store(),
        HostEligibility::new(only_rooted),
    )
    .unwrap();
    let res = snap.resolve(rootless.id()).unwrap();
    assert_eq!(
        res.implementation_id.as_ref().unwrap().as_str(),
        "zzz-rooted"
    );
    // neither eligible: unavailable
    let neither = HashMap::from([
        (descriptor_semantic_digest(&rootless, &c).unwrap(), false),
        (descriptor_semantic_digest(&rooted, &c).unwrap(), false),
    ]);
    let snap =
        common::build_snapshot(m, &registry, &store(), HostEligibility::new(neither)).unwrap();
    assert!(matches!(
        snap.resolve(rootless.id()),
        Err(ResolveError::Unavailable { .. })
    ));
}

#[test]
fn snapshot_is_immutable_when_overlay_changes_afterwards() {
    let (m, c) = manifest_two();
    let cal = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![cal.clone()],
        ProviderHealth::Ready,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let store = store();
    let events = EventBus::new(DeterministicUlidSource::new());
    let id = CapabilityId::new("calendar.events.read").unwrap();
    let elig = eligibility_for(&cal, &c, true);
    let snap = common::build_snapshot(m.clone(), &registry, &store, elig.clone()).unwrap();
    assert!(snap.resolve(&id).is_ok());
    // disable after the snapshot: the frozen snapshot is unaffected
    serea_capability::CapabilityRegistry::set_overlay(
        &store,
        &events,
        id.clone(),
        0,
        serea_capability::CapabilityOverlayState::Disabled,
        false,
        EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    assert!(snap.resolve(&id).is_ok());
    // the next snapshot sees the new overlay
    let next = common::build_snapshot(m, &registry, &store, elig).unwrap();
    assert!(matches!(
        next.resolve(&id),
        Err(ResolveError::Unavailable { .. })
    ));
}

#[test]
fn experimental_requires_opt_in() {
    let c = catalog();
    let mut offer = descriptor("calendar.events.read", "1.0.0", "calendar", None);
    let mut draft = CapabilityDescriptorDraft::from(offer);
    draft.experimental = true;
    offer = CapabilityDescriptor::new(draft).unwrap();
    let m = CapabilityManifestV1::build(
        vec![entry(&offer, 0, &c)],
        vec![(offer.id().clone(), offer.version().clone())],
        c.clone(),
    )
    .unwrap();
    let p = Arc::new(ScriptedProvider::new(
        "calendar",
        vec![offer.clone()],
        ProviderHealth::Ready,
    ));
    let registry = ProviderRegistry::build(vec![p]).unwrap();
    let store = store();
    // no opt-in -> unavailable
    let elig = eligibility_for(&offer, &c, true);
    let snap = common::build_snapshot(m.clone(), &registry, &store, elig.clone()).unwrap();
    assert!(matches!(
        snap.resolve(&offer.id().clone()),
        Err(ResolveError::Unavailable { .. })
    ));
    // opt-in -> resolvable
    let events = EventBus::new(DeterministicUlidSource::new());
    serea_capability::CapabilityRegistry::set_overlay(
        &store,
        &events,
        offer.id().clone(),
        0,
        serea_capability::CapabilityOverlayState::Enabled,
        true,
        serea_protocol::EpochMillis::new(1_796_000_000_000).unwrap(),
    )
    .unwrap();
    let snap2 = common::build_snapshot(m, &registry, &store, elig).unwrap();
    assert!(snap2.resolve(&offer.id().clone()).is_ok());
}
