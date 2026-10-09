use std::collections::BTreeMap;

use serea_capability::{CapabilitySchemaCatalogV1, CatalogError};

const PREFIX: &str = "https://serea.local/schemas/";

fn build(uri: &str, body: &str) -> Result<CapabilitySchemaCatalogV1, CatalogError> {
    CapabilitySchemaCatalogV1::build(BTreeMap::from([(
        format!("{PREFIX}{uri}"),
        format!(
            r#"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{PREFIX}{uri}",{body}}}"#
        ),
    )]))
}

#[test]
fn disjoint_json_types_accepted() {
    let r = build(
        "oneof1.json",
        r#""type":"object","additionalProperties":false,"properties":{"v":{"oneOf":[{"type":"string","maxLength":4},{"type":"integer"}]}}"#,
    );
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn common_required_discriminator_distinct_const_accepted() {
    let r = build(
        "oneof2.json",
        r#""type":"object","additionalProperties":false,"properties":{"v":{"oneOf":[{"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"const":"a"}}},{"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"const":"b"}}}]}}"#,
    );
    assert!(r.is_ok(), "{r:?}");
}

#[test]
fn same_discriminator_const_rejected() {
    let r = build(
        "oneof3.json",
        r#""type":"object","additionalProperties":false,"properties":{"v":{"oneOf":[{"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"const":"a"}}},{"type":"object","additionalProperties":false,"required":["kind"],"properties":{"kind":{"const":"a"}}}]}}"#,
    );
    assert!(matches!(r.unwrap_err(), CatalogError::OneOfNotDisjoint));
}

#[test]
fn overlapping_object_branches_rejected() {
    let r = build(
        "oneof4.json",
        r#""type":"object","additionalProperties":false,"properties":{"v":{"oneOf":[{"type":"object","additionalProperties":false,"properties":{"a":{"type":"string","maxLength":1}}},{"type":"object","additionalProperties":false,"properties":{"b":{"type":"string","maxLength":1}}}]}}"#,
    );
    assert!(matches!(r.unwrap_err(), CatalogError::OneOfNotDisjoint));
}

#[test]
fn unprovable_oneof_rejected() {
    let r = build(
        "oneof5.json",
        r#""type":"object","additionalProperties":false,"properties":{"v":{"oneOf":[{"type":"string","maxLength":4,"minLength":1},{"type":"string","maxLength":4}]}}"#,
    );
    assert!(matches!(r.unwrap_err(), CatalogError::OneOfNotDisjoint));
}

#[test]
fn branch_hidden_through_ref_still_checked() {
    let mut docs = BTreeMap::new();
    docs.insert(
        format!("{PREFIX}main.json"),
        format!(
            r##"{{"$schema":"https://json-schema.org/draft/2020-12/schema","$id":"{PREFIX}main.json","type":"object","additionalProperties":false,"properties":{{"v":{{"oneOf":[{{"$ref":"{PREFIX}a.json"}},{{"$ref":"{PREFIX}b.json"}}]}}}}}}"##
        ),
    );
    docs.insert(
        format!("{PREFIX}a.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","maxLength":4}"#.to_string(),
    );
    docs.insert(
        format!("{PREFIX}b.json"),
        r#"{"$schema":"https://json-schema.org/draft/2020-12/schema","type":"string","maxLength":4}"#.to_string(),
    );
    let r = CapabilitySchemaCatalogV1::build(docs);
    assert!(matches!(r.unwrap_err(), CatalogError::OneOfNotDisjoint));
}
