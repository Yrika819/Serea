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

fn actor(store: &Store, id: &str, epoch: i64) -> P7ExperimentalMemory {
    store
        .transact(|tx| {
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

fn draft(id: &str, body: &str, class: MemoryClass, source_id: &str) -> MemoryDraft {
    MemoryDraft {
        memory_id: id.into(),
        class,
        kind: MemoryKind::Semantic,
        body: body.into(),
        provenance: MemoryProvenance {
            source_id: source_id.into(),
            source_task_id: Some(format!("task-{id}")),
            source_class: class,
            metadata_bytes: 12,
        },
    }
}

#[test]
fn retrieval_is_owner_and_class_scoped_and_preserves_provenance_under_utf8_budget() {
    let store = store();
    let owner_a = actor(&store, "own_00000000000000000000000000000071", 71);
    let owner_b = actor(&store, "own_00000000000000000000000000000072", 72);
    for (id, body) in [
        ("a-spaced", "今日は 東京 で会議です。"),
        ("b-spaced", "東京 は来週の会議です。"),
        ("c-personal", "東京 での個人的な予定です。"),
    ] {
        owner_a
            .insert(
                &store,
                &draft(
                    id,
                    body,
                    if id == "c-personal" {
                        MemoryClass::Personal
                    } else {
                        MemoryClass::Public
                    },
                    &format!("conversation-{id}"),
                ),
                EpochMillis::new(1000).unwrap(),
            )
            .unwrap();
    }
    owner_b
        .insert(
            &store,
            &draft(
                "a-spaced",
                "東京 は別Ownerの予定です。",
                MemoryClass::Public,
                "other-owner",
            ),
            EpochMillis::new(1000).unwrap(),
        )
        .unwrap();

    let all = owner_a
        .retrieve_context(
            &store,
            MemoryClass::Public,
            "東京",
            EpochMillis::new(1001).unwrap(),
            20,
            20,
            4096,
        )
        .unwrap();
    assert_eq!(
        all.entries
            .iter()
            .map(|entry| entry.record.memory_id.as_str())
            .collect::<Vec<_>>(),
        ["a-spaced", "b-spaced"]
    );
    assert_eq!(all.entries[0].record.body, "今日は 東京 で会議です。");
    assert_eq!(all.entries[0].provenance.source_id, "conversation-a-spaced");
    assert_eq!(
        all.entries[0].provenance.source_task_id.as_deref(),
        Some("task-a-spaced")
    );
    assert_eq!(all.entries[0].provenance.source_class, MemoryClass::Public);
    assert_eq!(
        all.entries[0].context_bytes,
        all.entries[0].record.body.len() + all.entries[0].provenance.source_id.len()
    );
    assert_eq!(
        all.used_context_bytes,
        all.entries.iter().map(|e| e.context_bytes).sum::<usize>()
    );

    let fit_one = owner_a
        .retrieve_context(
            &store,
            MemoryClass::Public,
            "東京",
            EpochMillis::new(1001).unwrap(),
            20,
            20,
            all.entries[0].context_bytes,
        )
        .unwrap();
    assert_eq!(fit_one.entries.len(), 1);
    assert_eq!(fit_one.used_context_bytes, fit_one.entries[0].context_bytes);
    assert!(fit_one.has_more);

    let personal = owner_a
        .retrieve_context(
            &store,
            MemoryClass::Personal,
            "東京",
            EpochMillis::new(1001).unwrap(),
            20,
            20,
            4096,
        )
        .unwrap();
    assert_eq!(personal.entries.len(), 1);
    assert_eq!(personal.entries[0].record.memory_id, "c-personal");
    let other_owner = owner_b
        .retrieve_context(
            &store,
            MemoryClass::Public,
            "東京",
            EpochMillis::new(1001).unwrap(),
            20,
            20,
            4096,
        )
        .unwrap();
    assert_eq!(other_owner.entries.len(), 1);
    assert_eq!(other_owner.entries[0].provenance.source_id, "other-owner");
}

#[test]
fn retrieval_excludes_expired_inactive_tombstoned_and_unprovenanced_rows() {
    let store = store();
    let owner = actor(&store, "own_00000000000000000000000000000073", 73);
    for id in ["live", "expired", "inactive", "forgotten"] {
        let mut memory = draft(
            id,
            "東京 の合成記憶",
            MemoryClass::Public,
            "synthetic-source",
        );
        if id == "expired" {
            memory.kind = MemoryKind::Working;
        }
        owner
            .insert(&store, &memory, EpochMillis::new(1000).unwrap())
            .unwrap();
    }
    store
        .transact(|tx| {
            tx.inner.execute(
                "INSERT INTO p7_memory_items(memory_id,owner_id,class_rank,kind,body,created_at_ms,expires_at_ms) VALUES('no-provenance',?1,0,'SEMANTIC','東京 の合成記憶',1000,31536001000)",
                ["own_00000000000000000000000000000073"],
            )?;
            tx.inner.execute(
                "INSERT INTO p7_memory_tombstones(owner_id,memory_id,forget_op_id,ledger_generation,forgotten_at_ms) VALUES(?1,'forgotten','forget-synthetic',1,1001)",
                ["own_00000000000000000000000000000073"],
            )?;
            Ok(())
        })
        .unwrap();
    owner
        .supersede(&store, "inactive", "live", EpochMillis::new(1001).unwrap())
        .unwrap();

    let result = owner
        .retrieve_context(
            &store,
            MemoryClass::Public,
            "東京",
            EpochMillis::new(604_801_001).unwrap(),
            20,
            20,
            4096,
        )
        .unwrap();
    assert_eq!(result.entries.len(), 1);
    assert_eq!(result.entries[0].record.memory_id, "live");

    store
        .transact(|tx| {
            tx.inner.execute(
                "UPDATE p7_ledger_state SET read_state='RECOVERY_REQUIRED' WHERE owner_id=?1",
                ["own_00000000000000000000000000000073"],
            )?;
            Ok(())
        })
        .unwrap();
    assert!(
        owner
            .retrieve_context(
                &store,
                MemoryClass::Public,
                "東京",
                EpochMillis::new(604_801_001).unwrap(),
                20,
                20,
                4096,
            )
            .is_err()
    );
}

#[test]
fn japanese_fts_evaluation_records_exact_partial_variation_and_unrelated_behavior() {
    let store = store();
    let owner = actor(&store, "own_00000000000000000000000000000074", 74);
    for (id, body) in [
        ("spaced", "今日は 東京 で 会議 です。"),
        ("punctuation", "東京、会議は明日です。"),
        ("exact-phrase", "東京 会議"),
        ("continuous", "今日は東京で会議です。"),
        ("reading-variant", "今日は とうきょう で会議です。"),
        ("unrelated", "横浜 で昼食を予約しました。"),
    ] {
        owner
            .insert(
                &store,
                &draft(id, body, MemoryClass::Public, "synthetic-japanese-eval"),
                EpochMillis::new(1000).unwrap(),
            )
            .unwrap();
    }
    let ids = |query: &str| {
        owner
            .retrieve_context(
                &store,
                MemoryClass::Public,
                query,
                EpochMillis::new(1001).unwrap(),
                20,
                20,
                4096,
            )
            .unwrap()
            .entries
            .into_iter()
            .map(|entry| entry.record.memory_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(ids("東京"), ["exact-phrase", "punctuation", "spaced"]);
    assert_eq!(ids("東京 会議"), ["exact-phrase"]);
    assert!(ids("とうきょう").contains(&"reading-variant".to_owned()));
    assert!(ids("会議").contains(&"spaced".to_owned()));
    assert!(!ids("横浜").contains(&"spaced".to_owned()));
    assert!(!ids("東京").contains(&"continuous".to_owned()));
    assert!(!ids("東京").contains(&"unrelated".to_owned()));
}

#[test]
fn retrieval_rank_dedup_and_input_limits_are_deterministic() {
    let store = store();
    let owner = actor(&store, "own_00000000000000000000000000000075", 75);
    let duplicate = MemoryRetrievalEntry {
        record: MemoryRecord {
            memory_id: "same".into(),
            class: MemoryClass::Public,
            kind: MemoryKind::Semantic,
            body: "東京 会議".into(),
            expires_at_ms: 10_000,
        },
        provenance: MemoryProvenance {
            source_id: "source".into(),
            source_task_id: None,
            source_class: MemoryClass::Public,
            metadata_bytes: 0,
        },
        lexical_score: 0,
        context_bytes: 0,
    };
    let ranked =
        rank_and_dedup_candidates(vec![duplicate.clone(), duplicate], "東京 会議").unwrap();
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].lexical_score, 102);
    assert_eq!(ranked[0].context_bytes, "東京 会議".len() + "source".len());
    let weaker = MemoryRetrievalEntry {
        record: MemoryRecord {
            memory_id: "a-weaker".into(),
            class: MemoryClass::Public,
            kind: MemoryKind::Semantic,
            body: "東京、会議".into(),
            expires_at_ms: 10_000,
        },
        provenance: MemoryProvenance {
            source_id: "source".into(),
            source_task_id: None,
            source_class: MemoryClass::Public,
            metadata_bytes: 0,
        },
        lexical_score: 0,
        context_bytes: 0,
    };
    let mut exact = ranked[0].clone();
    exact.record.memory_id = "z-exact".into();
    let reordered = rank_and_dedup_candidates(vec![weaker, exact], "東京 会議").unwrap();
    assert_eq!(reordered[0].record.memory_id, "z-exact");
    assert!(reordered[0].lexical_score > reordered[1].lexical_score);

    assert!(
        owner
            .retrieve_context(
                &store,
                MemoryClass::Public,
                "",
                EpochMillis::new(1001).unwrap(),
                1,
                1,
                10
            )
            .is_err()
    );
    assert!(
        owner
            .retrieve_context(
                &store,
                MemoryClass::Public,
                "x",
                EpochMillis::new(1001).unwrap(),
                0,
                1,
                10
            )
            .is_err()
    );
    assert!(
        owner
            .retrieve_context(
                &store,
                MemoryClass::Public,
                "x",
                EpochMillis::new(1001).unwrap(),
                1,
                0,
                10
            )
            .is_err()
    );
    assert!(
        owner
            .retrieve_context(
                &store,
                MemoryClass::Public,
                "x",
                EpochMillis::new(1001).unwrap(),
                1,
                1,
                0
            )
            .is_err()
    );
}
