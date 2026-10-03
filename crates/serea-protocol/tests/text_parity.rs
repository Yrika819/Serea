//! ADR-0023 independent O/L/P expectations and exhaustive checked-in field inventory.
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use serea_protocol::{ids::*, schema::SchemaName, types::*};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Category {
    O,
    L,
    P,
}
const WHITESPACE: [char; 25] = [
    '\u{9}', '\u{a}', '\u{b}', '\u{c}', '\u{d}', '\u{20}', '\u{85}', '\u{a0}', '\u{1680}',
    '\u{2000}', '\u{2001}', '\u{2002}', '\u{2003}', '\u{2004}', '\u{2005}', '\u{2006}', '\u{2007}',
    '\u{2008}', '\u{2009}', '\u{200a}', '\u{2028}', '\u{2029}', '\u{202f}', '\u{205f}', '\u{3000}',
];
fn prefixes() -> [&'static str; 11] {
    [
        TaskId::PREFIX,
        StepId::PREFIX,
        ApprovalId::PREFIX,
        GrantId::PREFIX,
        RequestId::PREFIX,
        EventId::PREFIX,
        DeviceId::PREFIX,
        ScheduleId::PREFIX,
        ProposalId::PREFIX,
        ReceiptId::PREFIX,
        SessionId::PREFIX,
    ]
}
// Generation is table-driven from the production identifier authority. The
// independent corpus below deliberately does not infer expectations from regexes.
fn generated_pattern(category: Category) -> String {
    let whitespace =
        r"[\u0009-\u000d\u0020\u0085\u00a0\u1680\u2000-\u200a\u2028\u2029\u202f\u205f\u3000]";
    let end = r"(?![\s\S])";
    let mut pattern = format!(r"^(?!{whitespace})(?![\s\S]*{whitespace}{end})");
    if category == Category::O {
        let stems = prefixes().join("|");
        pattern.push_str(&format!(
            r"(?!(?:{stems})[0-7][0-9A-HJKMNP-TV-Z]{{25}}{end})"
        ));
        pattern.push_str(&format!(
            r"(?!{}[0-9a-f]{{64}}{end})(?!{}[0-9a-f]{{64}}{end})",
            IdempotencyKey::PREFIX,
            Digest::PREFIX
        ));
        pattern.push_str(&format!(
            r"(?!(?!(?:goallatch)\.)[a-z][a-z0-9_]{{1,31}}\.[a-z][a-z0-9_]{{1,31}}\.(?:{}){end})",
            CAPABILITY_VERBS.join("|")
        ));
    }
    pattern.push_str(if category == Category::P {
        r"[^\u0000-\u0008\u000b-\u001f\u007f-\u009f]+"
    } else {
        r"[^\u0000-\u001f\u007f-\u009f\u2028\u2029]+"
    });
    pattern.push_str(end);
    pattern
}
fn definition(category: Category) -> &'static str {
    match category {
        Category::O => "opaqueToken",
        Category::L => "singleLineLabel",
        Category::P => "prose",
    }
}
fn compile_property(name: SchemaName, pointer: &str) -> jsonschema::Validator {
    let doc = name.json().unwrap();
    let mut property = doc
        .pointer(pointer)
        .unwrap_or_else(|| panic!("missing {name}{pointer}"))
        .clone();
    property["$defs"] = doc["$defs"].clone();
    jsonschema::validator_for(&property).expect("property schema compiles")
}
fn scalar<T: DeserializeOwned>(value: &str) -> bool {
    serde_json::from_value::<T>(json!(value)).is_ok()
}
fn rust_accept(category: Category, field: &str, value: &str) -> bool {
    let (constructor, deserializer) = match field {
        "ActorId" => (ActorId::new(value).is_ok(), scalar::<ActorId>(value)),
        "LeaseOwner" => (LeaseOwner::new(value).is_ok(), scalar::<LeaseOwner>(value)),
        "ProviderReference" => (
            ProviderReference::new(value).is_ok(),
            scalar::<ProviderReference>(value),
        ),
        "TaskTitle" => (TaskTitle::new(value).is_ok(), scalar::<TaskTitle>(value)),
        "DescriptorTitle" => (
            DescriptorTitle::new(value).is_ok(),
            scalar::<DescriptorTitle>(value),
        ),
        "EffectSummary" => (
            EffectSummary::new(value).is_ok(),
            scalar::<EffectSummary>(value),
        ),
        "PlainSummary" => (
            PlainSummary::new(value).is_ok(),
            scalar::<PlainSummary>(value),
        ),
        "ErrorMessage" => (
            ErrorMessage::new(value).is_ok(),
            scalar::<ErrorMessage>(value),
        ),
        "DescriptorDescription" => (
            DescriptorDescription::new(value).is_ok(),
            scalar::<DescriptorDescription>(value),
        ),
        _ => panic!("unknown scalar"),
    };
    assert_eq!(
        constructor, deserializer,
        "constructor/deserialize {category:?}/{field}/{value:?}"
    );
    constructor
}
fn corpus() -> Vec<(String, [bool; 3])> {
    let mut cases = Vec::new();
    for value in ["", " ", "\t", "\n", "\r", "  "] {
        cases.push((value.into(), [false; 3]));
    }
    for value in [
        "calendar",
        "worker",
        "worker-1",
        "host-a3f9",
        "session-42.worker",
        "x",
        "w",
        "provider:handle/1234",
        "日本語の参照",
        "\u{feff}",
        "\u{200b}",
        "goallatch.goal.run",
        "calendar.events.unknown",
        "p.r.list",
    ] {
        cases.push((value.into(), [true; 3]));
    }
    cases.push(("x".repeat(5000), [true; 3]));
    cases.push(("語".repeat(5000), [true; 3]));
    for point in (0..=0x1f).chain(0x7f..=0x9f).chain([0x2028, 0x2029]) {
        let c = char::from_u32(point).unwrap();
        cases.push((
            format!("a{c}b"),
            [false, false, matches!(point, 9 | 10 | 0x2028 | 0x2029)],
        ));
        cases.push((format!("{c}a"), [false; 3]));
        cases.push((format!("a{c}"), [false; 3]));
    }
    for c in WHITESPACE {
        cases.push((format!("{c}word"), [false; 3]));
        cases.push((format!("word{c}"), [false; 3]));
        let control = c <= '\u{1f}' || ('\u{7f}'..='\u{9f}').contains(&c);
        let line = matches!(c, '\u{2028}' | '\u{2029}');
        cases.push((
            format!("a{c}b"),
            [
                !control && !line,
                !control && !line,
                !control || matches!(c, '\t' | '\n'),
            ],
        ));
    }
    for c in ['\u{feff}', '\u{200b}', '\u{180e}', '\u{2060}'] {
        for value in [format!("{c}word"), format!("word{c}"), format!("a{c}b")] {
            cases.push((value, [true; 3]));
        }
    }
    for prefix in [
        "tsk_", "stp_", "apr_", "grt_", "req_", "evt_", "dev_", "sch_", "prop_", "rcp_", "ses_",
    ] {
        for lead in ['0', '7'] {
            cases.push((
                format!("{prefix}{lead}{}", "Z".repeat(25)),
                [false, true, true],
            ));
        }
        for body in [
            format!("8{}", "Z".repeat(25)),
            "0".repeat(25),
            "0".repeat(27),
            format!("0{}I", "Z".repeat(24)),
            format!("0{}U", "Z".repeat(24)),
            "a".repeat(26),
        ] {
            cases.push((format!("{prefix}{body}"), [true; 3]));
        }
        let exact = format!("{prefix}0{}", "Z".repeat(25));
        for value in [
            format!("x{exact}"),
            format!("{exact}x"),
            format!("a:{exact}"),
            format!("{exact}\nnext"),
        ] {
            let expected = if value.contains('\n') {
                [false, false, true]
            } else {
                [true; 3]
            };
            cases.push((value, expected));
        }
    }
    for prefix in ["idk_", "sha256:"] {
        cases.push((format!("{prefix}{}", "a".repeat(64)), [false, true, true]));
        for hex in [
            "a".repeat(63),
            "a".repeat(65),
            "A".repeat(64),
            "g".repeat(64),
        ] {
            cases.push((format!("{prefix}{hex}"), [true; 3]));
        }
    }
    for prefix in ["idk:", "idk", "sha256_", "sha256"] {
        cases.push((format!("{prefix}{}", "a".repeat(64)), [true; 3]));
    }
    for verb in [
        "list", "read", "search", "open", "control", "write", "create", "send", "delete", "start",
        "status", "run", "cancel", "result",
    ] {
        for provider in [
            "pp".to_owned(),
            "p".repeat(32),
            "goallatch_foo".into(),
            "goallatch1".into(),
            "goallatch_".into(),
        ] {
            for resource in ["rr".to_owned(), "r".repeat(32)] {
                cases.push((format!("{provider}.{resource}.{verb}"), [false, true, true]));
            }
        }
        for value in [
            format!("goallatch.rr.{verb}"),
            format!("p.rr.{verb}"),
            format!("pp.r.{verb}"),
            format!("{}.rr.{verb}", "p".repeat(33)),
            format!("pp.{}.{verb}", "r".repeat(33)),
            format!("pp.rr.{verb}x"),
            format!("pp.rr.{verb}.extra"),
        ] {
            cases.push((value, [true; 3]));
        }
    }
    cases
}
fn verdict(category: Category, expected: [bool; 3]) -> bool {
    expected[match category {
        Category::O => 0,
        Category::L => 1,
        Category::P => 2,
    }]
}
const OCCURRENCES: [(SchemaName, &str, Category, &str); 11] = [
    (
        SchemaName::AssistantTask,
        "/properties/title",
        Category::L,
        "TaskTitle",
    ),
    (
        SchemaName::AssistantTask,
        "/properties/result_summary",
        Category::L,
        "PlainSummary",
    ),
    (
        SchemaName::AssistantTask,
        "/$defs/step/properties/lease_owner",
        Category::O,
        "LeaseOwner",
    ),
    (
        SchemaName::AssistantTask,
        "/$defs/receipt/properties/provider_reference",
        Category::O,
        "ProviderReference",
    ),
    (
        SchemaName::AssistantTask,
        "/$defs/receipt/properties/effect_summary",
        Category::L,
        "EffectSummary",
    ),
    (
        SchemaName::AssistantTask,
        "/$defs/actionError/properties/message",
        Category::P,
        "ErrorMessage",
    ),
    (
        SchemaName::ActionResult,
        "/$defs/actor/properties/id",
        Category::O,
        "ActorId",
    ),
    (
        SchemaName::ActionResult,
        "/$defs/receipt/properties/provider_reference",
        Category::O,
        "ProviderReference",
    ),
    (
        SchemaName::ActionResult,
        "/$defs/receipt/properties/effect_summary",
        Category::L,
        "EffectSummary",
    ),
    (
        SchemaName::ActionResult,
        "/$defs/error/properties/message",
        Category::P,
        "ErrorMessage",
    ),
    (
        SchemaName::Event,
        "/$defs/actor/properties/id",
        Category::O,
        "ActorId",
    ),
];
#[test]
fn every_actual_text_occurrence_matches_independent_expected_corpus_and_rust() {
    let cases = corpus();
    for (name, pointer, category, field) in OCCURRENCES {
        let validator = compile_property(name, pointer);
        for (value, expected) in &cases {
            let expected = verdict(category, *expected);
            assert_eq!(
                validator.is_valid(&json!(value)),
                expected,
                "schema {name}{pointer}/{category:?}/{value:?}"
            );
            assert_eq!(
                rust_accept(category, field, value),
                expected,
                "Rust {field}/{category:?}/{value:?}"
            );
        }
    }
}
#[test]
fn all_nine_rust_scalars_match_generated_surrogates_including_rust_only_fields() {
    // Descriptor fields and approval PlainSummary have no checked-in schema.
    // These are explicitly generated surrogates, NOT claimed actual occurrences.
    for (field, category) in [
        ("ActorId", Category::O),
        ("LeaseOwner", Category::O),
        ("ProviderReference", Category::O),
        ("TaskTitle", Category::L),
        ("DescriptorTitle", Category::L),
        ("EffectSummary", Category::L),
        ("PlainSummary", Category::L),
        ("ErrorMessage", Category::P),
        ("DescriptorDescription", Category::P),
    ] {
        let validator = jsonschema::validator_for(
            &json!({"type": "string", "pattern": generated_pattern(category)}),
        )
        .unwrap();
        for (value, expected) in corpus() {
            let expected = verdict(category, expected);
            assert_eq!(
                validator.is_valid(&json!(value)),
                expected,
                "surrogate {field}/{value:?}"
            );
            assert_eq!(
                rust_accept(category, field, &value),
                expected,
                "Rust {field}/{value:?}"
            );
        }
    }
}
#[test]
fn generated_patterns_are_identical_at_every_occurrence_and_pin_identifier_tables() {
    for (category, production) in [
        (Category::O, TextCategory::Opaque),
        (Category::L, TextCategory::Label),
        (Category::P, TextCategory::Prose),
    ] {
        assert_eq!(generated_pattern(category), text_pattern(production));
    }
    assert_eq!(
        prefixes(),
        [
            "tsk_", "stp_", "apr_", "grt_", "req_", "evt_", "dev_", "sch_", "prop_", "rcp_", "ses_"
        ]
    );
    assert_eq!(
        CAPABILITY_VERBS,
        [
            "list", "read", "search", "open", "control", "write", "create", "send", "delete",
            "start", "status", "run", "cancel", "result"
        ]
    );
    for (name, pointer, category, _) in OCCURRENCES {
        let doc = name.json().unwrap();
        let property = doc.pointer(pointer).unwrap();
        let text = property.get("anyOf").map_or(property, |v| &v[0]);
        assert_eq!(
            text["$ref"],
            format!("#/$defs/{}", definition(category)),
            "{name}{pointer}"
        );
        assert_eq!(
            doc["$defs"][definition(category)]["pattern"],
            generated_pattern(category)
        );
        assert!(
            doc["$defs"][definition(category)]
                .get("maxLength")
                .is_none()
        );
    }
}
#[test]
fn occurrence_inventory_covers_every_old_free_text_site_and_live_actor_receipt_paths() {
    fn inventory(value: &Value, pointer: &str, sites: &mut Vec<String>) {
        if let Some(object) = value.as_object() {
            if object.get("minLength") == Some(&json!(1))
                && object.contains_key("pattern")
                && !pointer.starts_with("/$defs/opaqueToken")
                && !pointer.starts_with("/$defs/singleLineLabel")
                && !pointer.starts_with("/$defs/prose")
            {
                panic!("uncategorized inline free-text site {pointer}");
            }
            if let Some(reference) = object.get("$ref").and_then(Value::as_str) {
                if [
                    "#/$defs/opaqueToken",
                    "#/$defs/singleLineLabel",
                    "#/$defs/prose",
                    "#/$defs/freeText",
                ]
                .contains(&reference)
                {
                    sites.push(pointer.trim_end_matches("/anyOf/0").to_owned());
                }
            }
            for (key, child) in object {
                inventory(child, &format!("{pointer}/{key}"), sites);
            }
        } else if let Some(array) = value.as_array() {
            for (i, child) in array.iter().enumerate() {
                inventory(child, &format!("{pointer}/{i}"), sites);
            }
        }
    }
    for name in SchemaName::ALL {
        let doc = name.json().unwrap();
        let mut sites = Vec::new();
        inventory(&doc, "", &mut sites);
        sites.sort();
        let mut expected: Vec<_> = OCCURRENCES
            .iter()
            .filter(|(n, _, _, _)| *n == name)
            .map(|(_, p, _, _)| (*p).to_owned())
            .collect();
        expected.sort();
        assert_eq!(sites, expected, "{name} actual occurrences");
    }
    let action = SchemaName::ActionResult.json().unwrap();
    assert_eq!(
        action["properties"]["evidence"]["items"]["$ref"],
        "#/$defs/evidence"
    );
    assert_eq!(
        action["$defs"]["evidence"]["properties"]["actor"]["$ref"],
        "#/$defs/actor"
    );
    assert_eq!(
        action["properties"]["receipt"]["anyOf"][0]["$ref"],
        "#/$defs/receipt"
    );
    let event = SchemaName::Event.json().unwrap();
    assert_eq!(event["properties"]["actor"]["$ref"], "#/$defs/actor");
    let task = SchemaName::AssistantTask.json().unwrap();
    assert_eq!(
        task["$defs"]["step"]["properties"]["side_effect_receipt"]["$ref"],
        "#/$defs/receiptOrNull"
    );
    assert_eq!(
        task["$defs"]["receiptOrNull"]["anyOf"][0]["$ref"],
        "#/$defs/receipt"
    );
}
