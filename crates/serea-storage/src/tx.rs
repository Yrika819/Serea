/// Opaque transaction capability. Blob and lease operations are scoped here;
/// there is no SQL escape hatch or independent lifecycle row mutation.
/// Later phases add whole-transition methods here, not independent writes on Store.
///
/// External callers cannot extract the underlying transaction:
/// ```compile_fail
/// use serea_storage::Tx;
/// fn bypass(tx: &Tx<'_>) { let _ = &tx.inner; }
/// ```
/// Nor does Tx expose an arbitrary SQL executor:
/// ```compile_fail
/// use serea_storage::Tx;
/// fn bypass(tx: &Tx<'_>) { tx.execute_batch("DELETE FROM tasks").unwrap(); }
/// ```
/// P2D does not expose deferred classified-text or parent-reference helpers:
/// ```compile_fail
/// use serea_storage::Tx;
/// let _ = Tx::put_classified_text;
/// ```
/// ```compile_fail
/// use serea_storage::Tx;
/// let _ = Tx::put_task_blob;
/// ```
/// ```compile_fail
/// use serea_storage::Tx;
/// let _ = Tx::put_step_blob;
/// ```
/// No deletion or independent row-level outcome writer is exposed:
/// ```compile_fail
/// use serea_storage::Store;
/// let _ = Store::delete_task;
/// ```
/// ```compile_fail
/// use serea_storage::{Tx, LeaseGuard, TransitionContext};
/// use serea_protocol::EpochMillis;
/// fn immutable(tx: &Tx<'_>, g: &LeaseGuard, c: &TransitionContext<'_>, now: EpochMillis) {
///     tx.begin_attempt(g, now, c).unwrap();
/// }
/// ```
/// ```compile_fail
/// use serea_storage::Tx;
/// let _ = Tx::commit_step;
/// ```
/// ```compile_fail
/// use serea_storage::Tx;
/// let _ = Tx::insert_receipt;
/// ```
/// ```compile_fail
/// use serea_storage::Tx;
/// let _ = Tx::append_journal;
/// ```
/// ```compile_fail
/// use serea_storage::Tx;
/// let _ = Tx::validate_guard;
/// ```
/// The public transaction capability remains usable without a SQL escape hatch:
/// ```
/// use serea_protocol::{Clock, EpochMillis, ProtocolError};
/// use serea_storage::{Store, StoreError, Tx};
/// struct Fixed;
/// impl Clock for Fixed {
///     fn now_ms(&self) -> Result<EpochMillis, ProtocolError> { EpochMillis::new(0) }
/// }
/// let store = Store::open_in_memory(&Fixed).unwrap();
/// let value = store.transact(|_: &mut Tx<'_>| Ok::<_, StoreError>(42)).unwrap();
/// assert_eq!(value, 42);
/// ```
pub struct Tx<'conn> {
    pub(crate) inner: rusqlite::Transaction<'conn>,
    pub(crate) protection: Option<std::sync::Arc<dyn crate::AtRestProtection>>,
    pub(crate) rollback_only: bool,
    // Capability provenance only; never a substitute for SQLite lease authority.
    pub(crate) origin: std::sync::Arc<std::sync::atomic::AtomicBool>,
}

impl Tx<'_> {
    pub(crate) fn ensure_active(&self) -> Result<(), crate::StoreError> {
        if self.rollback_only || self.inner.is_autocommit() {
            Err(crate::StoreError::Sqlite)
        } else {
            Ok(())
        }
    }
}
