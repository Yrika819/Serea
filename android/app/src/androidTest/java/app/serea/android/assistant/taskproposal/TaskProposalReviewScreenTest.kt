package app.serea.android.assistant.taskproposal

import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.click
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTouchInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.tasks.*
import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
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
        @Volatile var writes = 0
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
            writes++
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
        val (first, next, fresh) = runBlocking {
            Triple(
                TaskProposalReviewSession.prepare(proposal, repository),
                TaskProposalReviewSession.prepare(proposal, repository),
                TaskProposalReviewSession.prepare(proposal, repository),
            )
        }
        val createPreview = runBlocking {
            TaskProposalReviewSession.prepare(
                TaskProposal(
                    operation = TaskProposalOperation.CREATE,
                    title = "Created title",
                    description = "Created description",
                    priority = TaskPriority.HIGH,
                    dueDate = LocalDate.parse("2026-10-25"),
                    dueTime = LocalTime.parse("09:30"),
                    timeZoneId = "Asia/Tokyo",
                ),
                repository,
            )
        }
        val updatePreview = runBlocking {
            TaskProposalReviewSession.prepare(
                TaskProposal(
                    operation = TaskProposalOperation.UPDATE,
                    title = "Updated title",
                    description = "Updated description",
                    priority = TaskPriority.HIGH,
                    dueDate = LocalDate.parse("2026-10-25"),
                    targetTaskId = target.id,
                    expectedRevision = target.revision,
                ),
                repository,
            )
        }
        val completePreview = runBlocking {
            TaskProposalReviewSession.prepare(
                TaskProposal(
                    operation = TaskProposalOperation.COMPLETE,
                    targetTaskId = target.id,
                    expectedRevision = target.revision,
                ),
                repository,
            )
        }
        val shownSession = mutableStateOf(first)
        val results = mutableListOf<TaskProposalReviewResult>()
        var closeCount = 0

        compose.setContent {
            TaskProposalReviewScreen(
                session = shownSession.value,
                selectedTaskId = shownSession.value.snapshot.proposal.targetTaskId,
                onResult = { results.add(it) },
                onClose = { closeCount++ },
            )
        }

        compose.runOnIdle { shownSession.value = createPreview }
        compose.onNodeWithText("操作: 新規作成").assertExists()
        compose.onNodeWithText("タイトル: Created title").assertExists()
        compose.onNodeWithText("説明: Created description").assertExists()
        compose.onNodeWithText("優先度: 高").assertExists()
        compose.onNodeWithText("期限日: 2026-10-25").assertExists()
        compose.onNodeWithText("期限時刻: 09:30").assertExists()
        compose.onNodeWithText("タイムゾーン: Asia/Tokyo").assertExists()
        compose.onNodeWithText("対象タスク ID: 承認後に新しく割り当て").assertExists()
        compose.onNodeWithText("作成日時・更新日時: 承認時に設定").assertExists()
        compose.onNodeWithText("完了日時: なし").assertExists()
        assertTrue(repository.writes == 0)

        compose.runOnIdle { shownSession.value = updatePreview }
        compose.onNodeWithText("操作: 更新").assertExists()
        compose.onNodeWithText("対象タスク ID: ${target.id}").assertExists()
        compose.onNodeWithText("期待リビジョン: ${target.revision}").assertExists()
        compose.onNodeWithText("タイトル: Fixture task → Updated title").assertExists()
        compose.onNodeWithText("説明: Fixture description → Updated description").assertExists()
        compose.onNodeWithText("優先度: 標準 → 高").assertExists()
        compose.onNodeWithText("期限日: なし → 2026-10-25").assertExists()
        assertTrue(repository.writes == 0)

        compose.runOnIdle { shownSession.value = completePreview }
        compose.onNodeWithText("操作: 完了").assertExists()
        compose.onNodeWithText("対象タスク: Fixture task").assertExists()
        compose.onNodeWithText("状態: 未完了 → 完了").assertExists()
        assertTrue(repository.writes == 0)

        compose.runOnIdle { shownSession.value = first }
        compose.onNodeWithTag("task_proposal_delete_ack").assertIsOff()
        compose.onNodeWithText("操作: 削除").assertExists()
        compose.onNodeWithText("削除するタスク: Fixture task").assertExists()
        compose.onNodeWithText("対象タスク ID: ${target.id}").assertExists()
        compose.onNodeWithText("期待リビジョン: ${target.revision}").assertExists()
        compose.onNodeWithTag("task_proposal_delete_ack").performClick()
        compose.onNodeWithTag("task_proposal_delete_ack").assertIsOn()
        assertTrue(repository.writes == 0)

        compose.runOnIdle { shownSession.value = next }
        compose.onNodeWithTag("task_proposal_delete_ack").assertIsOff()
        compose.onNodeWithTag("task_proposal_approve").assertIsNotEnabled()
        assertTrue(!repository.deleted)
        assertTrue(repository.writes == 0)

        compose.onNodeWithTag("task_proposal_close").performClick()
        compose.waitUntil(5_000) { closeCount == 1 }
        assertTrue(results.single() == TaskProposalReviewResult.Declined)
        assertTrue(repository.writes == 0)
        assertTrue(!repository.deleted)

        compose.runOnIdle { shownSession.value = fresh }
        compose.onNodeWithTag("task_proposal_delete_ack").assertIsOff()
        compose.onNodeWithTag("task_proposal_delete_ack").performClick()
        compose.onNodeWithTag("task_proposal_approve").performTouchInput { click(); click() }
        compose.waitUntil(5_000) { repository.deleted }
        assertNull(runBlocking { repository.get(target.id) })
        assertTrue(repository.writes == 1)
        assertTrue(results.count { it is TaskProposalReviewResult.Approved } == 1)
        assertTrue(results.size == 2)
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
        val results = mutableListOf<TaskProposalReviewResult>()
        compose.setContent {
            TaskProposalReviewScreen(
                session = shownSession.value,
                selectedTaskId = target.id,
                onResult = { results.add(it) },
            )
        }
        assertTrue(runBlocking { expired.expireIfNeeded() })
        compose.onNodeWithText("期限切れです。状態を再取得・再検証し、新しいレビューで改めて承認してください。同じレビューは再実行できません。").assertExists()
        compose.onNodeWithTag("task_proposal_approve").assertIsNotEnabled()
        assertTrue(!repository.deleted)
        assertTrue(repository.writes == 0)

        val fresh = runBlocking { TaskProposalReviewSession.prepare(proposal, repository) { now } }
        compose.runOnIdle { shownSession.value = fresh }
        compose.onNodeWithText("このタスクを完全に削除することを理解しました。").assertExists()
        compose.onNodeWithTag("task_proposal_approve").assertIsNotEnabled()
        compose.onNodeWithTag("task_proposal_delete_ack").performClick()
        compose.onNodeWithTag("task_proposal_approve").performClick()
        compose.waitUntil(5_000) { repository.deleted }
        assertTrue(repository.writes == 1)
        assertTrue(results.count { it is TaskProposalReviewResult.Approved } == 1)
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
