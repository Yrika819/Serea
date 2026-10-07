//! Durable Scheduler runtime and deterministic local recurrence resolution.
#![forbid(unsafe_code)]

mod event_predicate;
mod recurrence;
mod runtime;
mod task_template;

pub use event_predicate::{
    EventCausality, EventPredicateError, EventPredicateV1, matches_host_event,
};
pub use recurrence::{
    CalendarRecurrenceKind, CalendarRecurrenceV1, RecurrenceError, ResolvedCalendarOccurrence,
    Weekday, occurrence_identity_key,
};
pub use runtime::{ScheduleDefinition, ScheduleError, Scheduler, SchedulerRecoveryReport};
pub use task_template::{MAX_SCHEDULE_TEMPLATE_BYTES, ScheduledTaskTemplateV1, TemplateError};
