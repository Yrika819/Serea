use crate::{StoreError, Tx};
use serea_protocol::{DataClass, EpochMillis, Seq, SereaEvent, canonicalize};

/// Appends a complete immutable Event Protocol object and assigns its host
/// sequence inside the caller's transaction.
impl Tx<'_> {
    pub fn append_event(
        &mut self,
        event: SereaEvent,
        retention_at: Option<EpochMillis>,
    ) -> Result<SereaEvent, StoreError> {
        self.ensure_active()?;
        if self.event_count >= 16 {
            return Err(StoreError::EventTransactionLimit);
        }
        let saved = self.operation_savepoint(|tx| tx.append_event_inner(event, retention_at))?;
        self.event_count += 1;
        Ok(saved)
    }

    fn append_event_inner(
        &mut self,
        mut event: SereaEvent,
        retention_at: Option<EpochMillis>,
    ) -> Result<SereaEvent, StoreError> {
        match event.data_class {
            DataClass::Public | DataClass::Personal => {}
            DataClass::Private => return Err(StoreError::AtRestProtectionUnavailable),
            DataClass::Secret | DataClass::Credential => {
                return Err(StoreError::EventClassRefused);
            }
        }

        let payload_source =
            serde_json::to_string(&event.payload).map_err(|_| StoreError::CanonicalJson)?;
        let payload = canonicalize(&payload_source).map_err(|_| StoreError::CanonicalJson)?;
        if payload.len() > 32_768 {
            return Err(StoreError::EventPayloadTooLarge);
        }

        let (last, retained, bytes): (i64, i64, i64) = self.inner.query_row(
            "SELECT last_allocated_seq,retained_count,event_store_bytes FROM event_store_state WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        let seq = last
            .checked_add(1)
            .ok_or(StoreError::EventSequenceOverflow)?;
        let seq_u64 = u64::try_from(seq).map_err(|_| StoreError::EventSequenceOverflow)?;
        event.seq = Seq::new(seq_u64);
        let event_source = serde_json::to_string(&event).map_err(|_| StoreError::CanonicalJson)?;
        let event_json = canonicalize(&event_source).map_err(|_| StoreError::CanonicalJson)?;
        let event_json = String::from_utf8(event_json).map_err(|_| StoreError::CanonicalJson)?;
        let content_bytes =
            i64::try_from(event_json.len()).map_err(|_| StoreError::EventStoreCapacity)?;
        // Deterministic logical accounting: retained canonical event bytes plus
        // eight bytes for the active sequence ledger row. SQLite page and WAL
        // overhead is excluded because it varies across builds and platforms.
        let charge = content_bytes
            .checked_add(8)
            .ok_or(StoreError::EventStoreCapacity)?;
        let updated_bytes = bytes
            .checked_add(charge)
            .ok_or(StoreError::EventStoreCapacity)?;
        if retained >= 1_000_000 || updated_bytes > 536_870_912 {
            return Err(StoreError::EventStoreCapacity);
        }

        self.inner.execute(
            "UPDATE event_store_state SET last_allocated_seq=?1,retained_count=retained_count+1,event_store_bytes=?2 WHERE singleton=1 AND last_allocated_seq=?3",
            rusqlite::params![seq, updated_bytes, last],
        )?;
        self.inner
            .execute("INSERT INTO event_sequence_ledger(seq) VALUES (?1)", [seq])?;
        self.inner.execute(
            "INSERT INTO event_content(seq,message_id,kind,occurred_at_ms,data_class_rank,event_json,payload_bytes,content_bytes,retention_at_ms)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9)",
            rusqlite::params![
                seq,
                event.message_id.to_string(),
                event.kind.wire_name(),
                event.occurred_at.to_epoch_millis().get(),
                i64::from(event.data_class.rank()),
                event_json,
                i64::try_from(payload.len()).map_err(|_| StoreError::EventPayloadTooLarge)?,
                content_bytes,
                retention_at.map(EpochMillis::get),
            ],
        )?;
        Ok(event)
    }
}
