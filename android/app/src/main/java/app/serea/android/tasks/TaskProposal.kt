package app.serea.android.tasks

import java.time.LocalDate
import java.time.LocalTime
import java.util.UUID

/** Untrusted suggestion data only. Construction/validation never writes to TaskRepository. */
enum class TaskProposalOperation { CREATE, UPDATE, COMPLETE, DELETE }

data class TaskProposal(
    val proposalId: String = UUID.randomUUID().toString(),
    val operation: TaskProposalOperation,
    val title: String? = null,
    val description: String? = null,
    val dueDate: LocalDate? = null,
    val dueTime: LocalTime? = null,
    val timeZoneId: String? = null,
    val priority: TaskPriority? = null,
    val targetTaskId: String? = null,
    val expectedRevision: Long? = null,
)

sealed interface ProposalValidation {
    data object Valid : ProposalValidation
    data class Invalid(val reason: String) : ProposalValidation
}

fun TaskProposal.validate(): ProposalValidation = try {
    TaskValidation.validateId(proposalId)
    when (operation) {
        TaskProposalOperation.CREATE -> {
            if (targetTaskId != null || expectedRevision != null) throw InvalidTaskException("Create proposals cannot target an existing task.")
            TaskValidation.validateDraft(TaskDraft(title ?: "", description.orEmpty(), priority ?: TaskPriority.NORMAL, dueDate, dueTime, timeZoneId))
        }
        TaskProposalOperation.UPDATE -> {
            TaskValidation.validateId(targetTaskId ?: throw InvalidTaskException("Update proposal needs a task ID."))
            if (expectedRevision == null || expectedRevision < 1) throw InvalidTaskException("Update proposal needs a revision.")
            TaskValidation.validateDraft(TaskDraft(title ?: "", description.orEmpty(), priority ?: TaskPriority.NORMAL, dueDate, dueTime, timeZoneId))
        }
        TaskProposalOperation.COMPLETE, TaskProposalOperation.DELETE -> {
            TaskValidation.validateId(targetTaskId ?: throw InvalidTaskException("Proposal needs a task ID."))
            if (expectedRevision == null || expectedRevision < 1) throw InvalidTaskException("Proposal needs a revision.")
        }
    }
    ProposalValidation.Valid
} catch (error: Exception) { ProposalValidation.Invalid(error.message ?: "Invalid proposal.") }

/** A UI must invoke this only from an explicit user confirmation. Provider adapters receive no repository reference. */
class TaskProposalConfirmer(private val repository: TaskRepository) {
    suspend fun confirm(proposal: TaskProposal, explicitlyApproved: Boolean, confirmedTargetTaskId: String? = null): Task? {
        if (!explicitlyApproved) throw SecurityException("Explicit user confirmation is required.")
        if (proposal.operation != TaskProposalOperation.CREATE && confirmedTargetTaskId != proposal.targetTaskId)
            throw SecurityException("The approved task ID does not match the proposal target.")
        when (val validation = proposal.validate()) {
            ProposalValidation.Valid -> Unit
            is ProposalValidation.Invalid -> throw InvalidTaskException(validation.reason)
        }
        val durableRepository = repository as? DurableTaskProposalApplyPort
        if (durableRepository != null) {
            return durableRepository.applyApprovedProposal(ApprovedTaskProposal(proposal))
        }
        return when (proposal.operation) {
                TaskProposalOperation.CREATE -> repository.createFromApprovedProposal(TaskDraft(proposal.title!!, proposal.description.orEmpty(), proposal.priority ?: TaskPriority.NORMAL, proposal.dueDate, proposal.dueTime, proposal.timeZoneId), proposal.proposalId)
                TaskProposalOperation.UPDATE -> repository.update(proposal.targetTaskId!!, proposal.expectedRevision!!, TaskDraft(proposal.title!!, proposal.description.orEmpty(), proposal.priority ?: TaskPriority.NORMAL, proposal.dueDate, proposal.dueTime, proposal.timeZoneId))
                TaskProposalOperation.COMPLETE -> repository.complete(proposal.targetTaskId!!, proposal.expectedRevision!!)
                TaskProposalOperation.DELETE -> { repository.delete(proposal.targetTaskId!!, proposal.expectedRevision!!); null }
            }
    }
}
