package app.serea.android.tasks

import java.time.Instant
import java.time.LocalDate
import java.util.concurrent.atomic.AtomicLong
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
    private val repository = EncryptedTaskRepository(MemoryDao(), TestCipher(), { Instant.ofEpochMilli(tick.getAndIncrement()) }, { "00000000-0000-0000-0000-000000000001" })

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
        assertEquals(listOf(first.id, high.id), repository.overdue(Instant.parse("2021-01-01T00:00:00Z")).map { it.id })
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
}
