package app.serea.android.tasks

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.assertIsOn
import androidx.compose.ui.test.assertIsOff
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import java.time.Instant
import java.time.LocalDate
import java.util.UUID
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TaskScreenTest {
    @get:Rule val compose = createComposeRule()

    private class FakeRepository : TaskRepository {
        private val tasks = linkedMapOf<String, Task>()
        private val proposalIds = mutableSetOf<String>()
        private var counter = 1
        private var tick = 1_800_000_000_000L
        private fun now() = Instant.ofEpochMilli(tick++)
        @Synchronized override suspend fun create(draft: TaskDraft): Task {
            TaskValidation.validateDraft(draft)
            val id = "00000000-0000-0000-0000-${(counter++).toString().padStart(12, '0')}"
            return Task(id, draft.title, draft.description, TaskStatus.TODO, draft.priority, now(), now(), draft.dueDate, draft.dueTime, draft.timeZoneId).also { tasks[id] = it }
        }
        @Synchronized override suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task {
            check(proposalIds.add(proposalId))
            return create(draft)
        }
        @Synchronized override suspend fun get(id: String) = tasks[id]
        @Synchronized override suspend fun list() = tasks.values.sortedBy { it.id }
        @Synchronized override suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task {
            TaskValidation.validateDraft(draft)
            val old = tasks[id] ?: throw TaskNotFoundException()
            if (old.revision != expectedRevision) throw TaskConflictException()
            return old.copy(title = draft.title, description = draft.description, priority = draft.priority, dueDate = draft.dueDate,
                dueTime = draft.dueTime, timeZoneId = draft.timeZoneId, updatedAt = now(), revision = old.revision + 1).also { tasks[id] = it }
        }
        @Synchronized override suspend fun complete(id: String, expectedRevision: Long): Task = mutate(id, expectedRevision) { it.copy(status = TaskStatus.COMPLETED, completedAt = now(), updatedAt = now()) }
        @Synchronized override suspend fun reopen(id: String, expectedRevision: Long): Task = mutate(id, expectedRevision) { it.copy(status = TaskStatus.TODO, completedAt = null, updatedAt = now()) }
        @Synchronized override suspend fun delete(id: String, expectedRevision: Long) {
            val old = tasks[id] ?: throw TaskNotFoundException()
            if (old.revision != expectedRevision) throw TaskConflictException()
            tasks.remove(id)
        }
        override suspend fun incomplete() = list().filter { it.status == TaskStatus.TODO }
        override suspend fun completed() = list().filter { it.status == TaskStatus.COMPLETED }
        override suspend fun dueToday(today: LocalDate) = incomplete().filter { it.dueDate == today }
        override suspend fun overdue(now: Instant, fallbackZone: java.time.ZoneId) = incomplete().filter { it.isOverdue(now) }
        private fun mutate(id: String, revision: Long, update: (Task) -> Task): Task {
            val old = tasks[id] ?: throw TaskNotFoundException()
            if (old.revision != revision) throw TaskConflictException()
            return update(old).copy(revision = old.revision + 1).also { tasks[id] = it }
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
        compose.onNodeWithTag("task_editor_error").assertIsDisplayed()
        compose.onNodeWithTag("task_title").performTextInput("Valid title")
        compose.onNodeWithTag("task_due_date").performTextInput("not-a-date")
        compose.onNodeWithTag("task_save").performClick()
        compose.onNodeWithTag("task_editor_error").assertIsDisplayed()
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

    private fun show(repository: FakeRepository) = compose.setContent {
        app.serea.android.SereaTheme { TaskScreen(TaskViewModel(repository)) }
    }

    private fun create(repository: FakeRepository, title: String) {
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput(title)
        compose.onNodeWithTag("task_save").performClick()
        compose.waitUntil(5_000) { runBlocking { repository.list().any { it.title == title } } }
    }
}
