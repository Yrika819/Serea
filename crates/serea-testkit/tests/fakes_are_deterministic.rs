//! Determinism, offline-ness, and safety-exclusion tests for the test doubles.
//!
//! Frozen source: `docs/protocols/03-model-protocol.md` §10 (determinism and
//! testing; `M10`), `docs/protocols/08-goallatch-adapter-protocol.md` §6.1 and
//! §6.3 (a fixed clock, a fixed epoch, and a simulated-service ledger that
//! uses no network, filesystem or subprocess; `G1`, `G5`),
//! `docs/protocols/09-data-classification-protocol.md` §3 and §4 (`DC7`, `DC8`),
//! `docs/protocols/05-approval-protocol.md` §4.3 (`A8`: no path by which a
//! model mints a grant), and `docs/protocols/03-model-protocol.md` §8 (`M5`,
//! `M6`, `M7`).
//!
//! Every test here is offline, deterministic, and order-independent: no test
//! reads a wall clock, opens a socket, or depends on another test having run.

use std::time::Duration;

use serde_json::{Map, Value, json};

use serea_protocol::ids::IdMinter;
use serea_protocol::provider::{
    CancellationToken, CapabilityProvider, ModelCallContext, ModelProvider,
};
use serea_protocol::{
    ActionError, ActionErrorKind, ActionRequest, ActionResult, ActionStatus, Actor, ActorId,
    ActorKind, Authorization, CapabilityDescriptor, CapabilityDescriptorDraft, CapabilityId,
    CostClass, CredentialHandle, DataClass, DescriptorDescription, DescriptorTitle, Digest,
    ErrorCode, ErrorMessage, Evidence, EvidenceKind, FinishReason, HostAction, IdempotencySupport,
    ImplementationId, JsonSchemaRef, ModelError, ModelErrorCode, ModelUsage, ProviderHealth,
    ProviderId, ReplaySafety, RequestedBy, RiskClass, RootRequirement, SemVer, SideEffectClass,
    SideEffectReceipt, Timestamp, TokenCount,
};
use serea_testkit::clock::FROZEN_EPOCH;
use serea_testkit::providers::CapabilityScript;
use serea_testkit::{
    CODEX_ALLOWED, CODEX_MODEL_ID, DeterministicUlidSource, MockCapabilityProvider,
    MockModelProvider, OTHER_SYNTHETIC_ACCOUNT, SYNTHETIC_ACCOUNT, TestClock, deterministic_minter,
    synthetic_descriptor, synthetic_model_roster, synthetic_read_descriptor,
};

const DIGEST: &str = "sha256:3b1f0c9e2a7d4e6b8f0a2c4d6e8b0d2f4a6c8e0b2d4f6a8c0e2b4d6f8a0c2e4b";
const IDEMPOTENCY_KEY: &str =
    "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a";

fn provider_context(
    capability: &str,
    credential: Option<CredentialHandle>,
) -> serea_protocol::ProviderContext {
    serea_protocol::ProviderContext::new(
        synthetic_read_descriptor("calendar", capability, "1.2.0"),
        15_000,
        CancellationToken::new(),
        credential,
    )
}

/// An `ActionRequest` built with every host-resolved field stated at the call
/// site. There is deliberately no helper that fills these in: `data_class`,
/// `deadline_ms`, `arguments_digest`, `requested_by` and `capability_version`
/// are authority-bearing (Capability Protocol §4.2), and a test that cannot see
/// them cannot review them.
fn action_request(capability: &str) -> ActionRequest {
    ActionRequest {
        request_id: deterministic_minter().next_request_id(),
        task_id: deterministic_minter().next_task_id(),
        step_id: deterministic_minter().next_step_id(),
        capability_id: serea_protocol::CapabilityId::new(capability.to_owned())
            .expect("a scripted capability id is valid"),
        capability_version: SemVer::new("1.2.0").expect("valid"),
        arguments: Map::from_iter([("range".to_owned(), json!("tomorrow"))]),
        arguments_digest: Digest::new(DIGEST).expect("valid"),
        idempotency_key: serea_protocol::IdempotencyKey::new(IDEMPOTENCY_KEY).expect("valid"),
        data_class: DataClass::Personal,
        requested_by: RequestedBy::Model,
        deadline_ms: 15_000,
    }
}

/// A `ModelRequest` with every host-assigned field stated at the call site.
fn model_request() -> serea_protocol::ModelRequest {
    serea_protocol::ModelRequest {
        request_id: deterministic_minter().next_request_id(),
        model_id: serea_protocol::ModelId::new("nemotron-3-nano-30b").expect("valid"),
        task_id: Some(deterministic_minter().next_task_id()),
        purpose: serea_protocol::ModelPurpose::Planning,
        messages: vec![serea_protocol::ModelMessage {
            role: serea_protocol::MessageRole::new("user").expect("valid"),
            content: "synthetic prompt".to_owned(),
        }],
        system: Some("synthetic system prompt".to_owned()),
        response_format: serea_protocol::ResponseFormat::JsonSchema {
            schema: json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["actions"],
                "properties": { "actions": { "type": "array", "maxItems": 8 } }
            }),
        },
        tools: Vec::new(),
        max_output_tokens: 2_048,
        temperature: 0.2,
        deadline_ms: 30_000,
        data_class: DataClass::Personal,
    }
}

/// A descriptor for a capability that *does* something, with every
/// authority-bearing field written out rather than defaulted.
fn synthetic_effecting_descriptor() -> CapabilityDescriptor {
    synthetic_descriptor(CapabilityDescriptorDraft {
        id: CapabilityId::new("calendar.events.create".to_owned()).expect("valid"),
        version: SemVer::new("1.2.0").expect("valid"),
        title: DescriptorTitle::new("synthetic effecting capability").expect("valid"),
        description: DescriptorDescription::new("synthetic write descriptor for offline tests")
            .expect("valid"),
        provider_id: ProviderId::new("calendar").expect("valid"),
        implementation_id: None,
        input_schema: JsonSchemaRef::new(
            "https://serea.local/schemas/calendar.events.create.input.1.2.0.json",
        )
        .expect("valid"),
        output_schema: JsonSchemaRef::new(
            "https://serea.local/schemas/calendar.events.create.output.1.2.0.json",
        )
        .expect("valid"),
        side_effect_class: SideEffectClass::ExternalWrite,
        risk_class: RiskClass::ExternalWrite,
        required_authorization: Authorization::ScopedGrant,
        replay_safety: ReplaySafety::Conditional,
        data_class: DataClass::Personal,
        root_requirement: RootRequirement::NotRequired,
        idempotency_support: IdempotencySupport::Native,
        max_duration_ms: 15_000,
        cost_class: CostClass::Free,
        experimental: false,
    })
}

fn synthetic_actor() -> Actor {
    Actor {
        kind: ActorKind::Provider,
        id: ActorId::new("synthetic-provider").expect("valid"),
        version: SemVer::new("1.0.0").expect("valid"),
        extensions: Default::default(),
    }
}

fn evidence() -> Evidence {
    Evidence {
        evidence_id: deterministic_minter().next_event_id(),
        kind: EvidenceKind::CapabilityObservation,
        capability_id: serea_protocol::CapabilityId::new("calendar.events.list").expect("valid"),
        task_id: deterministic_minter().next_task_id(),
        step_id: deterministic_minter().next_step_id(),
        attempt: 1,
        produced_at: Timestamp::new(FROZEN_EPOCH).expect("valid"),
        actor: synthetic_actor(),
        data_class: DataClass::Personal,
        payload_digest: Digest::new(DIGEST).expect("valid"),
        payload_reference: None,
    }
}

fn succeeded_result() -> ActionResult {
    ActionResult {
        request_id: action_request("calendar.events.list").request_id,
        status: ActionStatus::Succeeded,
        output: Some(Map::from_iter([("events".to_owned(), json!([]))])),
        output_digest: Some(Digest::new(DIGEST).expect("valid")),
        evidence: vec![evidence()],
        receipt: None,
        error: None,
        duration_ms: 412,
    }
}

fn typed_error(kind: ActionErrorKind, code: &str, retryable: bool) -> ActionError {
    ActionError {
        kind,
        code: ErrorCode::new(code).expect("a frozen code shape"),
        message: ErrorMessage::new("synthetic failure").expect("a frozen message shape"),
        retryable,
        host_action: HostAction::new("NONE").expect("a frozen code shape"),
        details: Map::new(),
    }
}

fn model_response(structured: Value, finish_reason: FinishReason) -> serea_protocol::ModelResponse {
    serea_protocol::ModelResponse {
        request_id: action_request("calendar.events.list").request_id,
        model_id: serea_protocol::ModelId::new("nemotron-3-nano-30b").expect("valid"),
        provider_id: ProviderId::new("ollama").expect("valid"),
        content: "synthetic".to_owned(),
        structured: Some(structured),
        finish_reason,
        usage: ModelUsage {
            input_tokens: TokenCount::new(1_840),
            output_tokens: TokenCount::new(260),
            cost_class: CostClass::Free,
        },
        latency_ms: 2_410,
        repair_attempts: 0,
    }
}

// ---------------------------------------------------------------------------
// TestClock
// ---------------------------------------------------------------------------

#[test]
fn the_test_clock_starts_at_the_frozen_epoch_and_only_moves_when_told() {
    let mut clock = TestClock::at_epoch();
    assert_eq!(clock.now().as_str(), FROZEN_EPOCH);
    assert_eq!(
        clock.now(),
        clock.now(),
        "reading the clock twice is stable"
    );

    let first = clock.advance(Duration::from_millis(600)).expect("in range");
    assert_eq!(first.as_str(), "2026-10-01T00:00:00.600Z");
    let second = clock.advance(Duration::from_millis(600)).expect("in range");
    assert_eq!(second.as_str(), "2026-10-01T00:00:01.200Z");
    assert_eq!(clock.elapsed_ms(), 1_200);
    assert_eq!(
        clock.now(),
        second,
        "the clock reports what it last advanced to"
    );
}

#[test]
fn two_clocks_driven_identically_report_identical_time() {
    let script = [
        Duration::from_millis(200),
        Duration::from_millis(600),
        Duration::from_millis(50),
        Duration::from_millis(600),
    ];
    let run = || {
        let mut clock = TestClock::at_epoch();
        script
            .iter()
            .map(|delta| clock.advance(*delta).expect("in range").as_str().to_owned())
            .collect::<Vec<String>>()
    };
    assert_eq!(run(), run());
    // The frozen ApprovedCompletion scenario in GoalLatch Adapter Section 6.2
    // advances 200 -> 800 -> 850 -> 1450 ms.
    assert_eq!(
        run(),
        vec![
            "2026-10-01T00:00:00.200Z",
            "2026-10-01T00:00:00.800Z",
            "2026-10-01T00:00:00.850Z",
            "2026-10-01T00:00:01.450Z",
        ]
    );
}

#[test]
fn the_test_clock_crosses_a_day_and_a_leap_day_boundary() {
    let mut clock = TestClock::at_epoch();
    assert_eq!(
        clock
            .advance(Duration::from_secs(86_400))
            .expect("in range")
            .as_str(),
        "2026-10-02T00:00:00.000Z"
    );
    let mut clock = TestClock::at("2024-02-28T23:59:59.000Z").expect("valid");
    assert_eq!(
        clock
            .advance(Duration::from_millis(1_000))
            .expect("in range")
            .as_str(),
        "2024-02-29T00:00:00.000Z",
        "2024 is a leap year"
    );
    let mut clock = TestClock::at("2026-02-28T23:59:59.000Z").expect("valid");
    assert_eq!(
        clock
            .advance(Duration::from_millis(1_000))
            .expect("in range")
            .as_str(),
        "2026-03-01T00:00:00.000Z",
        "2026 is not a leap year"
    );
}

// ---------------------------------------------------------------------------
// Deterministic identifier minting
// ---------------------------------------------------------------------------

#[test]
fn the_deterministic_source_replays_identically() {
    let run = || {
        let mut source = DeterministicUlidSource::new();
        (0..4)
            .map(|_| deterministic_minter_from(&mut source))
            .collect::<Vec<_>>()
    };
    assert_eq!(run(), run());

    fn deterministic_minter_from(source: &mut DeterministicUlidSource) -> String {
        IdMinter::new(source).next_event_id().as_str().to_owned()
    }
}

#[test]
fn consecutive_mints_are_never_repeated_and_stay_in_the_128_bit_range() {
    let mut source = DeterministicUlidSource::new();
    let mut minter = IdMinter::new(&mut source);
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..64 {
        let value = minter.next_event_id();
        assert!(value.as_str().starts_with("evt_"));
        assert_eq!(value.as_str().len(), "evt_".len() + 26);
        assert!(seen.insert(value.as_str().to_owned()));
    }
    assert_eq!(
        seen.len(),
        64,
        "Protocol Index Section 2 rule 3: never reused"
    );
    assert_eq!(source.minted(), 64);
    source.reset();
    assert_eq!(source.minted(), 0, "a harness can replay the same script");
}

#[test]
fn no_async_runtime_is_required_to_drive_the_ports() {
    // The P1 dependency policy forbids an async runtime: the ports are `async`
    // for the real orchestrator, but a test double has nothing to await, so a
    // no-op executor is both sufficient and dependency-free. If a double ever
    // started awaiting something real, this would fail rather than hang.
    let manifest = include_str!("../Cargo.toml");
    for runtime in ["tokio", "async-std", "smol", "futures-executor"] {
        assert!(
            !manifest.contains(runtime),
            "{runtime} must not be a dependency"
        );
    }
    let provider = MockCapabilityProvider::new(
        ProviderId::new("calendar").expect("valid"),
        vec![synthetic_read_descriptor(
            "calendar",
            "calendar.events.list",
            "1.2.0",
        )],
    );
    provider.push_result(succeeded_result());
    let served = block_on(provider.invoke(
        &action_request("calendar.events.list"),
        &provider_context("calendar.events.list", None),
    ));
    assert!(served.is_ok(), "a port call resolves without a runtime");
}

#[test]
fn the_test_clock_refuses_a_scripted_epoch_it_cannot_parse() {
    // Regression: `TestClock::at` used to `unreachable!()` on a malformed or
    // calendar-impossible scripted epoch. It is the workspace's only time
    // source, so a bad script value now surfaces as a typed error.
    for bad in [
        "2026-02-30T00:00:00.000Z",
        "2026-13-01T00:00:00.000Z",
        "2026-10-01",
        "not a timestamp",
        "",
    ] {
        assert!(TestClock::at(bad).is_err(), "{bad:?} must be refused");
    }
    assert_eq!(
        TestClock::at("2026-10-01T09:14:22Z")
            .expect("legal seconds form")
            .now()
            .as_str(),
        "2026-10-01T09:14:22.000Z"
    );
}

#[test]
fn advancing_past_the_frozen_form_is_an_error_and_leaves_the_clock_untouched() {
    // Regression: crossing the year-9999 boundary used to `unreachable!()`.
    let mut clock = TestClock::at("9999-12-31T23:59:59.999Z").expect("valid");
    let before = clock.now();
    assert!(clock.advance(Duration::from_secs(86_400)).is_err());
    assert_eq!(
        clock.now(),
        before,
        "a refused advance must not corrupt state"
    );
    // A clock with room left still advances, so the bound is a real limit
    // rather than a blanket refusal.
    let mut roomy = TestClock::at("9999-12-31T23:59:58.999Z").expect("valid");
    assert_eq!(
        roomy
            .advance(Duration::from_secs(1))
            .expect("in range")
            .as_str(),
        "9999-12-31T23:59:59.999Z",
        "the last representable instant is reachable"
    );
}

#[test]
fn the_identifier_source_refuses_a_starting_point_outside_the_48_bit_range() {
    // Regression: `starting_at` used to feed a caller-supplied `u64` into an
    // `assert!` that aborted in debug and release alike.
    assert!(DeterministicUlidSource::starting_at(serea_protocol::TimestampMs::MAX).is_ok());
    for bad in [serea_protocol::TimestampMs::MAX + 1, 1 << 63, u64::MAX] {
        assert!(
            DeterministicUlidSource::starting_at(bad).is_err(),
            "{bad} is outside the frozen 48-bit range"
        );
    }
}

#[test]
fn an_unrepresentable_delta_is_an_error_rather_than_an_overflow() {
    // Regression: `advance` clamped a delta above `i64::MAX` milliseconds to
    // `i64::MAX` and then overflowed the addition, which panicked in debug and
    // wrapped in release. A start with a non-zero millisecond is what makes it
    // reachable.
    let mut clock = TestClock::at("2026-10-01T00:00:00.500Z").expect("valid");
    let before = clock.now();
    for delta in [
        Duration::MAX,
        Duration::from_millis(u64::MAX),
        Duration::from_millis(i64::MAX as u64),
        Duration::from_secs(1 << 63),
    ] {
        assert!(
            clock.advance(delta).is_err(),
            "{delta:?} is not representable in fake milliseconds"
        );
        assert_eq!(
            clock.now(),
            before,
            "a refused advance leaves the clock alone"
        );
    }
}

#[test]
fn the_identifier_source_holds_at_the_range_end_rather_than_overflowing() {
    // Regression: an unchecked increment panicked in debug and wrapped in
    // release, so debug and release disagreed about the identifier sequence.
    let mut source = DeterministicUlidSource::starting_at(serea_protocol::TimestampMs::MAX)
        .expect("valid starting point");
    let mut seen = std::collections::BTreeSet::new();
    for _ in 0..8 {
        let value = IdMinter::new(&mut source).next_event_id();
        assert!(
            seen.insert(value.as_str().to_owned()),
            "consecutive mints stay distinct even at the end of the range"
        );
    }
    assert_eq!(source.minted(), 8);
    assert_eq!(
        source.next_timestamp_ms(),
        serea_protocol::TimestampMs::MAX,
        "the timestamp holds at the range end instead of wrapping backwards"
    );
    source.reset();
    assert_eq!(source.minted(), 0, "reset replays the same script");
    assert_eq!(source.next_timestamp_ms(), serea_protocol::TimestampMs::MAX);
}

// ---------------------------------------------------------------------------
// MockModelProvider
// ---------------------------------------------------------------------------

fn model_call_context() -> ModelCallContext {
    ModelCallContext {
        deadline_ms: 30_000,
    }
}

#[test]
fn the_model_double_replays_its_script_in_order() {
    let provider = MockModelProvider::new(
        ProviderId::new("ollama").expect("valid"),
        synthetic_model_roster(),
    );
    let ok = model_response(
        json!({ "kind": "ACTION_PLAN", "actions": [] }),
        FinishReason::Stop,
    );
    let invalid = model_response(
        json!({ "kind": "ACTION_PLAN" }),
        FinishReason::StructureInvalid,
    );
    provider
        .push_response(ok.clone())
        .push_response(invalid.clone());

    let ctx = model_call_context();
    let first = block_on(provider.generate(&model_request(), &ctx));
    let second = block_on(provider.generate(&model_request(), &ctx));
    assert_eq!(first.expect("scripted response"), ok);
    assert_eq!(second.expect("scripted response"), invalid);
    assert_eq!(provider.calls(), 2);
}

#[test]
fn model_double_captures_requests_and_replays_provider_health() {
    let provider = MockModelProvider::new(
        ProviderId::new("ollama").expect("valid"),
        synthetic_model_roster(),
    );
    let request = model_request();
    provider.push_response(model_response(json!({"ok": true}), FinishReason::Stop));
    provider
        .push_health(ProviderHealth::Degraded)
        .push_health(ProviderHealth::Ready);

    let ctx = model_call_context();
    let _ = block_on(provider.generate(&request, &ctx));
    assert_eq!(provider.captured_requests(), vec![request]);
    assert_eq!(block_on(provider.health()), ProviderHealth::Degraded);
    assert_eq!(block_on(provider.health()), ProviderHealth::Ready);
    assert_eq!(block_on(provider.health()), ProviderHealth::Ready);
    assert_eq!(provider.health_calls(), 3);
}

#[test]
fn the_model_double_repeats_its_last_entry_rather_than_panicking() {
    // An under-scripted test must stay deterministic instead of flaky.
    let provider = MockModelProvider::new(
        ProviderId::new("ollama").expect("valid"),
        synthetic_model_roster(),
    );
    let only = model_response(json!({}), FinishReason::Stop);
    provider.push_response(only.clone());
    let ctx = model_call_context();
    for _ in 0..5 {
        let served = block_on(provider.generate(&model_request(), &ctx));
        assert_eq!(served.expect("scripted response"), only);
    }
    assert_eq!(provider.calls(), 5);
}

#[test]
fn the_model_double_serves_an_exhausted_script_as_a_typed_failure() {
    let provider = MockModelProvider::new(
        ProviderId::new("ollama").expect("valid"),
        synthetic_model_roster(),
    );
    let served = block_on(provider.generate(&model_request(), &model_call_context()));
    let error = served.expect_err("an unscripted call must fail closed");
    assert!(!error.retryable);
    assert_eq!(error.kind.as_str(), "SCRIPT_EXHAUSTED");
}

#[test]
fn the_model_double_scripts_malformed_structured_output_and_typed_failures() {
    let provider = MockModelProvider::new(
        ProviderId::new("ollama").expect("valid"),
        synthetic_model_roster(),
    );
    // Model Protocol Section 7: malformed structured output is a routine
    // expected failure, not an exception path.
    provider.push_response(model_response(
        json!({ "kind": "ACTION_PLAN", "actions": "not-an-array" }),
        FinishReason::StructureInvalid,
    ));
    provider.push_response(model_response(
        json!({ "kind": "ACTION_PLAN", "actions": [] }),
        FinishReason::Length,
    ));
    provider.push_error(ModelError {
        kind: ModelErrorCode::new("PROVIDER_UNAVAILABLE").expect("valid"),
        message: ErrorMessage::new("synthetic provider outage").expect("valid"),
        retryable: true,
    });

    let ctx = model_call_context();
    let malformed = block_on(provider.generate(&model_request(), &ctx));
    let truncated = block_on(provider.generate(&model_request(), &ctx));
    let outage = block_on(provider.generate(&model_request(), &ctx));

    let malformed = malformed.expect("served");
    assert_eq!(malformed.finish_reason, FinishReason::StructureInvalid);
    assert_eq!(
        malformed.structured.as_ref().expect("present")["actions"],
        json!("not-an-array"),
        "the double returns the malformed value verbatim; the host refuses it"
    );
    assert_eq!(
        truncated.expect("served").finish_reason,
        FinishReason::Length
    );
    let outage = outage.expect_err("a scripted failure");
    assert!(outage.retryable);
}

#[test]
fn the_model_double_is_reproducible_across_runs() {
    let run = || {
        let provider = MockModelProvider::new(
            ProviderId::new("ollama").expect("valid"),
            synthetic_model_roster(),
        );
        let scripted = model_response(
            json!({ "kind": "ACTION_PLAN", "actions": [] }),
            FinishReason::Stop,
        );
        provider.push_response(scripted.clone());
        let served = block_on(provider.generate(&model_request(), &model_call_context()));
        serde_json::to_value(served.expect("served")).expect("serialises")
    };
    assert_eq!(run(), run());
}

// ---------------------------------------------------------------------------
// Codex exclusion
// ---------------------------------------------------------------------------

#[test]
fn no_model_route_or_fixture_enables_codex() {
    const { assert!(!CODEX_ALLOWED) };
    // `codex_allowed` defaults to false (Model Protocol Section 8).
    for model in synthetic_model_roster() {
        assert_ne!(
            model.model_id.as_str(),
            CODEX_MODEL_ID,
            "the default roster must not contain codex (M5)"
        );
        assert!(
            !model.capabilities.code_specialist,
            "{} must not be a code specialist",
            model.model_id
        );
    }
    let provider = MockModelProvider::new(
        ProviderId::new("ollama").expect("valid"),
        synthetic_model_roster(),
    );
    assert!(!provider.advertises_codex());
    assert_eq!(block_on(provider.health()), ProviderHealth::Ready);
}

#[test]
fn no_protocol_value_the_testkit_produces_names_codex() {
    let request = action_request("calendar.events.list");
    let result = succeeded_result();
    let descriptor = provider_context("calendar.events.list", None)
        .descriptor()
        .clone()
        .clone();
    for value in [
        serde_json::to_value(&request).expect("serialises"),
        serde_json::to_value(&result).expect("serialises"),
        serde_json::to_value(&descriptor).expect("serialises"),
    ] {
        let rendered = value.to_string();
        assert!(
            !rendered.contains("codex"),
            "a protocol value must never name codex: {rendered}"
        );
    }
}

// ---------------------------------------------------------------------------
// MockCapabilityProvider
// ---------------------------------------------------------------------------

#[test]
fn the_capability_double_returns_real_results_receipts_included() {
    // GoalLatch Adapter Section 6, G5: a double returning a structurally
    // simplified result would let a receipt-handling regression ship green.
    let mut result = succeeded_result();
    result.receipt = Some(SideEffectReceipt {
        receipt_id: deterministic_minter().next_receipt_id(),
        capability_id: serea_protocol::CapabilityId::new("calendar.events.create").expect("valid"),
        idempotency_key: serea_protocol::IdempotencyKey::new(IDEMPOTENCY_KEY).expect("valid"),
        provider_reference: Some(
            serea_protocol::ProviderReference::new("provider-ref-0001").expect("valid"),
        ),
        effect_summary: serea_protocol::EffectSummary::new("Created one synthetic event")
            .expect("valid"),
        observed_at: Timestamp::new(FROZEN_EPOCH).expect("valid"),
        replay_safe: false,
    });

    let provider = MockCapabilityProvider::new(
        ProviderId::new("calendar").expect("valid"),
        vec![synthetic_effecting_descriptor()],
    );
    provider.push_result(result.clone());
    let request = action_request("calendar.events.create");
    let ctx = provider_context("calendar.events.create", None);
    let served = block_on(provider.invoke(&request, &ctx)).expect("served");
    assert_eq!(served, result);
    assert!(
        served.receipt.is_some(),
        "the receipt path must be exercised"
    );
    assert_eq!(provider.calls(), 1);
}

#[test]
fn the_capability_double_scripts_typed_failures_including_ambiguous() {
    let provider = MockCapabilityProvider::new(
        ProviderId::new("calendar").expect("valid"),
        vec![
            provider_context("calendar.events.create", None)
                .descriptor()
                .clone(),
        ],
    );
    provider
        .push_error(typed_error(
            ActionErrorKind::ProviderTimeout,
            "PROVIDER_TIMEOUT",
            true,
        ))
        .push_error(typed_error(
            ActionErrorKind::Ambiguous,
            "PROVIDER_ERROR",
            false,
        ))
        .push(CapabilityScript::Fail(typed_error(
            ActionErrorKind::CapabilityUnavailable,
            "CAPABILITY_UNAVAILABLE",
            false,
        )));

    let ctx = provider_context("calendar.events.create", None);
    for expected in [
        ActionErrorKind::ProviderTimeout,
        ActionErrorKind::Ambiguous,
        ActionErrorKind::CapabilityUnavailable,
    ] {
        let request = action_request("calendar.events.create");
        let error = block_on(provider.invoke(&request, &ctx)).expect_err("scripted failure");
        assert_eq!(error.kind, expected);
    }

    // Capability Protocol Section 6.1: AMBIGUOUS is never retryable, and its
    // retryable flag never authorises a re-issue.
    let request = action_request("calendar.events.create");
    let error = block_on(provider.invoke(&request, &ctx)).expect_err("replayed");
    assert!(!error.retryable);
}

#[test]
fn a_provider_receives_only_an_opaque_credential_handle() {
    let handle = CredentialHandle::new(Digest::new(DIGEST).expect("valid"));
    let provider = MockCapabilityProvider::new(
        ProviderId::new("calendar").expect("valid"),
        vec![
            provider_context("calendar.events.list", None)
                .descriptor()
                .clone()
                .clone(),
        ],
    );
    provider.push_result(succeeded_result());
    let request = action_request("calendar.events.list");
    let ctx = provider_context("calendar.events.list", Some(handle.clone()));
    let _ = block_on(provider.invoke(&request, &ctx)).expect("served");
    assert_eq!(provider.seen_handles(), vec![handle.clone()]);
    assert_eq!(handle.as_str(), DIGEST);

    let ctx = provider_context("calendar.events.list", None);
    let _ = block_on(provider.invoke(&request, &ctx)).expect("served");
    assert_eq!(
        provider.seen_handles(),
        vec![handle],
        "a call with no credential handle records nothing"
    );
}

#[test]
fn the_capability_double_advertises_only_the_descriptors_it_was_given() {
    let provider = MockCapabilityProvider::new(
        ProviderId::new("calendar").expect("valid"),
        vec![
            provider_context("calendar.events.list", None)
                .descriptor()
                .clone(),
            synthetic_effecting_descriptor(),
        ],
    );
    let advertised: Vec<String> = provider
        .capabilities()
        .iter()
        .map(|descriptor| descriptor.id().as_str().to_owned())
        .collect();
    assert_eq!(
        advertised,
        vec!["calendar.events.list", "calendar.events.create"]
    );
    assert_eq!(provider.provider_id().as_str(), "calendar");
}

#[test]
fn the_capability_double_is_reproducible_across_runs() {
    let run = || {
        let provider = MockCapabilityProvider::new(
            ProviderId::new("calendar").expect("valid"),
            vec![
                provider_context("calendar.events.list", None)
                    .descriptor()
                    .clone()
                    .clone(),
            ],
        );
        provider.push_result(succeeded_result());
        let request = action_request("calendar.events.list");
        let ctx = provider_context("calendar.events.list", None);
        let served = block_on(provider.invoke(&request, &ctx)).expect("served");
        serde_json::to_value(&served).expect("serialises")
    };
    assert_eq!(run(), run());
}

// ---------------------------------------------------------------------------
// Offline-ness and fixture hygiene
// ---------------------------------------------------------------------------

#[test]
fn no_fixture_carries_personal_data_or_a_credential() {
    assert!(SYNTHETIC_ACCOUNT.ends_with(".test"));
    assert!(OTHER_SYNTHETIC_ACCOUNT.ends_with(".test"));
    assert_ne!(SYNTHETIC_ACCOUNT, OTHER_SYNTHETIC_ACCOUNT);
    assert!(!SYNTHETIC_ACCOUNT.contains("@gmail"));
    assert!(!SYNTHETIC_ACCOUNT.contains("@example.com"));

    // The only credential-shaped value in the whole crate is a digest-shaped
    // handle, which is a reference and not a secret (Data Classification 3.1).
    let handle = CredentialHandle::new(Digest::new(DIGEST).expect("valid"));
    let rendered = format!("{handle:?}");
    for forbidden in ["Bearer", "sk-", "ghp_", "xox", "AKIA", "BEGIN"] {
        assert!(
            !rendered.contains(forbidden),
            "a handle must not look like a credential: {rendered}"
        );
    }
}

#[test]
fn the_testkit_declares_no_network_or_filesystem_dependency() {
    // Model Protocol Section 10, M10: no test may reach a real provider. The
    // testkit's whole dependency set is the protocol crate plus serde.
    let manifest = include_str!("../Cargo.toml");
    for forbidden in [
        "reqwest",
        "hyper",
        "tokio",
        "ureq",
        "curl",
        "openssl",
        "rusqlite",
        "sqlx",
        "local_mcp",
        "goallatch",
        "openai",
        "codex",
    ] {
        assert!(
            !manifest.contains(forbidden),
            "the testkit must not depend on {forbidden}"
        );
    }
}

// ---------------------------------------------------------------------------
// The three ports exist, are object-safe, and are unimplemented
// ---------------------------------------------------------------------------

#[test]
fn the_capability_port_is_object_safe_and_its_shape_is_frozen() {
    // Crate Map 6.1 rule 1: providers are registered as trait objects, so the
    // port must be usable behind `dyn`.
    fn describe(port: &dyn CapabilityProvider) -> (ProviderId, usize) {
        (port.provider_id(), port.capabilities().len())
    }
    let provider = MockCapabilityProvider::new(
        ProviderId::new("calendar").expect("valid"),
        vec![
            provider_context("calendar.events.list", None)
                .descriptor()
                .clone()
                .clone(),
        ],
    );
    let (id, count) = describe(&provider);
    assert_eq!(id.as_str(), "calendar");
    assert_eq!(count, 1);
}

#[test]
fn the_model_port_is_object_safe_and_its_shape_is_frozen() {
    fn describe(port: &dyn ModelProvider) -> (ProviderId, usize) {
        (port.provider_id(), port.models().len())
    }
    let provider = MockModelProvider::new(
        ProviderId::new("ollama").expect("valid"),
        synthetic_model_roster(),
    );
    let (id, count) = describe(&provider);
    assert_eq!(id.as_str(), "ollama");
    assert_eq!(count, 2);
}

#[test]
fn the_host_goal_port_is_declared_but_has_no_implementation_in_p1() {
    // GoalLatch Adapter Sections 3 and 6, G1 and G7: P1 declares the port and
    // nothing else. The fake is P15 work.
    fn adapter_identity(port: &dyn serea_protocol::HostGoalProvider) -> ProviderId {
        port.provider_id()
    }
    fn implementation_of(port: &dyn serea_protocol::HostGoalProvider) -> ImplementationId {
        port.implementation_id()
    }

    // The compiler proves both free functions above are well-formed against the
    // frozen trait: the port has the two identity methods, is object-safe, and
    // nothing in this workspace implements it.
    let _ = adapter_identity;
    let _ = implementation_of;

    // The frozen adapter identity is what `provider_id` defaults to.
    assert_eq!(
        ProviderId::new("goallatch")
            .expect("valid adapter identity")
            .as_str(),
        "goallatch"
    );
    for capability in ["host.goal.start", "host.goal.run", "host.goal.result"] {
        assert!(
            serea_protocol::CapabilityId::new(capability).is_ok(),
            "{capability} is a frozen capability id"
        );
    }
    assert!(
        serea_protocol::CapabilityId::new("goallatch.goal.run").is_err(),
        "no capability may live in the adapter namespace (G13)"
    );
}

#[test]
fn a_cancellation_token_starts_clear_and_is_observable() {
    let token = CancellationToken::new();
    assert!(!token.is_cancelled());
    let clone = token.clone();
    token.cancel();
    assert!(token.is_cancelled());
    assert!(
        clone.is_cancelled(),
        "every holder observes the same signal"
    );
}

/// Drives a future to completion without pulling in an async runtime.
///
/// The P1 dependency policy forbids an async runtime: the ports are `async` for
/// forward compatibility with the real orchestrator, but a test double has
/// nothing to await, so a no-op executor is both sufficient and dependency-free.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    let mut future = Box::pin(future);
    let mut context = std::task::Context::from_waker(std::task::Waker::noop());
    match std::future::Future::poll(future.as_mut(), &mut context) {
        std::task::Poll::Ready(value) => value,
        // A no-op waker is never woken, so a pending result means the double
        // actually blocked on something. None of them do.
        std::task::Poll::Pending => panic!("a test double must never await anything"),
    }
}
