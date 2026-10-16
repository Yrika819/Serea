//! Disabled synthetic extraction admission and final-commit fixture.
//!
//! This deliberately does not call TaskEngine or a model. `BEGIN IMMEDIATE`
//! places the generation comparison and final SQLite insert in one commit
//! order with Forget. Expiry is driven by a persisted nondecreasing UTC clock;
//! backward clock movement fails closed, and UTC-day quota buckets never move
//! backward. This is a fixture contract, not a production lease authority.
use rusqlite::{Connection, OptionalExtension, Transaction, TransactionBehavior, params};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(1);
fn scratch() -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "serea-p7-extraction-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

fn cleanup(path: std::path::PathBuf) {
    let _ = std::fs::remove_dir_all(path);
}

#[test]
fn extraction_fence_child_entry() {
    let Ok(path) = std::env::var("SEREA_P7_EXTRACTION_CHILD_DB") else {
        return;
    };
    let owner = std::env::var("SEREA_P7_EXTRACTION_CHILD_OWNER").unwrap();
    let operation = std::env::var("SEREA_P7_EXTRACTION_CHILD_OP").unwrap();
    let now = std::env::var("SEREA_P7_EXTRACTION_CHILD_NOW")
        .unwrap()
        .parse()
        .unwrap();
    let fence = ExtractionFence::open(Path::new(&path), &owner);
    let result = fence.admit(&owner, &operation, now);
    println!("RESULT={result:?}");
    if std::env::var_os("SEREA_P7_EXTRACTION_CHILD_CRASH").is_some() && result.is_ok() {
        std::process::exit(73);
    }
    if std::env::var_os("SEREA_P7_EXTRACTION_CHILD_FINAL").is_some() {
        let result = fence.final_commit(&owner, &operation, "crash-memory", "synthetic", now);
        println!("FINAL={result:?}");
    }
}

fn spawn_extraction_child(
    path: &Path,
    operation: &str,
    now: i64,
    crash: bool,
) -> std::process::Child {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .arg("--exact")
        .arg("p7_extraction_fence_tests::extraction_fence_child_entry")
        .arg("--nocapture")
        .env("SEREA_P7_EXTRACTION_CHILD_DB", path)
        .env("SEREA_P7_EXTRACTION_CHILD_OWNER", "owner-a")
        .env("SEREA_P7_EXTRACTION_CHILD_OP", operation)
        .env("SEREA_P7_EXTRACTION_CHILD_NOW", now.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    if crash {
        command.env("SEREA_P7_EXTRACTION_CHILD_CRASH", "1");
    }
    command.spawn().unwrap()
}

const LEASE_TTL_MS: i64 = 60_000;
const DAILY_LIMIT: i64 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FenceError {
    WrongOwner,
    ClockRollback,
    Quota,
    Concurrent,
    Duplicate,
    StaleGeneration,
    Expired,
    Invalid,
}

struct ExtractionFence {
    db: Mutex<Connection>,
    owner: String,
}

impl ExtractionFence {
    fn open(path: &Path, owner: &str) -> Self {
        let db = Connection::open(path).unwrap();
        db.busy_timeout(std::time::Duration::from_secs(5)).unwrap();
        db.execute_batch(
            "PRAGMA journal_mode=WAL;
             CREATE TABLE IF NOT EXISTS extraction_owner(
                 owner TEXT PRIMARY KEY, generation INTEGER NOT NULL,
                 last_observed_ms INTEGER NOT NULL);
             CREATE TABLE IF NOT EXISTS extraction_daily(
                 owner TEXT NOT NULL, utc_day INTEGER NOT NULL, starts INTEGER NOT NULL,
                 PRIMARY KEY(owner,utc_day));
             CREATE TABLE IF NOT EXISTS extraction_lease(
                 operation_id TEXT PRIMARY KEY, owner TEXT NOT NULL,
                 captured_generation INTEGER NOT NULL, admitted_ms INTEGER NOT NULL,
                 expires_ms INTEGER NOT NULL, utc_day INTEGER NOT NULL,
                 state TEXT NOT NULL CHECK(state IN ('ACTIVE','COMMITTED','EXPIRED','FENCED')));
             CREATE TABLE IF NOT EXISTS extracted_fixture(
                 owner TEXT NOT NULL, operation_id TEXT NOT NULL UNIQUE,
                 memory_id TEXT NOT NULL, body TEXT NOT NULL,
                 PRIMARY KEY(owner,memory_id));",
        )
        .unwrap();
        db.execute(
            "INSERT OR IGNORE INTO extraction_owner(owner,generation,last_observed_ms) VALUES(?1,0,0)",
            [owner],
        ).unwrap();
        Self {
            db: Mutex::new(db),
            owner: owner.to_owned(),
        }
    }

    fn begin_clock(tx: &Transaction<'_>, owner: &str, now: i64) -> Result<(i64, i64), FenceError> {
        let (generation, last): (i64, i64) = tx
            .query_row(
                "SELECT generation,last_observed_ms FROM extraction_owner WHERE owner=?1",
                [owner],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()
            .map_err(|_| FenceError::WrongOwner)?
            .ok_or(FenceError::WrongOwner)?;
        if now < last {
            return Err(FenceError::ClockRollback);
        }
        tx.execute(
            "UPDATE extraction_owner SET last_observed_ms=?2 WHERE owner=?1",
            params![owner, now],
        )
        .map_err(|_| FenceError::Invalid)?;
        Ok((generation, last))
    }

    fn admit(&self, owner: &str, operation_id: &str, now: i64) -> Result<u64, FenceError> {
        if owner != self.owner || operation_id.is_empty() {
            return Err(FenceError::WrongOwner);
        }
        let mut db = self.db.lock().unwrap();
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| FenceError::Invalid)?;
        let (generation, _) = Self::begin_clock(&tx, owner, now)?;
        tx.execute("UPDATE extraction_lease SET state='EXPIRED' WHERE owner=?1 AND state='ACTIVE' AND expires_ms<=?2", params![owner,now]).map_err(|_| FenceError::Invalid)?;
        let active: i64 = tx
            .query_row(
                "SELECT count(*) FROM extraction_lease WHERE owner=?1 AND state='ACTIVE'",
                [owner],
                |r| r.get(0),
            )
            .map_err(|_| FenceError::Invalid)?;
        if active >= 1 {
            tx.commit().map_err(|_| FenceError::Invalid)?;
            return Err(FenceError::Concurrent);
        }
        let day = now.div_euclid(86_400_000);
        let starts: i64 = tx
            .query_row(
                "SELECT starts FROM extraction_daily WHERE owner=?1 AND utc_day=?2",
                params![owner, day],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| FenceError::Invalid)?
            .unwrap_or(0);
        if starts >= DAILY_LIMIT {
            tx.commit().map_err(|_| FenceError::Invalid)?;
            return Err(FenceError::Quota);
        }
        let Some(expires) = now.checked_add(LEASE_TTL_MS) else {
            tx.commit().map_err(|_| FenceError::Invalid)?;
            return Err(FenceError::Invalid);
        };
        if tx.execute("INSERT INTO extraction_lease(operation_id,owner,captured_generation,admitted_ms,expires_ms,utc_day,state) VALUES(?1,?2,?3,?4,?5,?6,'ACTIVE')", params![operation_id,owner,generation,now,expires,day]).is_err() {
            tx.commit().map_err(|_| FenceError::Invalid)?;
            return Err(FenceError::Duplicate);
        }
        tx.execute("INSERT INTO extraction_daily(owner,utc_day,starts) VALUES(?1,?2,1) ON CONFLICT(owner,utc_day) DO UPDATE SET starts=starts+1", params![owner,day]).map_err(|_| FenceError::Invalid)?;
        tx.commit().map_err(|_| FenceError::Invalid)?;
        Ok(generation as u64)
    }

    fn forget(&self, owner: &str, now: i64) -> Result<u64, FenceError> {
        if owner != self.owner {
            return Err(FenceError::WrongOwner);
        }
        let mut db = self.db.lock().unwrap();
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| FenceError::Invalid)?;
        let (generation, _) = Self::begin_clock(&tx, owner, now)?;
        let next = generation.checked_add(1).ok_or(FenceError::Invalid)?;
        tx.execute("DELETE FROM extracted_fixture WHERE owner=?1", [owner])
            .map_err(|_| FenceError::Invalid)?;
        tx.execute(
            "UPDATE extraction_owner SET generation=?2 WHERE owner=?1",
            params![owner, next],
        )
        .map_err(|_| FenceError::Invalid)?;
        tx.execute(
            "UPDATE extraction_lease SET state='FENCED' WHERE owner=?1 AND state='ACTIVE'",
            [owner],
        )
        .map_err(|_| FenceError::Invalid)?;
        tx.commit().map_err(|_| FenceError::Invalid)?;
        Ok(next as u64)
    }

    fn final_commit(
        &self,
        owner: &str,
        operation_id: &str,
        memory_id: &str,
        body: &str,
        now: i64,
    ) -> Result<(), FenceError> {
        if owner != self.owner {
            return Err(FenceError::WrongOwner);
        }
        let mut db = self.db.lock().unwrap();
        let tx = db
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|_| FenceError::Invalid)?;
        let (generation, _) = Self::begin_clock(&tx, owner, now)?;
        let lease: Option<(i64,i64,String)> = tx.query_row("SELECT captured_generation,expires_ms,state FROM extraction_lease WHERE operation_id=?1 AND owner=?2", params![operation_id,owner], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).optional().map_err(|_| FenceError::Invalid)?;
        let (captured, expires, state) = lease.ok_or(FenceError::WrongOwner)?;
        if state != "ACTIVE" || captured != generation {
            tx.commit().map_err(|_| FenceError::Invalid)?;
            return Err(FenceError::StaleGeneration);
        }
        if now >= expires {
            tx.execute(
                "UPDATE extraction_lease SET state='EXPIRED' WHERE operation_id=?1",
                [operation_id],
            )
            .map_err(|_| FenceError::Invalid)?;
            tx.commit().map_err(|_| FenceError::Invalid)?;
            return Err(FenceError::Expired);
        }
        if tx.execute(
            "INSERT INTO extracted_fixture(owner,operation_id,memory_id,body) VALUES(?1,?2,?3,?4)",
            params![owner, operation_id, memory_id, body],
        ).is_err() {
            tx.commit().map_err(|_| FenceError::Invalid)?;
            return Err(FenceError::Duplicate);
        }
        if std::env::var_os("SEREA_P7_EXTRACTION_CRASH_BEFORE_COMMIT").is_some() {
            std::process::exit(74);
        }
        tx.execute(
            "UPDATE extraction_lease SET state='COMMITTED' WHERE operation_id=?1",
            [operation_id],
        )
        .map_err(|_| FenceError::Invalid)?;
        tx.commit().map_err(|_| FenceError::Invalid)
    }

    fn row_count(&self) -> i64 {
        self.db
            .lock()
            .unwrap()
            .query_row("SELECT count(*) FROM extracted_fixture", [], |r| r.get(0))
            .unwrap()
    }
    fn starts(&self, day: i64) -> i64 {
        self.db
            .lock()
            .unwrap()
            .query_row(
                "SELECT starts FROM extraction_daily WHERE owner=?1 AND utc_day=?2",
                params![self.owner, day],
                |r| r.get(0),
            )
            .optional()
            .unwrap()
            .unwrap_or(0)
    }
}

#[test]
fn extraction_started_before_forget_cannot_reinsert_stale_data() {
    let dir = scratch();
    let f = ExtractionFence::open(&dir.join("fixture.sqlite"), "owner-a");
    assert_eq!(f.admit("owner-a", "op-a", 1_000), Ok(0));
    assert_eq!(f.forget("owner-a", 1_001), Ok(1));
    assert_eq!(
        f.final_commit("owner-a", "op-a", "memory-a", "synthetic", 1_002),
        Err(FenceError::StaleGeneration)
    );
    assert_eq!(f.row_count(), 0);
    assert_eq!(f.starts(0), 1);
    drop(f);
    cleanup(dir);
}

#[test]
fn extraction_admission_serializes_forget_race_and_quota_is_exact() {
    let dir = scratch();
    let path = dir.join("fixture.sqlite");
    let f = ExtractionFence::open(&path, "owner-a");
    assert_eq!(f.admit("owner-a", "op-a", 1_000), Ok(0));
    assert_eq!(
        f.admit("owner-a", "op-b", 1_001),
        Err(FenceError::Concurrent)
    );
    assert_eq!(
        f.admit("owner-b", "op-c", 1_002),
        Err(FenceError::WrongOwner)
    );
    assert_eq!(
        f.admit("owner-a", "op-a", 1_003),
        Err(FenceError::Concurrent)
    );
    assert_eq!(
        f.final_commit("owner-a", "op-a", "memory-a", "body", 1_004),
        Ok(())
    );
    assert_eq!(f.admit("owner-a", "op-b", 1_005), Ok(0));
    assert_eq!(f.starts(0), 2);
    assert_eq!(
        f.final_commit("owner-a", "op-b", "memory-b", "body", 1_006),
        Ok(())
    );
    assert_eq!(
        f.admit("owner-a", "op-a", 1_007),
        Err(FenceError::Duplicate)
    );
    assert_eq!(f.row_count(), 2);
    drop(f);
    cleanup(dir);
}

#[test]
fn extraction_clock_utc_boundary_and_abandoned_lease_expiry_are_deterministic() {
    let dir = scratch();
    let f = ExtractionFence::open(&dir.join("fixture.sqlite"), "owner-a");
    let boundary = 86_400_000;
    assert_eq!(f.admit("owner-a", "op-day-0", boundary - 1), Ok(0));
    assert_eq!(
        f.admit("owner-a", "op-backward", boundary - 2),
        Err(FenceError::ClockRollback)
    );
    assert_eq!(
        f.admit("owner-a", "op-day-1", boundary),
        Err(FenceError::Concurrent)
    );
    assert_eq!(
        f.admit("owner-a", "op-expired", boundary + LEASE_TTL_MS),
        Ok(0)
    );
    assert_eq!(f.starts(0), 1);
    assert_eq!(f.starts(1), 1);
    drop(f);
    cleanup(dir);
}

#[test]
fn extraction_generation_check_is_inside_final_sqlite_commit() {
    let dir = scratch();
    let f = ExtractionFence::open(&dir.join("fixture.sqlite"), "owner-a");
    assert_eq!(f.admit("owner-a", "op-a", 10), Ok(0));
    assert_eq!(
        f.final_commit("owner-a", "op-a", "memory-a", "body", 11),
        Ok(())
    );
    assert_eq!(f.forget("owner-a", 12), Ok(1));
    assert_eq!(f.row_count(), 0);
    drop(f);
    cleanup(dir);
}

#[test]
fn extraction_reopens_after_admission_and_expires_abandoned_lease() {
    let dir = scratch();
    let path = dir.join("fixture.sqlite");
    let first = ExtractionFence::open(&path, "owner-a");
    assert_eq!(first.admit("owner-a", "crashed-op", 100), Ok(0));
    drop(first); // models process termination after durable admission
    let reopened = ExtractionFence::open(&path, "owner-a");
    assert_eq!(
        reopened.admit("owner-a", "blocked", 101),
        Err(FenceError::Concurrent)
    );
    assert_eq!(
        reopened.admit("owner-a", "after-expiry", 100 + LEASE_TTL_MS),
        Ok(0)
    );
    assert_eq!(reopened.starts(0), 2);
    drop(reopened);
    cleanup(dir);
}

#[test]
fn extraction_daily_quota_stops_at_twenty_without_counter_drift() {
    let dir = scratch();
    let path = dir.join("fixture.sqlite");
    let f = ExtractionFence::open(&path, "owner-a");
    for n in 0..DAILY_LIMIT {
        let operation = format!("quota-{n}");
        let now = n * (LEASE_TTL_MS + 1) + 1;
        assert_eq!(f.admit("owner-a", &operation, now), Ok(0));
        assert_eq!(
            f.final_commit(
                "owner-a",
                &operation,
                &format!("memory-{n}"),
                "synthetic",
                now + 1
            ),
            Ok(())
        );
    }
    let blocked_at = DAILY_LIMIT * (LEASE_TTL_MS + 1) + 1;
    assert_eq!(
        f.admit("owner-a", "quota-21", blocked_at),
        Err(FenceError::Quota)
    );
    assert_eq!(f.starts(0), DAILY_LIMIT);
    assert_eq!(f.row_count(), DAILY_LIMIT);
    drop(f);
    cleanup(dir);
}

#[test]
fn independent_sqlite_connections_serialize_simultaneous_admission() {
    use std::sync::{Arc, Barrier};
    use std::thread;
    let dir = scratch();
    let path = dir.join("fixture.sqlite");
    let a = Arc::new(ExtractionFence::open(&path, "owner-a"));
    let b = Arc::new(ExtractionFence::open(&path, "owner-a"));
    let barrier = Arc::new(Barrier::new(2));
    let first = {
        let a = Arc::clone(&a);
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            a.admit("owner-a", "process-a", 1_000)
        })
    };
    let second = {
        let b = Arc::clone(&b);
        let barrier = Arc::clone(&barrier);
        thread::spawn(move || {
            barrier.wait();
            b.admit("owner-a", "process-b", 1_000)
        })
    };
    let outcomes = [first.join().unwrap(), second.join().unwrap()];
    assert_eq!(outcomes.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        outcomes
            .iter()
            .filter(|result| **result == Err(FenceError::Concurrent))
            .count(),
        1
    );
    drop(a);
    drop(b);
    cleanup(dir);
}

#[test]
fn independent_processes_admit_one_lease_and_crash_release_is_expiry_based() {
    let dir = scratch();
    let path = dir.join("fixture.sqlite");
    drop(ExtractionFence::open(&path, "owner-a"));
    let a = spawn_extraction_child(&path, "process-a", 1_000, false);
    let b = spawn_extraction_child(&path, "process-b", 1_000, false);
    let out_a = a.wait_with_output().unwrap();
    let out_b = b.wait_with_output().unwrap();
    assert!(out_a.status.success());
    assert!(out_b.status.success());
    let both = format!(
        "{}{}",
        String::from_utf8_lossy(&out_a.stdout),
        String::from_utf8_lossy(&out_b.stdout)
    );
    assert_eq!(both.matches("RESULT=Ok(0)").count(), 1);
    assert_eq!(both.matches("RESULT=Err(Concurrent)").count(), 1);

    // Forget-like restart does not clear an active lease. The process dies
    // after the admission transaction committed; only expiry frees the slot.
    let mut crash = spawn_extraction_child(&path, "crashed-process", 61_000, true);
    assert_eq!(crash.wait().unwrap().code(), Some(73));
    let reopened = ExtractionFence::open(&path, "owner-a");
    assert_eq!(
        reopened.admit("owner-a", "before-expiry", 61_001),
        Err(FenceError::Concurrent)
    );
    assert_eq!(reopened.admit("owner-a", "after-expiry", 121_000), Ok(0));
    assert_eq!(reopened.starts(0), 3);
    drop(reopened);
    cleanup(dir);
}

#[test]
fn process_crash_before_final_commit_leaves_no_extracted_row() {
    let dir = scratch();
    let path = dir.join("fixture.sqlite");
    let fence = ExtractionFence::open(&path, "owner-a");
    assert_eq!(fence.admit("owner-a", "precommit-crash", 5_000), Ok(0));
    drop(fence);
    let mut child = Command::new(std::env::current_exe().unwrap())
        .arg("--exact")
        .arg("p7_extraction_fence_tests::extraction_fence_child_entry")
        .arg("--nocapture")
        .env("SEREA_P7_EXTRACTION_CHILD_DB", &path)
        .env("SEREA_P7_EXTRACTION_CHILD_OWNER", "owner-a")
        .env("SEREA_P7_EXTRACTION_CHILD_OP", "precommit-crash")
        .env("SEREA_P7_EXTRACTION_CHILD_NOW", "5001")
        .env("SEREA_P7_EXTRACTION_CHILD_FINAL", "1")
        .env("SEREA_P7_EXTRACTION_CRASH_BEFORE_COMMIT", "1")
        .spawn()
        .unwrap();
    assert_eq!(child.wait().unwrap().code(), Some(74));
    let reopened = ExtractionFence::open(&path, "owner-a");
    assert_eq!(reopened.row_count(), 0);
    assert_eq!(
        reopened.admit("owner-a", "before-expiry", 5_002),
        Err(FenceError::Concurrent)
    );
    assert_eq!(reopened.admit("owner-a", "after-expiry", 65_000), Ok(0));
    assert_eq!(reopened.row_count(), 0);
    drop(reopened);
    cleanup(dir);
}
