use std::collections::BTreeMap;

use serea_capability::{CapabilitySchemaCatalogV1, CatalogError};

const PREFIX: &str = "https://serea.local/schemas/";

#[test]
fn valid_simple_document() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}simple.json"),
        format!(
            r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{PREFIX}simple.json","type":"object","additionalProperties":false,"properties":{{"name":{{"type":"string","maxLength":64}}}}}}"#
        ),
    )]);
    let catalog = CapabilitySchemaCatalogV1::build(docs).expect("valid catalog");
    assert!(
        catalog
            .document_digest(&format!("{PREFIX}simple.json"))
            .is_some()
    );
}

#[test]
fn duplicate_member_name_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}dup.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","maxLength":1,"maxLength":1048576}"#
            .to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::InvalidJson));
}

#[test]
fn duplicate_nested_member_name_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}dup2.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"a":{"type":"string","maxLength":1},"b":{"type":"string","maxLength":2,"maxLength":3}}}"#
            .to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::InvalidJson));
}

#[test]
fn invalid_draft_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}bad.json"),
        r#"{"$schema":"https://json-schema.org/draft/07/schema#","type":"object","additionalProperties":false}"#.to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::InvalidDraft));
}

#[test]
fn non_serea_local_uri_rejected() {
    let docs = BTreeMap::from([(
        "https://example.com/x.json".to_string(),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema"}"#.to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::InvalidUri));
}

#[test]
fn unknown_external_ref_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}a.json"),
        format!(
            r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{{"x":{{"$ref":"{PREFIX}missing.json"}}}}}}"#
        ),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::UnknownRef(_)));
}

#[test]
fn http_ref_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}a.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"x":{"$ref":"http://example.com/x.json"}}}"#.to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::ExternalRef(_)));
}

#[test]
fn file_ref_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}a.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"x":{"$ref":"file:///tmp/foo.json"}}}"#.to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::ExternalRef(_)));
}

#[test]
fn parent_traversal_ref_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}a.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"x":{"$ref":"../secret.json"}}}"#.to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::ExternalRef(_)));
}

#[test]
fn same_document_pointer_success() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}a.json"),
        r##"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"x":{"$ref":"#/$defs/item"}},"$defs":{"item":{"type":"string","maxLength":8}}}"##.to_string(),
    )]);
    let catalog = CapabilitySchemaCatalogV1::build(docs).expect("same-doc ref ok");
    assert!(
        catalog
            .document_digest(&format!("{PREFIX}a.json"))
            .is_some()
    );
}

#[test]
fn trusted_cross_document_ref_success() {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}a.json"),
        format!(
            r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{{"x":{{"$ref":"{PREFIX}b.json"}}}}}}"#
        ),
    );
    docs.insert(
        format!("{PREFIX}b.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","maxLength":8}"#.to_string(),
    );
    let catalog = CapabilitySchemaCatalogV1::build(docs).expect("cross-doc ref ok");
    assert!(catalog.contains_uri(&format!("{PREFIX}b.json")));
}

#[test]
fn direct_ref_cycle_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}a.json"),
        r##"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"x":{"$ref":"#/$defs/a"}},"$defs":{"a":{"$ref":"#/$defs/a"}}}"##.to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::CyclicRef));
}

#[test]
fn indirect_cross_document_cycle_rejected() {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}a.json"),
        format!(
            r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{{"x":{{"$ref":"{PREFIX}b.json"}}}}}}"#
        ),
    );
    docs.insert(
        format!("{PREFIX}b.json"),
        format!(
            r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{{"y":{{"$ref":"{PREFIX}a.json"}}}}}}"#
        ),
    );
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::CyclicRef));
}

#[test]
fn byte_limit_boundary() {
    let base = format!(
        r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{PREFIX}lim.json","type":"object","additionalProperties":false,"description":""#
    );
    let end = r#""}"#;
    let x = 65536usize - (base.len() + end.len());
    let schema = format!("{base}{}{end}", "a".repeat(x));
    let r =
        CapabilitySchemaCatalogV1::build(BTreeMap::from([(format!("{PREFIX}lim.json"), schema)]));
    assert!(r.is_ok(), "65536-byte document must compile: {r:?}");
    let too_big = format!("{base}{}{end}", "a".repeat(x + 2048));
    let err =
        CapabilitySchemaCatalogV1::build(BTreeMap::from([(format!("{PREFIX}lim.json"), too_big)]))
            .unwrap_err();
    assert!(matches!(err, CatalogError::TooLarge));
}

#[test]
fn depth_boundary_accepted_and_one_deeper_refused() {
    // A property chain nests two JSON levels per step: the `properties` map
    // and the subschema inside it. The compiler counts every JSON value from
    // the document root as level 1, so `levels` steps of object properties
    // reach level 1 + 2 * levels. ADR-0035 allows 64.
    fn property_chain(levels: usize) -> String {
        let mut open = String::new();
        let mut close = String::new();
        for _ in 0..levels {
            open.push_str(r#""a":{"type":"object","additionalProperties":false,"properties":{"#);
            close.push_str("}}");
        }
        format!(
            r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{{{open}"leaf":{{"type":"integer"}}{close}}}}}"#
        )
    }
    // 30 steps -> the deepest value sits at level 1 + 2*30 + 1 = 62
    let ok = property_chain(30);
    let docs = BTreeMap::from([(format!("{PREFIX}d62.json"), ok)]);
    let outcome = CapabilitySchemaCatalogV1::build(docs).map(|_| ());
    assert!(
        outcome.is_ok(),
        "depth 62 must compile, got {:?}",
        outcome.err()
    );
    // 32 steps -> the deepest value sits at level 66, past the bound
    let too_deep = property_chain(32);
    let err =
        CapabilitySchemaCatalogV1::build(BTreeMap::from([(format!("{PREFIX}d65.json"), too_deep)]))
            .unwrap_err();
    assert!(matches!(err, CatalogError::TooDeep));
}

#[test]
fn node_limit_boundary() {
    // 4096 schema nodes per document; an `enum` of many small entries lets
    // us approach the node ceiling without blowing past 256 properties or
    // the 64 KiB byte limit.
    fn enum_doc(count: usize) -> String {
        let mut s = String::from(
            r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","maxLength":2,"enum":["#,
        );
        for i in 0..count {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(r#""v{i}""#));
        }
        s.push_str("]}");
        s
    }
    // total nodes ≈ count + 5; 4090 entries stays within 4096
    let ok = enum_doc(4090);
    let r = CapabilitySchemaCatalogV1::build(BTreeMap::from([(format!("{PREFIX}n.json"), ok)]));
    assert!(r.is_ok(), "4090-entry enum must compile: {r:?}");
    // 4200 entries pushes the tree past 4096 nodes
    let big = enum_doc(4200);
    let err = CapabilitySchemaCatalogV1::build(BTreeMap::from([(format!("{PREFIX}n2.json"), big)]))
        .unwrap_err();
    assert!(matches!(err, CatalogError::TooManyNodes));
}

#[test]
fn properties_limit_boundary() {
    fn props(n: usize) -> String {
        let mut s = String::from("{");
        for i in 0..n {
            if i > 0 {
                s.push(',');
            }
            s.push_str(&format!(r#""p{i}":{{"type":"string","maxLength":1}}"#));
        }
        s.push('}');
        s
    }
    let ok = format!(
        r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{}}}"#,
        props(256)
    );
    let r = CapabilitySchemaCatalogV1::build(BTreeMap::from([(format!("{PREFIX}p256.json"), ok)]));
    assert!(r.is_ok());
    let bad = format!(
        r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{}}}"#,
        props(257)
    );
    let err =
        CapabilitySchemaCatalogV1::build(BTreeMap::from([(format!("{PREFIX}p257.json"), bad)]))
            .unwrap_err();
    assert!(matches!(err, CatalogError::TooManyProperties));
}

#[test]
fn pattern_properties_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}pp.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"patternProperties":{"^x":{"type":"string","maxLength":1}}}"#.to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::PatternProperties));
}

#[test]
fn open_object_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}open.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object"}"#.to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::OpenObject));
}

#[test]
fn unbounded_string_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}str.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string"}"#.to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::UnboundedString));
}

#[test]
fn unbounded_array_rejected() {
    let docs = BTreeMap::from([(
        format!("{PREFIX}arr.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"array"}"#.to_string(),
    )]);
    let err = CapabilitySchemaCatalogV1::build(docs).unwrap_err();
    assert!(matches!(err, CatalogError::UnboundedArray));
}

#[test]
fn valid_bounded_nested_schemas() {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}root.json"),
        format!(
            r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{{"name":{{"type":"string","maxLength":32}},"tags":{{"type":"array","maxItems":8,"items":{{"type":"string","maxLength":16}}}},"inner":{{"$ref":"{PREFIX}inner.json"}}}}}}"#
        ),
    );
    docs.insert(
        format!("{PREFIX}inner.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"object","additionalProperties":false,"properties":{"n":{"type":"integer"}}}"#.to_string(),
    );
    let catalog = CapabilitySchemaCatalogV1::build(docs).expect("bounded nested ok");
    assert!(
        catalog
            .document_digest(&format!("{PREFIX}root.json"))
            .is_some()
    );
}
