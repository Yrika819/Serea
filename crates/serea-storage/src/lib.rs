//! SQLite foundation and classified canonical JSON blobs for Serea (P2D).
//!
//! File-backed stores use WAL, FULL synchronization, explicit foreign keys and
//! a bounded busy timeout. The separate memory constructor is test-only in its
//! guarantees, not durable. Blob identity is canonical plaintext SHA-256 plus
//! class. PRIVATE blobs fail closed without injected protection; P2D ships no
//! real backend and claims no complete PRIVATE task/row support. SECRET and
//! CREDENTIAL are refused. No task/step, lease, engine, recovery, participant
//! or event runtime is implemented.

mod blob;
mod classify;
mod error;
mod migrate;
mod store;
mod tx;

pub use blob::BlobRef;
pub use classify::{AtRestProtection, AtRestProtectionError};
pub use error::StoreError;
pub use migrate::{Migration, Migrations};
pub use store::{CheckpointOutcome, Store};
pub use tx::Tx;

#[cfg(test)]
mod blob_tests;
#[cfg(test)]
mod foundation_tests;
#[cfg(test)]
mod protection_tests;
#[cfg(test)]
mod schema_tests;
