package app.serea.android.assistant.taskproposal

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.tasks.*
import java.time.Instant
import java.time.LocalDate
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TaskProposalReviewScreenTest {
    @get:Rule val compose = createComposeRule()

    private class FakeRepository(private val target: Task) : TaskRepository {
        @Volatile var deleted = false
        override suspend fun create(draft: TaskDraft): Task = error("unused")
        override suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task = error("unused")
        override suspend fun get(id: String): Task? = target.takeIf { id == target.id && !deleted }
        override suspend fun list() = listOfNotNull(get(target.id))
        override suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task = error("unused")
        override suspend fun complete(id: String, expectedRevision: Long): Task = error("unused")
        override suspend fun reopen(id: String, expectedRevision: Long): Task = error("unused")
        override suspend fun delete(id: String, expectedRevision: Long) {
            if (id != target.id || expectedRevision != target.revision || deleted) throw TaskConflictException()
            deleted = true
        }
        override suspend fun incomplete() = emptyList<Task>()
        override suspend fun completed() = emptyList<Task>()
        override suspend fun dueToday(today: LocalDate) = emptyList<Task>()
        override suspend fun overdue(now: Instant, fallbackZone: java.time.ZoneId) = emptyList<Task>()
    }

    @Test fun deleteAcknowledgementIsResetForNewSessionWithSameProposalId() {
        val target = task()
        val repository = FakeRepository(target)
        val proposal = TaskProposal(
            proposalId = "00000000-0000-0000-0000-000000000088",
            operation = TaskProposalOperation.DELETE,
            targetTaskId = target.id,
            expectedRevision = target.revision,
        )
        val (first, next) = runBlocking {
            TaskProposalReviewSession.prepare(proposal, repository) to TaskProposalReviewSession.prepare(proposal, repository)
        }
        val shownSession = mutableStateOf(first)

        compose.setContent {
            TaskProposalReviewScreen(
                session = shownSession.value,
                selectedTaskId = target.id,
                onResult = {},
            )
        }
        compose.onNodeWithTag("task_proposal_delete_ack").assertIsOff()
        compose.onNodeWithTag("task_proposal_delete_ack").performClick()
        compose.onNodeWithTag("task_proposal_delete_ack").assertIsOn()

        compose.runOnIdle { shownSession.value = next }
        compose.onNodeWithTag("task_proposal_delete_ack").assertIsOff()
        compose.onNodeWithTag("task_proposal_approve").assertIsNotEnabled()
        assertTrue(!repository.deleted)

        compose.onNodeWithTag("task_proposal_delete_ack").performClick()
        compose.onNodeWithTag("task_proposal_approve").performClick()
        compose.waitUntil(5_000) { repository.deleted }
        assertNull(runBlocking { repository.get(target.id) })
    }

    @Test fun expiredSessionShowsMessageAndCannotApproveUntilNewSessionIsConfirmed() {
        val target = task()
        val repository = FakeRepository(target)
        var now = Instant.parse("2026-10-20T12:00:00Z")
        val proposal = TaskProposal(
            operation = TaskProposalOperation.DELETE,
            targetTaskId = target.id,
            expectedRevision = target.revision,
        )
        val expired = runBlocking { TaskProposalReviewSession.prepare(proposal, repository) { now } }
        now = expired.snapshot.expiresAt.plusMillis(1)
        val shownSession = mutableStateOf(expired)
        compose.setContent {
            TaskProposalReviewScreen(
                session = shownSession.value,
                selectedTaskId = target.id,
                onResult = {},
            )
        }
        assertTrue(runBlocking { expired.expireIfNeeded() })
        compose.onNodeWithText("This review expired. Prepare a new review and approve it again.").assertExists()
        compose.onNodeWithTag("task_proposal_approve").assertIsNotEnabled()
        assertTrue(!repository.deleted)

        val fresh = runBlocking { TaskProposalReviewSession.prepare(proposal, repository) { now } }
        compose.runOnIdle { shownSession.value = fresh }
        compose.onNodeWithText("I understand this permanently deletes the task.").assertExists()
        compose.onNodeWithTag("task_proposal_approve").assertIsNotEnabled()
        compose.onNodeWithTag("task_proposal_delete_ack").performClick()
        compose.onNodeWithTag("task_proposal_approve").performClick()
        compose.waitUntil(5_000) { repository.deleted }
    }

    private fun task() = Task(
        id = "00000000-0000-0000-0000-000000000077",
        title = "Fixture task",
        description = "Fixture description",
        status = TaskStatus.TODO,
        priority = TaskPriority.NORMAL,
        createdAt = Instant.EPOCH,
        updatedAt = Instant.EPOCH,
        revision = 4,
    )
}
