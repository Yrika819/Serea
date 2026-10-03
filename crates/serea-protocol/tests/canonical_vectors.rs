//! Literal SCJ-1 and typed IDK-1 vectors from corrected ADR-0019.
use serea_protocol::{
    ActionRequest, CapabilityId, Digest, IdempotencyKey, SemVer, StepId, TaskId, canonicalize,
    derive_idempotency_key, digest_of,
};

const TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const STEP: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";

const SCJ: [(&str, &str, &str); 10] = [
    (
        r#"{"b":1,"a":2}"#,
        r#"{"a":2,"b":1}"#,
        "sha256:d3626ac30a87e6f7a6428233b3c68299976865fa5508e4267c5415c76af7a772",
    ),
    (
        r#"{"a":{"z":[3,1,2],"y":null},"b":true}"#,
        r#"{"a":{"y":null,"z":[3,1,2]},"b":true}"#,
        "sha256:754ee7a1aee4ccd0efc11f0a8de464fc62b47a2c784547ccf0fa37e3f03fdf2e",
    ),
    (
        "{}",
        "{}",
        "sha256:44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a",
    ),
    (
        "[]",
        "[]",
        "sha256:4f53cda18c2baa0c0354bb5f9a3ecbe5ed12ab4d8e11ba873c2f11161202b945",
    ),
    (
        "42",
        "42",
        "sha256:73475cb40a568e8da8a045ced110137e159f890ac4da883b6b17dc651b3a8049",
    ),
    (
        "-7",
        "-7",
        "sha256:a770d3270c9dcdedf12ed9fd70444f7c8a95c26cae3cae9bd867499090a2f14b",
    ),
    (
        "18446744073709551615",
        "18446744073709551615",
        "sha256:2cdb26265b4dc65e3b44d694f121fd6de99b9e4b8ae7f08d84bfa9537635ae43",
    ),
    (
        r#"{"k": "q\"b\\s\nt\tu\u0001v\u007f/é"}"#,
        r#"{"k":"q\"b\\s\nt\tu\u0001v\u007f/é"}"#,
        "sha256:e1e4c6bf233f76ae93bbd29dcd61c9d7704064e2d8ab627ba2310118de3d7a16",
    ),
    (
        r#"{ "a" : [ 1 , 2 ] , "b" : { } }"#,
        r#"{"a":[1,2],"b":{}}"#,
        "sha256:8c547cce7ccb1b89359479c0b71a0a4b62acfc54a2b2780fd34aaeb75f9e44b7",
    ),
    (
        r#"{"range":"tomorrow","limit":25,"opts":{"tz":"Asia/Tokyo","flags":["a","b"],"n":null}}"#,
        r#"{"limit":25,"opts":{"flags":["a","b"],"n":null,"tz":"Asia/Tokyo"},"range":"tomorrow"}"#,
        "sha256:12820828e332666cbc4a22dbaed9e5c192bdfbd7ce8a61ff2eb444a9e8538351",
    ),
];

macro_rules! scj_vector {
    ($name:ident, $index:expr) => {
        #[test]
        fn $name() {
            let (input, bytes, hash) = SCJ[$index];
            let result: Vec<u8> = canonicalize(input).unwrap();
            assert_eq!(result, bytes.as_bytes());
            let digest: Digest = digest_of(input).unwrap();
            assert_eq!(digest.as_str(), hash);
        }
    };
}
scj_vector!(scj_01_member_order, 0);
scj_vector!(scj_02_nested_order_and_array_order, 1);
scj_vector!(scj_03_empty_object, 2);
scj_vector!(scj_04_empty_array, 3);
scj_vector!(scj_05_positive_integer, 4);
scj_vector!(scj_06_negative_integer, 5);
scj_vector!(scj_07_unsigned_maximum, 6);
scj_vector!(scj_08_corrected_escape_provenance, 7);
scj_vector!(scj_09_whitespace, 8);
scj_vector!(scj_10_nested_document, 9);

fn key(task: &str, capability: &str, version: &str, arguments: &str) -> IdempotencyKey {
    derive_idempotency_key(
        &TaskId::new(task).unwrap(),
        &StepId::new(STEP).unwrap(),
        &CapabilityId::new(capability).unwrap(),
        &SemVer::new(version).unwrap(),
        arguments,
    )
    .unwrap()
}

macro_rules! idk_vector {
    ($name:ident, $task:expr, $cap:expr, $version:expr, $args:expr, $hash:expr) => {
        #[test]
        fn $name() {
            assert_eq!(key($task, $cap, $version, $args).as_str(), $hash);
        }
    };
}
idk_vector!(
    idk_01_named_fields,
    TASK,
    "calendar.events.list",
    "1.2.0",
    r#"{"range":"tomorrow","limit":25}"#,
    "idk_f8d17a2f6fb40db5a3421e035cde37a3234e628381cbb56791c14195f456b1cb"
);
idk_vector!(
    idk_02_member_order_invariance,
    TASK,
    "calendar.events.list",
    "1.2.0",
    r#"{"limit":25,"range":"tomorrow"}"#,
    "idk_f8d17a2f6fb40db5a3421e035cde37a3234e628381cbb56791c14195f456b1cb"
);
idk_vector!(
    idk_03_capability_and_empty_object,
    TASK,
    "calendar.event.create",
    "1.0.0",
    "{}",
    "idk_820ecdf813cba9cb22628af8fe0133965e201764d390c968ce23732e5c90ad0d"
);
idk_vector!(
    idk_04_version_participates,
    TASK,
    "calendar.events.list",
    "1.2.1",
    r#"{"range":"tomorrow","limit":25}"#,
    "idk_fe8bc5a29cd8df7c9092d894d9a29b16daa1df6f4f84ea98d2b44d47512c01e8"
);
idk_vector!(
    idk_05_task_participates,
    "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB",
    "calendar.events.list",
    "1.2.0",
    r#"{"range":"tomorrow","limit":25}"#,
    "idk_8303c302796fda66c38aec019cbb5f6b86309bbdcdb41001a08873cbc83c4fbd"
);
idk_vector!(
    legal_scalar_a,
    TASK,
    "pp.rr.list",
    "0.0.0",
    "-12",
    "idk_1796e5d503926eb7f41f6dd59234e4b8f6de7526650c8911f9d0e68c4f87abd1"
);
idk_vector!(
    legal_scalar_b,
    TASK,
    "pp.rr.list",
    "0.0.0-1",
    "2",
    "idk_c2df4132f97df1dddabce31306238753086962b0e3aea7366dfdb7fe3fdec06a"
);
idk_vector!(
    legal_object_a,
    TASK,
    "pp.rr.list",
    "0.0.0",
    r#"{"n":-12}"#,
    "idk_1a0326cb75273a11e56017804e9c1e187f60b3ab65eff3c87e8777bd6009f9f4"
);
idk_vector!(
    legal_object_b,
    TASK,
    "pp.rr.list",
    "0.0.0-1",
    r#"{"n":2}"#,
    "idk_8d00b0d917a9e90716a255d02af048fdd6589960b6dd336e188f99630b68ff35"
);

#[test]
fn historical_short_capability_cannot_enter_typed_derivation() {
    assert!(CapabilityId::new("p.r.list").is_err());
    assert!(serde_json::from_str::<CapabilityId>(r#""p.r.list""#).is_err());
}

#[test]
fn generic_scalar_collision_is_not_a_legal_action_request_collision() {
    assert_eq!(
        format!("{}{}{}", "pp.rr.list", "0.0.0", "-12"),
        format!("{}{}{}", "pp.rr.list", "0.0.0-1", "2")
    );
    for (version, arguments) in [
        ("0.0.0", "-12"),
        ("0.0.0-1", "2"),
        ("0.0.0", r#"{"n":-12}"#),
        ("0.0.0-1", r#"{"n":2}"#),
    ] {
        let document = serde_json::json!({
            "request_id": "req_01JQ8ZA4H6NFG8K2M6RTV9XCWB", "task_id": TASK, "step_id": STEP,
            "capability_id": "pp.rr.list", "capability_version": version,
            "arguments": serde_json::from_str::<serde_json::Value>(arguments).unwrap(),
            "arguments_digest": digest_of(arguments).unwrap(),
            "idempotency_key": key(TASK, "pp.rr.list", version, arguments),
            "data_class": "PUBLIC", "requested_by": "USER", "deadline_ms": 5000
        });
        assert_eq!(
            serde_json::from_value::<ActionRequest>(document).is_ok(),
            arguments.starts_with('{')
        );
    }
}
