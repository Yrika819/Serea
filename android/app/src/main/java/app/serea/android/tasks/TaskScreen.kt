package app.serea.android.tasks

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.weight
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import java.time.ZoneId
import java.time.format.DateTimeFormatter

@Composable
fun TaskScreen(viewModel: TaskViewModel) {
    val state by viewModel.state.collectAsState()
    Scaffold { padding ->
        Column(Modifier.fillMaxSize().padding(padding).padding(16.dp), verticalArrangement = Arrangement.spacedBy(10.dp)) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                Text("Tasks", style = MaterialTheme.typography.headlineMedium)
                Button(onClick = viewModel::addTask, modifier = Modifier.testTag("task_add")) { Text("Add task") }
            }
            state.error?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.testTag("task_error")) }
            state.message?.let { Text(it, color = MaterialTheme.colorScheme.primary) }
            when {
                state.loading -> Text("Loading tasks…")
                state.tasks.isEmpty() -> Text("No tasks yet.", modifier = Modifier.testTag("task_empty"))
                else -> LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(state.tasks, key = { it.id }) { task ->
                        Column(Modifier.fillMaxWidth().padding(8.dp)) {
                            Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                                Checkbox(checked = task.status == TaskStatus.COMPLETED,
                                    onCheckedChange = { viewModel.toggleComplete(task.id) }, enabled = !state.saving, modifier = Modifier.testTag("task_complete_${task.id}"))
                                Column(Modifier.weight(1f)) {
                                    Text(task.title, style = MaterialTheme.typography.titleMedium)
                                    if (task.description.isNotBlank()) Text(task.description, style = MaterialTheme.typography.bodyMedium)
                                    val due = task.dueDate?.let { date ->
                                        if (task.dueTime == null) "Due $date (all day)" else "Due $date ${task.dueTime} ${task.timeZoneId.orEmpty()}"
                                    }
                                    Text(listOfNotNull(task.priority.name, due, if (task.isOverdue) "Overdue" else null).joinToString(" · "), style = MaterialTheme.typography.bodySmall)
                                }
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                TextButton(onClick = { viewModel.editTask(task.id) }, modifier = Modifier.testTag("task_edit_${task.id}")) { Text("Edit") }
                                TextButton(onClick = { viewModel.requestDelete(task.id) }, modifier = Modifier.testTag("task_delete_${task.id}")) { Text("Delete") }
                            }
                        }
                    }
                }
            }
        }
    }
    if (state.editorVisible) key(state.editing?.id ?: "new") {
        TaskEditorDialog(state.editing, state.error, state.saving, viewModel::save, viewModel::cancelEditor)
    }
    val deleteId = state.deleteTargetId
    if (deleteId != null) {
        val target = state.tasks.firstOrNull { it.id == deleteId }
        AlertDialog(
            onDismissRequest = viewModel::cancelDelete,
            title = { Text("Delete task?") },
            text = { Text(target?.title ?: "This task is no longer available.") },
            confirmButton = { TextButton(enabled = target != null && !state.saving, onClick = viewModel::confirmDelete, modifier = Modifier.testTag("task_delete_confirm")) { Text("Delete") } },
            dismissButton = { TextButton(onClick = viewModel::cancelDelete, modifier = Modifier.testTag("task_delete_cancel")) { Text("Cancel") } },
        )
    }
}

@Composable
private fun TaskEditorDialog(task: Task?, error: String?, saving: Boolean, onSave: (TaskDraft) -> Unit, onCancel: () -> Unit) {
    var title by rememberSaveable { mutableStateOf(task?.title.orEmpty()) }
    var description by rememberSaveable { mutableStateOf(task?.description.orEmpty()) }
    var priorityName by rememberSaveable { mutableStateOf((task?.priority ?: TaskPriority.NORMAL).name) }
    var date by rememberSaveable { mutableStateOf(task?.dueDate?.toString().orEmpty()) }
    var time by rememberSaveable { mutableStateOf(task?.dueTime?.toString().orEmpty()) }
    var zone by rememberSaveable { mutableStateOf(task?.timeZoneId ?: ZoneId.systemDefault().id) }
    var validation by remember { mutableStateOf<String?>(null) }
    var priorityExpanded by remember { mutableStateOf(false) }
    AlertDialog(
        onDismissRequest = onCancel,
        title = { Text(if (task == null) "New task" else "Edit task") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(title, { title = it; validation = null }, label = { Text("Title") }, singleLine = true, modifier = Modifier.fillMaxWidth().testTag("task_title"))
                OutlinedTextField(description, { description = it }, label = { Text("Description / notes") }, modifier = Modifier.fillMaxWidth().testTag("task_description"))
                TextButton(onClick = { priorityExpanded = true }, modifier = Modifier.testTag("task_priority")) { Text("Priority: $priorityName") }
                DropdownMenu(expanded = priorityExpanded, onDismissRequest = { priorityExpanded = false }) {
                    TaskPriority.entries.forEach { item -> DropdownMenuItem(text = { Text(item.name) }, onClick = { priorityName = item.name; priorityExpanded = false }) }
                }
                OutlinedTextField(date, { date = it }, label = { Text("Due date (YYYY-MM-DD)") }, singleLine = true, modifier = Modifier.fillMaxWidth().testTag("task_due_date"))
                OutlinedTextField(time, { time = it }, label = { Text("Due time (HH:MM, optional)") }, singleLine = true, modifier = Modifier.fillMaxWidth().testTag("task_due_time"))
                if (time.isNotBlank()) OutlinedTextField(zone, { zone = it }, label = { Text("Timezone (IANA)") }, singleLine = true, modifier = Modifier.fillMaxWidth().testTag("task_timezone"))
                (validation ?: error)?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.testTag("task_editor_error")) }
            }
        },
        confirmButton = {
            TextButton(enabled = !saving, onClick = {
                try { onSave(parseTaskDraft(title, description, TaskPriority.valueOf(priorityName), date, time, zone)) }
                catch (failure: Exception) { validation = failure.message ?: "Check the task fields." }
            }, modifier = Modifier.testTag("task_save")) { Text("Save") }
        },
        dismissButton = { TextButton(onClick = onCancel, modifier = Modifier.testTag("task_cancel")) { Text("Cancel") } },
    )
}
