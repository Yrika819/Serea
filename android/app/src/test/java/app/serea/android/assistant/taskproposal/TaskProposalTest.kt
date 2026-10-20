package app.serea.android.assistant.taskproposal

import app.serea.android.tasks.*
import java.time.Instant
import java.time.LocalDate
import java.util.UUID
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import org.junit.Assert.*
import org.junit.Test

class TaskProposalTest {
    private class FakeTaskRepository : TaskRepository {
        private val lock = Any()
        val rows = linkedMapOf<String, Task>()
        val receipts = mutableMapOf<String, String>()
        var writes = 0
        var updateCalls = 0
        var updateEntered: CompletableDeferred<Unit>? = null
        var allowUpdate: CompletableDeferred<Unit>? = null

        override suspend fun create(draft: TaskDraft) = synchronized(lock) { createRow(draft) }
        override suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task = synchronized(lock) {
            if (receipts.containsKey(proposalId)) throw TaskConflictException()
            val task = createRow(draft)
            receipts[proposalId] = task.id
            task
        }
        private fun createRow(draft: TaskDraft) = Task(
            id = UUID.randomUUID().toString(), title = draft.title, description = draft.description,
            status = TaskStatus.TODO, priority = draft.priority, createdAt = Instant.EPOCH,
            updatedAt = Instant.EPOCH, dueDate = draft.dueDate, dueTime = draft.dueTime,
            timeZoneId = draft.timeZoneId,
        ).also { rows[it.id] = it; writes++ }

        override suspend fun get(id: String) = synchronized(lock) { rows[id] }
        override suspend fun list() = synchronized(lock) { rows.values.toList() }
        override suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task {
            synchronized(lock) { updateCalls++ }
            val (entered, allowed) = synchronized(lock) { updateEntered to allowUpdate }
            entered?.complete(Unit)
            allowed?.await()
            return synchronized(lock) {
                val old = checked(id, expectedRevision)
                old.copy(title = draft.title, description = draft.description, priority = draft.priority,
                    dueDate = draft.dueDate, dueTime = draft.dueTime, timeZoneId = draft.timeZoneId,
                    revision = old.revision + 1).also { rows[id] = it; writes++ }
            }
        }
        override suspend fun complete(id: String, expectedRevision: Long): Task = synchronized(lock) {
            val old = checked(id, expectedRevision)
            old.copy(status = TaskStatus.COMPLETED, completedAt = Instant.EPOCH.plusSeconds(1),
                revision = old.revision + 1).also { rows[id] = it; writes++ }
        }
        override suspend fun reopen(id: String, expectedRevision: Long) = error("unused")
        override suspend fun delete(id: String, expectedRevision: Long): Unit = synchronized(lock) {
            checked(id, expectedRevision); rows.remove(id); writes++; Unit
        }
        override suspend fun incomplete() = synchronized(lock) { rows.values.filter { it.status == TaskStatus.TODO } }
        override suspend fun completed() = synchronized(lock) { rows.values.filter { it.status == TaskStatus.COMPLETED } }
        override suspend fun dueToday(today: LocalDate) = emptyList<Task>()
        override suspend fun overdue(now: Instant, fallbackZone: java.time.ZoneId) = emptyList<Task>()

        private fun checked(id: String, revision: Long): Task {
            val old = rows[id] ?: throw TaskNotFoundException()
            if (old.revision != revision) throw TaskConflictException()
            return old
        }

        fun seed(title: String = "Old title") = synchronized(lock) {
            Task(
                UUID.randomUUID().toString(), title, "Old description", TaskStatus.TODO, TaskPriority.NORMAL,
                Instant.EPOCH, Instant.EPOCH, revision = 3,
            ).also { rows[it.id] = it }
        }

        fun changeRevision(id: String) = synchronized(lock) {
            rows[id] = rows.getValue(id).copy(revision = rows.getValue(id).revision + 1)
        }
    }

    @Test fun preparingReviewDoesNotWriteAndDisplaysCapturedRevision() = runBlocking {
        val repository = FakeTaskRepository()
        val target = repository.seed()
        val session = TaskProposalReviewSession.prepare(update(target, title = "New title"), repository)
        assertEquals(0, repository.writes)
        assertEquals(target, session.snapshot.previousTask)
        assertEquals(3L, session.snapshot.proposal.expectedRevision)
        assertEquals(TaskProposalReviewStatus.READY, session.status)
    }

    @Test fun declineAndNonApprovalNeverWriteAndCannotBeReused() = runBlocking {
        val repository = FakeTaskRepository()
        val proposal = create("Suggestion")
        val session = TaskProposalReviewSession.prepare(proposal, repository)
        assertEquals(TaskProposalReviewResult.Declined, session.approve(explicitlyApproved = false, selectedTaskId = null))
        assertEquals(TaskProposalReviewStatus.DECLINED, session.status)
        assertEquals(TaskProposalReviewResult.Rejected, session.approve(explicitlyApproved = true, selectedTaskId = null))
        assertEquals(0, repository.writes)
    }

    @Test fun expiryRejectsApproval() = runBlocking {
        val repository = FakeTaskRepository()
        var now = Instant.parse("2026-10-20T12:00:00Z")
        val session = TaskProposalReviewSession.prepare(create("Suggestion"), repository) { now }
        now = now.plusSeconds(301)
        assertEquals(TaskProposalReviewResult.Expired, session.approve(true, selectedTaskId = null))
        assertEquals(TaskProposalReviewStatus.EXPIRED, session.status)
        assertEquals(0, repository.writes)
    }

    @Test fun targetSelectionMismatchAndDangerousDeleteNeedExplicitAcknowledgement() = runBlocking {
        val repository = FakeTaskRepository()
        val target = repository.seed()
        val mismatch = TaskProposalReviewSession.prepare(delete(target), repository)
        assertEquals(TaskProposalReviewResult.Rejected, mismatch.approve(true, selectedTaskId = UUID.randomUUID().toString(), dangerousActionAcknowledged = true))
        assertEquals(0, repository.writes)

        val omittedSelection = TaskProposalReviewSession.prepare(delete(target), repository)
        assertEquals(TaskProposalReviewResult.Rejected, omittedSelection.approve(true, selectedTaskId = null, dangerousActionAcknowledged = true))
        assertEquals(0, repository.writes)

        val missingAcknowledgement = TaskProposalReviewSession.prepare(delete(target), repository)
        assertEquals(TaskProposalReviewResult.Rejected, missingAcknowledgement.approve(true, target.id, false))
        assertEquals(0, repository.writes)

        val approved = TaskProposalReviewSession.prepare(delete(target), repository)
        assertTrue(approved.snapshot.requiresDangerousConfirmation)
        assertEquals(TaskProposalReviewResult.Approved(TaskProposalOperation.DELETE), approved.approve(true, target.id, true))
        assertNull(repository.get(target.id))
        assertEquals(1, repository.writes)

        val another = repository.seed()
        val pendingReview = TaskProposalReviewSession.prepare(delete(another), repository)
        assertNotNull(repository.get(another.id))
        assertEquals(TaskProposalReviewResult.Rejected, pendingReview.approve(true, another.id, false))
        assertNotNull(repository.get(another.id))
        assertEquals(1, repository.writes)

        val explicitReview = TaskProposalReviewSession.prepare(delete(another), repository)
        assertEquals(TaskProposalReviewResult.Approved(TaskProposalOperation.DELETE), explicitReview.approve(true, another.id, true))
        assertNull(repository.get(another.id))
        assertEquals(2, repository.writes)
    }

    @Test fun revisionMismatchIsRejectedBeforeReview() = runBlocking {
        val repository = FakeTaskRepository()
        val target = repository.seed()
        val stale = update(target, title = "Replacement").copy(expectedRevision = target.revision - 1)
        assertThrows(TaskConflictException::class.java) { runBlocking { TaskProposalReviewSession.prepare(stale, repository) } }
        assertEquals(0, repository.writes)
    }

    @Test fun updateAndCompleteApplyOnlyAfterApprovalUsingCapturedRevision() = runBlocking {
        val repository = FakeTaskRepository()
        val target = repository.seed()
        val updateSession = TaskProposalReviewSession.prepare(update(target, title = "Revised"), repository)
        assertEquals(TaskProposalReviewResult.Approved(TaskProposalOperation.UPDATE), updateSession.approve(true, target.id))
        assertEquals("Revised", repository.get(target.id)?.title)
        assertEquals(4L, repository.get(target.id)?.revision)

        val current = repository.get(target.id)!!
        val completeSession = TaskProposalReviewSession.prepare(complete(current), repository)
        assertEquals(TaskProposalReviewResult.Approved(TaskProposalOperation.COMPLETE), completeSession.approve(true, current.id))
        assertEquals(TaskStatus.COMPLETED, repository.get(target.id)?.status)
        assertEquals(2, repository.writes)
    }

    @Test fun createProposalIsIdempotentAcrossConcurrentReviewSessions() = runBlocking {
        val repository = FakeTaskRepository()
        val proposal = create("Approved once")
        val first = TaskProposalReviewSession.prepare(proposal, repository)
        val second = TaskProposalReviewSession.prepare(proposal, repository)
        val results = listOf(
            async(Dispatchers.Default) { first.approve(true, selectedTaskId = null) },
            async(Dispatchers.Default) { second.approve(true, selectedTaskId = null) },
        ).map { it.await() }
        assertEquals(1, results.count { it is TaskProposalReviewResult.Approved })
        assertEquals(1, repository.writes)
        assertEquals(1, repository.rows.size)
    }

    @Test fun configRecreationRequiresFreshApprovalAndStaleRevisionConflicts() = runBlocking {
        val repository = FakeTaskRepository()
        val target = repository.seed()
        val oldReview = TaskProposalReviewSession.prepare(update(target, title = "New"), repository)
        // Simulate recreation: only the untrusted proposal is available, never approval state.
        assertEquals(0, repository.writes)
        val restoredReview = TaskProposalReviewSession.prepare(oldReview.snapshot.proposal, repository)
        assertEquals(TaskProposalReviewResult.Approved(TaskProposalOperation.UPDATE), restoredReview.approve(true, target.id))
        assertEquals(TaskProposalReviewResult.Conflict, oldReview.approve(true, target.id))
        assertEquals(1, repository.writes)
    }

    @Test fun invalidProposalShapesAreRejectedBeforeRepositoryAccess() = runBlocking {
        val repository = FakeTaskRepository()
        val invalid = listOf(
            TaskProposal(operation = TaskProposalOperation.CREATE, title = " "),
            TaskProposal(operation = TaskProposalOperation.CREATE, title = "x", targetTaskId = UUID.randomUUID().toString()),
            TaskProposal(operation = TaskProposalOperation.UPDATE, title = "x", targetTaskId = "not-an-id", expectedRevision = 1),
            TaskProposal(operation = TaskProposalOperation.UPDATE, title = "x", targetTaskId = UUID.randomUUID().toString(), expectedRevision = -1),
            TaskProposal(operation = TaskProposalOperation.CREATE, title = "x", dueTime = java.time.LocalTime.NOON),
            TaskProposal(operation = TaskProposalOperation.CREATE, title = "x", dueDate = LocalDate.parse("2026-10-20"), timeZoneId = "Not/AZone"),
        )
        invalid.forEach { assertTrue(it.validate() is ProposalValidation.Invalid) }
        invalid.forEach { proposal -> assertThrows(InvalidTaskException::class.java) { runBlocking { TaskProposalReviewSession.prepare(proposal, repository) } } }
        assertEquals(0, repository.writes)
        assertTrue(repository.rows.isEmpty())
    }

    @Test fun expiredSessionPublishesUiStateAndRequiresFreshApproval() = runBlocking {
        val repository = FakeTaskRepository()
        var now = Instant.parse("2026-10-20T12:00:00Z")
        val proposal = create("Fresh review required")
        val expired = TaskProposalReviewSession.prepare(proposal, repository) { now }
        now = expired.snapshot.expiresAt.plusMillis(1)
        assertTrue(expired.expireIfNeeded())
        assertEquals(TaskProposalReviewStatus.EXPIRED, expired.statusState.value)
        assertEquals(TaskProposalReviewResult.Expired, expired.approve(true, selectedTaskId = null))
        assertEquals(0, repository.writes)

        val refreshed = TaskProposalReviewSession.prepare(proposal, repository) { now }
        assertEquals(TaskProposalReviewStatus.READY, refreshed.statusState.value)
        assertEquals(TaskProposalReviewResult.Approved(TaskProposalOperation.CREATE), refreshed.approve(true, selectedTaskId = null))
        assertEquals(1, repository.writes)
    }

    @Test fun sameSessionDoubleApprovalAllowsAtMostOneRepositoryCall() = runBlocking {
        val repository = FakeTaskRepository().apply {
            updateEntered = CompletableDeferred()
            allowUpdate = CompletableDeferred()
        }
        val target = repository.seed()
        val session = TaskProposalReviewSession.prepare(update(target, "Single write"), repository)
        val first = async(Dispatchers.Default) { session.approve(true, target.id) }
        repository.updateEntered!!.await()
        assertEquals(TaskProposalReviewResult.Rejected, session.approve(true, target.id))
        assertEquals(0, repository.writes)
        repository.allowUpdate!!.complete(Unit)
        assertEquals(TaskProposalReviewResult.Approved(TaskProposalOperation.UPDATE), first.await())
        assertEquals(1, repository.writes)
    }

    @OptIn(ExperimentalCoroutinesApi::class)
    @Test fun cancellationDuringSaveLeavesSessionTerminalAndCannotRetry() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        val repository = FakeTaskRepository().apply {
            updateEntered = CompletableDeferred()
            allowUpdate = CompletableDeferred()
        }
        val target = repository.seed()
        val session = TaskProposalReviewSession.prepare(update(target, "Canceled write"), repository)
        val approval = async(dispatcher) { session.approve(true, target.id) }

        runCurrent()
        repository.updateEntered!!.await()
        assertEquals(TaskProposalReviewStatus.EXECUTING, session.status)
        approval.cancelAndJoin()
        runCurrent()

        assertTrue(session.status in setOf(
            TaskProposalReviewStatus.APPROVED, TaskProposalReviewStatus.DECLINED,
            TaskProposalReviewStatus.EXPIRED, TaskProposalReviewStatus.CONFLICT,
            TaskProposalReviewStatus.REJECTED,
        ))
        assertEquals(TaskProposalReviewStatus.REJECTED, session.status)
        assertEquals(TaskProposalReviewStatus.REJECTED, session.statusState.value)
        assertEquals(TaskProposalReviewResult.Rejected, session.approve(true, target.id))
        assertEquals(1, repository.updateCalls)
        assertEquals(0, repository.writes)
        assertEquals("Old title", repository.get(target.id)?.title)
    }

    @Test fun approvalAndDeclineRaceCannotExecuteAfterDeclineWinsOrDuplicateAnApproval() = runBlocking {
        val repository = FakeTaskRepository().apply {
            updateEntered = CompletableDeferred()
            allowUpdate = CompletableDeferred()
        }
        val target = repository.seed()
        val session = TaskProposalReviewSession.prepare(update(target, "One winner"), repository)
        val approval = async(Dispatchers.Default) { session.approve(true, target.id) }
        repository.updateEntered!!.await()
        assertEquals(TaskProposalReviewResult.Rejected, session.decline())
        repository.allowUpdate!!.complete(Unit)
        assertEquals(TaskProposalReviewResult.Approved(TaskProposalOperation.UPDATE), approval.await())
        assertEquals(1, repository.writes)
        assertEquals("One winner", repository.get(target.id)?.title)
    }

    @Test fun simultaneousApprovalAndDeclineHaveOneWinner() = runBlocking {
        val repository = FakeTaskRepository()
        val target = repository.seed()
        val session = TaskProposalReviewSession.prepare(update(target, "Race winner"), repository)
        val start = CompletableDeferred<Unit>()
        val approval = async(Dispatchers.Default) {
            start.await()
            session.approve(true, target.id)
        }
        val decline = async(Dispatchers.Default) {
            start.await()
            session.decline()
        }
        start.complete(Unit)
        val approvalResult = approval.await()
        val declineResult = decline.await()
        val approved = approvalResult is TaskProposalReviewResult.Approved
        val declined = declineResult is TaskProposalReviewResult.Declined
        assertTrue(approved.xor(declined))
        assertEquals(if (approved) 1 else 0, repository.writes)
    }

    @Test fun taskRevisionChangedAfterReviewIsRejectedWithoutApplyingProposal() = runBlocking {
        val repository = FakeTaskRepository()
        val target = repository.seed()
        val session = TaskProposalReviewSession.prepare(update(target, "Stale proposal"), repository)
        repository.changeRevision(target.id)
        assertEquals(TaskProposalReviewResult.Conflict, session.approve(true, target.id))
        assertEquals("Old title", repository.get(target.id)?.title)
        assertEquals(4L, repository.get(target.id)?.revision)
    }

    private fun create(title: String) = TaskProposal(operation = TaskProposalOperation.CREATE, title = title)
    private fun update(target: Task, title: String) = TaskProposal(
        operation = TaskProposalOperation.UPDATE, title = title, description = target.description,
        priority = target.priority, targetTaskId = target.id, expectedRevision = target.revision,
    )
    private fun complete(target: Task) = TaskProposal(
        operation = TaskProposalOperation.COMPLETE, targetTaskId = target.id, expectedRevision = target.revision,
    )
    private fun delete(target: Task) = TaskProposal(
        operation = TaskProposalOperation.DELETE, targetTaskId = target.id, expectedRevision = target.revision,
    )
}
