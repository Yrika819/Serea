package app.serea.android.tasks

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineDispatcher
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

data class TaskUiState(
    val tasks: List<Task> = emptyList(),
    val now: Instant = Instant.now(),
    val loading: Boolean = true,
    val saving: Boolean = false,
    val savingTaskId: String? = null,
    val editorSaving: Boolean = false,
    val editing: Task? = null,
    val editorVisible: Boolean = false,
    val editorInput: TaskEditorInput? = null,
    val operationNotice: String? = null,
    val operationNoticeIsError: Boolean = false,
    val deleteTargetId: String? = null,
    val deleteTargetRevision: Long? = null,
    val error: String? = null,
    val message: String? = null,
)

/** Sensitive, unsaved editor content stays in the activity ViewModel and is never written to SavedState. */
data class TaskEditorInput(
    val title: String = "",
    val description: String = "",
    val priorityName: String = TaskPriority.NORMAL.name,
    val dueDate: String = "",
    val dueTime: String = "",
    val timeZoneId: String = ZoneId.systemDefault().id,
) {
    companion object {
        fun from(task: Task?): TaskEditorInput = if (task == null) TaskEditorInput() else TaskEditorInput(
            title = task.title,
            description = task.description,
            priorityName = task.priority.name,
            dueDate = task.dueDate?.toString().orEmpty(),
            dueTime = task.dueTime?.toString().orEmpty(),
            timeZoneId = task.timeZoneId ?: ZoneId.systemDefault().id,
        )
    }
}

class TaskViewModel(
    private val repository: TaskRepository,
    private val clock: () -> Instant = Instant::now,
    private val ioDispatcher: CoroutineDispatcher = Dispatchers.IO,
) : ViewModel() {
    private val mutableState = MutableStateFlow(TaskUiState(now = clock()))
    val state: StateFlow<TaskUiState> = mutableState.asStateFlow()
    private val listGeneration = AtomicLong(0)
    private val editorGeneration = AtomicLong(0)
    private val deleteGeneration = AtomicLong(0)
    private val mutationGeneration = AtomicLong(0)

    init { refresh() }

    fun refresh() {
        val generation = listGeneration.incrementAndGet()
        val editorSession = editorGeneration.get()
        val deleteSession = deleteGeneration.get()
        mutableState.value = mutableState.value.copy(loading = true)
        viewModelScope.launch {
            try {
                val latest = withContext(ioDispatcher) { repository.list() }
                if (generation == listGeneration.get()) mutableState.value = mutableState.value.copy(tasks = latest, now = clock(), loading = false)
            } catch (cancelled: CancellationException) { throw cancelled }
            catch (error: Exception) {
                if (generation == listGeneration.get()) {
                    val sameUiSession = editorSession == editorGeneration.get() && deleteSession == deleteGeneration.get()
                    mutableState.value = mutableState.value.copy(
                        loading = false,
                        error = if (sameUiSession) error.toTaskUserMessage() else mutableState.value.error,
                    )
                }
            }
        }
    }
    fun addTask() {
        editorGeneration.incrementAndGet()
        mutableState.value = mutableState.value.copy(editing = null, editorVisible = true, editorInput = TaskEditorInput.from(null), editorSaving = false,
            operationNotice = null, operationNoticeIsError = false, error = null)
    }
    fun editTask(id: String) {
        if (mutableState.value.savingTaskId == id) return
        val target = mutableState.value.tasks.firstOrNull { it.id == id } ?: return showError("Task no longer exists.")
        editorGeneration.incrementAndGet()
        mutableState.value = mutableState.value.copy(editing = target, editorVisible = true, editorInput = TaskEditorInput.from(target), editorSaving = false,
            operationNotice = null, operationNoticeIsError = false, error = null)
    }
    fun updateEditorInput(input: TaskEditorInput) {
        if (mutableState.value.editorVisible) mutableState.value = mutableState.value.copy(editorInput = input)
    }
    fun cancelEditor() {
        editorGeneration.incrementAndGet()
        mutableState.value = mutableState.value.copy(editing = null, editorVisible = false, editorInput = null, editorSaving = false, error = null)
    }
    fun requestDelete(id: String) {
        val target = mutableState.value.tasks.firstOrNull { it.id == id } ?: return showError("Task no longer exists.")
        deleteGeneration.incrementAndGet()
        mutableState.value = mutableState.value.copy(deleteTargetId = target.id, deleteTargetRevision = target.revision, error = null)
    }
    fun cancelDelete() {
        deleteGeneration.incrementAndGet()
        mutableState.value = mutableState.value.copy(deleteTargetId = null, deleteTargetRevision = null)
    }

    fun save(draft: TaskDraft) {
        val snapshot = mutableState.value
        if (snapshot.saving || !snapshot.editorVisible) return
        val target = snapshot.editing
        val editorSession = editorGeneration.get()
        mutableState.value = snapshot.copy(editorSaving = true, operationNotice = null, operationNoticeIsError = false)
        launchMutation(
            operationTaskId = target?.id,
            block = { if (target == null) repository.create(draft) else repository.update(target.id, target.revision, draft) },
            onSuccess = {
                if (editorSession == editorGeneration.get()) {
                    mutableState.value = mutableState.value.copy(editorVisible = false, editing = null, editorInput = null, error = null, message = "Task saved.", operationNotice = null, operationNoticeIsError = false)
                } else {
                    mutableState.value = mutableState.value.copy(
                        operationNotice = "以前の編集画面で開始したタスクの保存が完了しました。",
                        operationNoticeIsError = false,
                    )
                }
            },
            onFailure = { message ->
                if (editorSession == editorGeneration.get()) showError(message, clearLoading = false)
                else setOperationNotice("以前の編集画面で開始したタスクの保存に失敗しました: $message", isError = true)
            },
            onRefreshFailure = {
                if (editorSession == editorGeneration.get()) showError("Task operation completed, but the task list could not be refreshed.")
                else setOperationNotice("以前の編集画面で保存は完了しましたが、タスク一覧を更新できませんでした。", isError = true)
            },
            onCancelled = {
                if (editorSession == editorGeneration.get()) showError("Task operation was interrupted. Refresh the task list to confirm its result.")
                else setOperationNotice("以前の編集画面で開始したタスク操作が中断されました。結果を一覧で確認してください。", isError = true)
            },
            onSettled = {
                if (editorSession == editorGeneration.get()) {
                    mutableState.value = mutableState.value.copy(editorSaving = false)
                }
            },
        )
    }

    fun confirmDelete() {
        val id = mutableState.value.deleteTargetId ?: return
        val revision = mutableState.value.deleteTargetRevision ?: return showError("Task changed. Refresh the list before deleting.")
        val deleteSession = deleteGeneration.get()
        launchMutation(
            operationTaskId = id,
            block = { repository.delete(id, revision) },
            onSuccess = {
                if (deleteSession == deleteGeneration.get()) {
                    mutableState.value = mutableState.value.copy(deleteTargetId = null, deleteTargetRevision = null, error = null, message = "Task deleted.")
                }
            },
            onFailure = { message -> if (deleteSession == deleteGeneration.get()) showError(message, clearLoading = false) },
            onRefreshFailure = {
                if (deleteSession == deleteGeneration.get()) showError("Task operation completed, but the task list could not be refreshed.")
            },
            onCancelled = {
                if (deleteSession == deleteGeneration.get()) showError("Task operation was interrupted. Refresh the task list to confirm its result.")
            },
        )
    }

    fun toggleComplete(id: String) {
        val target = mutableState.value.tasks.firstOrNull { it.id == id } ?: return showError("Task no longer exists.")
        val editorSession = editorGeneration.get()
        val deleteSession = deleteGeneration.get()
        launchMutation(
            operationTaskId = id,
            block = { if (target.status == TaskStatus.TODO) repository.complete(id, target.revision) else repository.reopen(id, target.revision) },
            onFailure = { message ->
                if (editorSession == editorGeneration.get() && deleteSession == deleteGeneration.get()) {
                    showError(message, clearLoading = false)
                }
            },
            onRefreshFailure = {
                if (editorSession == editorGeneration.get() && deleteSession == deleteGeneration.get()) {
                    showError("Task operation completed, but the task list could not be refreshed.")
                }
            },
            onCancelled = {
                if (editorSession == editorGeneration.get() && deleteSession == deleteGeneration.get()) {
                    showError("Task operation was interrupted. Refresh the task list to confirm its result.")
                }
            },
        )
    }

    fun clearMessage() { mutableState.value = mutableState.value.copy(message = null, error = null, operationNotice = null, operationNoticeIsError = false) }


    private fun launchMutation(
        operationTaskId: String? = null,
        block: suspend () -> Unit,
        onSuccess: () -> Unit = {},
        onFailure: (String) -> Unit = { showError(it) },
        onRefreshFailure: () -> Unit = {},
        onCancelled: () -> Unit = {},
        onSettled: () -> Unit = {},
    ) {
        if (mutableState.value.saving) return
        val mutation = mutationGeneration.incrementAndGet()
        var ownedListGeneration = listGeneration.incrementAndGet() // invalidate any list read started before this mutation
        var wasCancelled = false
        mutableState.value = mutableState.value.copy(saving = true, savingTaskId = operationTaskId)
        viewModelScope.launch {
            try {
                withContext(ioDispatcher) { block() }
                if (mutation == mutationGeneration.get()) {
                    onSuccess()
                    refreshAfterMutation(mutation, onRefreshFailure) { ownedListGeneration = it }
                }
            } catch (cancelled: CancellationException) {
                wasCancelled = true
                throw cancelled
            }
            catch (error: Exception) {
                if (mutation == mutationGeneration.get()) {
                    onFailure(error.toTaskUserMessage())
                    refreshAfterMutation(mutation, onFailure = {}, onGenerationStarted = { ownedListGeneration = it })
                }
            } finally {
                if (mutation == mutationGeneration.get()) {
                    val clearLoading = wasCancelled && listGeneration.get() == ownedListGeneration
                    if (clearLoading) onCancelled()
                    mutableState.value = mutableState.value.copy(
                        saving = false,
                        savingTaskId = null,
                        loading = if (clearLoading) false else mutableState.value.loading,
                    )
                    onSettled()
                }
            }
        }
    }

    private fun Exception.toTaskUserMessage(): String = when (this) {
        is TaskConflictException -> message ?: "Task changed elsewhere."
        is TaskNotFoundException -> message ?: "Task no longer exists."
        is InvalidTaskException -> message ?: "Invalid task."
        is TaskStorageUnavailableException -> message ?: "Task storage is unavailable."
        else -> "Task operation failed. Please try again."
    }

    private suspend fun refreshAfterMutation(
        mutation: Long,
        onFailure: () -> Unit,
        onGenerationStarted: (Long) -> Unit = {},
    ) {
        val generation = listGeneration.incrementAndGet()
        onGenerationStarted(generation)
        mutableState.value = mutableState.value.copy(loading = true)
        try {
            val latest = withContext(ioDispatcher) { repository.list() }
            if (mutation == mutationGeneration.get() && generation == listGeneration.get()) {
                mutableState.value = mutableState.value.copy(tasks = latest, now = clock(), loading = false)
            }
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            if (mutation == mutationGeneration.get() && generation == listGeneration.get()) {
                mutableState.value = mutableState.value.copy(loading = false)
                onFailure()
            }
        }
    }
    private fun showError(message: String, clearLoading: Boolean = true) {
        mutableState.value = mutableState.value.copy(error = message, loading = if (clearLoading) false else mutableState.value.loading)
    }

    private fun setOperationNotice(message: String, isError: Boolean) {
        mutableState.value = mutableState.value.copy(operationNotice = message, operationNoticeIsError = isError)
    }

    companion object {
        fun factory(repository: TaskRepository, clock: () -> Instant = Instant::now, ioDispatcher: CoroutineDispatcher = Dispatchers.IO) = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T = TaskViewModel(repository, clock, ioDispatcher) as T
        }
    }
}

internal fun parseTaskDraft(title: String, description: String, priority: TaskPriority, date: String, time: String, zone: String): TaskDraft {
    fun <T> parse(value: String, parser: (String) -> T): T? = if (value.isBlank()) null else try { parser(value.trim()) }
        catch (_: Exception) { throw InvalidTaskException("Enter a valid ISO date, 24-hour time, and timezone.") }
    val dueDate = parse(date, LocalDate::parse)
    val dueTime = parse(time, LocalTime::parse)
    val zoneId = if (dueDate == null) null else zone.trim().takeIf { it.isNotEmpty() }
    return TaskDraft(title, description, priority, dueDate, dueTime, zoneId).also(TaskValidation::validateDraft)
}
