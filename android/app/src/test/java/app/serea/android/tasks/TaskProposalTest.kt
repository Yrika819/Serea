package app.serea.android.tasks

import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import java.util.UUID
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test

class TaskProposalTest {
    private class RecordingRepository : TaskRepository {
        var writes = 0
        private val proposalIds = mutableSetOf<String>()
        private val created = mutableListOf<Task>()
        override suspend fun create(draft: TaskDraft): Task { writes++; return Task(UUID.randomUUID().toString(), draft.title, draft.description, TaskStatus.TODO, draft.priority, Instant.EPOCH, Instant.EPOCH, draft.dueDate, draft.dueTime, draft.timeZoneId).also(created::add) }
        override suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task {
            if (!proposalIds.add(proposalId)) throw TaskConflictException()
            return create(draft)
        }
        override suspend fun get(id: String) = created.firstOrNull { it.id == id }
        override suspend fun list() = created.toList()
        override suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task = error("unused")
        override suspend fun complete(id: String, expectedRevision: Long): Task = error("unused")
        override suspend fun reopen(id: String, expectedRevision: Long): Task = error("unused")
        override suspend fun delete(id: String, expectedRevision: Long) { error("unused") }
        override suspend fun incomplete() = created
        override suspend fun completed() = emptyList<Task>()
        override suspend fun dueToday(today: LocalDate) = emptyList<Task>()
        override suspend fun overdue(now: Instant, fallbackZone: java.time.ZoneId) = emptyList<Task>()
    }

    @Test fun creatingAndValidatingProposalNeverWrites() = runBlocking {
        val repo = RecordingRepository()
        val proposal = TaskProposal(operation = TaskProposalOperation.CREATE, title = "Suggested", description = "later",
            dueDate = LocalDate.parse("2026-12-01"), dueTime = LocalTime.of(10, 0), timeZoneId = "Asia/Tokyo")
        assertEquals(ProposalValidation.Valid, proposal.validate())
        assertEquals(0, repo.writes)
        assertThrows(SecurityException::class.java) { runBlocking { TaskProposalConfirmer(repo).confirm(proposal, false) } }
        assertEquals(0, repo.writes)
    }

    @Test fun invalidProposalAndDuplicateConfirmationAreRejected() = runBlocking {
        val repo = RecordingRepository()
        val confirmer = TaskProposalConfirmer(repo)
        val invalid = TaskProposal(operation = TaskProposalOperation.CREATE, title = " ")
        assertTrue(invalid.validate() is ProposalValidation.Invalid)
        assertThrows(InvalidTaskException::class.java) { runBlocking { confirmer.confirm(invalid, true) } }
        assertEquals(0, repo.writes)
        val proposal = TaskProposal(operation = TaskProposalOperation.CREATE, title = "approved")
        assertNotNull(confirmer.confirm(proposal, true))
        assertThrows(TaskConflictException::class.java) { runBlocking { confirmer.confirm(proposal, true) } }
        assertEquals(1, repo.writes)
        val update = TaskProposal(operation = TaskProposalOperation.UPDATE, title = "x", targetTaskId = "00000000-0000-0000-0000-000000000001", expectedRevision = 2)
        assertThrows(SecurityException::class.java) { runBlocking { confirmer.confirm(update, true, "00000000-0000-0000-0000-000000000002") } }
        assertEquals(1, repo.writes)
    }
}
