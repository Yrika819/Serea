//! `serea-policy` — deterministic policy evaluation, approval request and grant
//! lifecycle, and the P6 authorization evidence P8 revalidates.
//!
//! P6 owns everything between the frozen P5 `PreparedActionV1` and the P8
//! dispatch boundary (ADR-0037 through ADR-0040). It is a pure function of
//! durable state and trusted host input: it holds no model in the loop, no
//! network call in the loop, and no wall clock that the rules can read.
//!
//! The crate deliberately does **not** invoke a provider. P8 remains the first
//! phase permitted to call `CapabilityProvider::invoke`. It mints no `RequestId`,
//! creates no dispatch intent, runs no duplicate suppression and keeps no
//! repeated-action counter. Its output is `AuthorizationEvidenceV1` — evidence
//! about state at a named policy revision — never an execution permission and
//! never an `approved: bool`.
//!
//! Approval authority is lent, bounded, expiring and revocable. A grant binds an
//! exact task, an exact capability and version, an exact registry generation and
//! descriptor revision, an exact plan revision, an exact expiry, and — under
//! owner decision R2 — an explicitly enumerated set of 1 to 8 individually
//! approved actions, each with its own `StepId` and its own exact
//! `arguments_digest`. Membership, never scope or digest equality, is what
//! authorizes a consuming Step.
//!
//! No credential material is stored anywhere in this crate. `serea-storage`
//! refuses `SECRET` and `CREDENTIAL` mechanically, and the persisted approval row
//! is bounded to the ratified `PUBLIC`/`PERSONAL` ceiling.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
