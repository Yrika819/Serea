//! Deterministic task lifecycle orchestration. IDs, time and attribution are supplied
//! by the host; SQLite remains the lease, outcome and transaction authority.
//! No execution, recovery, event runtime or ambient clock is part of this crate.
//!
//! Recovery is a later phase, not a placeholder API:
//! ```compile_fail
//! use serea_task_engine::TaskEngine;
//! let _ = TaskEngine::recover;
//! ```
//! Execution is not a task lifecycle operation:
//! ```compile_fail
//! use serea_task_engine::TaskEngine;
//! let _ = TaskEngine::execute;
//! ```
//! The engine cannot expose its storage mutation capability:
//! ```compile_fail
//! use serea_task_engine::TaskEngine;
//! fn bypass(engine: &TaskEngine) { let _ = &engine.store; }
//! ```

#![forbid(unsafe_code)]

mod engine;
mod error;
mod journal;
mod transition;
mod types;

pub use engine::TaskEngine;
pub use error::EngineError;
pub use journal::TaskJournal;
pub use serea_storage::{
    CancellationOutcome, DeletionOutcome, LeaseGuard, StepCommit, StepFailure, StepOutcome,
    TransitionContext,
};
pub use transition::{TaskTransitionReason, legal_task_transition, task_transition_reason};
pub use types::{NewTask, Plan, PlanRevision, PlanStep, StepRecord, TaskRecord};
