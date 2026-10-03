//! Typed protocol parse, validation, and contract errors.
//!
//! Frozen source: `docs/protocols/00-protocol-index.md` §2 (a prefix exists so
//! a mis-routed value "fails loudly at validation"), §4.2 ("Unknown enum
//! variants fail closed everywhere"), and `docs/protocols/06-event-protocol.md`
//! §4 (a control-flow decision must never be made by parsing prose).
//!
//! Two rules shape this module.
//!
//! 1. **Display strings are diagnostic only.** Callers match on the enum
//!    variants, never on the rendered message (`docs/plans/P1-workspace-and-protocol-skeleton.md`,
//!    "Errors").
//! 2. **A rejected value is never echoed.** Every externally-derived protocol
//!    value is untrusted and may be credential-shaped, and an error value can
//!    reach a log line or a crash report. Errors therefore name the *domain*
//!    and a machine-readable *reason*, never the rejected bytes
//!    (`docs/protocols/09-data-classification-protocol.md` §3, `DC7`).

use std::fmt;

/// Why a protocol value was refused.
///
/// Each variant is a distinct machine-readable category; collapsing them into
/// one opaque string would force every caller back to parsing prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    /// An identifier did not match the grammar frozen in Protocol Index §2/§3.
    MalformedIdentifier {
        /// The identifier domain the value was offered to.
        domain: IdentifierDomain,
        /// The precise grammar rule the value broke.
        reason: IdentifierRejection,
    },
    /// A non-identifier protocol value did not match its frozen shape.
    MalformedValue {
        /// The field whose shape was violated.
        field: ValueField,
        /// The precise rule the value broke.
        reason: ValueRejection,
    },
    /// A shape that parses still violates a frozen host rule.
    ContractViolation {
        /// The frozen rule that was broken.
        rule: ContractRule,
    },
    /// A JSON document could not be interpreted at all.
    Serialization {
        /// The surface whose document was unusable.
        surface: &'static str,
        /// Why the document was unusable.
        reason: SerializationRejection,
    },
}

impl ProtocolError {
    /// The identifier domain this error concerns, when it concerns one.
    ///
    /// Returns `None` for every other category, so a caller can branch on the
    /// distinction without matching on prose.
    pub fn domain(self) -> Option<IdentifierDomain> {
        match self {
            ProtocolError::MalformedIdentifier { domain, .. } => Some(domain),
            _ => None,
        }
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ProtocolError::MalformedIdentifier { domain, reason } => {
                write!(f, "malformed {domain} identifier ({reason:?})")
            }
            ProtocolError::MalformedValue { field, reason } => {
                write!(f, "malformed {field} value ({reason:?})")
            }
            ProtocolError::ContractViolation { rule } => {
                write!(f, "frozen contract violation: {rule}")
            }
            ProtocolError::Serialization { surface, reason } => {
                write!(f, "{surface} is not usable as JSON: {reason:?}")
            }
        }
    }
}

impl std::error::Error for ProtocolError {}

/// The identifier domain a rejection belongs to, so a caller can tell a
/// mis-routed `TaskId` from a malformed `Digest` without parsing prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierDomain {
    /// A durable `AssistantTask`.
    TaskId,
    /// One step of an `AssistantTask`.
    StepId,
    /// An `ApprovalRequest`.
    ApprovalId,
    /// An `ApprovalGrant`.
    GrantId,
    /// One request/result correlation across a boundary.
    RequestId,
    /// A `SereaEvent`, an `Evidence` record, or a memory item.
    EventId,
    /// A paired device.
    DeviceId,
    /// A durable schedule.
    ScheduleId,
    /// A proactive-watcher proposal.
    ProposalId,
    /// A `SideEffectReceipt`.
    ReceiptId,
    /// A device session.
    SessionId,
    /// A derived per-step idempotency key.
    IdempotencyKey,
    /// A `<provider>.<resource>.<verb>` capability identifier.
    CapabilityId,
    /// A registered capability namespace.
    ProviderId,
    /// Which implementation is registered behind one `CapabilityId`.
    ImplementationId,
    /// A roster model identifier.
    ModelId,
    /// A `sha256:` content digest over canonical JSON.
    Digest,
}

impl fmt::Display for IdentifierDomain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            IdentifierDomain::TaskId => "TaskId",
            IdentifierDomain::StepId => "StepId",
            IdentifierDomain::ApprovalId => "ApprovalId",
            IdentifierDomain::GrantId => "GrantId",
            IdentifierDomain::RequestId => "RequestId",
            IdentifierDomain::EventId => "EventId",
            IdentifierDomain::DeviceId => "DeviceId",
            IdentifierDomain::ScheduleId => "ScheduleId",
            IdentifierDomain::ProposalId => "ProposalId",
            IdentifierDomain::ReceiptId => "ReceiptId",
            IdentifierDomain::SessionId => "SessionId",
            IdentifierDomain::IdempotencyKey => "IdempotencyKey",
            IdentifierDomain::CapabilityId => "CapabilityId",
            IdentifierDomain::ProviderId => "ProviderId",
            IdentifierDomain::ImplementationId => "ImplementationId",
            IdentifierDomain::ModelId => "ModelId",
            IdentifierDomain::Digest => "Digest",
        })
    }
}

/// The precise grammar rule an identifier broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IdentifierRejection {
    /// The value does not carry the frozen prefix for this domain.
    WrongPrefix,
    /// The body is not the frozen length.
    Length,
    /// The body contains a character outside the frozen alphabet.
    Character,
    /// A 26-character Crockford body whose leading character is above `7`, so
    /// the value does not fit the 128-bit ULID range.
    Range,
    /// A dot-separated identifier without exactly three segments.
    SegmentCount,
    /// A segment violates `[a-z][a-z0-9_]{1,31}`, or names a namespace that is
    /// prohibited in this position.
    Segment,
    /// The body is not exactly 64 lowercase hexadecimal characters.
    NotLowercaseHex,
    /// The `verb` segment is outside the frozen verb set
    /// (Capability Protocol §2).
    UnknownVerb,
    /// The value does not match the frozen `^[a-z0-9]+(-[a-z0-9]+)*$` shape.
    Pattern,
}

/// A non-identifier protocol field that carries its own frozen shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueField {
    /// An RFC 3339 UTC timestamp.
    Timestamp,
    /// A `serea.<surface>/<major>` wire surface name.
    WireSurface,
    /// The major envelope version.
    EnvelopeVersion,
    /// A descriptor SemVer version.
    CapabilityVersion,
    /// A capability schema reference.
    SchemaReference,
    /// The `actor.id` of an event.
    ActorId,
    /// A machine-readable `reason_code`.
    ReasonCode,
    /// A machine-readable `blocked_reason`.
    BlockedReason,
    /// A machine-readable task `failure_reason`.
    FailureReason,
    /// An `ActionError.code`.
    ErrorCode,
    /// An `ActionError.host_action`.
    HostAction,
    /// A diagnostic `ActionError.message`.
    ErrorMessage,
    /// An `AssistantTask.title`.
    TaskTitle,
    /// A `CapabilityDescriptor.title`.
    DescriptorTitle,
    /// A `CapabilityDescriptor.description`.
    DescriptorDescription,
    /// An `ApprovalRequest.plain_summary`.
    PlainSummary,
    /// A `SideEffectReceipt.effect_summary`.
    EffectSummary,
    /// A `SideEffectReceipt.provider_reference`.
    ProviderReference,
    /// A step `lease_owner`.
    LeaseOwner,
    /// An `AssistantTask.origin.kind`.
    TaskOriginKind,
    /// A `TaskStep.status`.
    StepStatus,
    /// A `ModelError` kind code.
    ModelErrorCode,
    /// A conversation message role.
    TokenRole,
    /// A gapless event sequence number, carried as a decimal string.
    SequenceNumber,
    /// A counted model-usage quantity, carried as a decimal string.
    TokenCount,
    /// A positive wire lease fencing generation (ADR-0018 §3).
    LeaseGeneration,
}

impl fmt::Display for ValueField {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ValueField::Timestamp => "timestamp",
            ValueField::WireSurface => "wire surface",
            ValueField::EnvelopeVersion => "envelope version",
            ValueField::CapabilityVersion => "capability version",
            ValueField::SchemaReference => "schema reference",
            ValueField::ActorId => "actor id",
            ValueField::ReasonCode => "reason code",
            ValueField::BlockedReason => "blocked reason",
            ValueField::FailureReason => "failure reason",
            ValueField::ErrorCode => "error code",
            ValueField::HostAction => "host action",
            ValueField::ErrorMessage => "error message",
            ValueField::TaskTitle => "task title",
            ValueField::DescriptorTitle => "descriptor title",
            ValueField::DescriptorDescription => "descriptor description",
            ValueField::PlainSummary => "plain summary",
            ValueField::EffectSummary => "effect summary",
            ValueField::ProviderReference => "provider reference",
            ValueField::LeaseOwner => "lease owner",
            ValueField::TaskOriginKind => "task origin kind",
            ValueField::StepStatus => "task step status",
            ValueField::ModelErrorCode => "model error code",
            ValueField::TokenRole => "message role",
            ValueField::SequenceNumber => "sequence number",
            ValueField::TokenCount => "token count",
            ValueField::LeaseGeneration => "lease generation",
        })
    }
}

/// The precise rule a non-identifier value broke.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueRejection {
    /// Empty, or only whitespace.
    Empty,
    /// Longer than the frozen or schema-declared bound.
    TooLong,
    /// The shape is not the frozen one.
    Malformed,
    /// Syntactically fine but semantically outside the permitted range.
    OutOfRange,
}

/// A frozen host rule a shape-level check can detect without runtime state.
///
/// P1 detects only what the frozen documents state as a *shape* requirement.
/// Behavioural rules — policy evaluation, approval consumption, reconciliation —
/// belong to their owning crates in P2 and later, and are deliberately not
/// represented here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContractRule {
    /// `CapabilityDescriptor.provider_id` must equal the first segment of its
    /// `CapabilityId` (Capability Protocol §3.1; Crate Map §6.1 rule 4).
    CapabilityProviderNamespaceMismatch,
    /// A `CREDENTIAL`-classified capability whose risk class is not
    /// `CREDENTIAL` is "an oxymoron the registry rejects at registration"
    /// (Data Classification §2.3).
    CapabilityCredentialClassContradiction,
    /// A payload declares an envelope major this build does not implement.
    /// Protocol Index §4.2 rule 1 forbids silently accepting it.
    UnsupportedEnvelopeMajor,
    /// A wire surface declares a major this build does not implement.
    /// Protocol Index §4.2 rule 1.
    UnsupportedWireSurfaceMajor,
    /// A supported envelope was delivered to a consumer for another surface.
    UnexpectedWireSurface,
    /// The capability tuple or receipt contradicts the step kind (ADR-0018 §4).
    StepKindFieldPresence,
    /// A known step status contradicts its field-presence matrix (ADR-0018 §3).
    StepStatusFieldPresence,
    /// WAITING is permitted only on the three wait kinds (ADR-0018 §2).
    StepWaitingKind,
    /// An extension duplicates a known TaskStep member, bypassing checked presence.
    StepReservedExtensionKey,
}

impl fmt::Display for ContractRule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            ContractRule::CapabilityProviderNamespaceMismatch => {
                "capability id provider segment does not equal descriptor provider_id"
            }
            ContractRule::CapabilityCredentialClassContradiction => {
                "a CREDENTIAL-class capability must carry risk_class CREDENTIAL"
            }
            ContractRule::UnsupportedEnvelopeMajor => {
                "the envelope major is not implemented by this build"
            }
            ContractRule::UnsupportedWireSurfaceMajor => {
                "the wire surface major is not implemented by this build"
            }
            ContractRule::UnexpectedWireSurface => "the wire surface does not match the consumer",
            ContractRule::StepKindFieldPresence => "step fields contradict the step kind",
            ContractRule::StepStatusFieldPresence => "step fields contradict the known status",
            ContractRule::StepWaitingKind => "WAITING requires a wait step kind",
            ContractRule::StepReservedExtensionKey => "step extension duplicates a known member",
        })
    }
}

/// Why a JSON document could not be interpreted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerializationRejection {
    /// The bytes are not valid JSON.
    MalformedJson,
    /// The bytes are valid JSON but not a JSON object.
    NotAnObject,
    /// The document is a JSON object the validator could not compile.
    SchemaUnusable,
}
