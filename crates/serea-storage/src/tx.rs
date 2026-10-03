/// Opaque transaction capability. P2C provides no row-level SQL escape hatch.
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
}
