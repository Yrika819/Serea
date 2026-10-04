use serea_protocol::TaskState;

/// Frozen edge classification, independent of operation-specific typed causes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskTransitionReason {
    StartPlanning,
    Replan,
    PlanPersisted,
    StartExecution,
    ContinueExecution,
    EnterVerification,
    VerificationRequiresExecution,
    VerificationComplete,
    AwaitApproval,
    AwaitUser,
    ResumeReady,
    Block,
    Cancel,
    Fail,
}
impl TaskTransitionReason {
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::StartPlanning => "START_PLANNING",
            Self::Replan => "REPLAN",
            Self::PlanPersisted => "PLAN_PERSISTED",
            Self::StartExecution => "START_EXECUTION",
            Self::ContinueExecution => "CONTINUE_EXECUTION",
            Self::EnterVerification => "ENTER_VERIFICATION",
            Self::VerificationRequiresExecution => "VERIFICATION_REQUIRES_EXECUTION",
            Self::VerificationComplete => "VERIFICATION_COMPLETE",
            Self::AwaitApproval => "AWAIT_APPROVAL",
            Self::AwaitUser => "AWAIT_USER",
            Self::ResumeReady => "RESUME_READY",
            Self::Block => "BLOCK",
            Self::Cancel => "CANCEL",
            Self::Fail => "FAIL",
        }
    }
}

/// Exhaustive outer match forces a deliberate decision when TaskState expands.
/// The independent literal 11-by-11 test oracle pins every edge, not just enum coverage.
pub fn task_transition_reason(from: TaskState, to: TaskState) -> Option<TaskTransitionReason> {
    use TaskState::*;
    use TaskTransitionReason as R;
    match from {
        Received => match to {
            Planning => Some(R::StartPlanning),
            Failed => Some(R::Fail),
            Cancelled => Some(R::Cancel),
            _ => None,
        },
        Planning => match to {
            Ready => Some(R::PlanPersisted),
            WaitingApproval => Some(R::AwaitApproval),
            WaitingUser => Some(R::AwaitUser),
            Blocked => Some(R::Block),
            Failed => Some(R::Fail),
            Cancelled => Some(R::Cancel),
            _ => None,
        },
        Ready => match to {
            Planning => Some(R::Replan),
            Executing => Some(R::StartExecution),
            WaitingApproval => Some(R::AwaitApproval),
            Failed => Some(R::Fail),
            Cancelled => Some(R::Cancel),
            _ => None,
        },
        Executing => match to {
            Ready => Some(R::ContinueExecution),
            WaitingApproval => Some(R::AwaitApproval),
            WaitingUser => Some(R::AwaitUser),
            Verifying => Some(R::EnterVerification),
            Blocked => Some(R::Block),
            Failed => Some(R::Fail),
            Cancelled => Some(R::Cancel),
            _ => None,
        },
        WaitingApproval => match to {
            Ready => Some(R::ResumeReady),
            Failed => Some(R::Fail),
            Cancelled => Some(R::Cancel),
            _ => None,
        },
        WaitingUser => match to {
            Planning => Some(R::Replan),
            Ready => Some(R::ResumeReady),
            Failed => Some(R::Fail),
            Cancelled => Some(R::Cancel),
            _ => None,
        },
        Verifying => match to {
            Executing => Some(R::VerificationRequiresExecution),
            Blocked => Some(R::Block),
            Completed => Some(R::VerificationComplete),
            Failed => Some(R::Fail),
            Cancelled => Some(R::Cancel),
            _ => None,
        },
        Blocked => match to {
            Planning => Some(R::Replan),
            Ready => Some(R::ResumeReady),
            Failed => Some(R::Fail),
            Cancelled => Some(R::Cancel),
            _ => None,
        },
        Completed | Failed | Cancelled => None,
    }
}

pub fn legal_task_transition(from: TaskState, to: TaskState) -> bool {
    task_transition_reason(from, to).is_some()
}
