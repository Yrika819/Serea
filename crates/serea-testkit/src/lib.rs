//! `serea-testkit` — deterministic, offline test doubles for Serea's protocol
//! surfaces.
//!
//! This crate is dev-only by construction (Crate Map §5.3): it is a workspace
//! member that no runtime crate may name, and `tests/workspace_smoke.py`
//! fails the build if any non-dev dependency table mentions it.
//!
//! What is deliberately **not** here:
//!
//! * No `FakeGoalLatchProvider` and no `HostGoalProvider` implementation. The
//!   fake is P15 work (GoalLatch Adapter §6, `G1`), and P1 must not create a
//!   GoalLatch connection or imply those capabilities are available at runtime.
//! * No network, filesystem, or subprocess access. Every double is in-memory.
//! * No wall clock and no randomness. `TestClock` and the deterministic
//!   identifier source are the only sources of time, so a repeated call under
//!   identical inputs returns an identical result (Model Protocol §10).
//! * No credential material. There is no fixture, snapshot, or constructor that
//!   can hold secret bytes (`DC7`).
//! * No "magic valid object" helper that fills in an authority-bearing field.
//!   Synthetic constants are provided for the values that carry no authority;
//!   anything the host resolves — `data_class`, `deadline_ms`,
//!   `arguments_digest`, `requested_by`, `capability_version` — is left to the
//!   caller to state explicitly at the construction site.
//!
//! The fixture values are synthetic. Personal-data-shaped fixtures use the
//! reserved `.test` domain, which RFC 2606 reserves for exactly this purpose.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
// See the note on the same lint set in `serea-protocol/src/lib.rs`.
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod clock;
pub mod models;
pub mod providers;

pub use clock::deterministic_minter;
pub use clock::{DeterministicUlidSource, FROZEN_EPOCH, TestClock};
pub use models::{MockModelProvider, ModelScript, synthetic_model_roster};
pub use providers::{
    CapabilityScript, MockCapabilityProvider, synthetic_descriptor, synthetic_read_descriptor,
};

/// A synthetic account address in the reserved `.test` domain. Never a real
/// address, never a real account.
pub const SYNTHETIC_ACCOUNT: &str = "alice@example.test";

/// A second synthetic account, so a test can distinguish two parties without
/// using anything resembling real personal data.
pub const OTHER_SYNTHETIC_ACCOUNT: &str = "bob@example.test";

/// `codex_allowed` defaults to `false` and is a host-level task setting
/// (Model Protocol §8; GoalLatch Adapter §5.1).
///
/// P1 has no durable task configuration type — the frozen `AssistantTask`
/// shape does not carry the setting, and `serea-core`'s `BoundConfig` owns it in
/// a later phase — so the default is stated here, once, explicitly, rather than
/// being implied by the absence of a field. `CRATE_MAP`-visible crates read
/// this constant rather than re-deciding it.
pub const CODEX_ALLOWED: bool = false;

/// The model identifier registered as *known but disabled* (Model Protocol
/// §5.1). No routing chain for any purpose contains it (`M5`, `M6`).
pub const CODEX_MODEL_ID: &str = "codex";
