//! Identifier grammar tests for Protocol Index §2/§3.
//!
//! Frozen source: `docs/protocols/00-protocol-index.md` §2 (identifier
//! grammar), §3 (capability identifier grammar), §4.3 (frozen at P0), and
//! `docs/protocols/01-capability-protocol.md` §2 (frozen verb set).
//!
//! All fixture values are synthetic or copied from the frozen §2 example table.
//! No test parses an identifier for meaning; these assert acceptance and
//! rejection only.

use serea_protocol::errors::{IdentifierDomain, IdentifierRejection, ProtocolError};
use serea_protocol::ids::{
    CAPABILITY_VERBS, IdMinter, IdempotencyKey, ModelId, TimestampMs, UlidSource, UlidValue,
};
use serea_protocol::{
    ApprovalId, CapabilityId, DeviceId, Digest, EventId, GrantId, ImplementationId, ProposalId,
    ProviderId, ReceiptId, RequestId, ScheduleId, SessionId, StepId, TaskId,
};

const TASK_ID: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const STEP_ID: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";
const APPROVAL_ID: &str = "apr_01JQ8ZA1D4NFG8K2M6RTV9XCWB";
const GRANT_ID: &str = "grt_01JQ8ZA7B3KMW9Q4TVY7XN2RDP";
const REQUEST_ID: &str = "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB";
const EVENT_ID: &str = "evt_01JQ8ZB7H2XKM9P4QW7NRT5YCD";
const DEVICE_ID: &str = "dev_01JQ8ZC5N8TVG3K6MRQ2XW9JHF";
const SCHEDULE_ID: &str = "sch_01JQ8ZD2P6WKR8T9XVY4NQ3ZMM";
const PROPOSAL_ID: &str = "prop_01JQ8ZE8Q3YHF7M2KTW9XN6RPB";
const RECEIPT_ID: &str = "rcp_01JQ8ZF4T7KMV2X9NPQ5RD8WCS";
const SESSION_ID: &str = "ses_01JQ8ZG9V5MXK3N7QW2RTF8YHB";
const IDEMPOTENCY_KEY: &str =
    "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a";
const DIGEST_HEX: &str = "3b1f0c9e2a7d4e6b8f0a2c4d6e8b0d2f4a6c8e0b2d4f6a8c0e2b4d6f8a0c2e4b";
const DIGEST: &str = "sha256:3b1f0c9e2a7d4e6b8f0a2c4d6e8b0d2f4a6c8e0b2d4f6a8c0e2b4d6f8a0c2e4b";
const ULID_BODY: &str = "01JQ8Z9K3M7QWXR4V2T6YH0BNA";

/// Protocol Index §2: "ULID is the only minting scheme. 26 characters of
/// Crockford Base32". `I`, `L`, `O` and `U` are excluded from the alphabet.
const CROCKFORD_ALPHABET: &str = "0123456789ABCDEFGHJKMNPQRSTVWXYZ";

mod ulid_prefixed_ids {
    use super::*;

    /// Every ULID-prefixed identifier domain, its prefix, and a valid value.
    const DOMAINS: [(IdentifierDomain, &str, &str); 11] = [
        (IdentifierDomain::TaskId, "tsk_", TASK_ID),
        (IdentifierDomain::StepId, "stp_", STEP_ID),
        (IdentifierDomain::ApprovalId, "apr_", APPROVAL_ID),
        (IdentifierDomain::GrantId, "grt_", GRANT_ID),
        (IdentifierDomain::RequestId, "req_", REQUEST_ID),
        (IdentifierDomain::EventId, "evt_", EVENT_ID),
        (IdentifierDomain::DeviceId, "dev_", DEVICE_ID),
        (IdentifierDomain::ScheduleId, "sch_", SCHEDULE_ID),
        (IdentifierDomain::ProposalId, "prop_", PROPOSAL_ID),
        (IdentifierDomain::ReceiptId, "rcp_", RECEIPT_ID),
        (IdentifierDomain::SessionId, "ses_", SESSION_ID),
    ];

    #[test]
    fn accepts_every_frozen_section_2_example() {
        for (domain, _, value) in DOMAINS {
            let parsed = parse(domain, value);
            assert_eq!(
                parsed.as_deref(),
                Ok(value),
                "{domain:?} must accept its frozen example"
            );
        }
    }

    #[test]
    fn prefixes_are_part_of_the_wire_format() {
        for (domain, prefix, _) in DOMAINS {
            for foreign in [
                "tsk_", "stp_", "apr_", "grt_", "req_", "evt_", "dev_", "sch_", "prop_", "rcp_",
                "ses_", "idk_", "sha256:", "TASK_", "",
            ] {
                if foreign == prefix {
                    continue;
                }
                let value = format!("{foreign}{ULID_BODY}");
                assert_eq!(
                    parse(domain, &value),
                    Err((IdentifierRejection::WrongPrefix, value)),
                    "{domain:?} must reject foreign prefix {foreign:?}"
                );
            }
        }
    }

    #[test]
    fn rejects_wrong_ulid_length() {
        for body in [
            "",
            "01JQ8Z9K3M7QWXR4V2T6YH0BN",
            "01JQ8Z9K3M7QWXR4V2T6YH0BNAA",
            &ULID_BODY[..20],
            &ULID_BODY.repeat(2),
        ] {
            let value = format!("tsk_{body}");
            assert_eq!(
                parse(IdentifierDomain::TaskId, &value),
                Err((IdentifierRejection::Length, value.clone())),
                "length {body:?} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_characters_outside_crockford_base32() {
        for body in [
            "01JQ8Z9K3M7QWXR4V2T6YH0BIA",      // I
            "01JQ8Z9K3M7QWXR4V2T6YH0BLA",      // L
            "01JQ8Z9K3M7QWXR4V2T6YH0BOA",      // O
            "01JQ8Z9K3M7QWXR4V2T6YH0BUA",      // U
            "01jq8z9k3m7qwxr4v2t6yh0bna",      // lowercase is not the canonical form
            "01JQ8Z9K3M7QWXR4V2T6YH0B A",      // space
            "01JQ8Z9K3M7QWXR4V2T6YH0B\tA",     // tab
            "01JQ8Z9K3M7QWXR4V2T6YH0B\nA",     // newline
            "01JQ8Z9K3M7QWXR4V2T6YH0B\rA",     // carriage return
            "01JQ8Z9K3M7QWXR4V2T6YH0B\0A",     // NUL
            "01JQ8Z9K3M7QWXR4V2T6YH0B\u{7f}A", // delete
            "01JQ8Z9K3M7QWXR4V2T6YH0B-A",      // hyphen
            "01JQ8Z9K3M7QWXR4V2T6YH0B_A",      // underscore
            "01JQ8Z9K3M7QWXR4V2T6YH0B{A",      // brace
            "01JQ8Z9K3M7QWXR4V2T6YH0B\\A",     // backslash
        ] {
            let value = format!("tsk_{body}");
            assert_eq!(
                parse(IdentifierDomain::TaskId, &value),
                Err((IdentifierRejection::Character, value)),
                "body {body:?} must be rejected"
            );
        }
    }

    #[test]
    fn accepts_every_character_of_the_crockford_alphabet() {
        // Guards against the alphabet being narrowed by accident.
        let body: String = CROCKFORD_ALPHABET.chars().cycle().take(26).collect();
        assert_eq!(body.len(), 26);
        assert!(TaskId::new(format!("tsk_{body}")).is_ok());
    }

    #[test]
    fn rejects_ulids_above_the_128_bit_range() {
        for body in [
            "81JQ8Z9K3M7QWXR4V2T6YH0BNA",
            "91JQ8Z9K3M7QWXR4V2T6YH0BNA",
            "Z1JQ8Z9K3M7QWXR4V2T6YH0BNA",
        ] {
            let value = format!("tsk_{body}");
            assert_eq!(
                parse(IdentifierDomain::TaskId, &value),
                Err((IdentifierRejection::Range, value)),
                "an overflowed 26-character body must be rejected"
            );
        }
        // '7' is the highest legal leading character of a 128-bit body.
        let at_limit = format!("tsk_7{}", &ULID_BODY[1..]);
        assert_eq!(at_limit.len(), "tsk_".len() + 26);
        assert!(TaskId::new(at_limit).is_ok());
    }

    #[test]
    fn a_valid_body_differs_from_the_rejected_cases_only_in_the_rejected_dimension() {
        assert!(TaskId::new(format!("tsk_{ULID_BODY}")).is_ok());
    }

    #[test]
    fn cross_domain_substitution_is_not_silently_accepted() {
        // Protocol Index §2 rule 2: prefixes exist so a mis-routed value fails
        // loudly instead of being accepted by the wrong subsystem.
        let foreign = [
            STEP_ID,
            EVENT_ID,
            RECEIPT_ID,
            SESSION_ID,
            GRANT_ID,
            APPROVAL_ID,
        ];
        for value in foreign {
            assert_eq!(
                TaskId::new(value),
                Err(ProtocolError::MalformedIdentifier {
                    domain: IdentifierDomain::TaskId,
                    reason: IdentifierRejection::WrongPrefix,
                }),
                "{value:?} must not be accepted as a TaskId"
            );
        }
        for value in [TASK_ID, REQUEST_ID, DEVICE_ID] {
            assert!(EventId::new(value).is_err());
        }
    }

    /// Round-trips `value` through the domain's constructor and reports the
    /// rejection reason, or the original string on success.
    fn parse(
        domain: IdentifierDomain,
        value: &str,
    ) -> Result<String, (IdentifierRejection, String)> {
        let result = match domain {
            IdentifierDomain::TaskId => TaskId::new(value).map(|id| id.as_str().to_owned()),
            IdentifierDomain::StepId => StepId::new(value).map(|id| id.as_str().to_owned()),
            IdentifierDomain::ApprovalId => ApprovalId::new(value).map(|id| id.as_str().to_owned()),
            IdentifierDomain::GrantId => GrantId::new(value).map(|id| id.as_str().to_owned()),
            IdentifierDomain::RequestId => RequestId::new(value).map(|id| id.as_str().to_owned()),
            IdentifierDomain::EventId => EventId::new(value).map(|id| id.as_str().to_owned()),
            IdentifierDomain::DeviceId => DeviceId::new(value).map(|id| id.as_str().to_owned()),
            IdentifierDomain::ScheduleId => ScheduleId::new(value).map(|id| id.as_str().to_owned()),
            IdentifierDomain::ProposalId => ProposalId::new(value).map(|id| id.as_str().to_owned()),
            IdentifierDomain::ReceiptId => ReceiptId::new(value).map(|id| id.as_str().to_owned()),
            IdentifierDomain::SessionId => SessionId::new(value).map(|id| id.as_str().to_owned()),
            other => panic!("{other:?} is not a ULID-prefixed domain"),
        };
        result.map_err(|err| match err {
            ProtocolError::MalformedIdentifier { reason, .. } => (reason, value.to_owned()),
            other => panic!("unexpected error for {value:?}: {other:?}"),
        })
    }
}

mod idempotency_key {
    use super::*;

    #[test]
    fn accepts_the_frozen_section_2_example() {
        let key = IdempotencyKey::new(IDEMPOTENCY_KEY).expect("frozen §2 example");
        assert_eq!(key.as_str(), IDEMPOTENCY_KEY);
        assert_eq!(key.to_string(), IDEMPOTENCY_KEY);
        assert_eq!(IdempotencyKey::PREFIX, "idk_");
    }

    #[test]
    fn rejects_everything_that_is_not_idk_plus_64_lowercase_hex() {
        let hex = "9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a";
        let cases: [(&str, IdentifierRejection); 9] = [
            (hex, IdentifierRejection::WrongPrefix),
            (
                &format!("idk_{}", &hex[..63]),
                IdentifierRejection::NotLowercaseHex,
            ),
            (
                "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9aa",
                IdentifierRejection::NotLowercaseHex,
            ),
            (
                "idk_9F2C1A7E4B6D0F8A3C5E9B1D7F2A4C6E8B0D3F5A7C9E1B4D6F8A0C2E4B6D8F9A",
                IdentifierRejection::NotLowercaseHex,
            ),
            (
                "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8g9",
                IdentifierRejection::NotLowercaseHex,
            ),
            ("idk_", IdentifierRejection::NotLowercaseHex),
            ("", IdentifierRejection::WrongPrefix),
            (
                "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a\n",
                IdentifierRejection::NotLowercaseHex,
            ),
            (
                "idk_9f2c1a7e4b6d0f8a3c5e9b1d7f2a4c6e8b0d3f5a7c9e1b4d6f8a0c2e4b6d8f9a\t",
                IdentifierRejection::NotLowercaseHex,
            ),
        ];
        for (value, expected) in cases {
            assert_eq!(
                IdempotencyKey::new(value),
                Err(ProtocolError::MalformedIdentifier {
                    domain: IdentifierDomain::IdempotencyKey,
                    reason: expected,
                }),
                "{value:?} must be rejected"
            );
        }
    }
}

mod digest {
    use super::*;

    #[test]
    fn accepts_the_frozen_wire_form() {
        let digest = Digest::new(DIGEST).expect("sha256: + 64 lowercase hex");
        assert_eq!(digest.as_str(), DIGEST);
        assert_eq!(digest.algorithm(), "sha256");
    }

    #[test]
    fn rejects_other_algorithms_and_malformed_hex() {
        let cases: [(&str, IdentifierRejection); 10] = [
            ("", IdentifierRejection::WrongPrefix),
            ("sha256:", IdentifierRejection::NotLowercaseHex),
            ("sha256:3b1f", IdentifierRejection::NotLowercaseHex),
            (DIGEST_HEX, IdentifierRejection::WrongPrefix),
            (
                &format!("sha512:{DIGEST_HEX}"),
                IdentifierRejection::WrongPrefix,
            ),
            (
                &format!("SHA256:{DIGEST_HEX}"),
                IdentifierRejection::WrongPrefix,
            ),
            (
                &format!("sha256:{}", DIGEST_HEX.to_uppercase()),
                IdentifierRejection::NotLowercaseHex,
            ),
            (
                &format!("sha256:{}z", &DIGEST_HEX[..63]),
                IdentifierRejection::NotLowercaseHex,
            ),
            (
                &format!("sha256:{DIGEST_HEX}\n"),
                IdentifierRejection::NotLowercaseHex,
            ),
            (
                &format!(" sha256:{DIGEST_HEX}"),
                IdentifierRejection::WrongPrefix,
            ),
        ];
        for (value, expected) in cases {
            assert_eq!(
                Digest::new(value),
                Err(ProtocolError::MalformedIdentifier {
                    domain: IdentifierDomain::Digest,
                    reason: expected,
                }),
                "{value:?} must be rejected"
            );
        }
    }
}

mod model_id {
    use super::*;

    #[test]
    fn accepts_the_frozen_pattern() {
        for value in [
            "nemotron-3-nano-30b",
            "gpt-oss-20b",
            "gemma-4-31b",
            "codex",
            "x",
            "a1",
        ] {
            let id = ModelId::new(value).unwrap_or_else(|e| panic!("{value:?} accepted: {e:?}"));
            assert_eq!(id.as_str(), value);
        }
    }

    #[test]
    fn rejects_anything_outside_the_frozen_pattern() {
        for value in [
            "",
            "-leading",
            "trailing-",
            "double--dash",
            "Uppercase",
            "has_underscore",
            "has.dot",
            "has space",
            "trailing ",
            "tab\there",
            "newline\nhere",
        ] {
            assert_eq!(
                ModelId::new(value),
                Err(ProtocolError::MalformedIdentifier {
                    domain: IdentifierDomain::ModelId,
                    reason: IdentifierRejection::Pattern,
                }),
                "{value:?} must be rejected"
            );
        }
    }
}

mod capability_id {
    use super::*;

    #[test]
    fn accepts_the_frozen_section_2_example() {
        let id = CapabilityId::new("calendar.events.list").expect("frozen §2 example");
        assert_eq!(id.as_str(), "calendar.events.list");
    }

    #[test]
    fn accepts_every_frozen_verb() {
        // Capability Protocol §2 freezes 14 verbs. Adding one is an
        // architecture-minor change and requires an ADR.
        assert_eq!(CAPABILITY_VERBS.len(), 14);
        for verb in CAPABILITY_VERBS {
            let value = format!("synthetic_resource.synthetic_noun.{verb}");
            assert_eq!(
                CapabilityId::new(value.clone()).map(|id| id.as_str().to_owned()),
                Ok(value),
                "frozen verb {verb:?} must form a valid CapabilityId"
            );
        }
    }

    #[test]
    fn rejects_other_than_three_segments() {
        let cases: [(&str, IdentifierRejection); 9] = [
            ("", IdentifierRejection::SegmentCount),
            ("calendar", IdentifierRejection::SegmentCount),
            ("calendar.events", IdentifierRejection::SegmentCount),
            (
                "calendar.events.list.extra",
                IdentifierRejection::SegmentCount,
            ),
            ("calendar.events.list.", IdentifierRejection::SegmentCount),
            (".events.list", IdentifierRejection::Segment),
            ("calendar..list", IdentifierRejection::Segment),
            ("cal.endary.events.list", IdentifierRejection::SegmentCount),
            ("calendar-events.list", IdentifierRejection::SegmentCount),
        ];
        for (value, expected) in cases {
            assert_eq!(
                CapabilityId::new(value),
                Err(ProtocolError::MalformedIdentifier {
                    domain: IdentifierDomain::CapabilityId,
                    reason: expected,
                }),
                "{value:?} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_a_verb_outside_the_frozen_set() {
        for verb in [
            "execute",
            "shell",
            "grant",
            "escalate",
            "approve",
            "list_",
            "run2",
            "deleteall",
            // `li_st` is a legal `[a-z][a-z0-9_]{1,31}` segment but not a frozen
            // verb: a grammar-legal segment must not widen the verb set.
            "li_st",
        ] {
            let value = format!("synthetic_resource.synthetic_noun.{verb}");
            assert_eq!(
                CapabilityId::new(value.clone()),
                Err(ProtocolError::MalformedIdentifier {
                    domain: IdentifierDomain::CapabilityId,
                    reason: IdentifierRejection::UnknownVerb,
                }),
                "{value:?} must be rejected"
            );
        }
    }

    #[test]
    fn rejects_malformed_segments() {
        for value in [
            "1calendar.events.list",
            "_calendar.events.list",
            "calendar.events.1list",
            "calendar.events.List",
            "calendar.events.LIST",
            "Calendar.events.list",
            "calendar.events.list extra",
            "calendar.events.li\nst",
            "calendar.events.li\tst",
        ] {
            assert_eq!(
                CapabilityId::new(value),
                Err(ProtocolError::MalformedIdentifier {
                    domain: IdentifierDomain::CapabilityId,
                    reason: IdentifierRejection::Segment,
                }),
                "{value:?} must be rejected"
            );
        }
        // `[a-z][a-z0-9_]{1,31}` makes 2 the minimum and 32 the maximum length.
        assert!(CapabilityId::new("ab.events.list").is_ok());
        assert!(CapabilityId::new("a.events.list").is_err());
        let longest = format!("{}.events.list", "a".repeat(32));
        assert!(CapabilityId::new(longest.clone()).is_ok());
        for overlong in [
            format!("{}.events.list", "a".repeat(33)),
            format!("calendar.{}.list", "a".repeat(33)),
            format!("calendar.events.{}", "a".repeat(33)),
        ] {
            assert!(CapabilityId::new(overlong).is_err());
        }
    }

    #[test]
    fn refuses_the_goallatch_adapter_namespace() {
        // GoalLatch Adapter §3.2 and G13: `host` is the registered namespace;
        // `goallatch` is the adapter identity and no CapabilityDescriptor may
        // begin with it.
        assert!(CapabilityId::new("host.goal.start").is_ok());
        for value in ["goallatch.goal.start", "goallatch.goal.run"] {
            assert_eq!(
                CapabilityId::new(value),
                Err(ProtocolError::MalformedIdentifier {
                    domain: IdentifierDomain::CapabilityId,
                    reason: IdentifierRejection::Segment,
                }),
                "{value:?} must not resolve to the adapter namespace"
            );
        }
    }
}

mod provider_and_implementation_ids {
    use super::*;

    #[test]
    fn provider_id_uses_the_capability_provider_segment_grammar() {
        for value in [
            "calendar",
            "gmail",
            "github",
            "web",
            "device",
            "host",
            "synthetic_provider",
            // The adapter identity is a ProviderId even though no capability may
            // live in that namespace (GoalLatch Adapter 3.2, G13).
            "goallatch",
        ] {
            assert!(
                ProviderId::new(value).is_ok(),
                "{value:?} is a legal identity"
            );
        }
        for value in [
            "",
            "1calendar",
            "_calendar",
            "Calendar",
            "cal.endary",
            "has space",
        ] {
            assert_eq!(
                ProviderId::new(value),
                Err(ProtocolError::MalformedIdentifier {
                    domain: IdentifierDomain::ProviderId,
                    reason: IdentifierRejection::Segment,
                }),
                "{value:?} must be rejected"
            );
        }
        assert!(ProviderId::new("a".repeat(32)).is_ok());
        assert!(ProviderId::new("a".repeat(33)).is_err());
    }

    #[test]
    fn implementation_id_accepts_every_value_named_in_the_frozen_documents() {
        for value in ["fake-goallatch", "mcp-goallatch", "android-rootless", "x"] {
            assert!(ImplementationId::new(value).is_ok(), "{value:?} accepted");
        }
        for value in [
            "",
            "Fake-GoalLatch",
            "fake_goallatch",
            "-x",
            "x-",
            "a--b",
            "a b",
        ] {
            assert_eq!(
                ImplementationId::new(value),
                Err(ProtocolError::MalformedIdentifier {
                    domain: IdentifierDomain::ImplementationId,
                    reason: IdentifierRejection::Pattern,
                }),
                "{value:?} must be rejected"
            );
        }
    }
}

mod minting {
    use super::*;

    #[test]
    fn a_millisecond_outside_the_frozen_48_bit_range_is_refused() {
        // Regression: the range check used to be an `assert!` on the minting
        // path, so a caller-supplied source value aborted the process in debug
        // and in release alike. It is now a typed error at the boundary.
        assert!(TimestampMs::new(0).is_ok());
        assert!(TimestampMs::new(TimestampMs::MAX).is_ok());
        assert!(TimestampMs::new(TimestampMs::MAX + 1).is_err());
        assert!(TimestampMs::new(u64::MAX).is_err());
        assert_eq!(TimestampMs::new(7).expect("valid").get(), 7);
        assert_eq!(
            TimestampMs::new(7)
                .expect("valid")
                .checked_next()
                .expect("next")
                .get(),
            8
        );
        assert_eq!(
            TimestampMs::new(TimestampMs::MAX)
                .expect("valid")
                .checked_next(),
            None
        );
    }

    /// Deterministic injected source. P1 has no wall clock and no randomness
    /// (Model Protocol §10; Policy Protocol §6), so minting is a pure function
    /// of the injected sequence.
    struct CountingSource {
        tick: u64,
    }

    impl CountingSource {
        fn new() -> Self {
            Self { tick: 0 }
        }
    }

    impl UlidSource for CountingSource {
        fn next_ulid(&mut self) -> UlidValue {
            self.tick += 1;
            // Fixed epoch plus an injected tick; fixed entropy pattern. Never
            // SystemTime::now(), which `.clippy.toml` bans workspace-wide.
            let timestamp = TimestampMs::new(1_700_000_000_000 + self.tick)
                .expect("a counter-derived millisecond is inside the frozen 48-bit range");
            UlidValue::new(timestamp, [0x5a; 10])
        }
    }

    #[test]
    fn minted_identifiers_are_valid_in_their_own_domain() {
        let mut minter = IdMinter::new(CountingSource::new());
        let task = minter.next_task_id();
        assert!(TaskId::new(task.as_str()).is_ok());
        let step = minter.next_step_id();
        assert!(StepId::new(step.as_str()).is_ok());
        assert!(task.as_str().starts_with(TaskId::PREFIX));
        assert!(step.as_str().starts_with(StepId::PREFIX));
    }

    #[test]
    fn minting_is_reproducible_for_the_same_injected_source() {
        let first = {
            let mut m = IdMinter::new(CountingSource::new());
            (m.next_event_id(), m.next_event_id(), m.next_request_id())
        };
        let second = {
            let mut m = IdMinter::new(CountingSource::new());
            (m.next_event_id(), m.next_event_id(), m.next_request_id())
        };
        assert_eq!(first, second);
    }

    #[test]
    fn consecutive_mints_never_repeat() {
        let mut minter = IdMinter::new(CountingSource::new());
        let minted = vec![
            minter.next_task_id().to_string(),
            minter.next_step_id().to_string(),
            minter.next_approval_id().to_string(),
            minter.next_grant_id().to_string(),
            minter.next_request_id().to_string(),
            minter.next_event_id().to_string(),
            minter.next_device_id().to_string(),
            minter.next_schedule_id().to_string(),
            minter.next_proposal_id().to_string(),
            minter.next_receipt_id().to_string(),
            minter.next_session_id().to_string(),
        ];
        assert_eq!(minted.len(), 11, "every minting domain is reachable");
        let mut unique = minted.clone();
        unique.sort();
        unique.dedup();
        assert_eq!(unique.len(), 11, "Protocol Index §2 rule 3: never reused");
    }

    #[test]
    fn the_frozen_crockford_alphabet_matches_the_implementation() {
        // Guards against an accidental alphabet widening during refactors.
        for byte in ULID_BODY.bytes() {
            assert!(CROCKFORD_ALPHABET.contains(byte as char));
        }
        for excluded in ['I', 'L', 'O', 'U'] {
            assert!(!CROCKFORD_ALPHABET.contains(excluded));
        }
    }
}
