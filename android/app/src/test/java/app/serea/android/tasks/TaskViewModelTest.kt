package app.serea.android.tasks

import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TestWatcher
import org.junit.runner.Description

@OptIn(ExperimentalCoroutinesApi::class)
class TaskViewModelTest {
    class MainDispatcherRule : TestWatcher() {
        val dispatcher = StandardTestDispatcher()
        override fun starting(description: Description) { Dispatchers.setMain(dispatcher) }
        override fun finished(description: Description) { Dispatchers.resetMain() }
    }

    @get:Rule val mainDispatcher = MainDispatcherRule()

    private class FakeRepository : TaskRepository {
        private val tasks = linkedMapOf<String, Task>()
        var updateEntered: CompletableDeferred<Unit>? = null
        var allowUpdate: CompletableDeferred<Unit>? = null
        var updateFailure: Exception? = null
        var listEntered: CompletableDeferred<Unit>? = null
        var allowList: CompletableDeferred<Unit>? = null
        var listFailure: Exception? = null
        var deleteEntered: CompletableDeferred<Unit>? = null
        var allowDelete: CompletableDeferred<Unit>? = null
        var completeEntered: CompletableDeferred<Unit>? = null
        var allowComplete: CompletableDeferred<Unit>? = null
        var completeFailure: Exception? = null

        fun seed(id: String, title: String) {
            tasks[id] = Task(
                id = id, title = title, description = "", status = TaskStatus.TODO,
                priority = TaskPriority.NORMAL, createdAt = Instant.EPOCH, updatedAt = Instant.EPOCH,
            )
        }

        override suspend fun create(draft: TaskDraft): Task = Task(
            id = "00000000-0000-0000-0000-000000000003", title = draft.title,
            description = draft.description, status = TaskStatus.TODO, priority = draft.priority,
            createdAt = Instant.EPOCH, updatedAt = Instant.EPOCH,
            dueDate = draft.dueDate, dueTime = draft.dueTime, timeZoneId = draft.timeZoneId,
        ).also { tasks[it.id] = it }

        override suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task = create(draft)
        override suspend fun get(id: String): Task? = tasks[id]
        override suspend fun list(): List<Task> {
            listEntered?.complete(Unit)
            allowList?.await()
            listFailure?.let { throw it }
            return tasks.values.toList()
        }

        override suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task {
            updateEntered?.complete(Unit)
            allowUpdate?.await()
            updateFailure?.let { throw it }
            val task = checked(id, expectedRevision).copy(
                title = draft.title, description = draft.description, priority = draft.priority,
                dueDate = draft.dueDate, dueTime = draft.dueTime, timeZoneId = draft.timeZoneId,
                revision = expectedRevision + 1,
            )
            tasks[id] = task
            return task
        }

        override suspend fun complete(id: String, expectedRevision: Long): Task {
            completeEntered?.complete(Unit)
            allowComplete?.await()
            completeFailure?.let { throw it }
            val task = checked(id, expectedRevision).copy(
                status = TaskStatus.COMPLETED,
                completedAt = Instant.EPOCH.plusSeconds(1),
                revision = expectedRevision + 1,
            )
            tasks[id] = task
            return task
        }

        override suspend fun reopen(id: String, expectedRevision: Long): Task =
            checked(id, expectedRevision).copy(status = TaskStatus.TODO, revision = expectedRevision + 1).also { tasks[id] = it }

        override suspend fun delete(id: String, expectedRevision: Long) {
            deleteEntered?.complete(Unit)
            allowDelete?.await()
            checked(id, expectedRevision)
            tasks.remove(id)
        }

        override suspend fun incomplete(): List<Task> = list().filter { it.status == TaskStatus.TODO }
        override suspend fun completed(): List<Task> = list().filter { it.status == TaskStatus.COMPLETED }
        override suspend fun dueToday(today: LocalDate): List<Task> = incomplete().filter { it.dueDate == today }
        override suspend fun overdue(now: Instant, fallbackZone: ZoneId): List<Task> = emptyList()

        private fun checked(id: String, revision: Long): Task {
            val task = tasks[id] ?: throw TaskNotFoundException()
            if (task.revision != revision) throw TaskConflictException()
            return task
        }
    }

    @Test fun lateSaveFromClosedEditorLeavesTheNewEditSessionOpen() = runTest(mainDispatcher.dispatcher) {
        val repository = FakeRepository().apply {
            seed("00000000-0000-0000-0000-000000000001", "Task A")
            seed("00000000-0000-0000-0000-000000000002", "Task B")
            updateEntered = CompletableDeferred()
            allowUpdate = CompletableDeferred()
        }
        val viewModel = TaskViewModel(repository, ioDispatcher = mainDispatcher.dispatcher)
        runCurrent()

        viewModel.editTask("00000000-0000-0000-0000-000000000001")
        viewModel.save(TaskDraft(title = "Task A saved"))
        runCurrent()
        assertTrue(repository.updateEntered!!.isCompleted)

        viewModel.cancelEditor()
        viewModel.editTask("00000000-0000-0000-0000-000000000002")
        val taskBEditor = viewModel.state.value.editing
        assertEquals("00000000-0000-0000-0000-000000000002", taskBEditor?.id)
        assertTrue(viewModel.state.value.editorVisible)

        repository.allowUpdate!!.complete(Unit)
        runCurrent()

        assertEquals("Task A saved", repository.get("00000000-0000-0000-0000-000000000001")?.title)
        assertEquals(taskBEditor, viewModel.state.value.editing)
        assertTrue(viewModel.state.value.editorVisible)
        assertFalse(viewModel.state.value.saving)
        assertNull(viewModel.state.value.message)
        assertNull(viewModel.state.value.error)
    }

    @Test fun lateSaveFailureDoesNotWriteAnErrorIntoANewerEditorSession() = runTest(mainDispatcher.dispatcher) {
        val repository = FakeRepository().apply {
            seed("00000000-0000-0000-0000-000000000001", "Task A")
            seed("00000000-0000-0000-0000-000000000002", "Task B")
            updateEntered = CompletableDeferred()
            allowUpdate = CompletableDeferred()
            updateFailure = TaskStorageUnavailableException("Task A storage failed.")
        }
        val viewModel = TaskViewModel(repository, ioDispatcher = mainDispatcher.dispatcher)
        runCurrent()
        viewModel.editTask("00000000-0000-0000-0000-000000000001")
        viewModel.save(TaskDraft(title = "Task A saved"))
        runCurrent()
        assertTrue(repository.updateEntered!!.isCompleted)

        viewModel.cancelEditor()
        viewModel.addTask()
        assertTrue(viewModel.state.value.editorVisible)
        assertNull(viewModel.state.value.editing)
        repository.allowUpdate!!.complete(Unit)
        runCurrent()

        assertTrue(viewModel.state.value.editorVisible)
        assertNull(viewModel.state.value.editing)
        assertNull(viewModel.state.value.error)
        assertNull(viewModel.state.value.message)
        assertFalse(viewModel.state.value.saving)
    }

    @Test fun lateDeleteDoesNotClearANewerDeleteConfirmation() = runTest(mainDispatcher.dispatcher) {
        val repository = FakeRepository().apply {
            seed("00000000-0000-0000-0000-000000000001", "Task A")
            seed("00000000-0000-0000-0000-000000000002", "Task B")
            deleteEntered = CompletableDeferred()
            allowDelete = CompletableDeferred()
        }
        val viewModel = TaskViewModel(repository, ioDispatcher = mainDispatcher.dispatcher)
        runCurrent()
        viewModel.requestDelete("00000000-0000-0000-0000-000000000001")
        viewModel.confirmDelete()
        runCurrent()
        assertTrue(repository.deleteEntered!!.isCompleted)

        viewModel.cancelDelete()
        viewModel.requestDelete("00000000-0000-0000-0000-000000000002")
        repository.allowDelete!!.complete(Unit)
        runCurrent()

        assertNull(repository.get("00000000-0000-0000-0000-000000000001"))
        assertEquals("00000000-0000-0000-0000-000000000002", viewModel.state.value.deleteTargetId)
        assertEquals(1L, viewModel.state.value.deleteTargetRevision)
        assertNull(viewModel.state.value.message)
        assertFalse(viewModel.state.value.saving)
    }

    @Test fun lateCompletionRefreshUpdatesTasksWithoutChangingANewerAddEditor() = runTest(mainDispatcher.dispatcher) {
        val repository = FakeRepository().apply {
            seed("00000000-0000-0000-0000-000000000001", "Task A")
            seed("00000000-0000-0000-0000-000000000002", "Task B")
            completeEntered = CompletableDeferred()
            allowComplete = CompletableDeferred()
        }
        val viewModel = TaskViewModel(repository, ioDispatcher = mainDispatcher.dispatcher)
        runCurrent()
        viewModel.toggleComplete("00000000-0000-0000-0000-000000000001")
        runCurrent()
        assertTrue(repository.completeEntered!!.isCompleted)

        viewModel.addTask()
        repository.allowComplete!!.complete(Unit)
        runCurrent()

        assertEquals(TaskStatus.COMPLETED, viewModel.state.value.tasks.first { it.id.endsWith("0001") }.status)
        assertTrue(viewModel.state.value.editorVisible)
        assertNull(viewModel.state.value.editing)
        assertFalse(viewModel.state.value.saving)
    }

    @Test fun failedRefreshAfterEditorChangesAlwaysClearsLoadingWithoutStaleError() = runTest(mainDispatcher.dispatcher) {
        val repository = FakeRepository().apply { seed("00000000-0000-0000-0000-000000000001", "Task A") }
        val viewModel = TaskViewModel(repository, ioDispatcher = mainDispatcher.dispatcher)
        runCurrent()
        repository.listEntered = CompletableDeferred()
        repository.allowList = CompletableDeferred()
        repository.listFailure = TaskStorageUnavailableException("List failed.")

        viewModel.refresh()
        runCurrent()
        assertTrue(repository.listEntered!!.isCompleted)
        assertTrue(viewModel.state.value.loading)
        viewModel.addTask()
        repository.allowList!!.complete(Unit)
        runCurrent()

        assertFalse(viewModel.state.value.loading)
        assertTrue(viewModel.state.value.editorVisible)
        assertNull(viewModel.state.value.error)
    }

    @Test fun failedPostMutationRefreshClearsLoadingAndReportsStaleList() = runTest(mainDispatcher.dispatcher) {
        val repository = FakeRepository().apply { seed("00000000-0000-0000-0000-000000000001", "Task A") }
        val viewModel = TaskViewModel(repository, ioDispatcher = mainDispatcher.dispatcher)
        runCurrent()
        repository.listFailure = TaskStorageUnavailableException("List failed.")

        viewModel.toggleComplete("00000000-0000-0000-0000-000000000001")
        runCurrent()

        assertEquals(TaskStatus.COMPLETED, repository.get("00000000-0000-0000-0000-000000000001")?.status)
        assertFalse(viewModel.state.value.loading)
        assertFalse(viewModel.state.value.saving)
        assertEquals("Task operation completed, but the task list could not be refreshed.", viewModel.state.value.error)
    }

    @Test fun cancelledMutationDoesNotLeaveAnInvalidatedRefreshLoading() = runTest(mainDispatcher.dispatcher) {
        val repository = FakeRepository().apply {
            seed("00000000-0000-0000-0000-000000000001", "Task A")
            completeFailure = CancellationException("Mutation was cancelled before it completed.")
        }
        val viewModel = TaskViewModel(repository, ioDispatcher = mainDispatcher.dispatcher)
        runCurrent()

        repository.listEntered = CompletableDeferred()
        repository.allowList = CompletableDeferred()
        viewModel.refresh()
        runCurrent()
        assertTrue(repository.listEntered!!.isCompleted)
        assertTrue(viewModel.state.value.loading)

        viewModel.toggleComplete("00000000-0000-0000-0000-000000000001")
        runCurrent()

        assertFalse(viewModel.state.value.saving)
        assertFalse(viewModel.state.value.loading)
        assertEquals("Task operation was interrupted. Refresh the task list to confirm its result.", viewModel.state.value.error)

        repository.allowList!!.complete(Unit)
        runCurrent()

        assertFalse(viewModel.state.value.loading)
        assertEquals(TaskStatus.TODO, viewModel.state.value.tasks.single().status)
    }
}
