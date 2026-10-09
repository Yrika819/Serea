//! Immutable, host-owned JSON Schema 2020-12 catalog (ADR-0035).
//!
//! One catalog is built once from exact trusted URIs under
//! `https://serea.local/schemas/`, checked structurally (size, depth, node and
//! property limits, closed objects, bounded strings/arrays, acyclic refs,
//! provably disjoint oneOf), canonicalized, digested, and compiled with the
//! in-memory `jsonschema` registry only. No network, filesystem, redirect, or
//! unknown URI ever resolves.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fmt;
use std::str::FromStr;

use jsonschema::{Draft, Registry};
use serde_json::Value;
use sha2::{Digest as _, Sha256};

/// Trusted local URI namespace for every catalog document.
pub const SCHEMA_URI_PREFIX: &str = "https://serea.local/schemas/";
/// Canonical byte limit per document (ADR-0035).
pub const MAX_SCHEMA_DOCUMENT_BYTES: usize = 65_536;
/// Maximum nesting depth (ADR-0035).
pub const MAX_SCHEMA_DEPTH: usize = 64;
/// Maximum total schema nodes (ADR-0035).
pub const MAX_SCHEMA_NODES: usize = 4_096;
/// Maximum properties per object (ADR-0035).
pub const MAX_SCHEMA_PROPERTIES: usize = 256;

/// Exact draft marker every catalog document must carry.
pub const SUPPORTED_DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";

/// Catalog construction/validation failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogError {
    InvalidUri,
    InvalidJson,
    InvalidDraft,
    UnknownRef(String),
    ExternalRef(String),
    CyclicRef,
    TooLarge,
    TooDeep,
    TooManyNodes,
    TooManyProperties,
    PatternProperties,
    OpenObject,
    UnboundedString,
    UnboundedArray,
    OneOfNotDisjoint,
    CompileFailed,
}

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}
impl std::error::Error for CatalogError {}

/// One immutable catalog entry.
#[derive(Debug, Clone)]
pub struct CatalogDocument {
    uri: String,
    parsed: Value,
    canonical: Vec<u8>,
    digest: serea_protocol::Digest,
}

impl CatalogDocument {
    pub fn uri(&self) -> &str {
        &self.uri
    }
    pub fn parsed(&self) -> &Value {
        &self.parsed
    }
    pub fn canonical(&self) -> &[u8] {
        &self.canonical
    }
    pub fn digest(&self) -> &serea_protocol::Digest {
        &self.digest
    }
}

/// The immutable, host-owned schema catalog.
#[derive(Debug, Clone)]
pub struct CapabilitySchemaCatalogV1 {
    documents: BTreeMap<String, CatalogDocument>,
    digest: serea_protocol::Digest,
}

impl CapabilitySchemaCatalogV1 {
    /// Builds and fully validates the catalog. Any failure rejects the whole
    /// catalog; no partial catalog escapes.
    pub fn build(documents: BTreeMap<String, String>) -> Result<Self, CatalogError> {
        let known: BTreeSet<String> = documents.keys().cloned().collect();
        let mut parsed_docs = BTreeMap::new();
        for (uri, text) in &documents {
            if !is_trusted_uri(uri) {
                return Err(CatalogError::InvalidUri);
            }
            let parsed = parse_document(text)?;
            if !parsed.is_object() {
                return Err(CatalogError::InvalidJson);
            }
            let draft_ok = parsed.get("$schema").and_then(Value::as_str) == Some(SUPPORTED_DRAFT);
            if !draft_ok {
                return Err(CatalogError::InvalidDraft);
            }
            let canonical = canonical_schema_bytes(&parsed);
            if canonical.len() > MAX_SCHEMA_DOCUMENT_BYTES {
                return Err(CatalogError::TooLarge);
            }
            validate_structure(&parsed, uri)?;
            validate_same_document_cycles(&parsed, uri, &known)?;
            parsed_docs.insert(
                uri.clone(),
                CatalogDocument {
                    uri: uri.clone(),
                    parsed,
                    canonical: canonical.clone(),
                    digest: sha256_digest(&canonical),
                },
            );
        }
        validate_cross_document_cycles(&parsed_docs)?;
        // oneOf disjointness needs ref resolution against the full catalog
        for (uri, doc) in &parsed_docs {
            validate_oneof(&doc.parsed, uri, &parsed_docs, &known)?;
        }
        // compile every document with trusted in-memory resources only
        {
            let registry = build_registry(&parsed_docs)?;
            for doc in parsed_docs.values() {
                jsonschema::options()
                    .with_draft(Draft::Draft202012)
                    .with_registry(&registry)
                    .build(&doc.parsed)
                    .map_err(|_| CatalogError::CompileFailed)?;
            }
        }
        let digest = catalog_digest(&parsed_docs);
        Ok(Self {
            documents: parsed_docs,
            digest,
        })
    }

    pub fn digest(&self) -> &serea_protocol::Digest {
        &self.digest
    }

    pub fn document_digest(&self, uri: &str) -> Option<&serea_protocol::Digest> {
        self.documents.get(uri).map(|d| &d.digest)
    }

    pub fn contains_uri(&self, uri: &str) -> bool {
        self.documents.contains_key(uri)
    }

    /// Compiles (on demand) and returns a validator for one document.
    /// Narrow P5D handoff; not a ToolCallProposalV1 parser.
    pub fn validator(&self, uri: &str) -> Option<jsonschema::Validator> {
        let doc = self.documents.get(uri)?;
        let registry = build_registry(&self.documents).ok()?;
        jsonschema::options()
            .with_draft(Draft::Draft202012)
            .with_registry(&registry)
            .build(&doc.parsed)
            .ok()
    }

    pub fn canonical(&self, uri: &str) -> Option<&[u8]> {
        self.documents.get(uri).map(|d| d.canonical.as_slice())
    }

    /// The parsed trusted document for `uri`. Callers receive catalog bytes
    /// only; no URI outside the catalog resolves.
    pub fn document(&self, uri: &str) -> Option<&Value> {
        self.documents.get(uri).map(|d| &d.parsed)
    }
}

/// Parses one catalog document, refusing a repeated object name.
///
/// A repeated member name would otherwise let the last value win silently,
/// so the digest identity of a document could differ from what a reviewer
/// reads. Catalog bytes are host-owned, so this is defense in depth rather
/// than the trust boundary, but digest identity must not depend on which
/// duplicate a parser happened to keep.
fn parse_document(text: &str) -> Result<Value, CatalogError> {
    crate::strict_json::parse_strict(text).map_err(|_| CatalogError::InvalidJson)
}

/// Deterministic canonical schema bytes: parse once, recursively sort object
/// keys by UTF-8 byte order, serialize compact UTF-8. Same bytes for any input
/// whitespace/key order; preserves JSON value semantics including non-integer
/// numbers. See module docs.
pub fn canonical_schema_bytes(value: &Value) -> Vec<u8> {
    let mut out = Vec::new();
    write_canonical(value, &mut out);
    out
}

fn write_canonical(value: &Value, out: &mut Vec<u8>) {
    match value {
        Value::Null => out.extend_from_slice(b"null"),
        Value::Bool(b) => out.extend_from_slice(if *b { b"true" } else { b"false" }),
        Value::Number(n) => {
            let mut buf = String::new();
            // serde_json Number Display is canonical for the parsed lexical form.
            buf.push_str(&n.to_string());
            out.extend_from_slice(buf.as_bytes());
        }
        Value::String(s) => {
            out.extend_from_slice(
                serde_json::to_string(s)
                    .expect("string serializes")
                    .as_bytes(),
            );
        }
        Value::Array(items) => {
            out.push(b'[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                write_canonical(item, out);
            }
            out.push(b']');
        }
        Value::Object(map) => {
            out.push(b'{');
            let mut members: Vec<(&String, &Value)> = map.iter().collect();
            members.sort_unstable_by(|(a, _), (b, _)| a.as_bytes().cmp(b.as_bytes()));
            for (i, (k, v)) in members.into_iter().enumerate() {
                if i > 0 {
                    out.push(b',');
                }
                out.extend_from_slice(serde_json::to_string(k).expect("key serializes").as_bytes());
                out.push(b':');
                write_canonical(v, out);
            }
            out.push(b'}');
        }
    }
}

pub(crate) fn sha256_digest(bytes: &[u8]) -> serea_protocol::Digest {
    let mut hex = String::with_capacity(64);
    for byte in Sha256::digest(bytes) {
        for shift in [4, 0] {
            let nibble = (byte >> shift) & 0xf;
            hex.push(char::from(if nibble < 10 {
                b'0' + nibble
            } else {
                b'a' + nibble - 10
            }));
        }
    }
    serea_protocol::Digest::new(format!("sha256:{hex}"))
        .expect("sha256 hex is always a valid Digest")
}

fn catalog_digest(docs: &BTreeMap<String, CatalogDocument>) -> serea_protocol::Digest {
    let mut entries = String::new();
    entries.push('[');
    for (i, doc) in docs.values().enumerate() {
        if i > 0 {
            entries.push(',');
        }
        entries.push_str(&format!(
            "{{\"uri\":{},\"digest\":\"{}\"}}",
            serde_json::to_string(&doc.uri).expect("uri serializes"),
            doc.digest.as_str()
        ));
    }
    entries.push(']');
    let projection = format!(
        "{{\"kind\":\"serea.capability-schema-catalog/1\",\"documents\":{}}}",
        entries
    );
    sha256_digest(projection.as_bytes())
}

fn is_trusted_uri(uri: &str) -> bool {
    if !uri.starts_with(SCHEMA_URI_PREFIX) {
        return false;
    }
    let rest = &uri[SCHEMA_URI_PREFIX.len()..];
    if rest.is_empty() {
        return false;
    }
    // no query, fragment, credentials, backslashes, percent tricks
    if rest.contains(['?', '#', '@', '\\', '%']) {
        return false;
    }
    if rest.contains("..") || rest.starts_with('/') {
        return false;
    }
    // single path segment namespace, no additional scheme chars
    rest.chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_' | '/'))
        && !rest.is_empty()
}

fn can_produce_object(node: &Value) -> bool {
    if type_names(node)
        .map(|ts| ts.iter().any(|t| t == "object"))
        .unwrap_or(false)
    {
        return true;
    }
    match node.get("type") {
        None => [
            "properties",
            "additionalProperties",
            "patternProperties",
            "required",
        ]
        .iter()
        .any(|k| node.get(k).is_some()),
        _ => false,
    }
}

fn can_produce_string(node: &Value) -> bool {
    if type_names(node)
        .map(|ts| ts.iter().any(|t| t == "string"))
        .unwrap_or(false)
    {
        return true;
    }
    match node.get("type") {
        None => ["maxLength", "minLength", "pattern"]
            .iter()
            .any(|k| node.get(k).is_some()),
        _ => false,
    }
}

fn can_produce_array(node: &Value) -> bool {
    if type_names(node)
        .map(|ts| ts.iter().any(|t| t == "array"))
        .unwrap_or(false)
    {
        return true;
    }
    match node.get("type") {
        None => ["items", "maxItems", "minItems", "prefixItems", "contains"]
            .iter()
            .any(|k| node.get(k).is_some()),
        _ => false,
    }
}

fn type_names(node: &Value) -> Option<Vec<String>> {
    match node.get("type") {
        Some(Value::String(t)) => Some(vec![t.clone()]),
        Some(Value::Array(ts)) => Some(
            ts.iter()
                .filter_map(|t| t.as_str().map(str::to_string))
                .collect(),
        ),
        _ => None,
    }
}

/// Subschema-bearing keys whose values are schemas or maps of schemas.
const SCHEMA_VALUE_KEYS: &[&str] = &[
    "items",
    "additionalItems",
    "contains",
    "propertyNames",
    "not",
    "if",
    "then",
    "else",
    "unevaluatedItems",
    "unevaluatedProperties",
];

const SCHEMA_LIST_KEYS: &[&str] = &["oneOf", "anyOf", "allOf", "prefixItems"];

const SCHEMA_KEYWORDS: &[&str] = &[
    "$schema",
    "$id",
    "$ref",
    "$defs",
    "definitions",
    "type",
    "properties",
    "required",
    "items",
    "additionalProperties",
    "additionalItems",
    "oneOf",
    "anyOf",
    "allOf",
    "not",
    "if",
    "then",
    "else",
    "enum",
    "const",
    "maxLength",
    "minLength",
    "maxItems",
    "minItems",
    "pattern",
    "format",
    "minimum",
    "maximum",
    "title",
    "description",
    "default",
    "contains",
    "propertyNames",
    "prefixItems",
    "dependentSchemas",
    "unevaluatedProperties",
    "unevaluatedItems",
];

// An object is treated as a schema iff it carries at least one recognized
// schema keyword; container maps (the value of "properties"/"$defs") and
// enum instance objects are skipped for closed/bounded keyword checks.
fn looks_like_schema(node: &Value) -> bool {
    match node {
        Value::Object(map) => map.keys().any(|k| SCHEMA_KEYWORDS.contains(&k.as_str())),
        _ => false,
    }
}

fn validate_structure(root: &Value, uri: &str) -> Result<(), CatalogError> {
    // depth, total nodes, per-object properties
    let mut stack: Vec<(&Value, usize)> = vec![(root, 1)];
    let mut total_nodes = 0usize;
    while let Some((node, depth)) = stack.pop() {
        total_nodes += 1;
        if depth > MAX_SCHEMA_DEPTH {
            return Err(CatalogError::TooDeep);
        }
        if total_nodes > MAX_SCHEMA_NODES {
            return Err(CatalogError::TooManyNodes);
        }
        match node {
            Value::Object(map) => {
                if map.contains_key("patternProperties") {
                    return Err(CatalogError::PatternProperties);
                }
                if let Some(props) = map.get("properties").and_then(Value::as_object) {
                    if props.len() > MAX_SCHEMA_PROPERTIES {
                        return Err(CatalogError::TooManyProperties);
                    }
                }
                if looks_like_schema(node) {
                    if can_produce_object(node)
                        && node.get("$ref").is_none()
                        && node.get("additionalProperties") != Some(&Value::Bool(false))
                    {
                        return Err(CatalogError::OpenObject);
                    }
                    if can_produce_string(node) && node.get("$ref").is_none() {
                        match node.get("maxLength") {
                            Some(Value::Number(n)) if n.is_u64() || n.is_i64() => {}
                            _ => return Err(CatalogError::UnboundedString),
                        }
                    }
                    if can_produce_array(node) && node.get("$ref").is_none() {
                        match node.get("maxItems") {
                            Some(Value::Number(n)) if n.is_u64() || n.is_i64() => {}
                            _ => return Err(CatalogError::UnboundedArray),
                        }
                    }
                }
                for (_k, v) in map {
                    stack.push((v, depth + 1));
                }
            }
            Value::Array(items) => {
                for item in items {
                    stack.push((item, depth + 1));
                }
            }
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
        }
    }
    let _ = uri;
    Ok(())
}

fn validate_same_document_cycles(
    root: &Value,
    uri: &str,
    known_uris: &BTreeSet<String>,
) -> Result<(), CatalogError> {
    // Within one document: every visited site, containment edges to
    // descendant sites, plus same-doc ref edges to normalized targets.
    let mut sites: Vec<(String, String)> = Vec::new();
    let mut adj: HashMap<(String, String), Vec<(String, String)>> = HashMap::new();
    let mut stack: Vec<(&Value, String)> = vec![(root, String::new())];
    while let Some((node, pointer)) = stack.pop() {
        sites.push((uri.to_string(), pointer.clone()));
        if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
            let (target_uri, target_pointer) = parse_ref(reference, uri, known_uris)?;
            if target_uri == uri {
                let normalized = match target_pointer.strip_prefix('#') {
                    Some(rest) => rest.to_string(),
                    None => target_pointer,
                };
                adj.entry((uri.to_string(), pointer.clone()))
                    .or_default()
                    .push((target_uri, normalized));
            }
        }
        match node {
            Value::Object(map) => {
                for (k, v) in map {
                    if v.is_object() {
                        stack.push((v, format!("{pointer}/{k}")));
                    } else if let Some(arr) = v.as_array() {
                        for (i, item) in arr.iter().enumerate() {
                            stack.push((item, format!("{pointer}/{k}/{i}")));
                        }
                    }
                }
            }
            Value::Array(arr) => {
                for (i, item) in arr.iter().enumerate() {
                    stack.push((item, format!("{pointer}/{i}")));
                }
            }
            _ => {}
        }
    }
    // Containment only has to hold between ref sites: evaluating a ref target
    // reaches the ref sites contained in it. Enumerating all node pairs would
    // be quadratic in document size; ref sites are few.
    let mut ref_sites: Vec<(String, String)> = sites
        .iter()
        .filter(|site| adj.contains_key(site))
        .cloned()
        .collect();
    ref_sites.sort();
    ref_sites.dedup();
    // Evaluating a ref site reaches every ref site inside its target, so the
    // edge runs site -> contained-in-target. Enumerating all node pairs would
    // be quadratic in document size; ref sites are few.
    let reach: Vec<Vec<(String, String)>> = ref_sites
        .iter()
        .map(|from| {
            let targets = adj.get(from).cloned().unwrap_or_default();
            ref_sites
                .iter()
                .filter(|to| targets.iter().any(|t| contains_pointer(&t.1, &to.1)))
                .cloned()
                .collect()
        })
        .collect();
    for (from, targets) in ref_sites.iter().zip(reach) {
        adj.entry(from.clone()).or_default().extend(targets);
    }
    let mut state: HashMap<(String, String), u8> = HashMap::new();
    for node in ref_sites {
        if state.get(&node).copied().unwrap_or(0) == 0 && reaches_cycle(&node, &adj, &mut state) {
            return Err(CatalogError::CyclicRef);
        }
    }
    Ok(())
}

/// Whether `container` addresses `pointer` or something inside it. The empty
/// pointer is the document root and contains every pointer.
fn contains_pointer(container: &str, pointer: &str) -> bool {
    container == pointer || container.is_empty() || pointer.starts_with(&format!("{container}/"))
}

fn validate_cross_document_cycles(
    docs: &BTreeMap<String, CatalogDocument>,
) -> Result<(), CatalogError> {
    // Document-level graph over cross-document refs.
    let mut edges: HashMap<String, Vec<String>> = HashMap::new();
    for (uri, doc) in docs {
        let mut stack = vec![(&doc.parsed, String::new())];
        while let Some((node, _pointer)) = stack.pop() {
            if let Some(reference) = node.get("$ref").and_then(Value::as_str) {
                let target_uri = parse_ref(
                    reference,
                    uri,
                    &docs.keys().cloned().collect::<BTreeSet<_>>(),
                )?
                .0;
                if target_uri != *uri {
                    edges.entry(uri.clone()).or_default().push(target_uri);
                }
            }
            match node {
                Value::Object(map) => {
                    for (_k, v) in map {
                        if v.is_object() {
                            stack.push((v, String::new()));
                        } else if let Some(arr) = v.as_array() {
                            for item in arr {
                                if item.is_object() {
                                    stack.push((item, String::new()));
                                }
                            }
                        }
                    }
                }
                Value::Array(arr) => {
                    for item in arr {
                        stack.push((item, String::new()));
                    }
                }
                _ => {}
            }
        }
    }
    let mut state: HashMap<String, u8> = HashMap::new();
    let nodes: Vec<String> = edges.keys().cloned().collect();
    for node in nodes {
        if state.get(&node).copied().unwrap_or(0) == 0
            && reaches_cycle_str(&node, &edges, &mut state)
        {
            return Err(CatalogError::CyclicRef);
        }
    }
    Ok(())
}

fn reaches_cycle_str(
    node: &str,
    adj: &HashMap<String, Vec<String>>,
    state: &mut HashMap<String, u8>,
) -> bool {
    state.insert(node.to_string(), 1);
    for next in adj.get(node).cloned().unwrap_or_default() {
        match state.get(next.as_str()).copied().unwrap_or(0) {
            1 => return true,
            0 if reaches_cycle_str(&next, adj, state) => return true,
            _ => {}
        }
    }
    state.insert(node.to_string(), 2);
    false
}

fn reaches_cycle(
    node: &(String, String),
    adj: &HashMap<(String, String), Vec<(String, String)>>,
    state: &mut HashMap<(String, String), u8>,
) -> bool {
    state.insert(node.clone(), 1);
    for next in adj.get(node).cloned().unwrap_or_default() {
        match state.get(&next).copied().unwrap_or(0) {
            1 => return true,
            0 if reaches_cycle(&next, adj, state) => return true,
            _ => {}
        }
    }
    state.insert(node.clone(), 2);
    false
}

fn parse_ref(
    reference: &str,
    current_uri: &str,
    known_uris: &BTreeSet<String>,
) -> Result<(String, String), CatalogError> {
    if reference.starts_with('#') {
        return Ok((current_uri.to_string(), reference.to_string()));
    }
    // exact trusted catalog URI, optionally with a JSON pointer fragment
    let (uri_part, fragment) = match reference.find('#') {
        Some(i) => (&reference[..i], &reference[i..]),
        None => (reference, ""),
    };
    if uri_part.is_empty() && fragment.is_empty() {
        // should not happen: starts with '#' handled above
        return Err(CatalogError::ExternalRef(reference.to_string()));
    }
    if !fragment.is_empty() && !fragment.starts_with("#/") && fragment != "#" {
        return Err(CatalogError::ExternalRef(reference.to_string()));
    }
    if !known_uris.contains(uri_part) {
        // distinguish unknown trusted-namespace document vs external
        if uri_part.starts_with(SCHEMA_URI_PREFIX) {
            return Err(CatalogError::UnknownRef(reference.to_string()));
        }
        return Err(CatalogError::ExternalRef(reference.to_string()));
    }
    Ok((uri_part.to_string(), fragment.to_string()))
}

fn validate_oneof(
    root: &Value,
    uri: &str,
    parsed_docs: &BTreeMap<String, CatalogDocument>,
    known_uris: &BTreeSet<String>,
) -> Result<(), CatalogError> {
    // walk all subschema positions, checking each oneOf
    let mut stack: Vec<&Value> = vec![root];
    while let Some(node) = stack.pop() {
        if let Some(obj) = node.as_object() {
            if let Some(branches) = obj.get("oneOf").and_then(Value::as_array) {
                if branches.len() < 2 {
                    return Err(CatalogError::OneOfNotDisjoint);
                }
                for (pair, branch) in branches.iter().enumerate() {
                    for other in &branches[pair + 1..] {
                        if !pairwise_disjoint(branch, other, uri, parsed_docs, known_uris) {
                            return Err(CatalogError::OneOfNotDisjoint);
                        }
                    }
                }
            }
            for (k, v) in obj {
                let subschemas: Vec<&Value> = match k.as_str() {
                    "properties" | "$defs" | "definitions" | "dependentSchemas"
                    | "patternProperties" => v
                        .as_object()
                        .map(|m| m.values().filter(|sv| sv.is_object()).collect())
                        .unwrap_or_default(),
                    key if SCHEMA_VALUE_KEYS.contains(&key) || key == "additionalProperties" => {
                        if v.is_object() { vec![v] } else { Vec::new() }
                    }
                    key if SCHEMA_LIST_KEYS.contains(&key) => v
                        .as_array()
                        .map(|arr| arr.iter().filter(|item| item.is_object()).collect())
                        .unwrap_or_default(),
                    _ => Vec::new(),
                };
                stack.extend(subschemas);
            }
        }
    }
    let _ = uri;
    Ok(())
}

fn pairwise_disjoint(
    a: &Value,
    b: &Value,
    uri: &str,
    parsed_docs: &BTreeMap<String, CatalogDocument>,
    known_uris: &BTreeSet<String>,
) -> bool {
    let a = resolve_ref(a, uri, parsed_docs, known_uris, 0);
    let b = resolve_ref(b, uri, parsed_docs, known_uris, 0);
    // pairwise incompatible JSON types
    if let (Some(ta), Some(tb)) = (type_names(a), type_names(b)) {
        let sa: HashSet<&String> = ta.iter().collect();
        let sb: HashSet<&String> = tb.iter().collect();
        if sa.is_disjoint(&sb) {
            return true;
        }
    }
    // common required discriminator property with pairwise distinct const
    if let (Some(pa), Some(pb)) = (discriminator_const(a), discriminator_const(b)) {
        if pa.0 == pb.0 && pa.1 != pb.1 {
            return true;
        }
    }
    // two distinct consts on the same property with distinct values
    false
}

/// Returns (property_name, const_value) when the branch requires the property
/// and pins it to a const.
fn discriminator_const(node: &Value) -> Option<(String, &Value)> {
    let required = node.get("required").and_then(Value::as_array)?;
    let props = node.get("properties").and_then(Value::as_object)?;
    for req in required {
        let name = req.as_str()?;
        let prop = props.get(name)?;
        if let Some(c) = prop.get("const") {
            return Some((name.to_string(), c));
        }
    }
    None
}

fn resolve_ref<'a>(
    node: &'a Value,
    uri: &str,
    parsed_docs: &'a BTreeMap<String, CatalogDocument>,
    known_uris: &BTreeSet<String>,
    depth: usize,
) -> &'a Value {
    // The catalog is acyclic, so this terminates; the bound keeps a malformed
    // pointer from walking the same document indefinitely.
    if depth > MAX_SCHEMA_DEPTH {
        return node;
    }
    match node.get("$ref").and_then(Value::as_str) {
        Some(reference) => match parse_ref(reference, uri, known_uris) {
            Ok((target_uri, pointer)) => {
                let Some(target_doc) = parsed_docs.get(&target_uri) else {
                    return node;
                };
                let target =
                    json_pointer(&target_doc.parsed, &pointer).unwrap_or(&target_doc.parsed);
                resolve_ref(target, &target_uri, parsed_docs, known_uris, depth + 1)
            }
            Err(_) => node,
        },
        None => node,
    }
}

fn json_pointer<'a>(root: &'a Value, pointer: &str) -> Option<&'a Value> {
    // pointer: "" | "#" | "#/a/b" possibly with escaped segments
    if pointer.is_empty() || pointer == "#" {
        return Some(root);
    }
    let path = pointer.strip_prefix("#/")?;
    let mut current = root;
    for segment in path.split('/') {
        let segment = segment.replace("~1", "/").replace("~0", "~");
        match current {
            Value::Object(m) => current = m.get(&segment)?,
            Value::Array(a) => current = a.get(usize::from_str(&segment).ok()?)?,
            _ => return None,
        }
    }
    Some(current)
}

fn build_registry(docs: &BTreeMap<String, CatalogDocument>) -> Result<Registry<'_>, CatalogError> {
    let mut registry = Registry::new();
    for doc in docs.values() {
        registry = registry
            .add(&doc.uri, &doc.parsed)
            .map_err(|_| CatalogError::CompileFailed)?;
    }
    registry.prepare().map_err(|_| CatalogError::CompileFailed)
}
