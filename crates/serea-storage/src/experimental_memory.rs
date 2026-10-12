//! Disabled-by-default synthetic P7 Memory prototype. Feature builds are
//! development-only and optimized builds reject the feature.
use crate::{Store, StoreError, Tx};
use rusqlite::{OptionalExtension, params};
use serea_protocol::EpochMillis;

/// Explicit test-only state for every candidate policy value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProvisionalValue {
    TestOnly,
}
/// Unratified bounded test configuration; never auto-promoted to accepted policy.
#[derive(Debug, Clone, Copy)]
pub struct ExperimentalMemoryConfig {
    /// Always TestOnly.
    pub status: ProvisionalValue,
    /// Candidate active item cap.
    pub max_items: usize,
    /// Candidate total retained item-row cap; refusal does not evict expired or superseded rows.
    pub max_retained_items: usize,
    /// Candidate body bytes per item.
    pub max_body_bytes: usize,
    /// Candidate aggregate body bytes.
    pub max_body_bytes_per_owner: usize,
    /// Candidate indexed bytes.
    pub max_fts_bytes_per_owner: usize,
    /// Candidate extraction starts/day.
    pub extraction_starts_per_day: usize,
    /// Candidate concurrent extraction count.
    pub concurrent_extractions: usize,
    /// Candidate WORKING lifetime in days.
    pub retention_working_days: usize,
    /// Candidate EPISODIC lifetime in days.
    pub retention_episodic_days: usize,
    /// Candidate SEMANTIC lifetime in days.
    pub retention_semantic_days: usize,
    /// Candidate PREFERENCE lifetime in days.
    pub retention_preference_days: usize,
    /// Candidate explicit renewals per item.
    pub max_renewals: usize,
    /// Candidate maximum cumulative age in days.
    pub max_cumulative_age_days: usize,
    /// Candidate garbage collection batch size.
    pub gc_batch: usize,
    /// Candidate search input bytes.
    pub max_query_bytes: usize,
    /// Candidate results per page.
    pub max_page: usize,
    /// Candidate FTS rows per query.
    pub max_candidates: usize,
    /// Candidate approximate SQLite virtual machine instructions per query.
    pub max_query_vm_ops: usize,
    /// Candidate provenance record bytes per item.
    pub max_provenance_bytes: usize,
    /// Candidate tombstone bytes per item.
    pub max_tombstone_bytes: usize,
    /// Candidate deletion reserve in bytes.
    pub deletion_reserve_bytes: usize,
    /// Candidate independent ledger frame cap.
    pub ledger_frame_cap: usize,
    /// Candidate maximum supported backup age in days.
    pub max_backup_age_days: usize,
    /// Candidate minimum tombstone retention in days.
    pub tombstone_min_days: usize,
}
/// Public and Personal only; restricted classes intentionally have no variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryClass {
    Public,
    Personal,
}
impl MemoryClass {
    fn rank(self) -> i64 {
        if self == Self::Public { 0 } else { 1 }
    }
}
/// Provisional retention class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MemoryKind {
    Working,
    Episodic,
    Semantic,
    Preference,
}
impl MemoryKind {
    fn name(self) -> &'static str {
        match self {
            Self::Working => "WORKING",
            Self::Episodic => "EPISODIC",
            Self::Semantic => "SEMANTIC",
            Self::Preference => "PREFERENCE",
        }
    }
    fn lifetime(self) -> i64 {
        86_400_000
            * match self {
                Self::Working => 7,
                Self::Episodic => 90,
                Self::Semantic | Self::Preference => 365,
            }
    }
}
/// Immutable source association; source_task_id is deliberately not a live FK.
#[derive(Debug, Clone)]
pub struct MemoryProvenance {
    pub source_id: String,
    pub source_task_id: Option<String>,
    pub source_class: MemoryClass,
    pub metadata_bytes: usize,
}
/// Synthetic insertion request.
#[derive(Debug, Clone)]
pub struct MemoryDraft {
    pub memory_id: String,
    pub class: MemoryClass,
    pub kind: MemoryKind,
    pub body: String,
    pub provenance: MemoryProvenance,
}
/// Retrieved synthetic memory row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryRecord {
    pub memory_id: String,
    pub class: MemoryClass,
    pub kind: MemoryKind,
    pub body: String,
    pub expires_at_ms: i64,
}
/// Stable bounded result page.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryPage {
    pub records: Vec<MemoryRecord>,
    pub has_more: bool,
}
struct SearchRequest<'a> {
    class: MemoryClass,
    query: &'a str,
    now: EpochMillis,
    after: Option<&'a str>,
    limit: usize,
}
/// Sealed owner context; only a deterministic unit-test authenticator can create it.
#[derive(Debug, Clone)]
pub struct P7ExperimentalMemory {
    owner_id: String,
    _test_auth_context: u64,
}
/// Provisional values supplied for synthetic testing only.
pub const EXPERIMENTAL_PROVISIONAL: ExperimentalMemoryConfig = ExperimentalMemoryConfig {
    status: ProvisionalValue::TestOnly,
    max_items: 1000,
    max_retained_items: 1000,
    max_body_bytes: 4096,
    max_body_bytes_per_owner: 4 * 1024 * 1024,
    max_fts_bytes_per_owner: 8 * 1024 * 1024,
    extraction_starts_per_day: 20,
    concurrent_extractions: 1,
    retention_working_days: 7,
    retention_episodic_days: 90,
    retention_semantic_days: 365,
    retention_preference_days: 365,
    max_renewals: 2,
    max_cumulative_age_days: 730,
    gc_batch: 100,
    max_query_bytes: 256,
    max_page: 20,
    max_candidates: 1000,
    max_query_vm_ops: 10_000_000,
    max_provenance_bytes: 4096,
    max_tombstone_bytes: 1024,
    deletion_reserve_bytes: 64 * 1024 * 1024,
    ledger_frame_cap: 10000,
    max_backup_age_days: 30,
    tombstone_min_days: 37,
};

impl P7ExperimentalMemory {
    /// Exposes the explicitly provisional configuration for tests and evidence.
    pub const fn config() -> ExperimentalMemoryConfig {
        EXPERIMENTAL_PROVISIONAL
    }
    /// Exact owner-bound lookup with expiry and recovery barrier enforcement.
    pub fn get(
        &self,
        store: &Store,
        id: &str,
        now: EpochMillis,
    ) -> Result<Option<MemoryRecord>, StoreError> {
        store.transact(|tx| { ensure_ready(tx,&self.owner_id)?; tx.inner.query_row(
            "SELECT memory_id,class_rank,kind,body,expires_at_ms FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2 AND active=1 AND expires_at_ms>?3",
            params![self.owner_id,id,now.get()],record_from_row).optional().map_err(StoreError::from) })
    }
    /// Transactional synthetic insert; SQLite triggers maintain quota and FTS.
    pub fn insert(
        &self,
        store: &Store,
        draft: &MemoryDraft,
        now: EpochMillis,
    ) -> Result<(), StoreError> {
        if draft.body.is_empty()
            || draft.body.len() > 4096
            || draft.provenance.metadata_bytes > 4096
            || draft.provenance.source_id.is_empty()
            || draft.provenance.source_id.len() > 1024
            || draft
                .provenance
                .metadata_bytes
                .checked_add(draft.provenance.source_id.len())
                .and_then(|n| {
                    n.checked_add(
                        draft
                            .provenance
                            .source_task_id
                            .as_ref()
                            .map_or(0, String::len),
                    )
                })
                .is_none_or(|n| n > 4096)
            || draft.provenance.source_class.rank() < draft.class.rank()
        {
            return Err(StoreError::Sqlite);
        }
        let expires = now
            .get()
            .checked_add(draft.kind.lifetime())
            .ok_or(StoreError::Sqlite)?;
        store.transact(|tx| { ensure_ready(tx,&self.owner_id)?;
            tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![draft.memory_id,self.owner_id,draft.class.rank(),draft.kind.name(),draft.body,now.get(),expires])?;
            tx.inner.execute("INSERT INTO p7_memory_provenance(memory_id,owner_id,source_id,source_task_id,source_class_rank,metadata_bytes,recorded_at_ms) VALUES(?1,?2,?3,?4,?5,?6,?7)",params![draft.memory_id,self.owner_id,draft.provenance.source_id,draft.provenance.source_task_id,draft.provenance.source_class.rank(),draft.provenance.metadata_bytes as i64,now.get()])?;
            Ok(()) })
    }
    /// Owner/class-filtered lexical retrieval with literal quoted FTS input and stable order.
    pub fn search(
        &self,
        store: &Store,
        class: MemoryClass,
        query: &str,
        now: EpochMillis,
        after: Option<&str>,
        limit: usize,
    ) -> Result<MemoryPage, StoreError> {
        if query.is_empty()
            || query.len() > 256
            || limit == 0
            || limit > 20
            || after.is_some_and(|cursor| cursor.len() > 128)
        {
            return Err(StoreError::Sqlite);
        }
        self.search_with_vm_budget(
            store,
            SearchRequest {
                class,
                query,
                now,
                after,
                limit,
            },
            EXPERIMENTAL_PROVISIONAL.max_query_vm_ops,
        )
    }

    fn search_with_vm_budget(
        &self,
        store: &Store,
        request: SearchRequest<'_>,
        vm_budget: usize,
    ) -> Result<MemoryPage, StoreError> {
        let literal = format!("\"{}\"", request.query.replace('"', "\"\""));
        store.transact(|tx| {
            let check_interval = vm_budget.clamp(1, 100);
            let intervals = vm_budget.div_ceil(check_interval);
            let mut remaining = intervals;
            tx.inner
                .progress_handler(check_interval as i32, Some(move || {
                    if remaining == 0 {
                        true
                    } else {
                        remaining -= 1;
                        false
                    }
                }))
                .map_err(StoreError::from)?;
            let result = (|| {
                ensure_ready(tx, &self.owner_id)?;
                tx.inner.execute("INSERT INTO p7_memory_fts(p7_memory_fts,rank) VALUES('integrity-check',1)",[])?;
                let mut s=tx.inner.prepare("SELECT m.memory_id,m.class_rank,m.kind,m.body,m.expires_at_ms FROM p7_memory_fts f JOIN p7_memory_items m ON m.memory_id=f.memory_id WHERE p7_memory_fts MATCH ?1 AND f.owner_id=?2 AND f.class_rank=?3 AND m.owner_id=?2 AND m.class_rank=?3 AND m.active=1 AND m.expires_at_ms>?4 AND (?5 IS NULL OR m.memory_id>?5) ORDER BY m.memory_id COLLATE BINARY LIMIT ?6")?;
                let rows=s.query_map(params![literal,self.owner_id,request.class.rank(),request.now.get(),request.after,(1001_i64)],record_from_row)?;
                let mut records = rows.collect::<Result<Vec<_>, _>>()?;
                let has_more = records.len() > request.limit;
                records.truncate(request.limit);
                Ok(MemoryPage { records, has_more })
            })();
            let reset = tx
                .inner
                .progress_handler(0, None::<fn() -> bool>)
                .map_err(StoreError::from);
            reset?;
            result
        })
    }
    /// Creates one owner-local successor edge; the former row remains retained but inactive.
    pub fn supersede(
        &self,
        store: &Store,
        prior_id: &str,
        successor_id: &str,
        now: EpochMillis,
    ) -> Result<(), StoreError> {
        if prior_id == successor_id {
            return Err(StoreError::Sqlite);
        }
        store.transact(|tx| {
            ensure_ready(tx, &self.owner_id)?;
            let prior: Option<i64> = tx.inner.query_row(
                "SELECT class_rank FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2 AND active=1 AND expires_at_ms>?3",
                params![self.owner_id, prior_id, now.get()], |r| r.get(0),
            ).optional()?;
            let successor: Option<i64> = tx.inner.query_row(
                "SELECT class_rank FROM p7_memory_items WHERE owner_id=?1 AND memory_id=?2 AND active=1 AND expires_at_ms>?3",
                params![self.owner_id, successor_id, now.get()], |r| r.get(0),
            ).optional()?;
            if prior.is_none() || successor.is_none() {
                return Err(StoreError::Sqlite);
            }
            tx.inner.execute(
                "INSERT INTO p7_supersession_edges(owner_id,prior_memory_id,successor_memory_id,created_at_ms) VALUES(?1,?2,?3,?4)",
                params![self.owner_id, prior_id, successor_id, now.get()],
            )?;
            tx.inner.execute(
                "UPDATE p7_memory_items SET active=0 WHERE owner_id=?1 AND memory_id=?2",
                params![self.owner_id, prior_id],
            )?;
            Ok(())
        })
    }
    /// Bounded stable list, with authoritative owner/class/expiry filters.
    pub fn list(
        &self,
        store: &Store,
        class: MemoryClass,
        now: EpochMillis,
        after: Option<&str>,
        limit: usize,
    ) -> Result<MemoryPage, StoreError> {
        if limit == 0 || limit > 20 || after.is_some_and(|cursor| cursor.len() > 128) {
            return Err(StoreError::Sqlite);
        }
        store.transact(|tx| { ensure_ready(tx,&self.owner_id)?;
            let mut s=tx.inner.prepare("SELECT memory_id,class_rank,kind,body,expires_at_ms FROM p7_memory_items WHERE owner_id=?1 AND class_rank=?2 AND active=1 AND expires_at_ms>?3 AND (?4 IS NULL OR memory_id>?4) ORDER BY memory_id COLLATE BINARY LIMIT ?5")?;
            let rows=s.query_map(params![self.owner_id,class.rank(),now.get(),after,(limit+1) as i64],record_from_row)?;
            let mut records=rows.collect::<Result<Vec<_>,_>>()?; let has_more=records.len()>limit; records.truncate(limit); Ok(MemoryPage{records,has_more})
        })
    }
}
fn ensure_ready(tx: &mut Tx<'_>, owner: &str) -> Result<(), StoreError> {
    let state:Option<(String,String)>=tx.inner.query_row("SELECT o.state,l.read_state FROM p7_owners o JOIN p7_ledger_state l USING(owner_id) WHERE owner_id=?1",[owner],|r|Ok((r.get(0)?,r.get(1)?))).optional()?;
    if state
        .as_ref()
        .is_some_and(|(a, b)| a == "ACTIVE" && b == "READY")
    {
        Ok(())
    } else {
        Err(StoreError::Sqlite)
    }
}
fn record_from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<MemoryRecord> {
    let rank: i64 = r.get(1)?;
    let kind: String = r.get(2)?;
    Ok(MemoryRecord {
        memory_id: r.get(0)?,
        class: if rank == 0 {
            MemoryClass::Public
        } else {
            MemoryClass::Personal
        },
        kind: match kind.as_str() {
            "WORKING" => MemoryKind::Working,
            "EPISODIC" => MemoryKind::Episodic,
            "SEMANTIC" => MemoryKind::Semantic,
            _ => MemoryKind::Preference,
        },
        body: r.get(3)?,
        expires_at_ms: r.get(4)?,
    })
}
#[cfg(test)]
impl P7ExperimentalMemory {
    pub(crate) fn fake(owner: String, nonce: u64) -> Self {
        Self {
            owner_id: owner,
            _test_auth_context: nonce,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use serea_protocol::{Clock, ProtocolError};
    struct Fixed;
    impl Clock for Fixed {
        fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
            EpochMillis::new(1000)
        }
    }
    fn store() -> Store {
        Store::open_in_memory(&Fixed).unwrap()
    }
    fn actor(s: &Store, id: &str, epoch: i64) -> P7ExperimentalMemory {
        s.transact(|tx| {
            tx.inner.execute(
                "INSERT INTO p7_owners VALUES(?1,?2,'ACTIVE',0)",
                params![id, epoch],
            )?;
            tx.inner.execute(
                "INSERT INTO p7_resource_accounting(owner_id) VALUES(?1)",
                [id],
            )?;
            tx.inner.execute(
                "INSERT INTO p7_ledger_state(owner_id,read_state) VALUES(?1,'READY')",
                [id],
            )?;
            Ok(())
        })
        .unwrap();
        P7ExperimentalMemory::fake(id.into(), epoch as u64)
    }
    fn draft(id: &str, body: &str, class: MemoryClass) -> MemoryDraft {
        MemoryDraft {
            memory_id: id.into(),
            class,
            kind: MemoryKind::Semantic,
            body: body.into(),
            provenance: MemoryProvenance {
                source_id: "synthetic-source".into(),
                source_task_id: Some("detached-task-label".into()),
                source_class: class,
                metadata_bytes: 16,
            },
        }
    }
    #[test]
    fn fts_japanese_isolation_class_expiry_and_literal_bounds() {
        let s = store();
        let a = actor(&s, "own_00000000000000000000000000000001", 1);
        let b = actor(&s, "own_00000000000000000000000000000002", 2);
        a.insert(
            &s,
            &draft("a1", "今日は 東京 で開発", MemoryClass::Personal),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        b.insert(
            &s,
            &draft("b1", "今日は 東京 で開発", MemoryClass::Personal),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        assert_eq!(
            a.search(
                &s,
                MemoryClass::Personal,
                "東京",
                EpochMillis::new(1001).unwrap(),
                None,
                20
            )
            .unwrap()
            .records
            .iter()
            .map(|r| r.memory_id.as_str())
            .collect::<Vec<_>>(),
            ["a1"]
        );
        assert!(
            a.get(&s, "b1", EpochMillis::new(1001).unwrap())
                .unwrap()
                .is_none()
        );
        assert!(
            a.search(
                &s,
                MemoryClass::Public,
                "東京",
                EpochMillis::new(1001).unwrap(),
                None,
                20
            )
            .unwrap()
            .records
            .is_empty()
        );
        assert!(
            a.search(
                &s,
                MemoryClass::Personal,
                "",
                EpochMillis::new(1001).unwrap(),
                None,
                20
            )
            .is_err()
        );
        assert!(
            a.search(
                &s,
                MemoryClass::Personal,
                &"x".repeat(257),
                EpochMillis::new(1001).unwrap(),
                None,
                20
            )
            .is_err()
        );
        assert!(
            a.get(&s, "a1", EpochMillis::new(365 * 86_400_000 + 1001).unwrap())
                .unwrap()
                .is_none()
        );
        assert_eq!(
            P7ExperimentalMemory::config().status,
            ProvisionalValue::TestOnly
        );
        assert_eq!(P7ExperimentalMemory::config().max_query_vm_ops, 10_000_000);
    }

    #[test]
    fn fts_search_stops_at_its_sqlite_vm_instruction_budget_and_clears_the_handler() {
        let s = store();
        let a = actor(&s, "own_00000000000000000000000000000024", 24);
        a.insert(
            &s,
            &draft("bounded", "synthetic lexical content", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        assert!(
            a.search_with_vm_budget(
                &s,
                SearchRequest {
                    class: MemoryClass::Public,
                    query: "synthetic",
                    now: EpochMillis::new(1001).unwrap(),
                    after: None,
                    limit: 20,
                },
                1,
            )
            .is_err()
        );
        assert!(
            a.search(
                &s,
                MemoryClass::Public,
                "synthetic",
                EpochMillis::new(1001).unwrap(),
                None,
                20,
            )
            .is_ok()
        );
    }
    #[test]
    fn fts_unicode61_handles_metacharacters_spaced_and_mixed_text() {
        let s = store();
        let a = actor(&s, "own_00000000000000000000000000000003", 3);
        for (id, body) in [
            ("m1", "日本語の連続テキスト"),
            ("m2", "日本 語 text"),
            ("m3", "Serea 日本語 2026!"),
            ("m4", "quote \" OR * NEAR(foo)"),
        ] {
            a.insert(
                &s,
                &draft(id, body, MemoryClass::Public),
                EpochMillis::new(1000).unwrap(),
            )
            .unwrap();
        }
        let q = |x: &str| {
            a.search(
                &s,
                MemoryClass::Public,
                x,
                EpochMillis::new(1001).unwrap(),
                None,
                20,
            )
            .unwrap()
        };
        assert!(!q("日本語").records.iter().any(|r| r.memory_id == "m1"));
        assert!(
            q("日本語の連続テキスト")
                .records
                .iter()
                .any(|r| r.memory_id == "m1")
        );
        assert!(q("日本").records.iter().any(|r| r.memory_id == "m2"));
        assert!(q("日本語").records.iter().any(|r| r.memory_id == "m3"));
        assert!(q("2026!").records.iter().any(|r| r.memory_id == "m3"));
        assert!(
            q("\" OR * NEAR(foo)")
                .records
                .iter()
                .any(|r| r.memory_id == "m4")
        );
    }
    #[test]
    fn listing_and_fts_pages_are_stable_and_respect_page_and_candidate_caps() {
        let s = store();
        let owner = "own_00000000000000000000000000000027";
        let a = actor(&s, owner, 27);
        s.transact(|tx| {
            for i in 0..1000 {
                let id = format!("page-{i:04}");
                tx.inner.execute(
                    "INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES(?1,?2,0,'WORKING','common lexical',1,999999999)",
                    params![id, owner],
                )?;
            }
            Ok(())
        })
        .unwrap();
        assert_eq!(P7ExperimentalMemory::config().max_candidates, 1000);
        let first = a
            .search(
                &s,
                MemoryClass::Public,
                "common",
                EpochMillis::new(1000).unwrap(),
                None,
                20,
            )
            .unwrap();
        assert_eq!(first.records.len(), 20);
        assert!(first.has_more);
        assert_eq!(first.records[0].memory_id, "page-0000");
        assert_eq!(first.records[19].memory_id, "page-0019");
        let second = a
            .search(
                &s,
                MemoryClass::Public,
                "common",
                EpochMillis::new(1000).unwrap(),
                Some("page-0019"),
                20,
            )
            .unwrap();
        assert_eq!(second.records[0].memory_id, "page-0020");
        assert_eq!(second.records[19].memory_id, "page-0039");
        assert!(second.has_more);
        let final_page = a
            .search(
                &s,
                MemoryClass::Public,
                "common",
                EpochMillis::new(1000).unwrap(),
                Some("page-0979"),
                20,
            )
            .unwrap();
        assert_eq!(final_page.records[0].memory_id, "page-0980");
        assert_eq!(final_page.records[19].memory_id, "page-0999");
        assert!(!final_page.has_more);
        let list = a
            .list(
                &s,
                MemoryClass::Public,
                EpochMillis::new(1000).unwrap(),
                Some("page-0997"),
                20,
            )
            .unwrap();
        assert_eq!(
            list.records
                .iter()
                .map(|r| r.memory_id.as_str())
                .collect::<Vec<_>>(),
            ["page-0998", "page-0999"]
        );
        assert!(!list.has_more);
    }

    #[test]
    fn supersession_is_owner_local_single_successor_and_removes_prior_from_search() {
        let s = store();
        let a = actor(&s, "own_00000000000000000000000000000007", 7);
        a.insert(
            &s,
            &draft("old", "old fact", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        a.insert(
            &s,
            &draft("new", "new fact", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        a.insert(
            &s,
            &draft("other", "other successor", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        a.supersede(&s, "old", "new", EpochMillis::new(1001).unwrap())
            .unwrap();
        assert!(
            a.get(&s, "old", EpochMillis::new(1002).unwrap())
                .unwrap()
                .is_none()
        );
        assert!(
            a.search(
                &s,
                MemoryClass::Public,
                "old",
                EpochMillis::new(1002).unwrap(),
                None,
                20
            )
            .unwrap()
            .records
            .is_empty()
        );
        assert!(
            a.supersede(&s, "old", "new", EpochMillis::new(1002).unwrap())
                .is_err()
        );
        assert!(s.transact(|tx|{tx.inner.execute("INSERT INTO p7_supersession_edges VALUES('own_00000000000000000000000000000007','old','other',1002)",[])?;Ok(())}).is_err());
        let accounting:(i64,i64)=s.transact(|tx|Ok(tx.inner.query_row("SELECT active_items,active_body_bytes FROM p7_resource_accounting WHERE owner_id=?1",["own_00000000000000000000000000000007"],|r|Ok((r.get(0)?,r.get(1)?)))?)).unwrap();
        assert_eq!(accounting.0, 2);
    }

    #[test]
    fn p7_synthetic_memory_and_feature_schema_reopen_cleanly() {
        let path = std::env::temp_dir().join(format!("serea-p7-reopen-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let store = Store::open(&path, &Fixed).unwrap();
        let owner = "own_00000000000000000000000000000012";
        let auth = actor(&store, owner, 12);
        auth.insert(
            &store,
            &draft("persisted", "synthetic reopen", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        drop(store);
        let reopened = Store::open(&path, &Fixed).unwrap();
        let auth = P7ExperimentalMemory::fake(owner.into(), 13);
        assert_eq!(
            auth.get(&reopened, "persisted", EpochMillis::new(1001).unwrap())
                .unwrap()
                .unwrap()
                .body,
            "synthetic reopen"
        );
        drop(reopened);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn provenance_trigger_failure_rolls_back_memory_quota_and_fts_atomically() {
        let s = store();
        let a = actor(&s, "own_00000000000000000000000000000011", 11);
        s.conn.lock().unwrap().execute_batch("CREATE TRIGGER p7_test_fail_provenance BEFORE INSERT ON p7_memory_provenance WHEN NEW.memory_id='fault' BEGIN SELECT RAISE(ABORT,'synthetic fail'); END;").unwrap();
        assert!(
            a.insert(
                &s,
                &draft("fault", "rollback token", MemoryClass::Public),
                EpochMillis::new(1000).unwrap()
            )
            .is_err()
        );
        let (items,bytes,fts):(i64,i64,i64)=s.transact(|tx|Ok(tx.inner.query_row("SELECT active_items,active_body_bytes,logical_fts_bytes FROM p7_resource_accounting WHERE owner_id=?1",["own_00000000000000000000000000000011"],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?)).unwrap();
        assert_eq!((items, bytes, fts), (0, 0, 0));
        let absent: i64 = s
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT count(*) FROM p7_memory_items WHERE memory_id='fault'",
                    [],
                    |r| r.get(0),
                )?)
            })
            .unwrap();
        assert_eq!(absent, 0);
        assert!(
            a.search(
                &s,
                MemoryClass::Public,
                "rollback",
                EpochMillis::new(1001).unwrap(),
                None,
                20
            )
            .unwrap()
            .records
            .is_empty()
        );
    }

    #[test]
    fn missing_authoritative_ledger_row_blocks_lookup_list_and_search() {
        let s = store();
        let a = actor(&s, "own_00000000000000000000000000000013", 13);
        a.insert(
            &s,
            &draft("m", "synthetic", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        s.conn
            .lock()
            .unwrap()
            .execute(
                "DELETE FROM p7_ledger_state WHERE owner_id=?1",
                ["own_00000000000000000000000000000013"],
            )
            .unwrap();
        let now = EpochMillis::new(1001).unwrap();
        assert!(a.get(&s, "m", now).is_err());
        assert!(a.list(&s, MemoryClass::Public, now, None, 20).is_err());
        assert!(
            a.search(&s, MemoryClass::Public, "synthetic", now, None, 20)
                .is_err()
        );
    }

    #[test]
    fn fts_content_mismatch_fails_closed_instead_of_returning_partial_search() {
        let s = store();
        let a = actor(&s, "own_00000000000000000000000000000008", 8);
        a.insert(
            &s,
            &draft("broken", "synthetic token", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        s.conn.lock().unwrap().execute(
            "INSERT INTO p7_memory_fts(p7_memory_fts,rowid,memory_id,owner_id,class_rank,body) SELECT 'delete',rowid,memory_id,owner_id,class_rank,body FROM p7_memory_items WHERE memory_id='broken'", [],
        ).unwrap();
        assert!(
            a.search(
                &s,
                MemoryClass::Public,
                "synthetic",
                EpochMillis::new(1001).unwrap(),
                None,
                20
            )
            .is_err()
        );
    }

    #[test]
    fn quota_trigger_rejects_exact_overflow_at_item_and_byte_boundaries() {
        let s = store();
        let a = actor(&s, "own_00000000000000000000000000000009", 9);
        s.transact(|tx| { tx.inner.execute("UPDATE p7_resource_accounting SET active_items=999,active_body_bytes=4194300,logical_fts_bytes=8388604 WHERE owner_id=?1", ["own_00000000000000000000000000000009"])?; Ok(()) }).unwrap();
        a.insert(
            &s,
            &draft("edge", "1234", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        assert!(
            a.insert(
                &s,
                &draft("overflow", "x", MemoryClass::Public),
                EpochMillis::new(1000).unwrap()
            )
            .is_err()
        );
        let (items,body,fts):(i64,i64,i64)=s.transact(|tx|Ok(tx.inner.query_row("SELECT active_items,active_body_bytes,logical_fts_bytes FROM p7_resource_accounting WHERE owner_id=?1",["own_00000000000000000000000000000009"],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?)))?)).unwrap();
        assert_eq!((items, body, fts), (1000, 4194304, 8388608));
    }

    #[test]
    fn retained_item_cap_refuses_instead_of_evicting_superseded_content() {
        let s = store();
        let a = actor(&s, "own_00000000000000000000000000000026", 26);
        a.insert(
            &s,
            &draft("retained", "must stay", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        s.transact(|tx| {
            tx.inner.execute(
                "UPDATE p7_resource_accounting SET retained_items=1000 WHERE owner_id=?1",
                ["own_00000000000000000000000000000026"],
            )?;
            Ok(())
        })
        .unwrap();
        assert!(
            a.insert(
                &s,
                &draft("refused", "no eviction", MemoryClass::Public),
                EpochMillis::new(1000).unwrap(),
            )
            .is_err()
        );
        assert_eq!(
            a.get(&s, "retained", EpochMillis::new(1001).unwrap())
                .unwrap()
                .unwrap()
                .body,
            "must stay"
        );
        assert_eq!(P7ExperimentalMemory::config().max_retained_items, 1000);
    }

    #[test]
    fn two_file_connections_serialize_duplicate_admission_and_quota() {
        use std::sync::{Arc, Barrier};
        use std::thread;
        let path = std::env::temp_dir().join(format!("serea-p7-admit-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let first = Store::open(&path, &Fixed).unwrap();
        let owner = "own_00000000000000000000000000000010";
        let a = actor(&first, owner, 10);
        first
            .transact(|tx| {
                tx.inner.execute(
                    "UPDATE p7_resource_accounting SET active_items=999 WHERE owner_id=?1",
                    [owner],
                )?;
                Ok(())
            })
            .unwrap();
        let second = Store::open(&path, &Fixed).unwrap();
        let b = P7ExperimentalMemory::fake(owner.into(), 11);
        let gate = Arc::new(Barrier::new(3));
        let handles = [(first, a), (second, b)]
            .into_iter()
            .map(|(store, auth)| {
                let gate = gate.clone();
                thread::spawn(move || {
                    gate.wait();
                    auth.insert(
                        &store,
                        &draft("race", "one", MemoryClass::Personal),
                        EpochMillis::new(1000).unwrap(),
                    )
                })
            })
            .collect::<Vec<_>>();
        gate.wait();
        let outcomes = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(outcomes.iter().filter(|r| r.is_ok()).count(), 1);
        drop(outcomes);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn two_file_connections_cannot_commit_opposite_supersession_edges() {
        use std::sync::{Arc, Barrier};
        use std::thread;

        let path = std::env::temp_dir().join(format!("serea-p7-cycle-{}.db", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let first = Store::open(&path, &Fixed).unwrap();
        let owner = "own_00000000000000000000000000000025";
        let a = actor(&first, owner, 25);
        for id in ["left", "right"] {
            a.insert(
                &first,
                &draft(id, "synthetic graph node", MemoryClass::Public),
                EpochMillis::new(1000).unwrap(),
            )
            .unwrap();
        }
        let second = Store::open(&path, &Fixed).unwrap();
        let b = P7ExperimentalMemory::fake(owner.into(), 26);
        let gate = Arc::new(Barrier::new(3));
        let handles = [(first, a, "left", "right"), (second, b, "right", "left")]
            .into_iter()
            .map(|(store, auth, prior, successor)| {
                let gate = gate.clone();
                thread::spawn(move || {
                    gate.wait();
                    auth.supersede(&store, prior, successor, EpochMillis::new(1001).unwrap())
                })
            })
            .collect::<Vec<_>>();
        gate.wait();
        let outcomes = handles
            .into_iter()
            .map(|h| h.join().unwrap())
            .collect::<Vec<_>>();
        let successes = outcomes.iter().filter(|result| result.is_ok()).count();
        assert_eq!(successes, 1);
        let verify = Store::open(&path, &Fixed).unwrap();
        let (edges, active): (i64, i64) = verify
            .transact(|tx| {
                Ok(tx.inner.query_row(
                    "SELECT (SELECT count(*) FROM p7_supersession_edges WHERE owner_id=?1),(SELECT active_items FROM p7_resource_accounting WHERE owner_id=?1)",
                    [owner],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )?)
            })
            .unwrap();
        assert_eq!((edges, active), (1, 1));
        drop(verify);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn private_cross_owner_cycle_and_transaction_rollback_are_rejected() {
        let s = store();
        let a = actor(&s, "own_00000000000000000000000000000004", 4);
        let _b = actor(&s, "own_00000000000000000000000000000005", 5);
        a.insert(
            &s,
            &draft("a", "alpha", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        a.insert(
            &s,
            &draft("b", "beta", MemoryClass::Personal),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        a.insert(
            &s,
            &draft("public-successor", "lower class", MemoryClass::Public),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();
        assert!(
            a.supersede(&s, "b", "public-successor", EpochMillis::new(1001).unwrap(),)
                .is_err()
        );
        assert!(s
            .transact(|tx| {
                tx.inner.execute(
                    "INSERT INTO p7_supersession_edges(owner_id,prior_memory_id,successor_memory_id,created_at_ms) VALUES(?1,'b','public-successor',1)",
                    ["own_00000000000000000000000000000004"],
                )?;
                Ok(())
            })
            .is_err());
        assert!(s.transact(|tx|{tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES('p','own_00000000000000000000000000000004',2,'SEMANTIC','bad',1,2)",[])?;Ok(())}).is_err());
        for rank in [3_i64, 4_i64] {
            assert!(s.transact(|tx|{tx.inner.execute("INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES(?1,'own_00000000000000000000000000000004',?2,'SEMANTIC','bad',1,2)",params![format!("restricted-{rank}"),rank])?;Ok(())}).is_err());
        }
        assert!(s.transact(|tx|{tx.inner.execute("INSERT INTO p7_supersession_edges VALUES('own_00000000000000000000000000000004','a','a',1)",[])?;Ok(())}).is_err());
        assert!(s.transact(|tx|{tx.inner.execute("INSERT INTO p7_supersession_edges VALUES('own_00000000000000000000000000000004','a','b',1)",[])?;tx.inner.execute("INSERT INTO p7_supersession_edges VALUES('own_00000000000000000000000000000004','b','a',1)",[])?;Ok(())}).is_err());
        assert!(s.transact(|tx|{tx.inner.execute("INSERT INTO p7_supersession_edges VALUES('own_00000000000000000000000000000005','a','b',1)",[])?;Ok(())}).is_err());
        let n:i64=s.transact(|tx|Ok(tx.inner.query_row("SELECT count(*) FROM p7_memory_provenance WHERE source_task_id='detached-task-label'",[],|r|r.get(0))?)).unwrap();
        assert_eq!(n, 3);
    }
}
