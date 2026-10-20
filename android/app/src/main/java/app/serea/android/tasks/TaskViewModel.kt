package app.serea.android.tasks

import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

data class TaskUiState(
    val tasks: List<Task> = emptyList(),
    val loading: Boolean = true,
    val saving: Boolean = false,
    val editing: Task? = null,
    val editorVisible: Boolean = false,
    val deleteTargetId: String? = null,
    val deleteTargetRevision: Long? = null,
    val error: String? = null,
    val message: String? = null,
)

class TaskViewModel(private val repository: TaskRepository) : ViewModel() {
    private val mutableState = MutableStateFlow(TaskUiState())
    val state: StateFlow<TaskUiState> = mutableState.asStateFlow()

    init { refresh() }

    fun refresh() = launchTask { mutableState.value = mutableState.value.copy(loading = true); repository.list().also { mutableState.value = mutableState.value.copy(tasks = it, loading = false) } }
    fun addTask() { mutableState.value = mutableState.value.copy(editing = null, editorVisible = true, error = null) }
    fun editTask(id: String) {
        val target = mutableState.value.tasks.firstOrNull { it.id == id } ?: return showError("Task no longer exists.")
        mutableState.value = mutableState.value.copy(editing = target, editorVisible = true, error = null)
    }
    fun cancelEditor() { mutableState.value = mutableState.value.copy(editing = null, editorVisible = false) }
    fun requestDelete(id: String) { mutableState.value.tasks.firstOrNull { it.id == id }?.let { mutableState.value = mutableState.value.copy(deleteTargetId = it.id, deleteTargetRevision = it.revision, error = null) } }
    fun cancelDelete() { mutableState.value = mutableState.value.copy(deleteTargetId = null, deleteTargetRevision = null) }

    fun save(draft: TaskDraft) {
        val snapshot = mutableState.value
        val target = snapshot.editing
        launchMutation {
            if (target == null) repository.create(draft) else repository.update(target.id, target.revision, draft)
            mutableState.value = mutableState.value.copy(editorVisible = false, editing = null, message = "Task saved.")
            refreshInline()
        }
    }

    fun confirmDelete() {
        val id = mutableState.value.deleteTargetId ?: return
        val revision = mutableState.value.deleteTargetRevision ?: return showError("Task changed. Refresh the list before deleting.")
        launchMutation {
            repository.delete(id, revision)
            mutableState.value = mutableState.value.copy(deleteTargetId = null, deleteTargetRevision = null, message = "Task deleted.")
            refreshInline()
        }
    }

    fun toggleComplete(id: String) {
        val target = mutableState.value.tasks.firstOrNull { it.id == id } ?: return showError("Task no longer exists.")
        launchMutation {
            if (target.status == TaskStatus.TODO) repository.complete(id, target.revision) else repository.reopen(id, target.revision)
            refreshInline()
        }
    }

    fun clearMessage() { mutableState.value = mutableState.value.copy(message = null, error = null) }

    private fun launchTask(block: suspend () -> Unit) = viewModelScope.launch {
        try { withContext(Dispatchers.IO) { block() } }
        catch (cancelled: CancellationException) { throw cancelled }
        catch (error: Exception) {
            showError(error.toTaskUserMessage())
            runCatching { withContext(Dispatchers.IO) { mutableState.value = mutableState.value.copy(tasks = repository.list(), loading = false) } }
        }
    }

    private fun launchMutation(block: suspend () -> Unit) {
        if (mutableState.value.saving) return
        mutableState.value = mutableState.value.copy(saving = true)
        viewModelScope.launch {
            try { withContext(Dispatchers.IO) { block() }; mutableState.value = mutableState.value.copy(saving = false) }
            catch (cancelled: CancellationException) { mutableState.value = mutableState.value.copy(saving = false); throw cancelled }
            catch (error: Exception) {
                mutableState.value = mutableState.value.copy(saving = false)
                showError(error.toTaskUserMessage())
                runCatching { withContext(Dispatchers.IO) { mutableState.value = mutableState.value.copy(tasks = repository.list(), loading = false) } }
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

    private suspend fun refreshInline() { mutableState.value = mutableState.value.copy(tasks = repository.list(), loading = false) }
    private fun showError(message: String) { mutableState.value = mutableState.value.copy(error = message, loading = false) }

    companion object {
        fun factory(repository: TaskRepository) = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T = TaskViewModel(repository) as T
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
