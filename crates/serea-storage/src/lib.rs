//! SQLite schema and transaction foundation for Serea (P2C).
//!
//! File-backed stores use WAL, FULL synchronization, explicit foreign keys and
//! a bounded busy timeout. The separate memory constructor is test-only in its
//! guarantees, not durable. No blob/classification, lease, engine, recovery,
//! journal participant or event runtime is implemented by this crate yet.

mod error;
mod migrate;
mod store;
mod tx;

pub use error::StoreError;
pub use migrate::{Migration, Migrations};
pub use store::{CheckpointOutcome, Store};
pub use tx::Tx;

#[cfg(test)]
mod foundation_tests;
#[cfg(test)]
mod schema_tests;
