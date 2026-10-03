//! Pure DDL contract tests: always execute the production migration, never a
//! reduced test schema. Memory connections here do not exercise Store policy.

use std::collections::BTreeMap;

use rusqlite::{Connection, Error, ffi, params_from_iter, types::Value};
use serea_protocol::{DataClass, RiskClass, StepKind, TaskKind, TaskState};

const MIGRATION: &str = include_str!("../migrations/0001_initial.sql");
const TASK: &str = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNA";
const STEP: &str = "stp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";
const RECEIPT: &str = "rcp_01JQ8Z9M3R2CVN8H5FWK7PQDSF";
const MIN: i64 = -62_167_219_200_000;
const MAX: i64 = 253_402_300_799_999;
const U32_MAX: i64 = 4_294_967_295;
const STATUSES: [&str; 7] = [
    "PLANNED",
    "LEASED",
    "EXECUTING",
    "WAITING",
    "SUCCEEDED",
    "FAILED",
    "RECONCILED_ABSENT",
];
const KINDS: [&str; 8] = [
    "CAPABILITY",
    "MODEL_TURN",
    "WAIT_APPROVAL",
    "WAIT_USER",
    "WAIT_SCHEDULE",
    "VERIFY",
    "NOTIFY",
    "DELEGATE",
];
const CLASSIFIED_TABLES: [&str; 7] = [
    "blobs",
    "tasks",
    "side_effect_receipts",
    "plan_revisions",
    "task_blob_refs",
    "step_blob_refs",
    "task_journal",
];
const JOURNAL_KINDS: [&str; 13] = [
    "TASK_INSERTED",
    "PLAN_PERSISTED",
    "TASK_STATE_CHANGED",
    "STEP_LEASE_ACQUIRED",
    "STEP_LEASE_RELEASED",
    "STEP_ATTEMPT_STARTED",
    "STEP_COMMITTED",
    "STEP_FAILED",
    "STEP_RECONCILED_ABSENT",
    "RECEIPT_RECORDED",
    "RECOVERY_DECISION",
    "TASK_CANCEL_REQUESTED",
    "TASK_TERMINAL",
];
const ERROR_FIELDS: [&str; 6] = [
    "error_kind",
    "error_code",
    "error_message",
    "error_retryable",
    "error_host_action",
    "error_details",
];
const CAPABILITY_FIELDS: [&str; 4] = [
    "provider_id",
    "capability_id",
    "capability_version",
    "idempotency_key",
];
// Table, column, SQL nullability. Keep this inventory independent of migration
// parsing so an omitted range check or newly added instant cannot go unnoticed.
const INSTANTS: [(&str, &str, bool); 14] = [
    ("schema_migrations", "applied_at_ms", false),
    ("tasks", "created_at_ms", false),
    ("tasks", "updated_at_ms", false),
    ("tasks", "deadline_at_ms", true),
    ("tasks", "cancelled_at_ms", true),
    ("task_steps", "started_at_ms", true),
    ("task_steps", "completed_at_ms", true),
    ("task_steps", "lease_expires_at_ms", true),
    ("side_effect_receipts", "observed_at_ms", false),
    ("leases", "acquired_at_ms", false),
    ("leases", "expires_at_ms", false),
    ("leases", "released_at_ms", true),
    ("plan_revisions", "created_at_ms", false),
    ("task_journal", "occurred_at_ms", false),
];

type Row = BTreeMap<&'static str, Value>;

fn text(value: &str) -> Value {
    Value::Text(value.to_owned())
}

fn digest() -> Value {
    text(&format!("sha256:{}", "a".repeat(64)))
}

fn key() -> Value {
    text(&format!("idk_{}", "a".repeat(64)))
}

fn schema() -> Connection {
    let db = Connection::open_in_memory().expect("open pure schema connection");
    db.execute_batch("PRAGMA foreign_keys = ON; BEGIN IMMEDIATE;")
        .unwrap();
    db.execute_batch(MIGRATION)
        .expect("production migration must build verbatim");
    db.execute_batch("COMMIT;").unwrap();
    assert_eq!(
        db.query_row("PRAGMA foreign_keys", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
    db
}

fn task(state: &str) -> Row {
    let mut row = Row::from([
        ("task_id", text(TASK)),
        ("kind", text("USER_REQUEST")),
        ("title", text("schema probe")),
        ("state", text(state)),
        ("origin_kind", text("USER")),
        ("data_class_rank", Value::Integer(0)),
        ("policy_class_rank", Value::Integer(0)),
        ("created_at_ms", Value::Integer(0)),
        ("updated_at_ms", Value::Integer(0)),
        ("max_model_calls", Value::Integer(3)),
        ("max_tool_calls", Value::Integer(3)),
        ("max_attempts_per_step", Value::Integer(3)),
    ]);
    if state == "CANCELLED" {
        row.insert("cancelled_at_ms", Value::Integer(0));
        row.insert("cancelled_by", text("USER"));
    }
    if state == "FAILED" {
        row.insert("failure_reason", text("PROVIDER_ERROR"));
    }
    row
}

fn error_values() -> [Value; 6] {
    [
        text("PROVIDER_ERROR"),
        text("UPSTREAM_5XX"),
        text("schema probe"),
        Value::Integer(0),
        text("RETRY"),
        text("{}"),
    ]
}

fn capability_kind(kind: &str) -> bool {
    matches!(kind, "CAPABILITY" | "DELEGATE" | "VERIFY")
}

fn wait_kind(kind: &str) -> bool {
    matches!(kind, "WAIT_APPROVAL" | "WAIT_USER" | "WAIT_SCHEDULE")
}

fn step(kind: &str, status: &str) -> Row {
    // Matches docs/plans/p2a-doc-probes.py's constructive fixture, including the
    // optional FAILED details omission and retained terminal generation.
    let mut row = Row::from([
        ("step_id", text(STEP)),
        ("task_id", text(TASK)),
        ("sequence", Value::Integer(0)),
        ("kind", text(kind)),
        ("status", text(status)),
        ("input_digest", digest()),
    ]);
    if capability_kind(kind) {
        for (column, value) in CAPABILITY_FIELDS.into_iter().zip([
            text("calendar"),
            text("calendar.events.list"),
            text("1.2.0"),
            key(),
        ]) {
            row.insert(column, value);
        }
    }
    if status != "PLANNED" {
        row.insert("attempt", Value::Integer(1));
        row.insert("lease_generation", Value::Integer(1));
    }
    if matches!(status, "LEASED" | "EXECUTING") {
        row.insert("lease_owner", text("worker-a"));
        row.insert("lease_expires_at_ms", Value::Integer(10));
    }
    if !matches!(status, "PLANNED" | "LEASED") {
        row.insert("started_at_ms", Value::Integer(1));
    }
    if matches!(status, "SUCCEEDED" | "FAILED" | "RECONCILED_ABSENT") {
        row.insert("completed_at_ms", Value::Integer(2));
    }
    if status == "SUCCEEDED" {
        row.insert("result_digest", digest());
    }
    if status == "FAILED" {
        for (column, value) in ERROR_FIELDS.into_iter().zip(error_values()).take(5) {
            row.insert(column, value);
        }
    }
    row
}

fn base_row(table: &str) -> Row {
    match table {
        "schema_migrations" => Row::from([
            ("version", Value::Integer(1)),
            ("name", text("0001_initial")),
            ("checksum", digest()),
            ("applied_at_ms", Value::Integer(0)),
        ]),
        "tasks" => task("READY"),
        "task_steps" => step("CAPABILITY", "SUCCEEDED"),
        "blobs" => Row::from([
            ("digest", digest()),
            ("data_class_rank", Value::Integer(0)),
            ("protection", text("NONE")),
            ("size_bytes", Value::Integer(2)),
            ("content", Value::Blob(b"{}".to_vec())),
        ]),
        "side_effect_receipts" => Row::from([
            ("receipt_id", text(RECEIPT)),
            ("task_id", text(TASK)),
            ("step_id", text(STEP)),
            ("capability_id", text("calendar.events.list")),
            ("idempotency_key", key()),
            ("effect_summary", text("schema probe")),
            ("observed_at_ms", Value::Integer(0)),
            ("replay_safe", Value::Integer(1)),
            ("data_class_rank", Value::Integer(0)),
        ]),
        "leases" => Row::from([
            ("step_id", text(STEP)),
            ("owner", text("worker-a")),
            ("generation", Value::Integer(1)),
            ("acquired_at_ms", Value::Integer(0)),
            ("expires_at_ms", Value::Integer(10)),
        ]),
        "plan_revisions" => Row::from([
            ("task_id", text(TASK)),
            ("plan_revision", Value::Integer(0)),
            ("created_at_ms", Value::Integer(0)),
            ("plan_digest", digest()),
            ("data_class_rank", Value::Integer(0)),
            ("step_count", Value::Integer(0)),
        ]),
        "task_blob_refs" => Row::from([
            ("task_id", text(TASK)),
            ("role", text("PLAN")),
            ("digest", digest()),
            ("data_class_rank", Value::Integer(0)),
        ]),
        "step_blob_refs" => Row::from([
            ("step_id", text(STEP)),
            ("role", text("INSTRUCTION")),
            ("digest", digest()),
            ("data_class_rank", Value::Integer(0)),
        ]),
        "task_journal" => Row::from([
            ("journal_id", text("schema-journal")),
            ("task_id", text(TASK)),
            ("step_id", text(STEP)),
            ("journal_seq", Value::Integer(1)),
            ("journal_kind", text("TASK_INSERTED")),
            ("actor_kind", text("HOST")),
            ("actor_id", text("schema-probe")),
            ("actor_version", text("1.0.0")),
            ("data_class_rank", Value::Integer(0)),
            ("occurred_at_ms", Value::Integer(0)),
        ]),
        _ => panic!("no fixture for {table}"),
    }
}

fn insert(db: &Connection, table: &str, row: &Row) -> rusqlite::Result<usize> {
    let columns = row.keys().copied().collect::<Vec<_>>().join(",");
    let placeholders = vec!["?"; row.len()].join(",");
    db.execute(
        &format!("INSERT INTO {table} ({columns}) VALUES ({placeholders})"),
        params_from_iter(row.values()),
    )
}

fn dependencies(table: &str) -> Connection {
    let db = schema();
    if !matches!(table, "tasks" | "blobs" | "schema_migrations") {
        insert(&db, "tasks", &task("READY")).unwrap();
    }
    if matches!(
        table,
        "side_effect_receipts" | "leases" | "step_blob_refs" | "task_journal"
    ) {
        insert(&db, "task_steps", &step("CAPABILITY", "SUCCEEDED")).unwrap();
    }
    if matches!(
        table,
        "plan_revisions" | "task_blob_refs" | "step_blob_refs"
    ) {
        for rank in 0..=2 {
            let mut blob = base_row("blobs");
            blob.insert("data_class_rank", Value::Integer(rank));
            blob.insert(
                "protection",
                text(if rank == 2 { "AT_REST" } else { "NONE" }),
            );
            insert(&db, "blobs", &blob).unwrap();
        }
    }
    db
}

fn assert_result(result: rusqlite::Result<usize>, expected: Option<i32>, context: &str) {
    match (result, expected) {
        (Ok(count), None) => assert_eq!(count, 1, "{context}"),
        (Err(Error::SqliteFailure(error, _)), Some(code)) => {
            assert_eq!(error.extended_code, code, "{context}");
        }
        (actual, expected) => panic!("{context}: expected constraint {expected:?}, got {actual:?}"),
    }
}

fn probe(db: &Connection, table: &str, row: &Row, expected: Option<i32>) {
    db.execute_batch("SAVEPOINT schema_probe;").unwrap();
    assert_result(
        insert(db, table, row),
        expected,
        &format!("{table}: {row:?}"),
    );
    db.execute_batch("ROLLBACK TO schema_probe; RELEASE schema_probe;")
        .unwrap();
}

fn check_probe(db: &Connection, table: &str, row: &Row, accepted: bool) {
    probe(
        db,
        table,
        row,
        if accepted {
            None
        } else {
            Some(ffi::SQLITE_CONSTRAINT_CHECK)
        },
    );
}

fn strings(db: &Connection, sql: &str) -> Vec<String> {
    db.prepare(sql)
        .unwrap()
        .query_map([], |r| r.get(0))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[test]
fn exact_table_index_and_trigger_inventory() {
    let db = schema();
    let mut expected = vec![
        "table:blobs",
        "table:leases",
        "table:plan_revisions",
        "table:schema_migrations",
        "table:side_effect_receipts",
        "table:step_blob_refs",
        "table:task_blob_refs",
        "table:task_journal",
        "table:task_steps",
        "table:tasks",
        "index:plan_revisions_digest",
        "index:step_blob_refs_digest",
        "index:task_blob_refs_digest",
        "index:task_steps_status_lease",
        "index:task_steps_task_status",
        "index:tasks_state",
        "index:sqlite_autoindex_schema_migrations_1",
        "index:sqlite_autoindex_blobs_1",
        "index:sqlite_autoindex_tasks_1",
        "index:sqlite_autoindex_task_steps_1",
        "index:sqlite_autoindex_task_steps_2",
        "index:sqlite_autoindex_task_steps_3",
        "index:sqlite_autoindex_side_effect_receipts_1",
        "index:sqlite_autoindex_side_effect_receipts_2",
        "index:sqlite_autoindex_leases_1",
        "index:sqlite_autoindex_plan_revisions_1",
        "index:sqlite_autoindex_task_blob_refs_1",
        "index:sqlite_autoindex_step_blob_refs_1",
        "index:sqlite_autoindex_task_journal_1",
        "index:sqlite_autoindex_task_journal_2",
        "trigger:tasks_policy_class_immutable",
        "trigger:tasks_data_class_monotonic",
        "trigger:side_effect_receipts_key_matches_step",
        "trigger:side_effect_receipts_task_matches_step",
        "trigger:side_effect_receipts_step_must_succeed",
        "trigger:task_steps_idempotency_key_immutable",
        "trigger:task_journal_step_task_matches",
    ];
    expected.sort_unstable();
    assert_eq!(
        strings(
            &db,
            "SELECT type || ':' || name FROM sqlite_schema ORDER BY type || ':' || name"
        ),
        expected
    );
    assert_eq!(db.query_row("SELECT count(*) FROM pragma_table_list WHERE schema = 'main' AND name NOT LIKE 'sqlite_%' AND strict = 1", [], |r| r.get::<_, i64>(0)).unwrap(), 10);
}

#[test]
fn every_kind_status_cell_is_constructible_exactly_when_legal() {
    let db = dependencies("task_steps");
    assert_eq!(StepKind::WIRE_NAMES, KINDS);
    let mut legal = 0;
    for kind in KINDS {
        for status in STATUSES {
            let accepted = status != "WAITING" || wait_kind(kind);
            check_probe(&db, "task_steps", &step(kind, status), accepted);
            legal += usize::from(accepted);
        }
    }
    assert_eq!(legal, 51);
}

#[test]
fn all_64_error_subsets_in_every_status() {
    let db = dependencies("task_steps");
    let mut legal = 0;
    for status in STATUSES {
        for mask in 0_u8..64 {
            let mut row = step(
                if status == "WAITING" {
                    "WAIT_USER"
                } else {
                    "MODEL_TURN"
                },
                status,
            );
            for (bit, (column, value)) in ERROR_FIELDS.into_iter().zip(error_values()).enumerate() {
                row.insert(
                    column,
                    if mask & (1 << bit) != 0 {
                        value
                    } else {
                        Value::Null
                    },
                );
            }
            let accepted = if status == "FAILED" {
                mask & 31 == 31
            } else {
                mask == 0
            };
            check_probe(&db, "task_steps", &row, accepted);
            legal += usize::from(accepted);
        }
    }
    assert_eq!(legal, 8); // Six empty tuples, FAILED with/without details.
}

#[test]
fn every_capability_tuple_subset_in_every_legal_kind_status_cell() {
    let db = dependencies("task_steps");
    for kind in KINDS {
        for status in STATUSES {
            if status == "WAITING" && !wait_kind(kind) {
                continue;
            }
            for mask in 0_u8..16 {
                let mut row = step(kind, status);
                for (bit, (column, value)) in CAPABILITY_FIELDS
                    .into_iter()
                    .zip([
                        text("calendar"),
                        text("calendar.events.list"),
                        text("1.2.0"),
                        key(),
                    ])
                    .enumerate()
                {
                    row.insert(
                        column,
                        if mask & (1 << bit) != 0 {
                            value
                        } else {
                            Value::Null
                        },
                    );
                }
                check_probe(
                    &db,
                    "task_steps",
                    &row,
                    mask == if capability_kind(kind) { 15 } else { 0 },
                );
            }
        }
    }
}

#[test]
fn step_presence_matrix_requires_or_forbids_each_independent_field() {
    let db = dependencies("task_steps");
    for status in STATUSES {
        let baseline = step(
            if status == "WAITING" {
                "WAIT_USER"
            } else {
                "MODEL_TURN"
            },
            status,
        );
        check_probe(&db, "task_steps", &baseline, true);
        for (column, present, required, forbidden) in [
            (
                "started_at_ms",
                Value::Integer(1),
                !matches!(status, "PLANNED" | "LEASED"),
                matches!(status, "PLANNED" | "LEASED"),
            ),
            (
                "completed_at_ms",
                Value::Integer(2),
                matches!(status, "SUCCEEDED" | "FAILED" | "RECONCILED_ABSENT"),
                matches!(status, "PLANNED" | "LEASED" | "EXECUTING" | "WAITING"),
            ),
            (
                "result_digest",
                digest(),
                status == "SUCCEEDED",
                matches!(status, "PLANNED" | "LEASED" | "EXECUTING" | "WAITING"),
            ),
            (
                "lease_owner",
                text("worker-a"),
                matches!(status, "LEASED" | "EXECUTING"),
                !matches!(status, "LEASED" | "EXECUTING"),
            ),
            (
                "lease_expires_at_ms",
                Value::Integer(10),
                matches!(status, "LEASED" | "EXECUTING"),
                !matches!(status, "LEASED" | "EXECUTING"),
            ),
        ] {
            for (value, accepted) in [(Value::Null, !required), (present, !forbidden)] {
                let mut row = baseline.clone();
                row.insert(column, value);
                check_probe(&db, "task_steps", &row, accepted);
            }
        }
    }
}

#[test]
fn all_task_states_and_cancellation_failure_blocked_presence_tuples() {
    let db = schema();
    let mut legal = 0;
    for state in TaskState::WIRE_NAMES {
        for mask in 0_u8..16 {
            let mut row = task(state);
            for (bit, (column, value)) in [
                ("cancelled_at_ms", Value::Integer(0)),
                ("cancelled_by", text("USER")),
                ("failure_reason", text("PROVIDER_ERROR")),
                ("blocked_reason", text("POLICY_DENIED")),
            ]
            .into_iter()
            .enumerate()
            {
                row.insert(
                    column,
                    if mask & (1 << bit) != 0 {
                        value
                    } else {
                        Value::Null
                    },
                );
            }
            let accepted = (mask & 3 == if *state == "CANCELLED" { 3 } else { 0 })
                && ((mask & 4 != 0) == (*state == "FAILED"))
                && (mask & 8 == 0 || *state == "BLOCKED");
            check_probe(&db, "tasks", &row, accepted);
            legal += usize::from(accepted);
        }
    }
    assert_eq!(TaskState::WIRE_NAMES.len(), 11);
    assert_eq!(legal, 12); // BLOCKED allows an absent or present reason.
}

#[test]
fn closed_task_kinds_states_and_step_statuses_refuse_unknown_values() {
    let db = schema();
    for kind in TaskKind::WIRE_NAMES {
        let mut row = task("READY");
        row.insert("kind", text(kind));
        check_probe(&db, "tasks", &row, true);
    }
    for (column, value) in [("kind", "FUTURE_KIND"), ("state", "FUTURE_STATE")] {
        let mut row = task("READY");
        row.insert(column, text(value));
        check_probe(&db, "tasks", &row, false);
    }
    let db = dependencies("task_steps");
    for (column, value) in [
        ("kind", "FUTURE_KIND"),
        ("status", "FUTURE_STATUS"),
        ("status", "SUPERSEDED"),
    ] {
        let mut row = step("MODEL_TURN", "PLANNED");
        row.insert(column, text(value));
        check_probe(&db, "task_steps", &row, false);
    }
}

#[test]
fn attempt_and_generation_are_u32_with_zero_only_for_planned_steps() {
    let db = dependencies("task_steps");
    for status in STATUSES {
        for column in ["attempt", "lease_generation"] {
            for value in [-1, 0, 1, U32_MAX, U32_MAX + 1] {
                let mut row = step(
                    if status == "WAITING" {
                        "WAIT_USER"
                    } else {
                        "MODEL_TURN"
                    },
                    status,
                );
                row.insert(column, Value::Integer(value));
                let accepted = if status == "PLANNED" {
                    value == 0
                } else {
                    (1..=U32_MAX).contains(&value)
                };
                check_probe(&db, "task_steps", &row, accepted);
            }
        }
    }
}

#[test]
fn lease_generation_baseline_is_positive_u32_not_a_cross_table_trigger() {
    let db = dependencies("leases");
    for generation in [-1, 0, 1, 2, U32_MAX, U32_MAX + 1] {
        let mut row = base_row("leases");
        row.insert("generation", Value::Integer(generation));
        check_probe(&db, "leases", &row, (1..=U32_MAX).contains(&generation));
    }
    // First acquisition must be legal before the step's SQL-zero copy changes.
    let db = dependencies("task_steps");
    insert(&db, "task_steps", &step("MODEL_TURN", "PLANNED")).unwrap();
    probe(&db, "leases", &base_row("leases"), None);
}

#[test]
fn lease_expiry_and_release_ordering() {
    let db = dependencies("leases");
    for (acquired, expires, released, accepted) in [
        (0, 1, None, true),
        (0, 1, Some(0), true),
        (0, 1, Some(2), true),
        (0, 0, None, false),
        (1, 0, None, false),
        (1, 2, Some(0), false),
    ] {
        let mut row = base_row("leases");
        row.insert("acquired_at_ms", Value::Integer(acquired));
        row.insert("expires_at_ms", Value::Integer(expires));
        row.insert(
            "released_at_ms",
            released.map_or(Value::Null, Value::Integer),
        );
        check_probe(&db, "leases", &row, accepted);
    }
}

#[test]
fn every_classified_table_caps_content_at_private() {
    for table in CLASSIFIED_TABLES {
        let db = dependencies(table);
        for rank in -1..=5 {
            let mut row = base_row(table);
            row.insert("data_class_rank", Value::Integer(rank));
            if table == "blobs" {
                row.insert(
                    "protection",
                    text(if rank == 2 { "AT_REST" } else { "NONE" }),
                );
            }
            check_probe(&db, table, &row, (0..=2).contains(&rank));
        }
    }
    let db = schema();
    for rank in [-1, 0, 7, 8] {
        let mut row = task("READY");
        row.insert("policy_class_rank", Value::Integer(rank));
        check_probe(&db, "tasks", &row, (0..=7).contains(&rank));
    }
}

#[test]
fn generated_class_labels_and_ranks_match_protocol_in_both_directions() {
    let db = schema();
    let classes = [
        DataClass::Public,
        DataClass::Personal,
        DataClass::Private,
        DataClass::Secret,
        DataClass::Credential,
    ];
    assert_eq!(
        classes.map(DataClass::wire_name).as_slice(),
        DataClass::WIRE_NAMES
    );
    for (rank, class) in classes.into_iter().enumerate() {
        assert_eq!(usize::from(class.rank()), rank);
        for table in ["tasks", "blobs"] {
            let mut row = base_row(table);
            row.insert("data_class_rank", Value::Integer(i64::from(class.rank())));
            if table == "blobs" {
                row.insert(
                    "protection",
                    text(if class == DataClass::Private {
                        "AT_REST"
                    } else {
                        "NONE"
                    }),
                );
            }
            if rank > 2 {
                check_probe(&db, table, &row, false);
                continue;
            }
            db.execute_batch("SAVEPOINT labels;").unwrap();
            insert(&db, table, &row).unwrap();
            let stored: (i64, String) = db
                .query_row(
                    &format!("SELECT data_class_rank, data_class FROM {table}"),
                    [],
                    |r| Ok((r.get(0)?, r.get(1)?)),
                )
                .unwrap();
            assert_eq!(
                stored,
                (i64::from(class.rank()), class.wire_name().to_owned())
            );
            assert_eq!(
                classes
                    .iter()
                    .find(|c| c.wire_name() == stored.1)
                    .unwrap()
                    .rank(),
                class.rank()
            );
            db.execute_batch("ROLLBACK TO labels; RELEASE labels;")
                .unwrap();
        }
    }
    let risks = [
        RiskClass::Observe,
        RiskClass::LocalState,
        RiskClass::ReversibleWrite,
        RiskClass::ExternalWrite,
        RiskClass::Communication,
        RiskClass::ElevatedDevice,
        RiskClass::Destructive,
        RiskClass::Credential,
    ];
    assert_eq!(
        risks.map(RiskClass::wire_name).as_slice(),
        RiskClass::WIRE_NAMES
    );
    for (rank, risk) in risks.into_iter().enumerate() {
        assert_eq!(usize::from(risk.rank()), rank);
        let mut row = task("READY");
        row.insert("policy_class_rank", Value::Integer(i64::from(risk.rank())));
        db.execute_batch("SAVEPOINT labels;").unwrap();
        insert(&db, "tasks", &row).unwrap();
        let stored: (i64, String) = db
            .query_row(
                "SELECT policy_class_rank, policy_class FROM tasks",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .unwrap();
        assert_eq!(
            stored,
            (i64::from(risk.rank()), risk.wire_name().to_owned())
        );
        assert_eq!(
            risks
                .iter()
                .find(|r| r.wire_name() == stored.1)
                .unwrap()
                .rank(),
            risk.rank()
        );
        db.execute_batch("ROLLBACK TO labels; RELEASE labels;")
            .unwrap();
    }
    for (table, column) in [
        ("tasks", "data_class"),
        ("tasks", "policy_class"),
        ("blobs", "data_class"),
    ] {
        let mut row = base_row(table);
        row.insert(column, text("PUBLIC"));
        probe(&db, table, &row, Some(ffi::SQLITE_ERROR));
    }
}

#[test]
fn blob_protection_length_and_class_scoped_deduplication() {
    let db = schema();
    for rank in 0..=2 {
        for protection in ["NONE", "AT_REST", "UNKNOWN"] {
            let mut row = base_row("blobs");
            row.insert("data_class_rank", Value::Integer(rank));
            row.insert("protection", text(protection));
            check_probe(
                &db,
                "blobs",
                &row,
                protection == if rank == 2 { "AT_REST" } else { "NONE" },
            );
        }
    }
    for size in [-1, 0, 1, 2, 3] {
        let mut row = base_row("blobs");
        row.insert("size_bytes", Value::Integer(size));
        check_probe(&db, "blobs", &row, size == 2);
    }
    let mut empty = base_row("blobs");
    empty.insert("content", Value::Blob(Vec::new()));
    empty.insert("size_bytes", Value::Integer(0));
    check_probe(&db, "blobs", &empty, true);
    insert(&db, "blobs", &base_row("blobs")).unwrap();
    probe(
        &db,
        "blobs",
        &base_row("blobs"),
        Some(ffi::SQLITE_CONSTRAINT_PRIMARYKEY),
    );
    let mut other_class = base_row("blobs");
    other_class.insert("data_class_rank", Value::Integer(1));
    probe(&db, "blobs", &other_class, None);
}

#[test]
fn plan_revision_and_blob_role_baselines_and_closed_vocabularies() {
    let db = dependencies("plan_revisions");
    for revision in [-1, 0, 1, U32_MAX + 1] {
        for count in [-1, 0, 1] {
            let mut row = base_row("plan_revisions");
            row.insert("plan_revision", Value::Integer(revision));
            row.insert("step_count", Value::Integer(count));
            check_probe(&db, "plan_revisions", &row, revision >= 0 && count >= 0);
        }
    }
    for (table, roles, illegal) in [
        (
            "task_blob_refs",
            &["PLAN", "PLAN_REVISION"][..],
            &["RESULT", "ARGUMENTS", "INSTRUCTION", "EVIDENCE_PAYLOAD"][..],
        ),
        (
            "step_blob_refs",
            &["ARGUMENTS", "INSTRUCTION", "RESULT"][..],
            &["PLAN", "PLAN_REVISION", "EVIDENCE_PAYLOAD"][..],
        ),
    ] {
        let db = dependencies(table);
        for role in roles.iter().chain(illegal).chain([&"UNKNOWN"]) {
            let mut row = base_row(table);
            row.insert("role", text(role));
            check_probe(&db, table, &row, roles.contains(role));
        }
    }
}

#[test]
fn composite_blob_foreign_keys_refuse_missing_or_wrong_class_references() {
    for table in ["plan_revisions", "task_blob_refs", "step_blob_refs"] {
        let db = dependencies(table);
        let mut row = base_row(table);
        probe(&db, table, &row, None);
        let digest_column = if table == "plan_revisions" {
            "plan_digest"
        } else {
            "digest"
        };
        row.insert(digest_column, text(&format!("sha256:{}", "b".repeat(64))));
        probe(&db, table, &row, Some(ffi::SQLITE_CONSTRAINT_FOREIGNKEY));
        db.execute("DELETE FROM blobs WHERE data_class_rank = 0", [])
            .unwrap();
        row.insert(digest_column, digest());
        probe(&db, table, &row, Some(ffi::SQLITE_CONSTRAINT_FOREIGNKEY));
        row.insert("data_class_rank", Value::Integer(2));
        probe(&db, table, &row, None);
        row.insert(
            if table == "step_blob_refs" {
                "step_id"
            } else {
                "task_id"
            },
            text("missing-parent"),
        );
        probe(&db, table, &row, Some(ffi::SQLITE_CONSTRAINT_FOREIGNKEY));
    }
}

#[test]
fn referenced_blobs_are_restricted_and_owner_deletion_cascades() {
    let db = dependencies("step_blob_refs");
    for table in [
        "plan_revisions",
        "task_blob_refs",
        "step_blob_refs",
        "leases",
        "side_effect_receipts",
        "task_journal",
    ] {
        insert(&db, table, &base_row(table)).unwrap();
    }
    assert_result(
        db.execute("DELETE FROM blobs WHERE data_class_rank = 0", []),
        Some(ffi::SQLITE_CONSTRAINT_TRIGGER),
        "ON DELETE RESTRICT",
    );
    assert_eq!(db.execute("DELETE FROM tasks", []).unwrap(), 1);
    for table in [
        "tasks",
        "task_steps",
        "plan_revisions",
        "task_blob_refs",
        "step_blob_refs",
        "leases",
        "side_effect_receipts",
        "task_journal",
    ] {
        assert_eq!(
            db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            0,
            "{table}"
        );
    }
    assert_eq!(db.execute("DELETE FROM blobs", []).unwrap(), 3);
}

#[test]
fn all_journal_kinds_construct_and_removed_or_event_only_names_do_not() {
    let db = dependencies("task_journal");
    for kind in JOURNAL_KINDS.into_iter().chain([
        "TASK_DELETED",
        "TASK_CREATED",
        "TASK_CANCELLED",
        "TASK_COMPLETED",
        "FUTURE_KIND",
    ]) {
        let mut row = base_row("task_journal");
        row.insert("journal_kind", text(kind));
        check_probe(&db, "task_journal", &row, JOURNAL_KINDS.contains(&kind));
    }
    for sequence in [-1, 0, 1, 2] {
        let mut row = base_row("task_journal");
        row.insert("journal_seq", Value::Integer(sequence));
        check_probe(&db, "task_journal", &row, sequence >= 1);
    }
    for step_id in [Value::Null, text(STEP), text("missing-step")] {
        let mut row = base_row("task_journal");
        let accepted = step_id != text("missing-step");
        row.insert("step_id", step_id);
        probe(
            &db,
            "task_journal",
            &row,
            if accepted {
                None
            } else {
                Some(ffi::SQLITE_CONSTRAINT_TRIGGER)
            },
        );
    }
    let other_task = "tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB";
    let mut parent = task("READY");
    parent.insert("task_id", text(other_task));
    insert(&db, "tasks", &parent).unwrap();
    let mut row = base_row("task_journal");
    row.insert("task_id", text(other_task));
    probe(
        &db,
        "task_journal",
        &row,
        Some(ffi::SQLITE_CONSTRAINT_TRIGGER),
    );
}

#[test]
fn receipt_baselines_and_binding_status_and_uniqueness_triggers() {
    let db = dependencies("side_effect_receipts");
    for replay_safe in [-1, 0, 1, 2] {
        let mut row = base_row("side_effect_receipts");
        row.insert("replay_safe", Value::Integer(replay_safe));
        // provider_reference is intentionally optional even when replay_safe=1.
        row.insert("provider_reference", Value::Null);
        check_probe(
            &db,
            "side_effect_receipts",
            &row,
            matches!(replay_safe, 0 | 1),
        );
    }
    for column in ["task_id", "idempotency_key"] {
        let mut row = base_row("side_effect_receipts");
        row.insert(
            column,
            if column == "task_id" {
                text("tsk_01JQ8Z9K3M7QWXR4V2T6YH0BNB")
            } else {
                text(&format!("idk_{}", "b".repeat(64)))
            },
        );
        probe(
            &db,
            "side_effect_receipts",
            &row,
            Some(ffi::SQLITE_CONSTRAINT_TRIGGER),
        );
    }
    for status in STATUSES {
        let db = dependencies("task_steps");
        insert(
            &db,
            "task_steps",
            &step(
                if status == "WAITING" {
                    "WAIT_USER"
                } else {
                    "CAPABILITY"
                },
                status,
            ),
        )
        .unwrap();
        probe(
            &db,
            "side_effect_receipts",
            &base_row("side_effect_receipts"),
            if status == "SUCCEEDED" {
                None
            } else {
                Some(ffi::SQLITE_CONSTRAINT_TRIGGER)
            },
        );
    }
    insert(
        &db,
        "side_effect_receipts",
        &base_row("side_effect_receipts"),
    )
    .unwrap();
    let mut duplicate = base_row("side_effect_receipts");
    duplicate.insert("receipt_id", text("rcp_01JQ8Z9M3R2CVN8H5FWK7PQDSG"));
    probe(
        &db,
        "side_effect_receipts",
        &duplicate,
        Some(ffi::SQLITE_CONSTRAINT_UNIQUE),
    );
}

#[test]
fn immutable_policy_and_key_and_monotonic_task_class() {
    let db = dependencies("task_steps");
    insert(&db, "task_steps", &step("CAPABILITY", "PLANNED")).unwrap();
    for sql in [
        "UPDATE tasks SET policy_class_rank = policy_class_rank",
        "UPDATE task_steps SET idempotency_key = idempotency_key",
        "UPDATE tasks SET data_class_rank = data_class_rank",
        "UPDATE tasks SET data_class_rank = 1",
        "UPDATE tasks SET data_class_rank = 2",
    ] {
        assert_result(db.execute(sql, []), None, sql);
    }
    for sql in [
        "UPDATE tasks SET policy_class_rank = 1",
        "UPDATE tasks SET data_class_rank = 1",
        "UPDATE task_steps SET idempotency_key = NULL",
    ] {
        assert_result(
            db.execute(sql, []),
            Some(ffi::SQLITE_CONSTRAINT_TRIGGER),
            sql,
        );
    }
}

#[test]
fn durable_unique_keys_and_null_idempotency_baseline() {
    for table in [
        "plan_revisions",
        "task_blob_refs",
        "step_blob_refs",
        "leases",
        "schema_migrations",
    ] {
        let db = dependencies(table);
        insert(&db, table, &base_row(table)).unwrap();
        probe(
            &db,
            table,
            &base_row(table),
            Some(ffi::SQLITE_CONSTRAINT_PRIMARYKEY),
        );
    }
    let db = dependencies("task_steps");
    insert(&db, "task_steps", &step("CAPABILITY", "PLANNED")).unwrap();
    let mut row = step("CAPABILITY", "PLANNED");
    row.insert("step_id", text("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSG"));
    row.insert("sequence", Value::Integer(1));
    probe(&db, "task_steps", &row, Some(ffi::SQLITE_CONSTRAINT_UNIQUE));
    row = step("MODEL_TURN", "PLANNED");
    row.insert("step_id", text("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSG"));
    probe(&db, "task_steps", &row, Some(ffi::SQLITE_CONSTRAINT_UNIQUE));
    row.insert("sequence", Value::Integer(1));
    insert(&db, "task_steps", &row).unwrap();
    row.insert("step_id", text("stp_01JQ8Z9M3R2CVN8H5FWK7PQDSH"));
    row.insert("sequence", Value::Integer(2));
    probe(&db, "task_steps", &row, None);
    let db = dependencies("task_journal");
    insert(&db, "task_journal", &base_row("task_journal")).unwrap();
    let mut row = base_row("task_journal");
    row.insert("journal_id", text("other-journal"));
    probe(
        &db,
        "task_journal",
        &row,
        Some(ffi::SQLITE_CONSTRAINT_UNIQUE),
    );
}

#[test]
fn strict_integer_and_blob_types_are_not_silently_truncated_or_reinterpreted() {
    for (table, column) in [
        ("schema_migrations", "applied_at_ms"),
        ("blobs", "size_bytes"),
        ("tasks", "max_model_calls"),
        ("task_steps", "attempt"),
        ("side_effect_receipts", "replay_safe"),
        ("leases", "generation"),
        ("plan_revisions", "step_count"),
        ("task_blob_refs", "data_class_rank"),
        ("step_blob_refs", "data_class_rank"),
        ("task_journal", "journal_seq"),
    ] {
        let db = dependencies(table);
        for value in [
            text("not-an-integer"),
            Value::Real(1.5),
            Value::Blob(vec![1]),
        ] {
            let mut row = base_row(table);
            row.insert(column, value);
            probe(&db, table, &row, Some(ffi::SQLITE_CONSTRAINT_DATATYPE));
        }
    }
    let db = schema();
    let mut row = base_row("blobs");
    row.insert("content", text("{}"));
    probe(&db, "blobs", &row, Some(ffi::SQLITE_CONSTRAINT_DATATYPE));
    let mut row = task("READY");
    row.insert("title", Value::Blob(b"schema probe".to_vec()));
    probe(&db, "tasks", &row, Some(ffi::SQLITE_CONSTRAINT_DATATYPE));
}

#[test]
fn json_object_columns_and_optional_error_and_journal_json() {
    let db = schema();
    for column in ["origin_extensions", "budget_extensions", "extensions"] {
        for value in ["{}", "{\"future\":true}", "[]", "null", "1", "not-json"] {
            let mut row = task("READY");
            row.insert(column, text(value));
            check_probe(&db, "tasks", &row, value.starts_with('{'));
        }
    }
    let db = dependencies("task_steps");
    for value in [
        Value::Null,
        text("{}"),
        text("{\"future\":true}"),
        text("[]"),
        text("null"),
        text("not-json"),
    ] {
        let accepted =
            matches!(&value, Value::Null) || matches!(&value, Value::Text(s) if s.starts_with('{'));
        let mut row = step("MODEL_TURN", "FAILED");
        row.insert("error_details", value);
        check_probe(&db, "task_steps", &row, accepted);
    }
    let db = dependencies("task_journal");
    for value in [
        Value::Null,
        text("{}"),
        text("[]"),
        text("null"),
        text("1"),
        text("not-json"),
    ] {
        let accepted = value != text("not-json");
        let mut row = base_row("task_journal");
        row.insert("payload_json", value);
        check_probe(&db, "task_journal", &row, accepted);
    }
}

#[test]
fn representative_identifier_digest_capability_and_code_grammar_constraints() {
    for (table, column) in [
        ("tasks", "task_id"),
        ("task_steps", "step_id"),
        ("side_effect_receipts", "receipt_id"),
        ("blobs", "digest"),
        ("task_steps", "input_digest"),
        ("task_steps", "result_digest"),
        ("task_journal", "payload_digest"),
        ("task_steps", "idempotency_key"),
    ] {
        let db = dependencies(table);
        let mut row = base_row(table);
        row.insert(column, text("wrong-prefix-and-length"));
        check_probe(&db, table, &row, false);
    }
    for (table, column, prefix) in [
        ("blobs", "digest", "sha256:"),
        ("task_steps", "input_digest", "sha256:"),
        ("task_steps", "result_digest", "sha256:"),
        ("task_journal", "payload_digest", "sha256:"),
        ("task_steps", "idempotency_key", "idk_"),
    ] {
        let db = dependencies(table);
        for body in [
            "A".repeat(64),
            "g".repeat(64),
            "a".repeat(63),
            "a".repeat(65),
        ] {
            let mut row = base_row(table);
            row.insert(column, text(&format!("{prefix}{body}")));
            check_probe(&db, table, &row, false);
        }
    }
    let db = dependencies("task_steps");
    for capability in ["goallatch.events.list".to_owned(), "a".repeat(100)] {
        let mut row = base_row("task_steps");
        row.insert("capability_id", text(&capability));
        check_probe(&db, "task_steps", &row, false);
    }
    for (table, column, baseline) in [
        ("tasks", "origin_kind", task("READY")),
        ("tasks", "blocked_reason", task("BLOCKED")),
        ("tasks", "failure_reason", task("FAILED")),
        ("tasks", "cancelled_by", task("CANCELLED")),
        ("task_steps", "error_code", step("MODEL_TURN", "FAILED")),
        (
            "task_steps",
            "error_host_action",
            step("MODEL_TURN", "FAILED"),
        ),
    ] {
        let db = dependencies(table);
        for code in [
            "FUTURE_CODE_9",
            "",
            "lowercase",
            "9FIRST",
            "BAD-CODE",
            "CODE\n",
        ] {
            let mut row = baseline.clone();
            row.insert(column, text(code));
            check_probe(&db, table, &row, code == "FUTURE_CODE_9");
        }
    }
}

#[test]
fn nonnegative_counters_revisions_and_task_instant_ordering() {
    for (table, columns) in [
        (
            "tasks",
            &[
                "max_model_calls",
                "max_tool_calls",
                "max_attempts_per_step",
                "plan_revision",
            ][..],
        ),
        ("task_steps", &["sequence", "plan_revision"][..]),
    ] {
        let db = dependencies(table);
        for column in columns {
            for value in [-1, 0, U32_MAX + 1] {
                let mut row = base_row(table);
                row.insert(column, Value::Integer(value));
                check_probe(&db, table, &row, value >= 0);
            }
        }
    }
    let db = schema();
    for column in ["updated_at_ms", "deadline_at_ms"] {
        for value in [-1, 0, 1] {
            let mut row = task("READY");
            row.insert(column, Value::Integer(value));
            check_probe(&db, "tasks", &row, value >= 0);
        }
    }
}

#[test]
fn removed_lease_token_generation_trigger_and_event_sequence_are_absent() {
    let db = schema();
    for table in strings(
        &db,
        "SELECT name FROM sqlite_schema WHERE type = 'table' ORDER BY name",
    ) {
        let columns = strings(
            &db,
            &format!("SELECT name FROM pragma_table_xinfo('{table}')"),
        );
        for removed in ["token", "lease_token", "event_seq"] {
            assert!(
                !columns.iter().any(|column| column == removed),
                "{table}.{removed}"
            );
        }
    }
    assert!(
        !strings(&db, "SELECT name FROM sqlite_schema")
            .iter()
            .any(|name| name == "leases_generation_matches_step")
    );
    assert!(!MIGRATION.contains("'TASK_DELETED'"));
}

// Extract a range CHECK from the real CREATE TABLE statement. This lets the
// two mathematically impossible lease endpoints be tested for range admission
// without weakening the table or inventing a second DDL. All other endpoints
// are additionally inserted into the fully constrained production schema.
fn epoch_check(table: &str, column: &str) -> String {
    let table_sql = MIGRATION
        .split_once(&format!("CREATE TABLE {table} ("))
        .unwrap()
        .1
        .split_once(") STRICT;")
        .unwrap()
        .0;
    let mut checks = Vec::new();

    for (offset, _) in table_sql.match_indices("CHECK") {
        let rest = table_sql[offset + "CHECK".len()..].trim_start();
        assert!(rest.starts_with('('));
        let mut depth = 0;
        let mut end = None;
        for (index, character) in rest.char_indices() {
            match character {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let expression = &rest[1..end.expect("balanced production CHECK")];
        if expression.contains(column)
            && expression.contains(&MIN.to_string())
            && expression.contains(&MAX.to_string())
        {
            checks.push(expression.to_owned());
        }
    }
    assert_eq!(
        checks.len(),
        1,
        "one bounded EpochMillis CHECK for {table}.{column}"
    );
    checks.pop().unwrap()
}

fn instant_row(table: &str, column: &'static str, value: Value) -> Row {
    let mut row = base_row(table);
    if table == "tasks" {
        row = task(if column == "cancelled_at_ms" && value != Value::Null {
            "CANCELLED"
        } else {
            "READY"
        });
        row.insert("created_at_ms", Value::Integer(MIN));
        row.insert("updated_at_ms", Value::Integer(MAX));
        if column == "created_at_ms" {
            row.insert("updated_at_ms", value.clone());
        }
    }
    if table == "task_steps" {
        row = if value == Value::Null {
            step("MODEL_TURN", "PLANNED")
        } else if column == "lease_expires_at_ms" {
            step("MODEL_TURN", "LEASED")
        } else {
            step("MODEL_TURN", "SUCCEEDED")
        };
        if value != Value::Null && column == "started_at_ms" {
            row.insert("completed_at_ms", Value::Integer(MAX));
        }
        if value != Value::Null && column == "completed_at_ms" {
            row.insert("started_at_ms", Value::Integer(MIN));
        }
    }
    if table == "leases" {
        row.insert("acquired_at_ms", Value::Integer(MIN));
        row.insert("expires_at_ms", Value::Integer(MAX));
    }
    row.insert(column, value);
    row
}

#[test]
fn exact_14_instant_columns_and_nullable_inventory() {
    let db = schema();
    let mut actual = Vec::new();
    for table in strings(&db, "SELECT name FROM sqlite_schema WHERE type = 'table'") {
        let mut stmt = db.prepare(&format!("SELECT name, \"notnull\", type FROM pragma_table_xinfo('{table}') WHERE name LIKE '%_at_ms'")).unwrap();
        for result in stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, bool>(1)?,
                    r.get::<_, String>(2)?,
                ))
            })
            .unwrap()
        {
            let (column, not_null, storage_type) = result.unwrap();
            assert_eq!(storage_type, "INTEGER", "{table}.{column}");
            actual.push((table.clone(), column, !not_null));
        }
    }
    actual.sort();
    let mut expected = INSTANTS
        .map(|(table, column, nullable)| (table.to_owned(), column.to_owned(), nullable))
        .to_vec();
    expected.sort();
    assert_eq!(actual, expected);
}

#[test]
fn every_production_epoch_check_admits_min_max_and_refuses_adjacent_outside_values() {
    let db = schema();
    for (table, column, nullable) in INSTANTS {
        let check = epoch_check(table, column);
        for (value, expected) in [(MIN - 1, false), (MIN, true), (MAX, true), (MAX + 1, false)] {
            let accepted: bool = db
                .query_row(
                    &format!("SELECT ({check}) FROM (SELECT ? AS {column})"),
                    [value],
                    |r| r.get(0),
                )
                .unwrap();
            assert_eq!(
                accepted, expected,
                "production CHECK {table}.{column} at {value}"
            );
        }
        if nullable {
            let accepted: Option<bool> = db
                .query_row(
                    &format!("SELECT ({check}) FROM (SELECT NULL AS {column})"),
                    [],
                    |r| r.get(0),
                )
                .unwrap();
            assert_ne!(
                accepted,
                Some(false),
                "SQL CHECK must preserve NULL for {table}.{column}"
            );
        }
    }
}

#[test]
fn instant_boundaries_round_trip_under_all_production_relational_constraints() {
    for (table, column, nullable) in INSTANTS {
        let db = dependencies(table);
        for value in [MIN, MAX] {
            let impossible = table == "leases"
                && ((column == "acquired_at_ms" && value == MAX)
                    || (column == "expires_at_ms" && value == MIN));
            if impossible {
                // Range admission is covered above; these persisted rows must
                // fail expiry > acquisition, not relax it to fake acceptance.
                check_probe(
                    &db,
                    table,
                    &instant_row(table, column, Value::Integer(value)),
                    false,
                );
                continue;
            }
            db.execute_batch("SAVEPOINT instant;").unwrap();
            insert(
                &db,
                table,
                &instant_row(table, column, Value::Integer(value)),
            )
            .unwrap_or_else(|error| panic!("{table}.{column} at {value}: {error}"));
            let stored: i64 = db
                .query_row(&format!("SELECT {column} FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(stored, value, "{table}.{column}");
            db.execute_batch("ROLLBACK TO instant; RELEASE instant;")
                .unwrap();
        }
        for value in [MIN - 1, MAX + 1] {
            let mut row = instant_row(table, column, Value::Integer(value));
            // Valid relative neighbors ensure an out-of-domain CHECK, rather
            // than the ordering invariant, causes the rejection. For the two
            // impossible lease edges the neighbor is also outside the domain;
            // each column's individual range rejection is verified above.
            if table == "leases" && column == "acquired_at_ms" && value == MAX + 1 {
                row.insert("expires_at_ms", Value::Integer(value + 1));
            }
            if table == "leases" && column == "expires_at_ms" && value == MIN - 1 {
                row.insert("acquired_at_ms", Value::Integer(value - 1));
            }
            if table == "tasks" && column == "updated_at_ms" && value == MIN - 1 {
                row.insert("created_at_ms", Value::Integer(value));
            }
            if table == "tasks" && column == "deadline_at_ms" && value == MIN - 1 {
                row.insert("created_at_ms", Value::Integer(value));
            }
            check_probe(&db, table, &row, false);
        }
        db.execute_batch("SAVEPOINT null_instant;").unwrap();
        let row = instant_row(table, column, Value::Null);
        assert_result(
            insert(&db, table, &row),
            if nullable {
                None
            } else {
                Some(ffi::SQLITE_CONSTRAINT_NOTNULL)
            },
            &format!("{table}.{column} NULL"),
        );
        if nullable {
            let stored: Option<i64> = db
                .query_row(&format!("SELECT {column} FROM {table}"), [], |r| r.get(0))
                .unwrap();
            assert_eq!(
                stored, None,
                "{table}.{column}: SQL NULL is absence, not zero"
            );
        }
        db.execute_batch("ROLLBACK TO null_instant; RELEASE null_instant;")
            .unwrap();
    }
    let db = dependencies("leases");
    for (acquired, expires) in [(MIN, MIN + 1), (MAX - 1, MAX)] {
        let mut row = base_row("leases");
        row.insert("acquired_at_ms", Value::Integer(acquired));
        row.insert("expires_at_ms", Value::Integer(expires));
        probe(&db, "leases", &row, None);
    }
}
