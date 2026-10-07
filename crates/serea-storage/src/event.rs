use crate::{Store, StoreError, Tx};
use rusqlite::{Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serea_protocol::{DataClass, EpochMillis, Seq, SereaEvent, WireSurface, canonicalize};

pub const MAX_EVENT_REPLAY_PAGE: u16 = 256;
pub const MAX_RETENTION_DELETE_BATCH: u16 = 512;

/// One typed Device Protocol/2 replay result. An expiry range contains no
/// identifying or event-derived data; corruption marks the first unproved seq.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReplayItem {
    Event {
        event: Box<SereaEvent>,
    },
    #[serde(rename = "INTENTIONALLY_EXPIRED_RANGE")]
    ExpiredRange {
        first_seq: Seq,
        last_seq: Seq,
    },
    HistoryExpiredPrefix {
        new_replay_boundary: Seq,
    },
    Corruption {
        first_unexplained_seq: Seq,
    },
}

/// A bounded, ordered replay result through one committed high-water snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventReplayPage {
    pub snapshot_high_water_seq: Seq,
    pub next_seq: Option<Seq>,
    pub items: Vec<ReplayItem>,
}

/// Result of one bounded whole-content retention transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRetentionReport {
    pub content_records_deleted: u16,
    pub expired_prefix_through: Seq,
}

impl Store {
    /// Replays an exclusive cursor through a single committed high-water
    /// snapshot. Missing unproved sequences are typed corruption, never expiry.
    pub fn replay_events(
        &self,
        after_seq: Option<Seq>,
        through_seq: Option<Seq>,
        limit: u16,
    ) -> Result<EventReplayPage, StoreError> {
        self.transact(|tx| tx.replay_events(after_seq, through_seq, limit))
    }

    /// Deletes whole records whose explicit retention deadline has passed.
    /// Consumer backlog does not extend the privacy deadline.
    pub fn expire_eligible_events(
        &self,
        now: EpochMillis,
        limit: u16,
    ) -> Result<EventRetentionReport, StoreError> {
        self.transact(|tx| tx.expire_eligible_events(now, limit))
    }
}

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

impl Tx<'_> {
    pub fn replay_events(
        &mut self,
        after_seq: Option<Seq>,
        through_seq: Option<Seq>,
        limit: u16,
    ) -> Result<EventReplayPage, StoreError> {
        self.ensure_active()?;
        if !(1..=MAX_EVENT_REPLAY_PAGE).contains(&limit) {
            return Err(StoreError::InvalidEventReplayPage);
        }
        let (high, prefix): (i64, i64) = self.inner.query_row(
            "SELECT last_allocated_seq,expired_prefix_through FROM event_store_state WHERE singleton=1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if prefix < 0 || high < prefix {
            return Err(StoreError::EventHistoryCorrupt);
        }
        let committed_high = u64::try_from(high).map_err(|_| StoreError::EventHistoryCorrupt)?;
        let high = through_seq.map_or(committed_high, Seq::get);
        if high > committed_high {
            return Err(StoreError::InvalidEventReplayPage);
        }
        let prefix = u64::try_from(prefix).map_err(|_| StoreError::EventHistoryCorrupt)?;
        let cursor = after_seq.map_or(0, Seq::get);
        let high_seq = Seq::new(high);

        if cursor < prefix {
            return Ok(EventReplayPage {
                snapshot_high_water_seq: high_seq,
                next_seq: Some(Seq::new(prefix)),
                items: vec![ReplayItem::HistoryExpiredPrefix {
                    new_replay_boundary: Seq::new(prefix),
                }],
            });
        }
        if cursor >= high {
            return Ok(EventReplayPage {
                snapshot_high_water_seq: high_seq,
                next_seq: None,
                items: Vec::new(),
            });
        }

        let mut items = Vec::new();
        let mut next = cursor
            .checked_add(1)
            .ok_or(StoreError::EventHistoryCorrupt)?;
        let mut last_verified = None;
        while next <= high && items.len() < usize::from(limit) {
            let sql_seq = i64::try_from(next).map_err(|_| StoreError::EventHistoryCorrupt)?;
            let content: Option<(String, String, String, i64, i64, i64, i64)> = self
                .inner
                .query_row(
                    "SELECT event_json,message_id,kind,occurred_at_ms,data_class_rank,payload_bytes,content_bytes
                     FROM event_content WHERE seq=?1",
                    [sql_seq],
                    |row| {
                        Ok((
                            row.get(0)?,
                            row.get(1)?,
                            row.get(2)?,
                            row.get(3)?,
                            row.get(4)?,
                            row.get(5)?,
                            row.get(6)?,
                        ))
                    },
                )
                .optional()?;
            let ledger_exists: bool = self.inner.query_row(
                "SELECT EXISTS(SELECT 1 FROM event_sequence_ledger WHERE seq=?1)",
                [sql_seq],
                |row| row.get(0),
            )?;
            let expired: Option<(i64, i64)> = self
                .inner
                .query_row(
                    "SELECT first_seq,last_seq FROM event_expired_ranges
                     WHERE first_seq<=?1 AND last_seq>=?1 ORDER BY first_seq DESC LIMIT 1",
                    [sql_seq],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .optional()?;

            if let Some((
                json,
                expected_id,
                expected_kind,
                occurred,
                class_rank,
                payload_bytes,
                content_bytes,
            )) = content
            {
                if !ledger_exists || expired.is_some() {
                    items.push(ReplayItem::Corruption {
                        first_unexplained_seq: Seq::new(next),
                    });
                    break;
                }
                let event: SereaEvent = match serde_json::from_str(&json) {
                    Ok(event) => event,
                    Err(_) => {
                        items.push(ReplayItem::Corruption {
                            first_unexplained_seq: Seq::new(next),
                        });
                        break;
                    }
                };
                let canonical = match canonicalize(&json) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        items.push(ReplayItem::Corruption {
                            first_unexplained_seq: Seq::new(next),
                        });
                        break;
                    }
                };
                let payload_source = match serde_json::to_string(&event.payload) {
                    Ok(json) => json,
                    Err(_) => {
                        items.push(ReplayItem::Corruption {
                            first_unexplained_seq: Seq::new(next),
                        });
                        break;
                    }
                };
                let payload = match canonicalize(&payload_source) {
                    Ok(bytes) => bytes,
                    Err(_) => {
                        items.push(ReplayItem::Corruption {
                            first_unexplained_seq: Seq::new(next),
                        });
                        break;
                    }
                };
                if event.seq != Seq::new(next)
                    || event.kind.wire_name() != expected_kind
                    || event.message_id.as_str() != expected_id
                    || canonical != json.as_bytes()
                    || event.occurred_at.to_epoch_millis().get() != occurred
                    || i64::from(event.data_class.rank()) != class_rank
                    || i64::try_from(payload.len()).ok() != Some(payload_bytes)
                    || i64::try_from(json.len()).ok() != Some(content_bytes)
                    || event.surface.as_str() != WireSurface::EVENT
                    || !event.envelope_version.is_supported()
                {
                    items.push(ReplayItem::Corruption {
                        first_unexplained_seq: Seq::new(next),
                    });
                    break;
                }
                items.push(ReplayItem::Event {
                    event: Box::new(event),
                });
                last_verified = Some(Seq::new(next));
                next = next.checked_add(1).ok_or(StoreError::EventHistoryCorrupt)?;
                continue;
            }

            if let Some((first, last)) = expired {
                let first = u64::try_from(first).map_err(|_| StoreError::EventHistoryCorrupt)?;
                let last = u64::try_from(last).map_err(|_| StoreError::EventHistoryCorrupt)?;
                if first > next || last < next || last > high {
                    items.push(ReplayItem::Corruption {
                        first_unexplained_seq: Seq::new(next),
                    });
                    break;
                }
                let end = i64::try_from(last).map_err(|_| StoreError::EventHistoryCorrupt)?;
                let overlap: bool = self.inner.query_row(
                    "SELECT EXISTS(SELECT 1 FROM event_sequence_ledger WHERE seq BETWEEN ?1 AND ?2)
                     OR EXISTS(SELECT 1 FROM event_content WHERE seq BETWEEN ?1 AND ?2)",
                    rusqlite::params![sql_seq, end],
                    |row| row.get(0),
                )?;
                if overlap {
                    items.push(ReplayItem::Corruption {
                        first_unexplained_seq: Seq::new(next),
                    });
                    break;
                }
                items.push(ReplayItem::ExpiredRange {
                    first_seq: Seq::new(next),
                    last_seq: Seq::new(last),
                });
                last_verified = Some(Seq::new(last));
                next = last.checked_add(1).ok_or(StoreError::EventHistoryCorrupt)?;
                continue;
            }

            items.push(ReplayItem::Corruption {
                first_unexplained_seq: Seq::new(next),
            });
            break;
        }

        Ok(EventReplayPage {
            snapshot_high_water_seq: high_seq,
            next_seq: last_verified,
            items,
        })
    }

    pub fn expire_eligible_events(
        &mut self,
        now: EpochMillis,
        limit: u16,
    ) -> Result<EventRetentionReport, StoreError> {
        self.ensure_active()?;
        if !(1..=MAX_RETENTION_DELETE_BATCH).contains(&limit) {
            return Err(StoreError::InvalidRetentionDeleteBatch);
        }
        self.operation_savepoint(|tx| {
            let candidates: Vec<(i64, i64)> = {
                let mut statement = tx.inner.prepare(
                    "SELECT seq,content_bytes FROM event_content
                     WHERE retention_at_ms IS NOT NULL AND retention_at_ms<=?1
                     ORDER BY seq LIMIT ?2",
                )?;
                statement
                    .query_map(rusqlite::params![now.get(), i64::from(limit)], |row| {
                        Ok((row.get(0)?, row.get(1)?))
                    })?
                    .collect::<Result<_, _>>()?
            };
            let mut deleted = 0_u16;
            for (seq, content_bytes) in candidates {
                let accounted: bool = tx.inner.query_row(
                    "SELECT EXISTS(SELECT 1 FROM event_sequence_ledger WHERE seq=?1)
                     AND NOT EXISTS(SELECT 1 FROM event_expired_ranges
                                    WHERE first_seq<=?1 AND last_seq>=?1)",
                    [seq],
                    |row| row.get(0),
                )?;
                if !accounted || content_bytes < 0 {
                    return Err(StoreError::EventHistoryCorrupt);
                }
                let first = u64::try_from(seq).map_err(|_| StoreError::EventHistoryCorrupt)?;
                merge_expired_range(tx, first)?;
                let removed = tx
                    .inner
                    .execute("DELETE FROM event_content WHERE seq=?1", [seq])?;
                if removed != 1 {
                    return Err(StoreError::EventHistoryCorrupt);
                }
                let removed = tx
                    .inner
                    .execute("DELETE FROM event_sequence_ledger WHERE seq=?1", [seq])?;
                if removed != 1 {
                    return Err(StoreError::EventHistoryCorrupt);
                }
                let _ = content_bytes;
                deleted = deleted
                    .checked_add(1)
                    .ok_or(StoreError::EventStoreCapacity)?;
            }
            let prefix = compact_expired_prefix(tx)?;
            let retained: i64 =
                tx.inner
                    .query_row("SELECT count(*) FROM event_content", [], |row| row.get(0))?;
            let logical_bytes: i64 = tx.inner.query_row(
                "SELECT COALESCE((SELECT sum(content_bytes+8) FROM event_content),0)
                     + (SELECT count(*)*16 FROM event_expired_ranges)",
                [],
                |row| row.get(0),
            )?;
            let updated = tx.inner.execute(
                "UPDATE event_store_state SET retained_count=?1,event_store_bytes=?2
                 WHERE singleton=1 AND ?1 BETWEEN 0 AND 1000000
                   AND ?2 BETWEEN 0 AND 536870912",
                rusqlite::params![retained, logical_bytes],
            )?;
            if updated != 1 {
                return Err(StoreError::EventStoreCapacity);
            }
            Ok(EventRetentionReport {
                content_records_deleted: deleted,
                expired_prefix_through: Seq::new(prefix),
            })
        })
    }
}

fn merge_expired_range(tx: &mut Tx<'_>, seq: u64) -> Result<(), StoreError> {
    let seq_i64 = i64::try_from(seq).map_err(|_| StoreError::EventHistoryCorrupt)?;
    let mut first = seq_i64;
    let mut last = seq_i64;
    let previous_seq = seq_i64
        .checked_sub(1)
        .ok_or(StoreError::EventHistoryCorrupt)?;
    let prior: Option<(i64, i64)> = tx
        .inner
        .query_row(
            "SELECT first_seq,last_seq FROM event_expired_ranges WHERE last_seq=?1",
            [previous_seq],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let next: Option<(i64, i64)> = match seq_i64.checked_add(1) {
        Some(next_seq) => tx
            .inner
            .query_row(
                "SELECT first_seq,last_seq FROM event_expired_ranges WHERE first_seq=?1",
                [next_seq],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?,
        None => None,
    };
    if let Some((range_first, _range_last)) = prior {
        first = range_first;
        tx.inner.execute(
            "DELETE FROM event_expired_ranges WHERE first_seq=?1",
            [range_first],
        )?;
    }
    if let Some((range_first, range_last)) = next {
        last = range_last;
        tx.inner.execute(
            "DELETE FROM event_expired_ranges WHERE first_seq=?1",
            [range_first],
        )?;
    }
    tx.inner.execute(
        "INSERT INTO event_expired_ranges(first_seq,last_seq) VALUES (?1,?2)",
        rusqlite::params![first, last],
    )?;
    Ok(())
}

fn compact_expired_prefix(tx: &mut Tx<'_>) -> Result<u64, StoreError> {
    let (mut prefix, high): (i64, i64) = tx.inner.query_row(
        "SELECT expired_prefix_through,last_allocated_seq FROM event_store_state WHERE singleton=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;
    loop {
        if prefix >= high {
            break;
        }
        let first = prefix
            .checked_add(1)
            .ok_or(StoreError::EventHistoryCorrupt)?;
        let range: Option<(i64, i64)> = tx
            .inner
            .query_row(
                "SELECT first_seq,last_seq FROM event_expired_ranges WHERE first_seq=?1",
                [first],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((range_first, range_last)) = range else {
            break;
        };
        if range_last > high {
            return Err(StoreError::EventHistoryCorrupt);
        }
        let detailed: bool = tx.inner.query_row(
            "SELECT EXISTS(SELECT 1 FROM event_sequence_ledger WHERE seq BETWEEN ?1 AND ?2)
             OR EXISTS(SELECT 1 FROM event_content WHERE seq BETWEEN ?1 AND ?2)",
            rusqlite::params![range_first, range_last],
            |row| row.get(0),
        )?;
        if detailed {
            return Err(StoreError::EventHistoryCorrupt);
        }
        tx.inner.execute(
            "DELETE FROM event_expired_ranges WHERE first_seq=?1",
            [range_first],
        )?;
        tx.inner.execute(
            "UPDATE event_store_state SET expired_prefix_through=?1 WHERE singleton=1",
            [range_last],
        )?;
        prefix = range_last;
    }
    u64::try_from(prefix).map_err(|_| StoreError::EventHistoryCorrupt)
}

pub(crate) fn validate_event_history(conn: &Connection) -> Result<(), StoreError> {
    let (high, prefix, retained, stored_bytes): (i64, i64, i64, i64) = conn.query_row(
        "SELECT last_allocated_seq,expired_prefix_through,retained_count,event_store_bytes
         FROM event_store_state WHERE singleton=1",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    if prefix < 0 || high < prefix {
        return Err(StoreError::EventHistoryCorrupt);
    }
    let counts: (i64, i64, i64, i64) = conn.query_row(
        "SELECT
           (SELECT count(*) FROM event_sequence_ledger),
           (SELECT count(*) FROM event_content),
           (SELECT COALESCE(sum(content_bytes),0) FROM event_content),
           (SELECT count(*) FROM event_expired_ranges)",
        [],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
    )?;
    if counts.0 != counts.1 || counts.1 != retained || counts.2 < 0 {
        return Err(StoreError::EventHistoryCorrupt);
    }
    let expected_bytes = counts
        .2
        .checked_add(
            counts
                .0
                .checked_mul(8)
                .ok_or(StoreError::EventHistoryCorrupt)?,
        )
        .and_then(|value| value.checked_add(counts.3.checked_mul(16)?))
        .ok_or(StoreError::EventHistoryCorrupt)?;
    if stored_bytes != expected_bytes {
        return Err(StoreError::EventHistoryCorrupt);
    }
    let cross_mismatch: bool = conn.query_row(
        "SELECT EXISTS(
           SELECT 1 FROM event_content c LEFT JOIN event_sequence_ledger l USING(seq)
           WHERE l.seq IS NULL OR c.seq<=?1 OR c.seq>?2
         ) OR EXISTS(
           SELECT 1 FROM event_sequence_ledger l LEFT JOIN event_content c USING(seq)
           WHERE c.seq IS NULL OR l.seq<=?1 OR l.seq>?2
         ) OR EXISTS(
           SELECT 1 FROM event_expired_ranges r
           WHERE r.first_seq<=?1 OR r.last_seq>?2 OR r.first_seq>r.last_seq
         ) OR EXISTS(
           SELECT 1 FROM event_expired_ranges r JOIN event_sequence_ledger l
             ON l.seq BETWEEN r.first_seq AND r.last_seq
         ) OR EXISTS(
           SELECT 1 FROM event_expired_ranges left_range
           JOIN event_expired_ranges right_range
             ON left_range.last_seq < 9223372036854775807
            AND right_range.first_seq=left_range.last_seq+1
         )",
        rusqlite::params![prefix, high],
        |row| row.get(0),
    )?;
    if cross_mismatch {
        return Err(StoreError::EventHistoryCorrupt);
    }

    let mut last_covered = prefix;
    let mut coverage = conn.prepare(
        "SELECT first_seq,last_seq FROM (
           SELECT seq AS first_seq,seq AS last_seq FROM event_sequence_ledger
           UNION ALL
           SELECT first_seq,last_seq FROM event_expired_ranges
         ) ORDER BY first_seq,last_seq",
    )?;
    let rows = coverage.query_map([], |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)))?;
    for row in rows {
        let (first, last) = row?;
        if first
            != last_covered
                .checked_add(1)
                .ok_or(StoreError::EventHistoryCorrupt)?
            || last < first
            || last > high
        {
            return Err(StoreError::EventHistoryCorrupt);
        }
        last_covered = last;
    }
    if last_covered != high {
        return Err(StoreError::EventHistoryCorrupt);
    }

    let mut content = conn.prepare(
        "SELECT seq,message_id,kind,occurred_at_ms,data_class_rank,event_json,payload_bytes,content_bytes
         FROM event_content ORDER BY seq",
    )?;
    let rows = content.query_map([], |row| {
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
            row.get::<_, i64>(3)?,
            row.get::<_, i64>(4)?,
            row.get::<_, String>(5)?,
            row.get::<_, i64>(6)?,
            row.get::<_, i64>(7)?,
        ))
    })?;
    for row in rows {
        let (seq, message_id, kind, occurred, class_rank, json, payload_bytes, content_bytes) =
            row?;
        let event: SereaEvent =
            serde_json::from_str(&json).map_err(|_| StoreError::EventHistoryCorrupt)?;
        let canonical = canonicalize(&json).map_err(|_| StoreError::EventHistoryCorrupt)?;
        let payload_source =
            serde_json::to_string(&event.payload).map_err(|_| StoreError::EventHistoryCorrupt)?;
        let payload = canonicalize(&payload_source).map_err(|_| StoreError::EventHistoryCorrupt)?;
        if canonical != json.as_bytes()
            || event.seq
                != Seq::new(u64::try_from(seq).map_err(|_| StoreError::EventHistoryCorrupt)?)
            || event.message_id.as_str() != message_id
            || event.kind.wire_name() != kind
            || event.occurred_at.to_epoch_millis().get() != occurred
            || i64::from(event.data_class.rank()) != class_rank
            || i64::try_from(payload.len()).map_err(|_| StoreError::EventHistoryCorrupt)?
                != payload_bytes
            || i64::try_from(json.len()).map_err(|_| StoreError::EventHistoryCorrupt)?
                != content_bytes
            || event.surface.as_str() != WireSurface::EVENT
            || !event.envelope_version.is_supported()
        {
            return Err(StoreError::EventHistoryCorrupt);
        }
    }
    Ok(())
}
