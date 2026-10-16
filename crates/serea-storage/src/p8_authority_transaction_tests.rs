//! Test-only P8B transaction mechanics prototype.
//!
//! This intentionally uses synthetic tables and is not an implementation of
//! the production authority model. It is compiled only with `cfg(test)` and
//! has no public or Host-reachable API. Its purpose is to prove that the
//! grant-use, budget, IDK, RequestId, intent and event writes can be grouped
//! into one SQLite transaction and that failures leave no partial state.

use rusqlite::{Connection, TransactionBehavior, params};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_DB: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FaultPoint {
    None,
    AfterGrantAndBudget,
    AfterIntent,
    EventInsert,
    BeforeCommit,
}

#[derive(Debug)]
struct DispatchAttempt<'a> {
    task_id: &'a str,
    step_id: &'a str,
    grant_id: &'a str,
    request_id: &'a str,
    idk: &'a str,
    now_ms: i64,
    lease_owner: &'a str,
    lease_generation: i64,
    plan_revision: i64,
    prepared_digest: &'a str,
    policy_revision: i64,
    registry_generation: i64,
    descriptor_digest: &'a str,
    overlay_revision: i64,
}

#[derive(Debug, PartialEq, Eq)]
enum Refusal {
    TaskUnavailable,
    Cancelled,
    Deadline,
    LeaseMismatch,
    PlanMismatch,
    PolicyMismatch,
    GrantUnavailable,
    RegistryMismatch,
    OverlayMismatch,
    ProviderUnavailable,
    BudgetExceeded,
    Duplicate,
    InjectedFailure,
}

struct AuthorityDb(PathBuf);

impl AuthorityDb {
    fn new() -> Self {
        let id = NEXT_DB.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!(
            "serea-p8-authority-prototype-{}-{id}",
            std::process::id()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("authority.sqlite");
        let db = Self(path);
        db.seed();
        db
    }

    fn connect(&self) -> Connection {
        let conn = Connection::open(&self.0).unwrap();
        conn.pragma_update(None, "foreign_keys", "ON").unwrap();
        conn.busy_timeout(std::time::Duration::from_secs(2))
            .unwrap();
        conn
    }

    fn seed(&self) {
        let conn = self.connect();
        conn.execute_batch(
            "CREATE TABLE tasks(task_id TEXT PRIMARY KEY,state TEXT NOT NULL,cancelled INTEGER NOT NULL,deadline_ms INTEGER NOT NULL,plan_revision INTEGER NOT NULL);
             CREATE TABLE steps(step_id TEXT PRIMARY KEY,task_id TEXT NOT NULL REFERENCES tasks(task_id),status TEXT NOT NULL,lease_owner TEXT NOT NULL,lease_generation INTEGER NOT NULL,lease_expires_ms INTEGER NOT NULL,plan_revision INTEGER NOT NULL,prepared_digest TEXT NOT NULL);
             CREATE TABLE grants(grant_id TEXT PRIMARY KEY,step_id TEXT NOT NULL,status TEXT NOT NULL,expires_ms INTEGER NOT NULL,uses_remaining INTEGER NOT NULL);
             CREATE TABLE policies(revision INTEGER PRIMARY KEY,decision TEXT NOT NULL);
             CREATE TABLE registry(generation INTEGER PRIMARY KEY,descriptor_digest TEXT NOT NULL,available INTEGER NOT NULL);
             CREATE TABLE overlays(revision INTEGER PRIMARY KEY);
             CREATE TABLE budgets(task_id TEXT PRIMARY KEY REFERENCES tasks(task_id),limit_count INTEGER NOT NULL,used_count INTEGER NOT NULL);
             CREATE TABLE idk_reservations(idk TEXT PRIMARY KEY,step_id TEXT NOT NULL UNIQUE);
             CREATE TABLE dispatch_intents(request_id TEXT PRIMARY KEY,idk TEXT NOT NULL UNIQUE,step_id TEXT NOT NULL UNIQUE,task_id TEXT NOT NULL,grant_id TEXT NOT NULL);
             CREATE TABLE events(event_id TEXT PRIMARY KEY,request_id TEXT NOT NULL UNIQUE);
             INSERT INTO tasks VALUES('task-a','EXECUTING',0,1000,7);
             INSERT INTO steps VALUES('step-a','task-a','LEASED','worker-a',4,900,7,'sha256:prepared');
             INSERT INTO grants VALUES('grant-a','step-a','ACTIVE',800,2);
             INSERT INTO policies VALUES(11,'ALLOW');
             INSERT INTO registry VALUES(3,'sha256:descriptor',1);
             INSERT INTO overlays VALUES(5);
             INSERT INTO budgets VALUES('task-a',2,0);",
        )
        .unwrap();
    }

    /// The test-only authority transaction contains no provider I/O. It makes
    /// no production authorization claim and returns only after commit.
    fn commit(&self, a: &DispatchAttempt<'_>, fault: FaultPoint) -> Result<(), Refusal> {
        let mut conn = self.connect();
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| Refusal::InjectedFailure)?;

        let task: (String, bool, i64, i64) = tx
            .query_row(
                "SELECT state,cancelled,deadline_ms,plan_revision FROM tasks WHERE task_id=?1",
                [a.task_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|_| Refusal::TaskUnavailable)?;
        if task.0 != "EXECUTING" {
            return Err(Refusal::TaskUnavailable);
        }
        if task.1 {
            return Err(Refusal::Cancelled);
        }
        if a.now_ms >= task.2 {
            return Err(Refusal::Deadline);
        }
        if task.3 != a.plan_revision {
            return Err(Refusal::PlanMismatch);
        }

        let step: (String, String, i64, i64, i64, String) = tx
            .query_row(
                "SELECT status,lease_owner,lease_generation,lease_expires_ms,plan_revision,prepared_digest FROM steps WHERE step_id=?1 AND task_id=?2",
                params![a.step_id, a.task_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .map_err(|_| Refusal::TaskUnavailable)?;
        if step.0 != "LEASED"
            || step.1 != a.lease_owner
            || step.2 != a.lease_generation
            || step.3 <= a.now_ms
        {
            return Err(Refusal::LeaseMismatch);
        }
        if step.4 != a.plan_revision || step.5 != a.prepared_digest {
            return Err(Refusal::PlanMismatch);
        }

        let policy: (i64, String) = tx
            .query_row("SELECT revision,decision FROM policies LIMIT 1", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .map_err(|_| Refusal::PolicyMismatch)?;
        if policy.0 != a.policy_revision || policy.1 != "ALLOW" {
            return Err(Refusal::PolicyMismatch);
        }
        let grant: (String, i64, i64, String) = tx
            .query_row(
                "SELECT status,expires_ms,uses_remaining,step_id FROM grants WHERE grant_id=?1",
                [a.grant_id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .map_err(|_| Refusal::GrantUnavailable)?;
        if grant.0 != "ACTIVE" || grant.1 <= a.now_ms || grant.2 <= 0 || grant.3 != a.step_id {
            return Err(Refusal::GrantUnavailable);
        }
        let registry: (String, bool) = tx
            .query_row(
                "SELECT descriptor_digest,available FROM registry WHERE generation=?1",
                [a.registry_generation],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| Refusal::RegistryMismatch)?;
        if registry.0 != a.descriptor_digest {
            return Err(Refusal::RegistryMismatch);
        }
        if !registry.1 {
            return Err(Refusal::ProviderUnavailable);
        }
        let overlay_exists: bool = tx
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM overlays WHERE revision=?1)",
                [a.overlay_revision],
                |r| r.get(0),
            )
            .map_err(|_| Refusal::OverlayMismatch)?;
        if !overlay_exists {
            return Err(Refusal::OverlayMismatch);
        }
        let (used, limit): (i64, i64) = tx
            .query_row(
                "SELECT used_count,limit_count FROM budgets WHERE task_id=?1",
                [a.task_id],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| Refusal::BudgetExceeded)?;
        if used >= limit {
            return Err(Refusal::BudgetExceeded);
        }

        // Grant use, budget reservation, IDK, RequestId, Intent and Event share
        // this one write transaction. Any injected failure drops/rolls it back.
        tx.execute(
            "UPDATE grants SET uses_remaining=uses_remaining-1 WHERE grant_id=?1",
            [a.grant_id],
        )
        .map_err(|_| Refusal::GrantUnavailable)?;
        tx.execute(
            "UPDATE budgets SET used_count=used_count+1 WHERE task_id=?1",
            [a.task_id],
        )
        .map_err(|_| Refusal::BudgetExceeded)?;
        if fault == FaultPoint::AfterGrantAndBudget {
            return Err(Refusal::InjectedFailure);
        }
        tx.execute(
            "INSERT INTO idk_reservations VALUES(?1,?2)",
            params![a.idk, a.step_id],
        )
        .map_err(|_| Refusal::Duplicate)?;
        tx.execute(
            "INSERT INTO dispatch_intents VALUES(?1,?2,?3,?4,?5)",
            params![a.request_id, a.idk, a.step_id, a.task_id, a.grant_id],
        )
        .map_err(|_| Refusal::Duplicate)?;
        if fault == FaultPoint::AfterIntent {
            return Err(Refusal::InjectedFailure);
        }
        if fault == FaultPoint::EventInsert {
            return Err(Refusal::InjectedFailure);
        }
        tx.execute(
            "INSERT INTO events VALUES(?1,?2)",
            params![format!("event-{}", a.request_id), a.request_id],
        )
        .map_err(|_| Refusal::Duplicate)?;
        if fault == FaultPoint::BeforeCommit {
            return Err(Refusal::InjectedFailure);
        }
        tx.commit().map_err(|_| Refusal::InjectedFailure)
    }

    fn counts(&self) -> (i64, i64, i64, i64, i64) {
        let conn = self.connect();
        conn.query_row(
            "SELECT (SELECT uses_remaining FROM grants WHERE grant_id='grant-a'),(SELECT used_count FROM budgets WHERE task_id='task-a'),(SELECT count(*) FROM idk_reservations),(SELECT count(*) FROM dispatch_intents),(SELECT count(*) FROM events)",
            [],
            |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)),
        ).unwrap()
    }
}

impl Drop for AuthorityDb {
    fn drop(&mut self) {
        std::fs::remove_dir_all(self.0.parent().unwrap()).unwrap();
    }
}

fn attempt() -> DispatchAttempt<'static> {
    DispatchAttempt {
        task_id: "task-a",
        step_id: "step-a",
        grant_id: "grant-a",
        request_id: "request-a",
        idk: "idk-a",
        now_ms: 100,
        lease_owner: "worker-a",
        lease_generation: 4,
        plan_revision: 7,
        prepared_digest: "sha256:prepared",
        policy_revision: 11,
        registry_generation: 3,
        descriptor_digest: "sha256:descriptor",
        overlay_revision: 5,
    }
}

#[test]
fn p8_test_transaction_rolls_back_every_write_at_each_injected_boundary() {
    for fault in [
        FaultPoint::AfterGrantAndBudget,
        FaultPoint::AfterIntent,
        FaultPoint::EventInsert,
        FaultPoint::BeforeCommit,
    ] {
        let db = AuthorityDb::new();
        assert_eq!(db.commit(&attempt(), fault), Err(Refusal::InjectedFailure));
        // A distinct connection observes the rolled-back durable state.
        assert_eq!(db.counts(), (2, 0, 0, 0, 0), "fault point: {fault:?}");
    }
}

#[test]
fn p8_test_transaction_commits_authority_rows_and_event_together() {
    let db = AuthorityDb::new();
    assert_eq!(db.commit(&attempt(), FaultPoint::None), Ok(()));
    assert_eq!(db.counts(), (1, 1, 1, 1, 1));
    assert_eq!(
        db.commit(&attempt(), FaultPoint::None),
        Err(Refusal::Duplicate)
    );
    assert_eq!(db.counts(), (1, 1, 1, 1, 1));
}

#[test]
fn p8_test_transaction_revalidates_cancellation_deadline_lease_policy_registry_and_budget() {
    let cases: [(&str, &str, i64); 6] = [
        (
            "cancelled",
            "UPDATE tasks SET cancelled=1 WHERE task_id='task-a'",
            100,
        ),
        (
            "deadline",
            "UPDATE tasks SET deadline_ms=100 WHERE task_id='task-a'",
            100,
        ),
        (
            "lease",
            "UPDATE steps SET lease_generation=5 WHERE step_id='step-a'",
            100,
        ),
        ("policy", "UPDATE policies SET revision=12", 100),
        (
            "registry",
            "UPDATE registry SET descriptor_digest='sha256:changed'",
            100,
        ),
        (
            "budget",
            "UPDATE budgets SET used_count=2 WHERE task_id='task-a'",
            100,
        ),
    ];
    for (name, update, now) in cases {
        let db = AuthorityDb::new();
        db.connect().execute_batch(update).unwrap();
        let mut a = attempt();
        a.now_ms = now;
        let expected = match name {
            "cancelled" => Refusal::Cancelled,
            "deadline" => Refusal::Deadline,
            "lease" => Refusal::LeaseMismatch,
            "policy" => Refusal::PolicyMismatch,
            "registry" => Refusal::RegistryMismatch,
            "budget" => Refusal::BudgetExceeded,
            _ => unreachable!(),
        };
        assert_eq!(
            db.commit(&a, FaultPoint::None),
            Err(expected),
            "case {name}"
        );
        assert_eq!(
            db.counts(),
            (2, if name == "budget" { 2 } else { 0 }, 0, 0, 0)
        );
    }
}
