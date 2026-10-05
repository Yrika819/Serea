//! P2H test-only crash and fault seam.
//!
//! # Why this module exists and why it is nearly empty
//!
//! The P2 test matrix requires *real process death* inside named private
//! transaction stages (N1-N6). SQLite offers no deterministic mid-`COMMIT`
//! injection through `rusqlite` (test matrix §3.1), and a plain `Err` is an
//! in-process rollback, which is a different property from durability. So the
//! seam is a compiled capability that a **child process** reaches and then dies
//! in, while a **separate fresh process** verifies the durable result.
//!
//! # Isolation
//!
//! The whole module is gated on `#[cfg(feature = "p2h-fault-injection")]` at
//! its `pub mod fault;` declaration. That feature:
//!
//! * is declared as `p2h-fault-injection = []` with no `default` feature, so
//!   `cargo build` and `cargo build --release` never compile it;
//! * is requested from exactly one place, the `[dev-dependencies]` edge of
//!   `serea-task-engine`, which Cargo's resolver v2 unifies only when building
//!   targets that need dev-dependencies (this crate's tests).
//!
//! Therefore a release or production build of `serea-storage` contains no
//! window type, no arming entry point, no process code and no registry: there
//! is nothing to arm even in principle. There is deliberately **no**
//! environment-variable switch, no inert callback stored in `Store`, and no
//! `Option<hook>` field that merely sits as `None` in production.
//!
//! # Reach points
//!
//! Every `reach` call is `#[cfg]`-gated in the production source, so a
//! default build compiles to exactly the same statements it did before this
//! module existed. Arming is thread-local and one-shot: an armed window fires
//! at most once and is disarmed before its action runs.
//!
//! # Actions
//!
//! * [`Action::Crash`] writes its acknowledgement file and then blocks
//!   indefinitely. The file is a deterministic, explicit IPC acknowledgement
//!   that the named stage was reached; the supervising parent sends SIGKILL at
//!   that point. No sleep is ever the authority.
//! * [`Action::Fail`] returns a typed [`StoreError`] from the enclosing stage.
//!   This is a fault-injection rollback, **not** a crash, and is labelled as
//!   such by its tests.

use std::cell::RefCell;
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::StoreError;

/// A named private transaction stage reachable only from inside storage.
///
/// The discriminants exist solely so a test can name a window; no window
/// carries data, a callback, a payload or a lease authority.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Window {
    /// Inside `Store::transact_in`, before `BEGIN IMMEDIATE` is issued.
    BeforeBegin,
    /// Inside `Store::transact_in`, after the transaction is open, before any
    /// statement of the caller's body has run.
    AfterBegin,
    /// Inside `Tx::insert_task`, after the `tasks` row is written and before
    /// the audit/journal row.
    AfterTaskInsert,
    /// Inside the outcome fence, after the fenced `UPDATE` statement and
    /// before its `rows_affected` inspection.
    BeforeFenceInspection,
    /// Inside the outcome fence, after the fenced `UPDATE` is accepted and
    /// before the result blob, reference, receipt, task aggregate, journal
    /// batch and lease release are written.
    AfterFencedStepWrite,
    /// Inside `Tx::record_transition`, after the whole batch is validated and
    /// before its first journal `INSERT`.
    BeforeJournalInsert,
    /// Inside a method savepoint, at the point a failed `RELEASE` poisons the
    /// outer transaction. Reachable from task, lifecycle and recovery
    /// operations, which all use `Tx::operation_savepoint`.
    BeforeSavepointRelease,
    /// Inside `Store::transact_in`, after every write of the transaction and
    /// immediately before `COMMIT`.
    BeforeCommit,
    /// Inside `Store::transact_in`, immediately after `COMMIT` returned `Ok`
    /// and before the caller can observe success.
    AfterCommit,
}

/// What an armed window does when it is reached.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Announce the window through a file, then die by external signal. The
    /// acknowledgement path is a test-harness artefact and never a durable
    /// store path.
    Crash { ack: PathBuf },
    /// Return this typed error from the enclosing stage.
    Fail(StoreError),
}

#[derive(Debug, Clone)]
struct Armed {
    window: Window,
    /// How many times this window is reached and ignored before it fires. The
    /// transaction envelope windows (`BeforeBegin`, `AfterBegin`,
    /// `BeforeCommit`, `AfterCommit`) are shared by *every* transaction, so a
    /// test that wants the outcome transaction must skip the setup ones. Stage
    /// windows such as `AfterFencedStepWrite` are reached only by the outcome
    /// path and normally use a skip of zero.
    skip: usize,
    action: Action,
}

thread_local! {
    /// Thread-local and one-shot: a window armed on one test thread can never
    /// fire on another, so parallel tests cannot interfere.
    static ARMED: RefCell<Option<Armed>> = const { RefCell::new(None) };
}

impl Window {
    /// Arms this thread to fire `action` at this window's next reach.
    ///
    /// Returns [`StoreError::ConstraintViolation`] when this thread already has
    /// an armed window, because silently replacing one would make a crash test
    /// prove nothing.
    pub fn arm(self, action: Action) -> Result<(), StoreError> {
        self.arm_after(0, action)
    }

    /// Arms this thread to fire `action` only once this window has been reached
    /// and ignored `skip` times.
    ///
    /// The transaction envelope windows are shared by every transaction, so a
    /// crash test that stages a task, plan, lease and attempt must skip those
    /// transactions to reach the outcome transaction itself.
    pub fn arm_after(self, skip: usize, action: Action) -> Result<(), StoreError> {
        ARMED.with(|slot| {
            let mut slot = slot.borrow_mut();
            if slot.is_some() {
                return Err(StoreError::ConstraintViolation);
            }
            *slot = Some(Armed {
                window: self,
                skip,
                action,
            });
            Ok(())
        })
    }

    /// Whether this thread still has an armed window. Disarming happens when
    /// the window fires, so a test can prove its window actually ran.
    pub fn is_armed(self) -> bool {
        ARMED.with(|slot| {
            slot.borrow()
                .as_ref()
                .map(|armed| armed.window == self)
                .unwrap_or(false)
        })
    }
}

/// Internal hook called from the named private stage. Panicking or unwinding
/// here would defeat the point: every window must either return normally or
/// block until the process is killed.
pub(crate) fn reach(window: Window) -> Result<(), StoreError> {
    let action = ARMED.with(|slot| {
        let mut slot = slot.borrow_mut();
        let armed = slot.as_mut()?;
        if armed.window != window {
            return None;
        }
        if armed.skip > 0 {
            armed.skip -= 1;
            return None;
        }
        slot.take().map(|armed| armed.action)
    });
    match action {
        None => Ok(()),
        Some(Action::Fail(error)) => Err(error),
        Some(Action::Crash { ack }) => {
            announce(&ack, window);
            // Block forever. The supervising parent kills this process once it
            // observes the acknowledgement, so no timeout is invented here and
            // no timer can turn a crash into a clean return.
            loop {
                std::thread::park();
            }
        }
    }
}

/// Writes the acknowledgement atomically enough for a polling parent: create
/// then sync, so its existence is never observed half-written.
fn announce(ack: &Path, window: Window) {
    if let Some(parent) = ack.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::File::create(ack) {
        Ok(mut file) => {
            let _ = writeln!(file, "{window:?}");
            let _ = file.sync_all();
        }
        // Failing to acknowledge must not silently continue into the window.
        Err(_) => std::process::abort(),
    }
}
