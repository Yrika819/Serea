package app.serea.android.tasks

import java.time.Instant
import java.time.LocalDate
import java.util.concurrent.atomic.AtomicLong
import app.serea.android.assistant.taskproposal.TaskProposalReviewSession
import app.serea.android.assistant.taskproposal.TaskProposalReviewResult
import app.serea.android.assistant.taskproposal.TaskProposalReviewStatus
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test

class TaskRepositoryTest {
    private class MemoryDao : TaskDao {
        private val rows = linkedMapOf<String, TaskRow>()
        private val receipts = linkedMapOf<String, ProposalReceiptRow>()
        override suspend fun allRows() = synchronized(rows) { rows.values.toList() }
        override suspend fun row(id: String) = synchronized(rows) { rows[id] }
        override suspend fun insert(row: TaskRow) { synchronized(rows) { if (rows.containsKey(row.id)) throw TaskConflictException(); rows[row.id] = row } }
        override suspend fun insertReceipt(row: ProposalReceiptRow) { synchronized(rows) { if (receipts.containsKey(row.proposalId)) throw TaskConflictException(); receipts[row.proposalId] = row } }
        override suspend fun receipt(proposalId: String) = synchronized(rows) { receipts[proposalId] }
        override suspend fun insertTaskForProposal(task: TaskRow, receipt: ProposalReceiptRow) = synchronized(rows) {
            if (rows.containsKey(task.id) || receipts.containsKey(receipt.proposalId)) throw TaskConflictException()
            rows[task.id] = task; receipts[receipt.proposalId] = receipt
        }
        override suspend fun compareAndSet(id: String, expectedRevision: Long, nextRevision: Long, encrypted: String) = synchronized(rows) {
            val row = rows[id] ?: return@synchronized 0
            if (row.revision != expectedRevision) 0 else { rows[id] = row.copy(revision = nextRevision, encryptedRecord = encrypted); 1 }
        }
        override suspend fun deleteIfRevisionMatches(id: String, expectedRevision: Long) = synchronized(rows) {
            if (rows[id]?.revision == expectedRevision) { rows.remove(id); 1 } else 0
        }
        override suspend fun deleteById(id: String) = synchronized(rows) { if (rows.remove(id) != null) 1 else 0 }
    }
    private class TestCipher : TaskCipher {
        override fun encrypt(id: String, plaintext: ByteArray) = java.util.Base64.getEncoder().encodeToString(plaintext)
        override fun decrypt(id: String, ciphertext: String) = java.util.Base64.getDecoder().decode(ciphertext)
    }
    private val tick = AtomicLong(1_800_000_000_000)
    private val dao = MemoryDao()
    private val repository = EncryptedTaskRepository(dao, TestCipher(), { Instant.ofEpochMilli(tick.getAndIncrement()) }, { "00000000-0000-0000-0000-000000000001" })

    @Test fun createReadUpdateCompleteReopenAndDelete() {
        runBlocking {
            val created = repository.create(TaskDraft("一次締切 📌", "memo\nnext", TaskPriority.HIGH, LocalDate.parse("2027-01-02")))
            assertEquals(created, repository.get(created.id))
            val changed = repository.update(created.id, created.revision, TaskDraft("changed", "text", TaskPriority.LOW))
            assertEquals(2L, changed.revision)
            assertThrows(TaskConflictException::class.java) { runBlocking { repository.update(created.id, created.revision, TaskDraft("stale")) } }
            val completed = repository.complete(created.id, changed.revision)
            assertEquals(TaskStatus.COMPLETED, completed.status); assertNotNull(completed.completedAt)
            assertEquals(completed, repository.complete(created.id, completed.revision))
            val reopened = repository.reopen(created.id, completed.revision)
            assertNull(reopened.completedAt)
            repository.delete(created.id, reopened.revision)
            assertNull(repository.get(created.id))
            assertThrows(TaskNotFoundException::class.java) { runBlocking { repository.update(created.id, reopened.revision, TaskDraft("late")) } }
        }
    }

    @Test fun listsSortsFiltersAndLateUpdateCannotResurrectDeletedTask() = runBlocking {
        val nextId = AtomicLong(1)
        val repository = EncryptedTaskRepository(MemoryDao(), TestCipher(),
            { Instant.ofEpochMilli(tick.getAndIncrement()) }) {
            "00000000-0000-0000-0000-${nextId.getAndIncrement().toString().padStart(12, '0')}"
        }
        val first = repository.create(TaskDraft("low", priority = TaskPriority.LOW, dueDate = LocalDate.parse("2020-01-01")))
        val high = repository.create(TaskDraft("high", priority = TaskPriority.HIGH, dueDate = LocalDate.parse("2020-01-01")))
        assertEquals(listOf(high.id, first.id), repository.list().map { it.id })
        assertEquals(listOf(high.id, first.id), repository.overdue(Instant.parse("2021-01-01T00:00:00Z")).map { it.id })
        repository.delete(first.id, first.revision)
        assertThrows(TaskNotFoundException::class.java) { runBlocking { repository.update(first.id, first.revision, TaskDraft("late")) } }
        assertNull(repository.get(first.id))
    }

    @Test fun duplicateCreateIdRejectedAndConcurrentMutationIsConsistent() = runBlocking {
        val created = repository.create(TaskDraft("original"))
        assertThrows(TaskConflictException::class.java) { runBlocking { repository.create(TaskDraft("duplicate")) } }
        val first = async { runCatching { repository.update(created.id, created.revision, TaskDraft("first")) } }
        val second = async { runCatching { repository.update(created.id, created.revision, TaskDraft("second")) } }
        assertEquals(1, listOf(first.await(), second.await()).count { it.isSuccess })
        assertEquals(2L, repository.get(created.id)?.revision)
    }

    @Test fun daoCancellationDuringUpdatePropagatesWithoutBecomingStorageFailure() = runBlocking {
        val memoryDao = MemoryDao()
        var cancelWrites = false
        val dao = object : TaskDao by memoryDao {
            override suspend fun compareAndSet(id: String, expectedRevision: Long, nextRevision: Long, encrypted: String): Int {
                if (cancelWrites) throw CancellationException("DAO update was cancelled")
                return memoryDao.compareAndSet(id, expectedRevision, nextRevision, encrypted)
            }
        }
        val repository = EncryptedTaskRepository(dao, TestCipher(), { Instant.ofEpochMilli(tick.getAndIncrement()) }, { "00000000-0000-0000-0000-000000000002" })
        val created = repository.create(TaskDraft("cancelled update"))
        cancelWrites = true

        val error = assertThrows(CancellationException::class.java) {
            runBlocking { repository.update(created.id, created.revision, TaskDraft("must not be saved")) }
        }

        assertEquals("DAO update was cancelled", error.message)
        cancelWrites = false
        assertEquals(created, repository.get(created.id))
    }

    @Test fun approvedMutationsConsumeDurableProposalReceiptsAcrossRepositoryRecreation() = runBlocking {
        val sharedDao = MemoryDao()
        val nextId = AtomicLong(1)
        val clock = { Instant.ofEpochMilli(tick.getAndIncrement()) }
        fun reopenedRepository() = EncryptedTaskRepository(sharedDao, TestCipher(), clock) {
            "00000000-0000-0000-0000-${nextId.getAndIncrement().toString().padStart(12, '0')}"
        }
        val first = reopenedRepository()
        val confirmer = TaskProposalConfirmer(first)
        val create = TaskProposal(
            proposalId = "10000000-0000-0000-0000-000000000001",
            operation = TaskProposalOperation.CREATE,
            title = "approved create",
        )
        val created = checkNotNull(confirmer.confirm(create, explicitlyApproved = true))
        assertThrows(TaskConflictException::class.java) {
            runBlocking { TaskProposalConfirmer(reopenedRepository()).confirm(create, explicitlyApproved = true) }
        }

        val target = first.create(TaskDraft("before update"))
        val update = TaskProposal(
            proposalId = "10000000-0000-0000-0000-000000000002",
            operation = TaskProposalOperation.UPDATE,
            title = "after update",
            targetTaskId = target.id,
            expectedRevision = target.revision,
        )
        val updated = checkNotNull(confirmer.confirm(update, true, target.id))
        assertEquals(2L, updated.revision)
        assertThrows(TaskConflictException::class.java) {
            runBlocking { TaskProposalConfirmer(reopenedRepository()).confirm(update, true, target.id) }
        }

        val complete = TaskProposal(
            proposalId = "10000000-0000-0000-0000-000000000003",
            operation = TaskProposalOperation.COMPLETE,
            targetTaskId = updated.id,
            expectedRevision = updated.revision,
        )
        val completed = checkNotNull(confirmer.confirm(complete, true, updated.id))
        assertEquals(TaskStatus.COMPLETED, completed.status)
        assertThrows(TaskConflictException::class.java) {
            runBlocking { TaskProposalConfirmer(reopenedRepository()).confirm(complete, true, updated.id) }
        }

        val removeTarget = first.create(TaskDraft("approved delete"))
        val delete = TaskProposal(
            proposalId = "10000000-0000-0000-0000-000000000004",
            operation = TaskProposalOperation.DELETE,
            targetTaskId = removeTarget.id,
            expectedRevision = removeTarget.revision,
        )
        assertNull(confirmer.confirm(delete, true, removeTarget.id))
        assertThrows(TaskConflictException::class.java) {
            runBlocking { TaskProposalConfirmer(reopenedRepository()).confirm(delete, true, removeTarget.id) }
        }

        val restored = reopenedRepository()
        assertEquals(created, restored.get(created.id))
        assertEquals("after update", restored.get(updated.id)?.title)
        assertEquals(TaskStatus.COMPLETED, restored.get(updated.id)?.status)
        assertNull(restored.get(removeTarget.id))
        assertEquals(removeTarget.id, sharedDao.receipt(delete.proposalId)?.taskId)
    }

    @Test fun lostMutationOutcomeRemainsUnknownAndReceiptBlocksReplayAfterRecreation() = runBlocking {
        val backing = MemoryDao()
        var loseNextResult = false
        val faultingDao = object : TaskDao by backing {
            override suspend fun compareAndSetTaskForProposal(
                id: String,
                expectedRevision: Long,
                nextRevision: Long,
                encrypted: String,
                receipt: ProposalReceiptRow,
            ) {
                backing.compareAndSetTaskForProposal(id, expectedRevision, nextRevision, encrypted, receipt)
                if (loseNextResult) throw CancellationException("fixture lost the committed result")
            }
        }
        val stableClock = { Instant.parse("2026-10-20T10:00:00Z") }
        val initialRepository = EncryptedTaskRepository(faultingDao, TestCipher(), stableClock) {
            "00000000-0000-0000-0000-000000000099"
        }
        val target = initialRepository.create(TaskDraft("before"))
        val proposal = TaskProposal(
            proposalId = "10000000-0000-0000-0000-000000000009",
            operation = TaskProposalOperation.UPDATE,
            targetTaskId = target.id,
            expectedRevision = target.revision,
            title = "committed but result lost",
        )
        val session = TaskProposalReviewSession.prepare(proposal, initialRepository, stableClock)
        loseNextResult = true
        val call = async { session.approve(explicitlyApproved = true, selectedTaskId = target.id) }
        try {
            call.await()
            fail("the simulated lost result must propagate cancellation")
        } catch (_: CancellationException) { }
        assertEquals(TaskProposalReviewStatus.OUTCOME_UNKNOWN, session.status)

        val reopened = EncryptedTaskRepository(backing, TestCipher(), stableClock) {
            "00000000-0000-0000-0000-000000000100"
        }
        assertEquals("committed but result lost", reopened.get(target.id)?.title)
        assertEquals(2L, reopened.get(target.id)?.revision)
        assertThrows(TaskConflictException::class.java) {
            runBlocking { TaskProposalConfirmer(reopened).confirm(proposal, true, target.id) }
        }
        assertEquals(2L, reopened.get(target.id)?.revision)
    }

    @Test fun durableBridgeStillRequiresApprovalMatchingDeleteTargetAndFreshReview() = runBlocking {
        val sharedDao = MemoryDao()
        val now = Instant.parse("2026-10-20T10:00:00Z")
        val repository = EncryptedTaskRepository(sharedDao, TestCipher(), { now }) {
            "00000000-0000-0000-0000-000000000101"
        }
        val target = repository.create(TaskDraft("delete only after confirmation"))
        val proposal = TaskProposal(
            proposalId = "10000000-0000-0000-0000-000000000010",
            operation = TaskProposalOperation.DELETE,
            targetTaskId = target.id,
            expectedRevision = target.revision,
        )
        val confirmer = TaskProposalConfirmer(repository)

        assertThrows(SecurityException::class.java) {
            runBlocking { confirmer.confirm(proposal, explicitlyApproved = false, confirmedTargetTaskId = target.id) }
        }
        assertThrows(SecurityException::class.java) {
            runBlocking { confirmer.confirm(proposal, explicitlyApproved = true, confirmedTargetTaskId = null) }
        }
        assertEquals(target, repository.get(target.id))
        assertEquals(null, sharedDao.receipt(proposal.proposalId))

        var reviewClock = now
        val review = TaskProposalReviewSession.prepare(proposal, repository) { reviewClock }
        reviewClock = review.snapshot.expiresAt.plusMillis(1)
        assertTrue(review.expireIfNeeded())
        assertEquals(TaskProposalReviewStatus.EXPIRED, review.status)
        assertEquals(TaskProposalReviewResult.Expired, review.approve(true, selectedTaskId = target.id))
        assertEquals(target, repository.get(target.id))
        assertEquals(null, sharedDao.receipt(proposal.proposalId))
    }
}
