package app.serea.android.assistant.taskproposal

import app.serea.android.tasks.InvalidTaskException
import app.serea.android.tasks.ProposalValidation
import app.serea.android.tasks.Task
import app.serea.android.tasks.TaskConflictException
import app.serea.android.tasks.TaskNotFoundException
import app.serea.android.tasks.TaskProposal
import app.serea.android.tasks.TaskProposalConfirmer
import app.serea.android.tasks.TaskProposalOperation
import app.serea.android.tasks.TaskRepository
import app.serea.android.tasks.validate
import java.time.Duration
import java.time.Instant
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withContext

/** Values shown to the user before approval. Task text stays in memory and is never logged here. */
data class TaskProposalReviewSnapshot(
    val proposal: TaskProposal,
    val previousTask: Task?,
    val expiresAt: Instant,
) {
    val requiresDangerousConfirmation: Boolean get() = proposal.operation == TaskProposalOperation.DELETE
}

enum class TaskProposalReviewStatus { READY, EXECUTING, APPROVED, DECLINED, EXPIRED, CONFLICT, REJECTED, OUTCOME_UNKNOWN }

sealed interface TaskProposalReviewResult {
    data class Approved(val operation: TaskProposalOperation) : TaskProposalReviewResult
    data object Declined : TaskProposalReviewResult
    data object Expired : TaskProposalReviewResult
    data object Conflict : TaskProposalReviewResult
    data object Rejected : TaskProposalReviewResult
    data object OutcomeUnknown : TaskProposalReviewResult
}

/**
 * An ephemeral, single-use approval gate. It is deliberately not Parcelable or persistable:
 * activity/process restoration must create a new review and require a fresh explicit approval.
 */
class TaskProposalReviewSession private constructor(
    val snapshot: TaskProposalReviewSnapshot,
    private val confirmer: TaskProposalConfirmer,
    private val clock: () -> Instant,
) {
    private val mutex = Mutex()
    private val mutableStatus = MutableStateFlow(TaskProposalReviewStatus.READY)
    val statusState: StateFlow<TaskProposalReviewStatus> = mutableStatus.asStateFlow()
    val status: TaskProposalReviewStatus get() = mutableStatus.value

    /**
     * selectedTaskId is the UI's current selection at the instant the user confirms. It must
     * still match the reviewed target. The delete acknowledgement is a separate explicit action.
     */
    suspend fun approve(
        explicitlyApproved: Boolean,
        selectedTaskId: String?,
        dangerousActionAcknowledged: Boolean = false,
    ): TaskProposalReviewResult {
        if (!explicitlyApproved) return decline()
        val accepted = mutex.withLock {
            if (status != TaskProposalReviewStatus.READY) return@withLock false
            if (!clock().isBefore(snapshot.expiresAt)) {
                mutableStatus.value = TaskProposalReviewStatus.EXPIRED
                return@withLock false
            }
            if (selectedTaskId != snapshot.proposal.targetTaskId ||
                snapshot.requiresDangerousConfirmation && !dangerousActionAcknowledged
            ) {
                mutableStatus.value = TaskProposalReviewStatus.REJECTED
                return@withLock false
            }
            mutableStatus.value = TaskProposalReviewStatus.EXECUTING
            true
        }
        if (!accepted) return when (status) {
            TaskProposalReviewStatus.EXPIRED -> TaskProposalReviewResult.Expired
            TaskProposalReviewStatus.CONFLICT -> TaskProposalReviewResult.Conflict
            TaskProposalReviewStatus.OUTCOME_UNKNOWN -> TaskProposalReviewResult.OutcomeUnknown
            else -> TaskProposalReviewResult.Rejected
        }

        return try {
            confirmer.confirm(
                proposal = snapshot.proposal,
                explicitlyApproved = true,
                confirmedTargetTaskId = selectedTaskId,
            )
            mutex.withLock { mutableStatus.value = TaskProposalReviewStatus.APPROVED }
            TaskProposalReviewResult.Approved(snapshot.proposal.operation)
        } catch (error: CancellationException) {
            // The repository call may have crossed its write boundary; never make this gate retryable.
            withContext(NonCancellable) {
                mutex.withLock { mutableStatus.value = TaskProposalReviewStatus.OUTCOME_UNKNOWN }
            }
            throw error
        } catch (_: TaskConflictException) {
            mutex.withLock { mutableStatus.value = TaskProposalReviewStatus.CONFLICT }
            TaskProposalReviewResult.Conflict
        } catch (_: TaskNotFoundException) {
            mutex.withLock { mutableStatus.value = TaskProposalReviewStatus.CONFLICT }
            TaskProposalReviewResult.Conflict
        } catch (_: InvalidTaskException) {
            mutex.withLock { mutableStatus.value = TaskProposalReviewStatus.REJECTED }
            TaskProposalReviewResult.Rejected
        } catch (_: Exception) {
            // A generic repository failure can occur after the underlying mutation committed.
            mutex.withLock { mutableStatus.value = TaskProposalReviewStatus.OUTCOME_UNKNOWN }
            TaskProposalReviewResult.OutcomeUnknown
        }
    }

    suspend fun decline(): TaskProposalReviewResult = mutex.withLock {
        if (status == TaskProposalReviewStatus.READY && !clock().isBefore(snapshot.expiresAt)) {
            mutableStatus.value = TaskProposalReviewStatus.EXPIRED
            return@withLock TaskProposalReviewResult.Expired
        }
        if (status != TaskProposalReviewStatus.READY) return@withLock when (status) {
            TaskProposalReviewStatus.EXPIRED -> TaskProposalReviewResult.Expired
            TaskProposalReviewStatus.CONFLICT -> TaskProposalReviewResult.Conflict
            TaskProposalReviewStatus.OUTCOME_UNKNOWN -> TaskProposalReviewResult.OutcomeUnknown
            else -> TaskProposalReviewResult.Rejected
        }
        mutableStatus.value = TaskProposalReviewStatus.DECLINED
        TaskProposalReviewResult.Declined
    }

    /** Refreshes the observable status when a screen's expiration timer fires. */
    suspend fun expireIfNeeded(): Boolean = mutex.withLock {
        if (status != TaskProposalReviewStatus.READY || clock().isBefore(snapshot.expiresAt)) return@withLock false
        mutableStatus.value = TaskProposalReviewStatus.EXPIRED
        true
    }

    companion object {
        private val maximumLifetime = Duration.ofMinutes(5)

        /** Validates the untrusted proposal and captures the exact target revision before review. */
        suspend fun prepare(
            proposal: TaskProposal,
            repository: TaskRepository,
            now: () -> Instant = Instant::now,
        ): TaskProposalReviewSession {
            when (val validation = proposal.validate()) {
                ProposalValidation.Valid -> Unit
                is ProposalValidation.Invalid -> throw InvalidTaskException(validation.reason)
            }
            val createdAt = now()
            val expiresAt = createdAt.plus(maximumLifetime)
            val target = if (proposal.operation == TaskProposalOperation.CREATE) null else {
                repository.get(proposal.targetTaskId!!)
                    ?: throw TaskConflictException()
            }
            if (target != null &&
                (target.id != proposal.targetTaskId || target.revision != proposal.expectedRevision)
            ) throw TaskConflictException()
            if (proposal.operation == TaskProposalOperation.COMPLETE && target?.status != app.serea.android.tasks.TaskStatus.TODO)
                throw TaskConflictException()
            return TaskProposalReviewSession(
                TaskProposalReviewSnapshot(proposal, target, expiresAt),
                TaskProposalConfirmer(repository),
                now,
            )
        }
    }
}
