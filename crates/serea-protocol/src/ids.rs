//! Strongly typed protocol identifiers.
//!
//! Frozen source: `docs/protocols/00-protocol-index.md` §2 (identifier
//! grammar) and §3 (capability identifier grammar), plus the frozen verb set
//! in `docs/protocols/01-capability-protocol.md` §2.
//!
//! Every identifier is an opaque newtype. The rules this module enforces:
//!
//! * a value carries its domain's frozen prefix, so a mis-routed identifier
//!   fails loudly instead of being accepted by the wrong subsystem
//!   (Protocol Index §2 rule 2);
//! * the only minting scheme is a 26-character Crockford Base32 ULID body;
//! * **no accessor exposes a timestamp or a prefix separately.** The only read
//!   accessor is `as_str()`, so no consumer can recover creation time from an
//!   identifier even though a ULID body is time-ordered (Protocol Index §2
//!   rule 4);
//! * the newtypes are deliberately **not** `Ord`, so an identifier cannot be
//!   used as a proxy for time.

use std::fmt;
use std::str::FromStr;

use serde::de::{self, Deserializer};
use serde::{Deserialize, Serialize, Serializer};

use crate::errors::{
    IdentifierDomain, IdentifierRejection, ProtocolError, ValueField, ValueRejection,
};

/// Crockford Base32: the digits plus the uppercase letters, excluding the
/// easily confused `I`, `L`, `O` and `U` (Protocol Index §2 rule 1).
const CROCKFORD_ALPHABET: &[u8; 32] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
const ULID_BODY_LENGTH: usize = 26;
/// A 26-character Base32 value stops fitting the 128-bit ULID range once the
/// leading character is above `7`.
const ULID_MAX_LEADING: u8 = b'7';
const HEX_LENGTH: usize = 64;
/// `[a-z][a-z0-9_]{1,31}` — Protocol Index §3.
const SEGMENT_MIN_LENGTH: usize = 2;
const SEGMENT_MAX_LENGTH: usize = 32;
const CAPABILITY_SEGMENT_COUNT: usize = 3;
/// GoalLatch Adapter §3.2: `goallatch` is the adapter identity, never a
/// registered capability namespace (`G13`).
const PROHIBITED_NAMESPACES: [&str; 1] = ["goallatch"];

/// The frozen `verb` segment of a `CapabilityId` (Capability Protocol §2).
///
/// Adding a verb is an architecture-minor change and requires an ADR
/// (Protocol Index §3); this list is the P0 freeze.
pub const CAPABILITY_VERBS: [&str; 14] = [
    "list", "read", "search", "open", "control", "write", "create", "send", "delete", "start",
    "status", "run", "cancel", "result",
];

fn malformed(domain: IdentifierDomain, reason: IdentifierRejection) -> ProtocolError {
    ProtocolError::MalformedIdentifier { domain, reason }
}

/// True when `segment` matches `[a-z][a-z0-9_]{1,31}`.
fn is_namespace_segment(segment: &str) -> bool {
    let bytes = segment.as_bytes();
    matches!(bytes.len(), SEGMENT_MIN_LENGTH..=SEGMENT_MAX_LENGTH)
        && bytes[0].is_ascii_lowercase()
        && bytes[1..]
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'_')
}

/// True when `value` matches `^[a-z0-9]+(-[a-z0-9]+)*$`.
fn is_kebab_pattern(value: &str) -> bool {
    value.split('-').all(|group| {
        !group.is_empty()
            && group
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit())
    })
}

/// Declares an opaque identifier newtype: a validating constructor, a
/// validating `FromStr`, `Display`, and a fail-closed `Deserialize` that
/// routes through the same validator as `new` (Protocol Index §4.2 rule 3).
macro_rules! declare_id {
    ($(#[$meta:meta])* $name:ident, $domain:ident, $prefix:literal, $validate:expr) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            /// The frozen wire prefix for this domain, or `""` for the
            /// identifier families the frozen grammar does not prefix.
            pub const PREFIX: &'static str = $prefix;
            /// The frozen identifier domain, for machine-readable errors.
            pub const DOMAIN: IdentifierDomain = IdentifierDomain::$domain;

            /// Validates `value` against this domain's frozen grammar.
            pub fn new(value: impl Into<String>) -> Result<Self, ProtocolError> {
                #[allow(clippy::redundant_closure_call)]
                let validator: fn(String) -> Result<String, ProtocolError> = $validate;
                validator(value.into()).map(Self)
            }

            /// The exact wire form. This is the only read accessor; nothing here
            /// exposes a timestamp or a prefix separately.
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }

        impl FromStr for $name {
            type Err = ProtocolError;
            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Self::new(value)
            }
        }

        impl Serialize for $name {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                serializer.serialize_str(&self.0)
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                let raw = String::deserialize(deserializer)?;
                // The rejected value is not echoed: it is externally derived
                // and may be credential-shaped (Data Classification §3, DC7).
                Self::new(raw).map_err(de::Error::custom)
            }
        }
    };
}

macro_rules! ulid_validator {
    ($domain:ident, $prefix:literal) => {
        |value: String| -> Result<String, ProtocolError> {
            let domain = IdentifierDomain::$domain;
            let Some(body) = value.strip_prefix($prefix) else {
                return Err(malformed(domain, IdentifierRejection::WrongPrefix));
            };
            let bytes = body.as_bytes();
            if bytes.len() != ULID_BODY_LENGTH {
                return Err(malformed(domain, IdentifierRejection::Length));
            }
            if !bytes.iter().all(|b| CROCKFORD_ALPHABET.contains(b)) {
                return Err(malformed(domain, IdentifierRejection::Character));
            }
            if bytes[0] > ULID_MAX_LEADING {
                return Err(malformed(domain, IdentifierRejection::Range));
            }
            Ok(value)
        }
    };
}

macro_rules! hex_validator {
    ($domain:ident, $prefix:literal) => {
        |value: String| -> Result<String, ProtocolError> {
            let domain = IdentifierDomain::$domain;
            let Some(body) = value.strip_prefix($prefix) else {
                return Err(malformed(domain, IdentifierRejection::WrongPrefix));
            };
            let is_hex = body.len() == HEX_LENGTH
                && body
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b));
            if is_hex {
                Ok(value)
            } else {
                Err(malformed(domain, IdentifierRejection::NotLowercaseHex))
            }
        }
    };
}

macro_rules! capability_validator {
    () => {
        |value: String| -> Result<String, ProtocolError> {
            let domain = IdentifierDomain::CapabilityId;
            let segments: Vec<&str> = value.split('.').collect();
            if segments.len() != CAPABILITY_SEGMENT_COUNT {
                return Err(malformed(domain, IdentifierRejection::SegmentCount));
            }
            if segments.iter().any(|s| !is_namespace_segment(s))
                || PROHIBITED_NAMESPACES.contains(&segments[0])
            {
                return Err(malformed(domain, IdentifierRejection::Segment));
            }
            if !CAPABILITY_VERBS.contains(&segments[2]) {
                return Err(malformed(domain, IdentifierRejection::UnknownVerb));
            }
            Ok(value)
        }
    };
}

macro_rules! provider_validator {
    () => {
        |value: String| -> Result<String, ProtocolError> {
            let domain = IdentifierDomain::ProviderId;
            if is_namespace_segment(&value) {
                Ok(value)
            } else {
                Err(malformed(domain, IdentifierRejection::Segment))
            }
        }
    };
}

macro_rules! kebab_validator {
    ($domain:ident) => {
        |value: String| -> Result<String, ProtocolError> {
            if is_kebab_pattern(&value) {
                Ok(value)
            } else {
                Err(malformed(
                    IdentifierDomain::$domain,
                    IdentifierRejection::Pattern,
                ))
            }
        }
    };
}

declare_id!(
    /// `tsk_` + ULID — a durable `AssistantTask` (Protocol Index §2).
    TaskId,
    TaskId,
    "tsk_",
    ulid_validator!(TaskId, "tsk_")
);
declare_id!(
    /// `stp_` + ULID — one step of an `AssistantTask` (Protocol Index §2).
    StepId,
    StepId,
    "stp_",
    ulid_validator!(StepId, "stp_")
);
declare_id!(
    /// `apr_` + ULID — an `ApprovalRequest` (Protocol Index §2).
    ApprovalId,
    ApprovalId,
    "apr_",
    ulid_validator!(ApprovalId, "apr_")
);
declare_id!(
    /// `grt_` + ULID — an `ApprovalGrant` (Protocol Index §2).
    GrantId,
    GrantId,
    "grt_",
    ulid_validator!(GrantId, "grt_")
);
declare_id!(
    /// `req_` + ULID — one cross-boundary request/result correlation
    /// (Protocol Index §2; Capability Protocol §4).
    RequestId,
    RequestId,
    "req_",
    ulid_validator!(RequestId, "req_")
);
declare_id!(
    /// `evt_` + ULID — a `SereaEvent`, an `Evidence` record, or a memory item
    /// (Protocol Index §2). The frozen registry has no separate evidence or
    /// memory-item prefix, and adding one requires an ADR.
    EventId,
    EventId,
    "evt_",
    ulid_validator!(EventId, "evt_")
);
declare_id!(
    /// `dev_` + ULID — a paired device (Protocol Index §2).
    DeviceId,
    DeviceId,
    "dev_",
    ulid_validator!(DeviceId, "dev_")
);
declare_id!(
    /// `sch_` + ULID — a durable schedule (Protocol Index §2).
    ScheduleId,
    ScheduleId,
    "sch_",
    ulid_validator!(ScheduleId, "sch_")
);
declare_id!(
    /// `prop_` + ULID — a proactive-watcher proposal (Protocol Index §2).
    ProposalId,
    ProposalId,
    "prop_",
    ulid_validator!(ProposalId, "prop_")
);
declare_id!(
    /// `rcp_` + ULID — a `SideEffectReceipt` (Protocol Index §2).
    ReceiptId,
    ReceiptId,
    "rcp_",
    ulid_validator!(ReceiptId, "rcp_")
);
declare_id!(
    /// `ses_` + ULID — a device session (Protocol Index §2).
    SessionId,
    SessionId,
    "ses_",
    ulid_validator!(SessionId, "ses_")
);
declare_id!(
    /// `idk_` + 64 lowercase hex — the derived per-step idempotency key
    /// (Capability Protocol §8.2). The key is derived from the *request*, never
    /// from the attempt, so every attempt of one step reuses it and a genuine
    /// second execution must carry a distinct `StepId` and key.
    IdempotencyKey,
    IdempotencyKey,
    "idk_",
    hex_validator!(IdempotencyKey, "idk_")
);
declare_id!(
    /// `sha256:` + 64 lowercase hex — a content digest over canonical JSON
    /// (Protocol Index §2, §5).
    Digest,
    Digest,
    "sha256:",
    hex_validator!(Digest, "sha256:")
);
declare_id!(
    /// `^[a-z0-9]+(-[a-z0-9]+)*$` — a roster model identifier
    /// (Protocol Index §2).
    ModelId,
    ModelId,
    "",
    kebab_validator!(ModelId)
);
declare_id!(
    /// `<provider>.<resource>.<verb>`: exactly three segments, each
    /// `[a-z][a-z0-9_]{1,31}`, with `verb` drawn from the frozen verb set
    /// (Protocol Index §3; Capability Protocol §2).
    CapabilityId,
    CapabilityId,
    "",
    capability_validator!()
);
declare_id!(
    /// A registered capability namespace. Protocol Index §3 fixes it to the
    /// same `[a-z][a-z0-9_]{1,31}` grammar a `CapabilityId`'s provider segment
    /// uses, because a descriptor is only registrable when the two are equal.
    ///
    /// Unlike a `CapabilityId` provider segment this accepts `goallatch`, because
    /// that is the adapter identity `HostGoalProvider` reports (GoalLatch Adapter
    /// §3.2). The prohibition is on the capability namespace, not the identity.
    ProviderId,
    ProviderId,
    "",
    provider_validator!()
);
declare_id!(
    /// Names which implementation of a capability is registered behind one
    /// `CapabilityId` — for example the rootless and root `device.*` variants
    /// (Capability Protocol §3.1; Crate Map §6.2).
    ///
    /// The frozen documents name `fake-goallatch` and `mcp-goallatch` without
    /// freezing a grammar, so P1 validates the shape every named value shares
    /// rather than inventing a closed set.
    ImplementationId,
    ImplementationId,
    "",
    kebab_validator!(ImplementationId)
);

impl Digest {
    /// The hash algorithm named by the wire form. `sha256` is the only frozen
    /// algorithm at `serea-arch/0.1.0`.
    pub fn algorithm(&self) -> &'static str {
        "sha256"
    }
}

/// A validated 48-bit millisecond timestamp for ULID minting
/// (Protocol Index §2 rule 1).
///
/// The validation is the point: the range is a type-level invariant, so
/// [`UlidValue`] cannot be constructed out of range and minting has no
/// reachable failure path. It is a public validated type because an
/// [`UlidSource`] implementation is caller-supplied code and its input is
/// caller-supplied data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TimestampMs(u64);

impl TimestampMs {
    /// The frozen 48-bit millisecond range is `0..=2^48-1`.
    pub const MAX: u64 = (1 << 48) - 1;

    /// Validates `millis` against the frozen 48-bit range.
    pub fn new(millis: u64) -> Result<Self, ProtocolError> {
        if millis > Self::MAX {
            Err(ProtocolError::MalformedValue {
                field: ValueField::Timestamp,
                reason: ValueRejection::OutOfRange,
            })
        } else {
            Ok(Self(millis))
        }
    }

    /// The millisecond value.
    pub const fn get(self) -> u64 {
        self.0
    }

    /// The next millisecond, or `None` at the end of the frozen range.
    pub const fn checked_next(self) -> Option<Self> {
        match self.0.checked_add(1) {
            Some(next) if next <= Self::MAX => Some(Self(next)),
            _ => None,
        }
    }
}

impl fmt::Display for TimestampMs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Mint-time ULID material: a 48-bit millisecond timestamp plus 80 bits of
/// entropy (Protocol Index §2 rule 1).
///
/// It is deliberately not constructible from an identifier string and exposes
/// no accessor, so minting is the only direction in which a timestamp exists
/// inside this crate. The timestamp is a validated [`TimestampMs`], so this
/// constructor cannot fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UlidValue {
    timestamp_ms: TimestampMs,
    entropy: [u8; 10],
}

impl UlidValue {
    const ENTROPY_BITS: u32 = 80;

    /// Builds minting material from a validated timestamp and injected entropy.
    pub fn new(timestamp_ms: TimestampMs, entropy: [u8; 10]) -> Self {
        Self {
            timestamp_ms,
            entropy,
        }
    }

    fn encode(&self) -> String {
        let value = (u128::from(self.timestamp_ms.get()) << Self::ENTROPY_BITS)
            | u128::from_be_bytes({
                let mut bytes = [0u8; 16];
                bytes[6..].copy_from_slice(&self.entropy);
                bytes
            });
        // 26 characters carry 5 bits each, but the value is 128 bits, so the
        // leading character holds only 3 significant bits and can never exceed
        // '7' while the timestamp stays inside its 48-bit range.
        (0..ULID_BODY_LENGTH)
            .map(|index| {
                let shift = 5 * (ULID_BODY_LENGTH as u32 - 1 - index as u32);
                let digit = ((value >> shift) & 0x1f) as usize;
                char::from(CROCKFORD_ALPHABET[digit])
            })
            .collect()
    }
}

/// An injected source of minting material.
///
/// P1 has no wall clock and no randomness source: everything that can vary is
/// injected (Model Protocol §10; Policy Protocol §6), which is what makes
/// identifier minting reproducible in tests.
pub trait UlidSource {
    /// Returns the next minting value. Implementations must not consult a wall
    /// clock or unseeded randomness; `.clippy.toml` bans the std entry points
    /// workspace-wide.
    fn next_ulid(&mut self) -> UlidValue;
}

impl<T: UlidSource + ?Sized> UlidSource for &mut T {
    fn next_ulid(&mut self) -> UlidValue {
        (**self).next_ulid()
    }
}

/// Mints identifiers in the frozen domains from an injected [`UlidSource`].
///
/// Minting is the only place a timestamp exists in this crate, and it never
/// reads one back out of an identifier (Protocol Index §2 rule 4).
#[derive(Debug, Clone)]
pub struct IdMinter<S> {
    source: S,
}

impl<S: UlidSource> IdMinter<S> {
    /// Wraps an injected source.
    pub fn new(source: S) -> Self {
        Self { source }
    }

    fn next_with(&mut self, prefix: &str) -> String {
        format!("{prefix}{}", self.source.next_ulid().encode())
    }
}

macro_rules! declare_minter {
    ($($method:ident => $ty:ident, $prefix:literal;)*) => {
        impl<S: UlidSource> IdMinter<S> {
            $(
                #[doc = concat!("Mints a `", stringify!($ty), "`.")]
                pub fn $method(&mut self) -> $ty {
                    $ty::new(self.next_with($prefix)).unwrap_or_else(|error| {
                        unreachable!("a frozen prefix plus a minted 128-bit ULID is valid: {error:?}")
                    })
                }
            )*
        }
    };
}

declare_minter! {
    next_task_id => TaskId, "tsk_";
    next_step_id => StepId, "stp_";
    next_approval_id => ApprovalId, "apr_";
    next_grant_id => GrantId, "grt_";
    next_request_id => RequestId, "req_";
    next_event_id => EventId, "evt_";
    next_device_id => DeviceId, "dev_";
    next_schedule_id => ScheduleId, "sch_";
    next_proposal_id => ProposalId, "prop_";
    next_receipt_id => ReceiptId, "rcp_";
    next_session_id => SessionId, "ses_";
}
