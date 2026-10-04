//! SQLite foundation, classified JSON blobs and durable lease authority (P2E).
//!
//! File-backed stores use WAL, FULL synchronization, explicit foreign keys and
//! a bounded busy timeout. The separate memory constructor is test-only in its
//! guarantees, not durable. Blob identity is canonical plaintext SHA-256 plus
//! class. PRIVATE blobs fail closed without injected protection; P2D ships no
//! real backend and claims no complete PRIVATE task/row support. SECRET and
//! CREDENTIAL are refused. Transaction-scoped acquisition/renewal/release use
//! SQLite authority. No begin/outcome/task lifecycle, engine, recovery,
//! participant, receipt/journal or event runtime is implemented.

mod blob;
mod classify;
mod error;
mod lease;
mod migrate;
mod store;
mod tx;

pub use blob::BlobRef;
pub use classify::{AtRestProtection, AtRestProtectionError};
pub use error::StoreError;
pub use lease::LeaseGuard;
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
