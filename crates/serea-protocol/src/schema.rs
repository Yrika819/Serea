//! The checked-in JSON Schema 2020-12 contracts and the validator that reads
//! them.
//!
//! Frozen source: `docs/protocols/00-protocol-index.md` §5 ("JSON Schema
//! 2020-12 for every `input_schema` and `output_schema`"; fail-closed
//! compatibility rules), Capability Protocol §3.1 (closed-world constraints on
//! capability schemas), and Data Classification §4 (an allowlist, not a
//! denylist, on security-sensitive input).
//!
//! Two properties this module is built to guarantee:
//!
//! * **The schema documents are the contract.** They are embedded from
//!   `schemas/*.json` at compile time and validated as-is. There is deliberately
//!   no second, Rust-only description of the same constraints that could drift
//!   from the checked-in document silently.
//! * **No resolution escapes the process.** The `jsonschema` dependency is
//!   built with `default-features = false`, so its `resolve-http`,
//!   `resolve-file`, `resolve-async` and TLS features are off. A `$ref` can
//!   only resolve inside the document it appears in, so validating a payload
//!   can never read the filesystem or the network.

use std::fmt;
use std::sync::OnceLock;

use jsonschema::Draft;
use serde_json::Value;

use crate::errors::{ProtocolError, SerializationRejection};

/// The deepest instance [`validate`] will walk.
///
/// `serde_json`'s own 128-level recursion guard only applies when parsing *text*.
/// A `Value` handed straight to `validate` — which is the ordinary shape once
/// P2 reads a stored blob with `from_value` — has no guard, and a recursive
/// schema over a deep enough value exhausts the stack, which aborts the process
/// rather than unwinding. Capability Protocol §3.1 requires no schema to be
/// "recursive without an explicit depth bound"; this is that bound, enforced on
/// the instance side as well.
///
/// 64 is far deeper than any capability schema in the frozen set needs (the
/// deepest is `arguments` → a per-capability object three levels down).
pub const MAX_INSTANCE_DEPTH: usize = 64;

/// One of the five checked-in schema documents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchemaName {
    /// The cross-boundary envelope — Protocol Index §6. Not a wire surface: the
    /// envelope carries the `surface` of the message it carries.
    Envelope,
    /// `serea.action/1` request — Capability Protocol §4. Closed.
    ActionRequest,
    /// `serea.action/1` result — Capability Protocol §5. Closed.
    ActionResult,
    /// `serea.task/1` task — Task Protocol §2. Forward-compatible.
    AssistantTask,
    /// `serea.event/1` event — Event Protocol §2. Forward-compatible.
    Event,
}

impl SchemaName {
    /// Every checked-in document, in a stable order.
    pub const ALL: [SchemaName; 5] = [
        SchemaName::Envelope,
        SchemaName::ActionRequest,
        SchemaName::ActionResult,
        SchemaName::AssistantTask,
        SchemaName::Event,
    ];

    /// The checked-in document's file name.
    pub fn file_name(self) -> &'static str {
        match self {
            SchemaName::Envelope => "envelope.schema.json",
            SchemaName::ActionRequest => "action-request.schema.json",
            SchemaName::ActionResult => "action-result.schema.json",
            SchemaName::AssistantTask => "assistant-task.schema.json",
            SchemaName::Event => "event.schema.json",
        }
    }

    /// The exact text of the checked-in document.
    pub fn document(self) -> &'static str {
        match self {
            SchemaName::Envelope => include_str!("../schemas/envelope.schema.json"),
            SchemaName::ActionRequest => include_str!("../schemas/action-request.schema.json"),
            SchemaName::ActionResult => include_str!("../schemas/action-result.schema.json"),
            SchemaName::AssistantTask => include_str!("../schemas/assistant-task.schema.json"),
            SchemaName::Event => include_str!("../schemas/event.schema.json"),
        }
    }

    /// The document parsed as JSON.
    ///
    /// This is the only place a schema document is interpreted, so it is the
    /// only place a document can be found unusable.
    pub fn json(self) -> Result<Value, SchemaError> {
        serde_json::from_str(self.document()).map_err(|error| SchemaError::SchemaDocumentUnusable {
            schema: self,
            source: ProtocolError::Serialization {
                surface: self.file_name(),
                reason: SerializationRejection::MalformedJson,
            },
            detail: error.to_string(),
        })
    }

    /// The compile-time index into [`SchemaName::ALL`].
    ///
    /// Exhaustive on purpose: a new variant that is not added to `ALL` is a
    /// compile error rather than a silent fallback to another document, which
    /// is the drift this module exists to prevent.
    fn index(self) -> usize {
        match self {
            SchemaName::Envelope => 0,
            SchemaName::ActionRequest => 1,
            SchemaName::ActionResult => 2,
            SchemaName::AssistantTask => 3,
            SchemaName::Event => 4,
        }
    }
}

impl fmt::Display for SchemaName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.file_name())
    }
}

/// One reason an instance did not satisfy a schema.
///
/// Carried as data rather than as a rendered string so a caller can decide
/// programmatically, without parsing prose (Event Protocol §4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchemaViolation {
    /// JSON Pointer into the instance.
    pub instance_path: String,
    /// JSON Pointer into the schema document.
    pub schema_path: String,
    /// The validator's diagnostic. Diagnostic only.
    pub message: String,
}

/// Why schema validation could not proceed or did not succeed.
#[derive(Debug, Clone, PartialEq)]
pub enum SchemaError {
    /// The checked-in document is not usable, so nothing can be validated
    /// against it. This is a repository defect, not caller input.
    SchemaDocumentUnusable {
        /// The document at fault.
        schema: SchemaName,
        /// The typed reason.
        source: ProtocolError,
        /// The validator's own diagnostic. Diagnostic only.
        detail: String,
    },
    /// The instance does not satisfy the document. Fail closed.
    InstanceInvalid {
        /// The document that was applied.
        schema: SchemaName,
        /// Every violation found, in validator order.
        violations: Vec<SchemaViolation>,
    },
    /// The instance is nested deeper than [`MAX_INSTANCE_DEPTH`].
    ///
    /// Refused before the validator runs, so a hostile or corrupt stored blob
    /// cannot exhaust the stack.
    TooDeep {
        /// The document that would have been applied.
        schema: SchemaName,
        /// The depth that was found, one past the limit.
        depth: usize,
        /// The limit that was exceeded.
        limit: usize,
    },
}

impl fmt::Display for SchemaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SchemaError::SchemaDocumentUnusable {
                schema,
                source,
                detail,
            } => {
                write!(f, "{schema} is not usable: {source} ({detail})")
            }
            SchemaError::TooDeep {
                schema,
                depth,
                limit,
            } => {
                write!(
                    f,
                    "instance nests {depth} levels into {schema}, past the {limit}-level \
                     bound this validator enforces"
                )
            }
            SchemaError::InstanceInvalid { schema, violations } => {
                write!(f, "instance does not satisfy {schema}: ")?;
                for (index, violation) in violations.iter().enumerate() {
                    if index > 0 {
                        f.write_str("; ")?;
                    }
                    write!(
                        f,
                        "{} {}",
                        if violation.instance_path.is_empty() {
                            "<root>"
                        } else {
                            violation.instance_path.as_str()
                        },
                        violation.message
                    )?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for SchemaError {}

/// One compiled validator per checked-in document, compiled on first use.
///
/// `Result` is cached so a defective document produces the same typed error on
/// every call instead of re-parsing.
fn compiled() -> &'static [OnceLock<Result<jsonschema::Validator, String>>; 5] {
    static COMPILED: [OnceLock<Result<jsonschema::Validator, String>>; 5] = [
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
        OnceLock::new(),
    ];
    &COMPILED
}

/// Returns the compiled validator for `name`, failing closed if the document is
/// unusable.
pub fn validator(name: SchemaName) -> Result<&'static jsonschema::Validator, SchemaError> {
    let slot = &compiled()[name.index()];
    let outcome = slot.get_or_init(|| match name.json() {
        Ok(document) => jsonschema::options()
            .with_draft(Draft::Draft202012)
            .build(&document)
            .map_err(|error| format!("{error}")),
        Err(_) => Err("document is not valid JSON".to_owned()),
    });
    outcome
        .as_ref()
        .map_err(|detail| SchemaError::SchemaDocumentUnusable {
            schema: name,
            source: ProtocolError::Serialization {
                surface: name.file_name(),
                reason: SerializationRejection::SchemaUnusable,
            },
            detail: detail.clone(),
        })
}

/// Validates `instance` against the checked-in document for `name`.
///
/// Returns `Err` on any violation: this is the fail-closed boundary
/// (Capability Protocol §3.1). A caller that wants a boolean must ask
/// deliberately via [`is_valid`].
pub fn validate(name: SchemaName, instance: &Value) -> Result<(), SchemaError> {
    let schema = validator(name)?;
    let depth = instance_depth(instance);
    if depth > MAX_INSTANCE_DEPTH {
        return Err(SchemaError::TooDeep {
            schema: name,
            depth,
            limit: MAX_INSTANCE_DEPTH,
        });
    }
    if schema.is_valid(instance) {
        return Ok(());
    }
    let violations = schema
        .iter_errors(instance)
        .map(|error| SchemaViolation {
            instance_path: error.instance_path().to_string(),
            schema_path: error.schema_path().to_string(),
            // The validator's own rendering quotes the offending instance value.
            // Externally derived values are untrusted and may be
            // credential-shaped, and an error value can reach a log or a crash
            // report (`errors.rs`, Data Classification §3, `DC7`), so the reason
            // is derived from the *kind* — which carries a limit or an index,
            // never the bytes — and the instance is named by its JSON Pointer.
            message: describe(&error),
        })
        .collect();
    Err(SchemaError::InstanceInvalid {
        schema: name,
        violations,
    })
}

/// The nesting depth of `value`, counting the root as one level.
///
/// Iterative over an explicit stack, so the guard that exists to prevent a stack
/// overflow cannot itself overflow.
/// The nesting depth of `value`, counting the root as one level.
///
/// One pass, one explicit stack, no recursion: the guard that exists to prevent
/// a stack overflow must not be able to cause one, and a recursive walk over a
/// deep chain is quadratic as well as fatal.
fn instance_depth(value: &Value) -> usize {
    let mut stack: Vec<(&Value, usize)> = vec![(value, 1)];
    let mut deepest = 0usize;
    while let Some((node, depth)) = stack.pop() {
        deepest = deepest.max(depth);
        match node {
            Value::Array(items) => {
                for item in items {
                    stack.push((item, depth + 1));
                }
            }
            Value::Object(map) => {
                for child in map.values() {
                    stack.push((child, depth + 1));
                }
            }
            _ => {}
        }
    }
    deepest
}

/// A value-free description of why an instance failed.
///
/// The validator's own rendering quotes the offending instance, and four of its
/// error kinds (`AnyOf`, `OneOf`, `PropertyNames`, `AdditionalProperties`) nest
/// a full sub-error, so a naive `Debug` re-prints the value. Externally derived
/// values are untrusted and may be credential-shaped, and an error value can
/// reach a log or a crash report (Data Classification §3, `DC7`), so this walks
/// the kinds that can embed instance data and renders everything else from the
/// schema side alone.
///
/// `error.keyword()` is safe by construction: `jsonschema` returns it from a
/// `match` over literal keyword strings, never from instance data.
fn describe(error: &jsonschema::ValidationError<'_>) -> String {
    use jsonschema::error::ValidationErrorKind as Kind;
    let kind = error.kind();
    match kind {
        // The four kinds that embed a sub-error or the offending names. A
        // `Debug` of any of them re-prints the instance.
        Kind::AnyOf { .. }
        | Kind::OneOfNotValid { .. }
        | Kind::OneOfMultipleValid { .. }
        | Kind::PropertyNames { .. } => "does not match any permitted form".to_owned(),
        Kind::AdditionalProperties { .. } | Kind::UnevaluatedProperties { .. } => {
            "undeclared property".to_owned()
        }
        Kind::UnevaluatedItems { .. } | Kind::UniqueItems | Kind::Contains => {
            "array constraint not satisfied".to_owned()
        }
        // Limits come from the checked-in document, never from the instance.
        Kind::Minimum { limit }
        | Kind::Maximum { limit }
        | Kind::ExclusiveMinimum { limit }
        | Kind::ExclusiveMaximum { limit } => format!("{}: {limit}", kind.keyword()),
        Kind::MinLength { limit }
        | Kind::MaxLength { limit }
        | Kind::MinItems { limit }
        | Kind::MaxItems { limit }
        | Kind::MinProperties { limit }
        | Kind::MaxProperties { limit } => format!("{}: {limit}", kind.keyword()),
        Kind::AdditionalItems { limit } => format!("{}: {limit}", kind.keyword()),
        // Everything else is reported by keyword alone. `kind.keyword()` is safe
        // by construction: `jsonschema` returns it from a `match` over literal
        // keyword strings. The wildcard arm means a variant added to a future
        // `jsonschema` release cannot reintroduce an echo.
        _ => kind.keyword().to_owned(),
    }
}

/// Whether `instance` satisfies the document for `name`.
///
/// Provided for a caller that genuinely wants a boolean. A caller making a
/// security decision must use [`validate`] so the violations are available.
pub fn is_valid(name: SchemaName, instance: &Value) -> Result<bool, SchemaError> {
    match validate(name, instance) {
        Ok(()) => Ok(true),
        Err(SchemaError::InstanceInvalid { .. }) => Ok(false),
        // A document that will not compile, or an instance too deep to walk, is
        // a repository or caller fault, not a simple "no".
        Err(other) => Err(other),
    }
}
