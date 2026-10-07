//! P4B RED-first durable attempt and usage transaction tests.

use crate::{
    ModelAttemptRelationKind, ModelAttemptState, ModelCallAttemptDraft, ModelCallCompletion,
    ModelDeploymentClass, ModelFailureUsage, ModelPriceSnapshot, ModelResponseStorage, Store,
    StoreError, UsdMicros, UtcAccountingDay,
};
use serea_protocol::{
    Clock, CostClass, DataClass, EpochMillis, FinishReason, ModelId, ModelPurpose, ProtocolError,
    ProviderId, RequestId, TaskId, TokenCount,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Barrier};

static NEXT: AtomicU64 = AtomicU64::new(0);

struct FixedClock;
impl Clock for FixedClock {
    fn now_ms(&self) -> Result<EpochMillis, ProtocolError> {
        EpochMillis::new(1_767_225_600_000)
    }
}

struct TempDb(PathBuf);
impl TempDb {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "serea-p4b-accounting-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir.join("store.sqlite"))
    }
}
impl Drop for TempDb {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn id(value: u8) -> RequestId {
    RequestId::new(format!("req_000000000000000000000000{value:02}")).unwrap()
}

fn task_id() -> TaskId {
    TaskId::new("tsk_00000000000000000000000001").unwrap()
}

fn price(input: u64, output: u64) -> ModelPriceSnapshot {
    ModelPriceSnapshot::new(CostClass::Paid, "price-2026-10", input, output)
}

fn draft(
    request_id: RequestId,
    task_id: Option<TaskId>,
    price: ModelPriceSnapshot,
) -> ModelCallAttemptDraft {
    ModelCallAttemptDraft {
        request_id,
        task_id,
        purpose: ModelPurpose::Chat,
        model_id: ModelId::new("nemotron-3-nano-30b").unwrap(),
        provider_id: ProviderId::new("ollama").unwrap(),
        deployment_class: ModelDeploymentClass::Local,
        data_class: DataClass::Public,
        relation_kind: ModelAttemptRelationKind::None,
        parent_request_id: None,
        fallback_from_model_id: None,
        price,
        max_context_tokens: 4_096,
        effective_max_output_tokens: 1_024,
        dispatch_intent_at: EpochMillis::new(1_767_225_600_000).unwrap(),
    }
}

fn insert_task(store: &Store) {
    let conn = store.conn.lock().unwrap();
    conn.execute(
        "INSERT INTO tasks(task_id,kind,title,state,origin_kind,data_class_rank,policy_class_rank,created_at_ms,updated_at_ms,max_model_calls,max_tool_calls,max_attempts_per_step)
         VALUES (?1,'MAINTENANCE','fixture','EXECUTING','SYSTEM',0,0,0,0,12,0,0)",
        [task_id().as_str()],
    )
    .unwrap();
}

fn delete_task_with_blob_sweep(store: &Store) {
    store
        .transact(|tx| {
            let candidates = crate::task::task_blob_candidates(&tx.inner, &task_id())?;
            tx.inner
                .execute("DELETE FROM tasks WHERE task_id=?1", [task_id().as_str()])?;
            crate::task::sweep_blob_candidates(&tx.inner, &candidates)?;
            Ok(())
        })
        .unwrap();
}

#[test]
fn utc_accounting_day_uses_epoch_utc_boundaries() {
    let before = UtcAccountingDay::from_epoch_millis(EpochMillis::new(86_399_999).unwrap());
    let after = UtcAccountingDay::from_epoch_millis(EpochMillis::new(86_400_000).unwrap());
    let epoch = UtcAccountingDay::from_epoch_millis(EpochMillis::new(0).unwrap());
    assert_eq!(before.get() + 1, after.get());
    assert_eq!(epoch.get(), 0);
}

#[test]
fn reservation_is_atomic_with_call_budget_spend_and_active_task_gate() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    insert_task(&store);
    let cap = UsdMicros::new(10_000_000).unwrap();
    let attempt = store
        .reserve_model_call(
            draft(id(1), Some(task_id()), price(1_000_000, 1_000_000)),
            cap,
        )
        .unwrap();
    assert_eq!(attempt.state, ModelAttemptState::DispatchIntent);
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 1);
    assert_eq!(
        store.reserve_model_call(draft(id(2), Some(task_id()), price(1, 1)), cap),
        Err(StoreError::ModelCallInFlight)
    );
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 1);
    assert_eq!(
        store.reserve_model_call(draft(id(1), None, price(1, 1)), cap),
        Err(StoreError::DuplicateModelRequestId)
    );
}

#[test]
fn durable_turn_counter_counts_primary_only_and_survives_attempt_retention() {
    let path = TempDb::new();
    let store = Store::open(&path.0, &FixedClock).unwrap();
    insert_task(&store);
    let cap = UsdMicros::new(10_000_000).unwrap();

    store
        .reserve_model_call(draft(id(1), Some(task_id()), price(1, 1)), cap)
        .unwrap();
    assert_eq!(store.task_model_turn_count(&task_id()).unwrap(), 1);
    store
        .fail_model_call(&id(1), "UPSTREAM_UNAVAILABLE", EpochMillis::new(2).unwrap())
        .unwrap();

    let mut fallback = draft(id(2), Some(task_id()), price(1, 1));
    fallback.relation_kind = ModelAttemptRelationKind::Fallback;
    fallback.parent_request_id = Some(id(1));
    fallback.model_id = ModelId::new("gpt-oss-20b").unwrap();
    fallback.fallback_from_model_id = Some(ModelId::new("nemotron-3-nano-30b").unwrap());
    store.reserve_model_call(fallback, cap).unwrap();
    assert_eq!(store.task_model_turn_count(&task_id()).unwrap(), 1);
    store
        .fail_model_call(&id(2), "UPSTREAM_UNAVAILABLE", EpochMillis::new(3).unwrap())
        .unwrap();

    let mut repair = draft(id(3), Some(task_id()), price(1, 1));
    repair.relation_kind = ModelAttemptRelationKind::Repair;
    repair.parent_request_id = Some(id(2));
    store.reserve_model_call(repair, cap).unwrap();
    assert_eq!(store.task_model_turn_count(&task_id()).unwrap(), 1);
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 3);
    store
        .fail_model_call(&id(3), "MODEL_OUTPUT_INVALID", EpochMillis::new(4).unwrap())
        .unwrap();

    let retained_at = EpochMillis::new(1_767_225_600_000 + 31 * 24 * 60 * 60 * 1000).unwrap();
    assert_eq!(
        store.retain_model_call_attempts(retained_at, 512).unwrap(),
        3
    );
    assert_eq!(store.task_model_turn_count(&task_id()).unwrap(), 1);
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 3);

    drop(store);
    let store = Store::open(&path.0, &FixedClock).unwrap();
    assert_eq!(store.task_model_turn_count(&task_id()).unwrap(), 1);
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 3);

    store
        .reserve_model_call(draft(id(4), Some(task_id()), price(1, 1)), cap)
        .unwrap();
    assert_eq!(store.task_model_turn_count(&task_id()).unwrap(), 2);
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 4);
}

#[cfg(feature = "p2h-fault-injection")]
#[test]
fn failed_dispatch_intent_rolls_back_call_and_turn_counters_together() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    insert_task(&store);
    crate::fault::Window::AfterModelAttemptInsert
        .arm(crate::fault::Action::Fail(StoreError::Sqlite))
        .unwrap();
    assert_eq!(
        store.reserve_model_call(
            draft(id(1), Some(task_id()), price(1, 1)),
            UsdMicros::new(10_000_000).unwrap(),
        ),
        Err(StoreError::Sqlite)
    );
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 0);
    assert_eq!(store.task_model_turn_count(&task_id()).unwrap(), 0);
    assert!(store.get_model_call_attempt(&id(1)).unwrap().is_none());
}

#[test]
fn spend_reservation_allows_exact_cap_and_refuses_cap_plus_one() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let cap = UsdMicros::new(5_120).unwrap();
    store
        .reserve_model_call(draft(id(1), None, price(1_000_000, 1_000_000)), cap)
        .unwrap();
    assert_eq!(
        store
            .utc_day_spend_occupancy(UtcAccountingDay::from_epoch_millis(
                EpochMillis::new(1_767_225_600_000).unwrap()
            ))
            .unwrap(),
        cap
    );
    assert_eq!(
        store.reserve_model_call(draft(id(2), None, price(1, 1)), cap),
        Err(StoreError::DailySpendExceeded)
    );
}

#[test]
fn definite_failure_counts_as_call_and_unknown_usage_keeps_reservation() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    insert_task(&store);
    let cap = UsdMicros::new(10_000_000).unwrap();
    let reserved = store
        .reserve_model_call(
            draft(id(3), Some(task_id()), price(1_000_000, 1_000_000)),
            cap,
        )
        .unwrap()
        .reserved_cost_usd_micros;
    store
        .fail_model_call(&id(3), "PROVIDER_UNAVAILABLE", EpochMillis::new(3).unwrap())
        .unwrap();
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 1);
    assert_eq!(
        store.task_model_token_usage(&task_id()).unwrap(),
        TokenCount::new(0)
    );
    assert_eq!(
        store
            .utc_day_spend_occupancy(UtcAccountingDay::from_epoch_millis(
                EpochMillis::new(1_767_225_600_000).unwrap()
            ))
            .unwrap(),
        reserved
    );
    store
        .reserve_model_call(
            draft(id(12), Some(task_id()), price(1_000_000, 1_000_000)),
            cap,
        )
        .unwrap();
    store
        .fail_model_call_with_usage(
            &id(12),
            "PROVIDER_REJECTED",
            EpochMillis::new(4).unwrap(),
            ModelFailureUsage {
                input_tokens: TokenCount::new(4),
                output_tokens: TokenCount::new(6),
                latency_ms: 3,
                repair_attempts: 0,
                recorded_at: EpochMillis::new(4).unwrap(),
            },
        )
        .unwrap();
    assert_eq!(
        store.task_model_token_usage(&task_id()).unwrap(),
        TokenCount::new(10)
    );
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 2);
    let settled = store.model_usage_for_request(&id(12)).unwrap().unwrap();
    assert_eq!(settled.cost_usd_micros, UsdMicros::new(10).unwrap());
    assert_eq!(
        store
            .utc_day_spend_occupancy(UtcAccountingDay::from_epoch_millis(
                EpochMillis::new(1_767_225_600_000).unwrap()
            ))
            .unwrap(),
        UsdMicros::new(5_130).unwrap()
    );
}

#[test]
fn fallback_and_repair_attempts_share_the_task_call_budget() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    insert_task(&store);
    let cap = UsdMicros::new(0).unwrap();
    for n in 1..=12 {
        let request = id(n);
        let mut call = draft(request.clone(), Some(task_id()), price(0, 0));
        if n % 3 == 0 {
            call.relation_kind = ModelAttemptRelationKind::Repair;
            call.parent_request_id = Some(id(n - 1));
        } else if n % 3 == 1 && n > 1 {
            call.relation_kind = ModelAttemptRelationKind::Fallback;
            call.parent_request_id = Some(id(n - 1));
            call.fallback_from_model_id = Some(ModelId::new("gpt-oss-20b").unwrap());
        }
        store.reserve_model_call(call, cap).unwrap();
        store
            .fail_model_call(
                &request,
                "DEFINITE_FAILURE",
                EpochMillis::new(i64::from(n)).unwrap(),
            )
            .unwrap();
    }
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 12);
    assert_eq!(
        store.reserve_model_call(draft(id(13), Some(task_id()), price(0, 0)), cap),
        Err(StoreError::ModelCallBudgetExceeded)
    );
}

#[test]
fn private_and_secret_attempts_fail_closed() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    for (n, data_class) in [
        (4, DataClass::Private),
        (5, DataClass::Secret),
        (6, DataClass::Credential),
    ] {
        let mut call = draft(id(n), None, price(0, 0));
        call.data_class = data_class;
        assert_eq!(
            store.reserve_model_call(call, UsdMicros::new(10).unwrap()),
            Err(StoreError::ModelDataClassRefused)
        );
    }
}

#[test]
fn output_bound_and_response_class_are_enforced_before_completion() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let cap = UsdMicros::new(10_000_000).unwrap();
    let mut personal = draft(id(8), None, price(0, 0));
    personal.data_class = DataClass::Personal;
    store.reserve_model_call(personal, cap).unwrap();
    let too_many = ModelCallCompletion {
        input_tokens: TokenCount::new(1),
        output_tokens: TokenCount::new(1_025),
        latency_ms: 1,
        repair_attempts: 0,
        finish_reason: FinishReason::Stop,
        recorded_at: EpochMillis::new(8).unwrap(),
        accepted_response: ModelResponseStorage {
            canonical_json: br#""answer""#.to_vec(),
            data_class: DataClass::Personal,
        },
    };
    assert_eq!(
        store.complete_model_call(&id(8), too_many),
        Err(StoreError::ModelUsageExceedsBound)
    );
    let downgraded = ModelCallCompletion {
        input_tokens: TokenCount::new(1),
        output_tokens: TokenCount::new(1),
        latency_ms: 1,
        repair_attempts: 0,
        finish_reason: FinishReason::Stop,
        recorded_at: EpochMillis::new(8).unwrap(),
        accepted_response: ModelResponseStorage {
            canonical_json: br#""answer""#.to_vec(),
            data_class: DataClass::Public,
        },
    };
    assert_eq!(
        store.complete_model_call(&id(8), downgraded),
        Err(StoreError::ModelDataClassRefused)
    );
    assert_eq!(
        store.list_unfinished_model_call_attempts().unwrap().len(),
        1
    );
}

#[test]
fn response_document_over_bound_is_refused_before_blob_write() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    store
        .reserve_model_call(draft(id(14), None, price(0, 0)), UsdMicros::new(0).unwrap())
        .unwrap();
    let completion = ModelCallCompletion {
        input_tokens: TokenCount::new(0),
        output_tokens: TokenCount::new(0),
        latency_ms: 0,
        repair_attempts: 0,
        finish_reason: FinishReason::Stop,
        recorded_at: EpochMillis::new(1).unwrap(),
        accepted_response: ModelResponseStorage {
            canonical_json: vec![b' '; crate::MAX_MODEL_RESPONSE_BYTES + 1],
            data_class: DataClass::Public,
        },
    };
    assert_eq!(
        store.complete_model_call(&id(14), completion),
        Err(StoreError::ModelUsageExceedsBound)
    );
    let conn = store.conn.lock().unwrap();
    let blobs: i64 = conn
        .query_row("SELECT count(*) FROM blobs", [], |row| row.get(0))
        .unwrap();
    assert_eq!(blobs, 0);
}

#[test]
fn task_deletion_nulls_accounting_links_and_removes_response_content() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    insert_task(&store);
    let request = id(9);
    let mut call = draft(request.clone(), Some(task_id()), price(0, 0));
    call.data_class = DataClass::Personal;
    store
        .reserve_model_call(call, UsdMicros::new(100).unwrap())
        .unwrap();
    store
        .complete_model_call(
            &request,
            ModelCallCompletion {
                input_tokens: TokenCount::new(5),
                output_tokens: TokenCount::new(5),
                latency_ms: 2,
                repair_attempts: 0,
                finish_reason: FinishReason::Stop,
                recorded_at: EpochMillis::new(9).unwrap(),
                accepted_response: ModelResponseStorage {
                    canonical_json: br#"{"answer":"kept"}"#.to_vec(),
                    data_class: DataClass::Personal,
                },
            },
        )
        .unwrap();
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 1);
    assert_eq!(
        store.task_model_token_usage(&task_id()).unwrap(),
        TokenCount::new(10)
    );
    let digest = store
        .get_model_call_attempt(&request)
        .unwrap()
        .unwrap()
        .response_blob
        .unwrap()
        .digest()
        .clone();
    delete_task_with_blob_sweep(&store);
    let deleted_attempt = store.get_model_call_attempt(&request).unwrap().unwrap();
    assert_eq!(deleted_attempt.task_id, None);
    assert!(!deleted_attempt.response_storage_allowed);
    assert_eq!(store.get_model_call_response(&request).unwrap(), None);
    store.verify_integrity().unwrap();
    let conn = store.conn.lock().unwrap();
    let retained: i64 = conn
        .query_row(
            "SELECT count(*) FROM blobs WHERE digest=?1 AND data_class_rank=1",
            [digest.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(retained, 0);
    let usage_task: Option<String> = conn
        .query_row(
            "SELECT task_id FROM model_usage WHERE request_id=?1",
            [request.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(usage_task, None);
}

#[test]
fn attempt_and_usage_retention_preserve_separate_30_and_365_day_lifetimes() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    insert_task(&store);
    let dispatched_at = EpochMillis::new(1_000).unwrap();
    let completed_at = EpochMillis::new(2_000).unwrap();
    let mut call = draft(id(10), Some(task_id()), price(0, 0));
    call.dispatch_intent_at = dispatched_at;
    store
        .reserve_model_call(call, UsdMicros::new(0).unwrap())
        .unwrap();
    store
        .complete_model_call(
            &id(10),
            ModelCallCompletion {
                input_tokens: TokenCount::new(1),
                output_tokens: TokenCount::new(1),
                latency_ms: 1,
                repair_attempts: 0,
                finish_reason: FinishReason::Stop,
                recorded_at: completed_at,
                accepted_response: ModelResponseStorage {
                    canonical_json: br#""ok""#.to_vec(),
                    data_class: DataClass::Public,
                },
            },
        )
        .unwrap();
    assert_eq!(
        store
            .retain_model_call_attempts(
                EpochMillis::new(ATTEMPT_RETENTION_MS_TEST + 2_001).unwrap(),
                10
            )
            .unwrap(),
        1
    );
    assert!(store.get_model_call_attempt(&id(10)).unwrap().is_none());
    assert_eq!(store.task_model_call_count(&task_id()).unwrap(), 1);
    let conn = store.conn.lock().unwrap();
    let (usage_count, linked_request): (i64, Option<String>) = conn
        .query_row(
            "SELECT count(*),max(request_id) FROM model_usage",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(usage_count, 1);
    assert_eq!(linked_request, None);
    drop(conn);
    assert_eq!(
        store
            .retain_model_usage(
                EpochMillis::new(USAGE_RETENTION_MS_TEST + 2_000).unwrap(),
                10
            )
            .unwrap(),
        0
    );
    assert_eq!(
        store
            .retain_model_usage(
                EpochMillis::new(USAGE_RETENTION_MS_TEST + 2_001).unwrap(),
                10
            )
            .unwrap(),
        1
    );
    assert!(
        store
            .retain_model_usage(EpochMillis::new(0).unwrap(), 0)
            .is_err()
    );
}

#[test]
fn ambiguous_attempt_for_active_task_is_not_pruned_by_age() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    insert_task(&store);
    let mut call = draft(id(11), Some(task_id()), price(0, 0));
    call.dispatch_intent_at = EpochMillis::new(1_000).unwrap();
    store
        .reserve_model_call(call, UsdMicros::new(0).unwrap())
        .unwrap();
    store
        .mark_model_call_ambiguous(
            &id(11),
            "AMBIGUOUS_DISPATCH",
            EpochMillis::new(2_000).unwrap(),
        )
        .unwrap();
    assert_eq!(
        store
            .retain_model_call_attempts(
                EpochMillis::new(ATTEMPT_RETENTION_MS_TEST + 2_001).unwrap(),
                10
            )
            .unwrap(),
        0
    );
    assert!(store.get_model_call_attempt(&id(11)).unwrap().is_some());
}

const ATTEMPT_RETENTION_MS_TEST: i64 = 30 * 24 * 60 * 60 * 1000;
const USAGE_RETENTION_MS_TEST: i64 = 365 * 24 * 60 * 60 * 1000;

#[test]
fn two_independent_connections_cannot_reserve_same_task_or_overspend_day() {
    let path = TempDb::new();
    let setup = Store::open(&path.0, &FixedClock).unwrap();
    insert_task(&setup);
    drop(setup);
    let first = Arc::new(Store::open(&path.0, &FixedClock).unwrap());
    let second = Arc::new(Store::open(&path.0, &FixedClock).unwrap());
    let barrier = Arc::new(Barrier::new(2));
    let cap = UsdMicros::new(5_120).unwrap();
    let a_store = first.clone();
    let a_barrier = barrier.clone();
    let a = std::thread::spawn(move || {
        a_barrier.wait();
        a_store.reserve_model_call(
            draft(id(20), Some(task_id()), price(1_000_000, 1_000_000)),
            UsdMicros::new(10_000).unwrap(),
        )
    });
    let b_store = second.clone();
    let b_barrier = barrier.clone();
    let b = std::thread::spawn(move || {
        b_barrier.wait();
        b_store.reserve_model_call(
            draft(id(21), Some(task_id()), price(1_000_000, 1_000_000)),
            UsdMicros::new(10_000).unwrap(),
        )
    });
    let results = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(StoreError::ModelCallInFlight))
            .count(),
        1
    );
    assert_eq!(first.task_model_call_count(&task_id()).unwrap(), 1);
    assert_eq!(first.task_model_turn_count(&task_id()).unwrap(), 1);
    drop(first);
    drop(second);

    let first = Arc::new(Store::open(&path.0, &FixedClock).unwrap());
    let second = Arc::new(Store::open(&path.0, &FixedClock).unwrap());
    // Use a new empty accounting day; each reservation independently requests the whole cap.
    let day_time = EpochMillis::new(86_400_000).unwrap();
    let make = |request_id| {
        let mut call = draft(request_id, None, price(1_000_000, 1_000_000));
        call.dispatch_intent_at = day_time;
        call
    };
    let barrier = Arc::new(Barrier::new(2));
    let a_store = first.clone();
    let a_barrier = barrier.clone();
    let a_call = make(id(22));
    let a = std::thread::spawn(move || {
        a_barrier.wait();
        a_store.reserve_model_call(a_call, cap)
    });
    let b_store = second.clone();
    let b_barrier = barrier.clone();
    let b_call = make(id(23));
    let b = std::thread::spawn(move || {
        b_barrier.wait();
        b_store.reserve_model_call(b_call, cap)
    });
    let results = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(StoreError::DailySpendExceeded))
            .count(),
        1
    );
    assert_eq!(
        first
            .utc_day_spend_occupancy(UtcAccountingDay::from_epoch_millis(day_time))
            .unwrap(),
        cap
    );
}

#[test]
fn duplicate_request_id_and_terminal_transition_race_are_serialized() {
    let path = TempDb::new();
    let first = Arc::new(Store::open(&path.0, &FixedClock).unwrap());
    let second = Arc::new(Store::open(&path.0, &FixedClock).unwrap());
    let cap = UsdMicros::new(100_000).unwrap();
    first
        .reserve_model_call(draft(id(24), None, price(0, 0)), cap)
        .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let a_store = first.clone();
    let a_barrier = barrier.clone();
    let a = std::thread::spawn(move || {
        a_barrier.wait();
        a_store.complete_model_call(
            &id(24),
            ModelCallCompletion {
                input_tokens: TokenCount::new(1),
                output_tokens: TokenCount::new(1),
                latency_ms: 1,
                repair_attempts: 0,
                finish_reason: FinishReason::Stop,
                recorded_at: EpochMillis::new(1).unwrap(),
                accepted_response: ModelResponseStorage {
                    canonical_json: br#""ok""#.to_vec(),
                    data_class: DataClass::Public,
                },
            },
        )
    });
    let b_store = second.clone();
    let b_barrier = barrier.clone();
    let b = std::thread::spawn(move || {
        b_barrier.wait();
        b_store.mark_model_call_ambiguous(
            &id(24),
            "AMBIGUOUS_DISPATCH",
            EpochMillis::new(1).unwrap(),
        )
    });
    let results = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(StoreError::InvalidModelCallTransition))
            .count(),
        1
    );
    let final_attempt = first.get_model_call_attempt(&id(24)).unwrap().unwrap();
    assert!(matches!(
        final_attempt.state,
        ModelAttemptState::Completed | ModelAttemptState::Ambiguous
    ));

    let duplicate_path = TempDb::new();
    let a_store = Arc::new(Store::open(&duplicate_path.0, &FixedClock).unwrap());
    let b_store = Arc::new(Store::open(&duplicate_path.0, &FixedClock).unwrap());
    let barrier = Arc::new(Barrier::new(2));
    let a_store_clone = a_store.clone();
    let a_barrier = barrier.clone();
    let a = std::thread::spawn(move || {
        a_barrier.wait();
        a_store_clone.reserve_model_call(draft(id(25), None, price(0, 0)), cap)
    });
    let b_store_clone = b_store.clone();
    let b_barrier = barrier.clone();
    let b = std::thread::spawn(move || {
        b_barrier.wait();
        b_store_clone.reserve_model_call(draft(id(25), None, price(0, 0)), cap)
    });
    let results = [a.join().unwrap(), b.join().unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| **result == Err(StoreError::DuplicateModelRequestId))
            .count(),
        1
    );
}

#[test]
fn task_deletion_racing_terminal_accounting_preserves_fk_and_usage_invariants() {
    let path = TempDb::new();
    let setup = Store::open(&path.0, &FixedClock).unwrap();
    insert_task(&setup);
    setup
        .reserve_model_call(
            draft(id(26), Some(task_id()), price(0, 0)),
            UsdMicros::new(10).unwrap(),
        )
        .unwrap();
    drop(setup);
    let accounting = Arc::new(Store::open(&path.0, &FixedClock).unwrap());
    let deletion = Arc::new(Store::open(&path.0, &FixedClock).unwrap());
    let barrier = Arc::new(Barrier::new(2));
    let complete_store = accounting.clone();
    let complete_barrier = barrier.clone();
    let complete = std::thread::spawn(move || {
        complete_barrier.wait();
        complete_store.complete_model_call(
            &id(26),
            ModelCallCompletion {
                input_tokens: TokenCount::new(1),
                output_tokens: TokenCount::new(1),
                latency_ms: 1,
                repair_attempts: 0,
                finish_reason: FinishReason::Stop,
                recorded_at: EpochMillis::new(2).unwrap(),
                accepted_response: ModelResponseStorage {
                    canonical_json: br#""ok""#.to_vec(),
                    data_class: DataClass::Public,
                },
            },
        )
    });
    let delete_store = deletion.clone();
    let delete_barrier = barrier.clone();
    let delete = std::thread::spawn(move || {
        delete_barrier.wait();
        delete_task_with_blob_sweep(&delete_store);
        1
    });
    assert!(complete.join().unwrap().is_ok());
    assert_eq!(delete.join().unwrap(), 1);
    let attempt = accounting.get_model_call_attempt(&id(26)).unwrap().unwrap();
    assert_eq!(attempt.task_id, None);
    assert!(!attempt.response_storage_allowed);
    assert_eq!(accounting.get_model_call_response(&id(26)).unwrap(), None);
    assert_eq!(
        accounting
            .model_usage_for_request(&id(26))
            .unwrap()
            .unwrap()
            .task_id,
        None
    );
    accounting.verify_integrity().unwrap();
}

#[test]
fn ambiguous_attempt_remains_unfinished_and_keeps_full_reservation() {
    let store = Store::open_in_memory(&FixedClock).unwrap();
    let cap = UsdMicros::new(10_000_000).unwrap();
    let reserved = store
        .reserve_model_call(draft(id(1), None, price(1_000_000, 1_000_000)), cap)
        .unwrap()
        .reserved_cost_usd_micros;
    store
        .mark_model_call_ambiguous(&id(1), "AMBIGUOUS_DISPATCH", EpochMillis::new(1).unwrap())
        .unwrap();
    assert_eq!(
        store.get_model_call_attempt(&id(1)).unwrap().unwrap().state,
        ModelAttemptState::Ambiguous
    );
    assert_eq!(
        store.list_unfinished_model_call_attempts().unwrap().len(),
        0
    );
    assert_eq!(
        store
            .utc_day_spend_occupancy(UtcAccountingDay::from_epoch_millis(
                EpochMillis::new(1_767_225_600_000).unwrap()
            ))
            .unwrap(),
        reserved
    );
    assert_eq!(
        store.mark_model_call_ambiguous(&id(1), "AMBIGUOUS_DISPATCH", EpochMillis::new(2).unwrap()),
        Err(StoreError::InvalidModelCallTransition)
    );
}

#[test]
fn completion_is_one_transaction_and_response_recovers_after_reopen() {
    let path = TempDb::new();
    let cap = UsdMicros::new(10_000_000).unwrap();
    let store = Store::open(&path.0, &FixedClock).unwrap();
    let request = id(7);
    store
        .reserve_model_call(
            draft(request.clone(), None, price(1_000_000, 1_000_000)),
            cap,
        )
        .unwrap();
    let completion = ModelCallCompletion {
        input_tokens: TokenCount::new(10),
        output_tokens: TokenCount::new(20),
        latency_ms: 9,
        repair_attempts: 0,
        finish_reason: FinishReason::Stop,
        recorded_at: EpochMillis::new(2).unwrap(),
        accepted_response: ModelResponseStorage {
            canonical_json: br#"{"text":"accepted"}"#.to_vec(),
            data_class: DataClass::Public,
        },
    };
    store.complete_model_call(&request, completion).unwrap();
    drop(store); // Caller loses the returned value after commit.

    let reopened = Store::open(&path.0, &FixedClock).unwrap();
    let attempt = reopened.get_model_call_attempt(&request).unwrap().unwrap();
    assert_eq!(attempt.state, ModelAttemptState::Completed);
    assert!(attempt.response_blob.is_some());
    assert_eq!(
        reopened
            .model_usage_for_request(&request)
            .unwrap()
            .unwrap()
            .cost_usd_micros,
        UsdMicros::new(30).unwrap()
    );
    assert_eq!(
        reopened.get_model_call_response(&request).unwrap().unwrap(),
        br#"{"text":"accepted"}"#
    );
    assert_eq!(
        reopened.complete_model_call(
            &request,
            ModelCallCompletion {
                input_tokens: TokenCount::new(10),
                output_tokens: TokenCount::new(20),
                latency_ms: 9,
                repair_attempts: 0,
                finish_reason: FinishReason::Stop,
                recorded_at: EpochMillis::new(2).unwrap(),
                accepted_response: ModelResponseStorage {
                    canonical_json: br#"{"text":"accepted"}"#.to_vec(),
                    data_class: DataClass::Public
                },
            }
        ),
        Err(StoreError::InvalidModelCallTransition)
    );
}
