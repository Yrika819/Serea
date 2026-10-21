package app.serea.android.tasks

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import java.time.Instant
import java.time.LocalDate
import java.util.UUID
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.CompletableDeferred
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.After
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TaskScreenTest {
    @get:Rule val compose = createComposeRule()
    private val viewModelStore = ViewModelStore()

    @After fun clearViewModels() = viewModelStore.clear()

    private class FakeRepository : TaskRepository {
        private val lock = Any()
        private val tasks = linkedMapOf<String, Task>()
        private val proposalIds = mutableSetOf<String>()
        private var counter = 1
        private var tick = 1_800_000_000_000L
        var updateEntered: CompletableDeferred<Unit>? = null
        var allowUpdate: CompletableDeferred<Unit>? = null
        var updateFailure: Exception? = null
        private fun now() = synchronized(lock) { Instant.ofEpochMilli(tick++) }
        fun seed(task: Task) { synchronized(lock) { tasks[task.id] = task } }
        override suspend fun create(draft: TaskDraft): Task {
            TaskValidation.validateDraft(draft)
            return synchronized(lock) {
                val id = "00000000-0000-0000-0000-${(counter++).toString().padStart(12, '0')}"
                Task(id, draft.title, draft.description, TaskStatus.TODO, draft.priority, now(), now(), draft.dueDate, draft.dueTime, draft.timeZoneId)
                    .also { tasks[id] = it }
            }
        }
        override suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task {
            check(synchronized(lock) { proposalIds.add(proposalId) })
            return create(draft)
        }
        override suspend fun get(id: String) = synchronized(lock) { tasks[id] }
        override suspend fun list() = synchronized(lock) { tasks.values.sortedBy { it.id } }
        override suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task {
            TaskValidation.validateDraft(draft)
            updateEntered?.complete(Unit)
            allowUpdate?.await()
            updateFailure?.let { throw it }
            return synchronized(lock) {
                val old = tasks[id] ?: throw TaskNotFoundException()
                if (old.revision != expectedRevision) throw TaskConflictException()
                old.copy(title = draft.title, description = draft.description, priority = draft.priority, dueDate = draft.dueDate,
                    dueTime = draft.dueTime, timeZoneId = draft.timeZoneId, updatedAt = now(), revision = old.revision + 1)
                    .also { tasks[id] = it }
            }
        }
        override suspend fun complete(id: String, expectedRevision: Long): Task = mutate(id, expectedRevision) { it.copy(status = TaskStatus.COMPLETED, completedAt = now(), updatedAt = now()) }
        override suspend fun reopen(id: String, expectedRevision: Long): Task = mutate(id, expectedRevision) { it.copy(status = TaskStatus.TODO, completedAt = null, updatedAt = now()) }
        override suspend fun delete(id: String, expectedRevision: Long) {
            synchronized(lock) {
                val old = tasks[id] ?: throw TaskNotFoundException()
                if (old.revision != expectedRevision) throw TaskConflictException()
                tasks.remove(id)
            }
        }
        override suspend fun incomplete() = list().filter { it.status == TaskStatus.TODO }
        override suspend fun completed() = list().filter { it.status == TaskStatus.COMPLETED }
        override suspend fun dueToday(today: LocalDate) = incomplete().filter { it.dueDate == today }
        override suspend fun overdue(now: Instant, fallbackZone: java.time.ZoneId) = incomplete().filter { it.isOverdue(now) }
        private fun mutate(id: String, revision: Long, update: (Task) -> Task): Task {
            return synchronized(lock) {
                val old = tasks[id] ?: throw TaskNotFoundException()
                if (old.revision != revision) throw TaskConflictException()
                update(old).copy(revision = old.revision + 1).also { tasks[id] = it }
            }
        }
    }

    @Test fun emptyCreateEditCompleteAndReopenAreReachable() {
        val repository = FakeRepository()
        show(repository)
        compose.waitUntil(5_000) { compose.onAllNodesWithTag("task_empty").fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithTag("task_empty").assertIsDisplayed()
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput("Review 📚")
        compose.onNodeWithTag("task_description").performTextInput("notes")
        compose.onNodeWithTag("task_due_date").performTextInput("2026-11-01")
        compose.onNodeWithTag("task_save").performClick()
        compose.waitUntil(5_000) { runBlocking { repository.list().isNotEmpty() } }
        var task = runBlocking { repository.list().single() }
        compose.waitUntil(5_000) { compose.onAllNodesWithText("Review 📚").fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithText("Review 📚").assertIsDisplayed()
        compose.onNodeWithTag("task_complete_${task.id}").performClick()
        compose.waitUntil(5_000) { runBlocking { repository.get(task.id)?.status == TaskStatus.COMPLETED } }
        compose.onNodeWithTag("task_complete_${task.id}").assertIsOn()
        compose.onNodeWithTag("task_complete_${task.id}").performClick()
        compose.waitUntil(5_000) { runBlocking { repository.get(task.id)?.status == TaskStatus.TODO } }
        compose.onNodeWithTag("task_complete_${task.id}").assertIsOff()
        compose.onNodeWithTag("task_edit_${task.id}").performClick()
        compose.onNodeWithTag("task_title").performTextClearance()
        compose.onNodeWithTag("task_title").performTextInput("Edited title")
        compose.onNodeWithTag("task_save").performClick()
        compose.waitUntil(5_000) { runBlocking { repository.get(task.id)?.title == "Edited title" } }
        compose.waitUntil(5_000) { compose.onAllNodesWithText("Edited title").fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithText("Edited title").assertIsDisplayed()
    }

    @Test fun invalidTitleAndDateAreShownWithoutRepositoryWrite() {
        val repository = FakeRepository()
        show(repository)
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_save").performClick()
        val titleError = "タイトルを入力してください。"
        compose.onNodeWithTag("task_editor_scroll").performScrollToNode(hasText(titleError))
        compose.onNodeWithText(titleError).assertIsDisplayed()
        compose.onNodeWithTag("task_title").performTextInput("Valid title")
        compose.onNodeWithTag("task_due_date").performTextInput("not-a-date")
        compose.onNodeWithTag("task_save").performClick()
        val dateError = "期限日は YYYY-MM-DD 形式で入力してください。"
        compose.onNodeWithTag("task_editor_scroll").performScrollToNode(hasText(dateError))
        compose.onNodeWithText(dateError).assertIsDisplayed()
        assertEquals(0, runBlocking { repository.list().size })
    }

    @Test fun deleteCancelPreservesTaskAndConfirmRemovesIt() {
        val repository = FakeRepository()
        show(repository)
        create(repository, "Keep or delete")
        val task = runBlocking { repository.list().single() }
        compose.onNodeWithTag("task_delete_${task.id}").performClick()
        compose.onNodeWithTag("task_delete_cancel").performClick()
        assertEquals(1, runBlocking { repository.list().size })
        compose.onNodeWithTag("task_delete_${task.id}").performClick()
        compose.onNodeWithTag("task_delete_confirm").performClick()
        compose.waitUntil(5_000) { runBlocking { repository.list().isEmpty() } }
        compose.waitUntil(5_000) { compose.onAllNodesWithTag("task_empty").fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithTag("task_empty").assertIsDisplayed()
    }

    @Test fun deletingFrozenTaskIdCannotDeleteAnotherTask() {
        val repository = FakeRepository()
        show(repository)
        create(repository, "First")
        create(repository, "Second")
        val ids = runBlocking { repository.list().map { it.id } }
        compose.onNodeWithTag("task_delete_${ids.first()}").performClick()
        compose.onNodeWithTag("task_delete_cancel").performClick()
        assertEquals(listOf("First", "Second"), runBlocking { repository.list().map { it.title } })
        compose.onNodeWithTag("task_delete_${ids.last()}").performClick()
        compose.onNodeWithTag("task_delete_confirm").performClick()
        compose.waitUntil(5_000) { runBlocking { repository.get(ids.last()) == null } }
        assertEquals("First", runBlocking { repository.get(ids.first())?.title })
    }

    @Test fun lateSaveFromClosedEditorPreservesTheNewEditorsUnsavedDraft() {
        val repository = FakeRepository().apply {
            seed(Task("00000000-0000-0000-0000-000000000001", "Task A", "", TaskStatus.TODO,
                TaskPriority.NORMAL, Instant.EPOCH, Instant.EPOCH))
            seed(Task("00000000-0000-0000-0000-000000000002", "Task B", "", TaskStatus.TODO,
                TaskPriority.NORMAL, Instant.EPOCH, Instant.EPOCH))
            updateEntered = CompletableDeferred()
            allowUpdate = CompletableDeferred()
        }
        val viewModel = show(repository)
        compose.waitUntil(5_000) { compose.onAllNodesWithText("Task A").fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithTag("task_edit_00000000-0000-0000-0000-000000000001").performClick()
        compose.onNodeWithTag("task_title").performTextClearance()
        compose.onNodeWithTag("task_title").performTextInput("Task A saved")
        compose.onNodeWithTag("task_save").performClick()
        compose.waitUntil(5_000) { runBlocking { repository.updateEntered!!.isCompleted } }
        compose.onNodeWithTag("task_title").assertIsNotEnabled()
        compose.onNodeWithTag("task_editor_saving").assertIsDisplayed()
        compose.onNodeWithText("閉じる").assertIsDisplayed()

        compose.onNodeWithTag("task_cancel").performClick()
        compose.onNodeWithTag("task_edit_00000000-0000-0000-0000-000000000001").assertIsNotEnabled()
        compose.onNodeWithTag("task_edit_00000000-0000-0000-0000-000000000002").performClick()
        compose.onNodeWithTag("task_title").assertIsEnabled()
        compose.onNodeWithTag("task_save").assertIsNotEnabled()
        compose.onNodeWithTag("task_editor_waiting").assertIsDisplayed()
        compose.onNodeWithTag("task_title").performTextClearance()
        compose.onNodeWithTag("task_title").performTextInput("Task B unsaved draft")
        repository.allowUpdate!!.complete(Unit)
        compose.waitUntil(5_000) { !viewModel.state.value.saving }

        compose.waitUntil(5_000) { runBlocking { repository.get("00000000-0000-0000-0000-000000000001")?.title == "Task A saved" } }
        compose.onNodeWithTag("task_operation_notice").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_operation_notice").assertTextContains("保存が完了しました。", substring = true)
        compose.waitUntil(5_000) {
            val config = compose.onNodeWithTag("task_title").fetchSemanticsNode().config
            SemanticsProperties.EditableText in config && config[SemanticsProperties.EditableText].text == "Task B unsaved draft"
        }
        compose.onNodeWithTag("task_save").assertIsEnabled()
        val draftConfig = compose.onNodeWithTag("task_title").fetchSemanticsNode().config
        assertTrue(SemanticsProperties.EditableText in draftConfig)
        assertEquals("Task B unsaved draft", draftConfig[SemanticsProperties.EditableText].text)
        assertEquals("Task B", runBlocking { repository.get("00000000-0000-0000-0000-000000000002")?.title })

        compose.onNodeWithTag("task_cancel").performClick()
        compose.onNodeWithTag("task_edit_00000000-0000-0000-0000-000000000001").performClick()
        assertTrue(compose.onAllNodesWithTag("task_operation_notice").fetchSemanticsNodes().isEmpty())
        repository.updateEntered = CompletableDeferred()
        repository.allowUpdate = CompletableDeferred()
        repository.updateFailure = TaskStorageUnavailableException("fixture storage failure")
        compose.onNodeWithTag("task_title").performTextClearance()
        compose.onNodeWithTag("task_title").performTextInput("Task A failed")
        compose.onNodeWithTag("task_save").performClick()
        compose.waitUntil(5_000) { runBlocking { repository.updateEntered!!.isCompleted } }
        compose.onNodeWithTag("task_cancel").performClick()
        compose.onNodeWithTag("task_edit_00000000-0000-0000-0000-000000000002").performClick()
        compose.onNodeWithTag("task_title").performTextClearance()
        compose.onNodeWithTag("task_title").performTextInput("Task B newer draft")
        repository.allowUpdate!!.complete(Unit)
        compose.waitUntil(5_000) { !viewModel.state.value.saving }
        compose.onNodeWithTag("task_operation_notice").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_operation_notice").assertTextContains("保存に失敗しました", substring = true)
        assertTrue(viewModel.state.value.operationNoticeIsError)
        val newerDraft = compose.onNodeWithTag("task_title").fetchSemanticsNode().config
        assertEquals("Task B newer draft", newerDraft[SemanticsProperties.EditableText].text)
        assertEquals("Task A saved", runBlocking { repository.get("00000000-0000-0000-0000-000000000001")?.title })
    }

    private fun show(repository: FakeRepository): TaskViewModel {
        val viewModel = ViewModelProvider(viewModelStore, TaskViewModel.factory(repository))
            .get(TaskViewModel::class.java)
        compose.setContent { app.serea.android.SereaTheme { TaskScreen(viewModel) } }
        return viewModel
    }

    private fun create(repository: FakeRepository, title: String) {
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput(title)
        compose.onNodeWithTag("task_save").performClick()
        compose.waitUntil(5_000) { runBlocking { repository.list().any { it.title == title } } }
    }
}
