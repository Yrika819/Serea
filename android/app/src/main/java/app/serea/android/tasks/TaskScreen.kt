package app.serea.android.tasks

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.DatePicker
import androidx.compose.material3.DatePickerDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TimePicker
import androidx.compose.material3.rememberDatePickerState
import androidx.compose.material3.rememberTimePickerState
import androidx.compose.material3.ExperimentalMaterial3Api
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
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import java.time.DateTimeException
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId
import java.time.Instant
import java.time.ZoneOffset
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
                                    Text(listOfNotNull(task.priority.name, due, if (task.isOverdue(state.now)) "Overdue" else null).joinToString(" · "), style = MaterialTheme.typography.bodySmall)
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
@OptIn(ExperimentalMaterial3Api::class)
private fun TaskEditorDialog(task: Task?, error: String?, saving: Boolean, onSave: (TaskDraft) -> Unit, onCancel: () -> Unit) {
    var title by rememberSaveable { mutableStateOf(task?.title.orEmpty()) }
    var description by rememberSaveable { mutableStateOf(task?.description.orEmpty()) }
    var priorityName by rememberSaveable { mutableStateOf((task?.priority ?: TaskPriority.NORMAL).name) }
    var date by rememberSaveable { mutableStateOf(task?.dueDate?.toString().orEmpty()) }
    var time by rememberSaveable { mutableStateOf(task?.dueTime?.toString().orEmpty()) }
    var zone by rememberSaveable { mutableStateOf(task?.timeZoneId ?: ZoneId.systemDefault().id) }
    var titleError by remember { mutableStateOf<String?>(null) }
    var dateError by remember { mutableStateOf<String?>(null) }
    var timeError by remember { mutableStateOf<String?>(null) }
    var zoneError by remember { mutableStateOf<String?>(null) }
    var priorityExpanded by remember { mutableStateOf(false) }
    var datePickerOpen by remember { mutableStateOf(false) }
    var timePickerOpen by remember { mutableStateOf(false) }
    val dateFormatter = remember { DateTimeFormatter.ISO_LOCAL_DATE }
    fun clearDateErrors() { dateError = null; timeError = null; zoneError = null }
    fun submit() {
        titleError = if (title.isBlank()) "タイトルを入力してください。" else if (title.codePointCount(0, title.length) > TaskValidation.MAX_TITLE_CODE_POINTS) "タイトルは200文字以内で入力してください。" else null
        dateError = null; timeError = null; zoneError = null
        val parsedDate = if (date.isBlank()) null else try { LocalDate.parse(date.trim(), dateFormatter) }
            catch (_: DateTimeException) { dateError = "期限日は YYYY-MM-DD 形式で入力してください。"; null }
        val parsedTime = if (time.isBlank()) null else try {
            val value = time.trim()
            if (!value.matches(Regex("(?:[01][0-9]|2[0-3]):[0-5][0-9]"))) throw DateTimeException("not HH:mm")
            LocalTime.parse(value, DateTimeFormatter.ISO_LOCAL_TIME)
        }
            catch (_: DateTimeException) { timeError = "時刻は24時間制の HH:MM 形式で入力してください。"; null }
        if (parsedTime != null && parsedDate == null && dateError == null) dateError = "時刻を設定するには期限日が必要です。"
        val zoneId = if (parsedDate != null) try {
            val value = zone.trim()
            if (value !in ZoneId.getAvailableZoneIds()) throw DateTimeException("not IANA")
            ZoneId.of(value)
        } catch (_: Exception) { zoneError = "有効な IANA タイムゾーンを入力してください（例: Asia/Tokyo）。"; null } else null
        if (parsedTime != null && parsedDate != null && zoneId != null) {
            val offsetCount = zoneId.rules.getValidOffsets(parsedDate.atTime(parsedTime)).size
            if (offsetCount == 0) timeError = "この時刻は夏時間の切り替えで存在しません。日付または時刻を変更してください。"
            else if (offsetCount > 1) timeError = "この時刻は夏時間の切り替えで重複します。別の時刻を選んでください。"
        }
        if (titleError == null && dateError == null && timeError == null && zoneError == null) {
            try { onSave(parseTaskDraft(title, description, TaskPriority.valueOf(priorityName), date, time, zone)) }
            catch (failure: InvalidTaskException) {
                val message = when (failure.message) {
                    "Title is required." -> "タイトルを入力してください。"
                    "Title is too long." -> "タイトルは200文字以内で入力してください。"
                    "A due time requires a due date." -> "時刻を設定するには期限日が必要です。"
                    "Invalid IANA timezone." -> "有効な IANA タイムゾーンを入力してください。"
                    "A timezone requires a due date." -> "タイムゾーンを使うには期限日が必要です。"
                    "A timezone is required for a due time." -> "時刻を設定するにはタイムゾーンが必要です。"
                    "Due time is ambiguous or does not exist in this timezone." -> "この時刻は夏時間の切り替えで存在しないか、重複しています。日付・時刻を確認してください。"
                    else -> "入力内容を確認してください。"
                }
                when {
                    message.contains("タイトル") -> titleError = message
                    message.contains("時刻") -> timeError = message
                    message.contains("タイムゾーン") -> zoneError = message
                    else -> dateError = message
                }
            }
        }
    }
    AlertDialog(
        onDismissRequest = onCancel,
        title = { Text(if (task == null) "New task" else "Edit task") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(title, { title = it; titleError = null }, label = { Text("Title") }, isError = titleError != null, supportingText = titleError?.let { msg -> { Text(msg, Modifier.semantics { liveRegion = LiveRegionMode.Polite }) } }, singleLine = true, modifier = Modifier.fillMaxWidth().testTag("task_title"))
                OutlinedTextField(description, { description = it }, label = { Text("Description / notes") }, modifier = Modifier.fillMaxWidth().testTag("task_description"))
                TextButton(onClick = { priorityExpanded = true }, modifier = Modifier.testTag("task_priority")) { Text("Priority: $priorityName") }
                DropdownMenu(expanded = priorityExpanded, onDismissRequest = { priorityExpanded = false }) {
                    TaskPriority.entries.forEach { item -> DropdownMenuItem(text = { Text(item.name) }, onClick = { priorityName = item.name; priorityExpanded = false }) }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    OutlinedTextField(date, { date = it; clearDateErrors() }, label = { Text("Due date (YYYY-MM-DD)") }, isError = dateError != null, supportingText = dateError?.let { msg -> { Text(msg, Modifier.semantics { liveRegion = LiveRegionMode.Polite }) } }, singleLine = true, modifier = Modifier.weight(1f).testTag("task_due_date"))
                    TextButton(onClick = { datePickerOpen = true }, modifier = Modifier.testTag("task_date_picker_open")) { Text("Pick date") }
                }
                Text(if (date.isBlank()) "日付のみの期限にもできます。" else "時刻を空欄にすると、この日付の終日タスクになります。", style = MaterialTheme.typography.bodySmall)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    OutlinedTextField(time, { time = it; timeError = null }, label = { Text("Due time (24-hour, optional)") }, isError = timeError != null, supportingText = timeError?.let { msg -> { Text(msg, Modifier.semantics { liveRegion = LiveRegionMode.Polite }) } }, singleLine = true, modifier = Modifier.weight(1f).testTag("task_due_time"))
                    TextButton(onClick = { timePickerOpen = true }, modifier = Modifier.testTag("task_time_picker_open")) { Text("Pick time") }
                }
                if (date.isNotBlank()) {
                    OutlinedTextField(zone, { zone = it; zoneError = null }, label = { Text("Timezone (IANA)") }, isError = zoneError != null, supportingText = { Text(zoneError ?: "例: Asia/Tokyo。現在の候補: ${ZoneId.systemDefault().id}", Modifier.semantics { if (zoneError != null) liveRegion = LiveRegionMode.Polite }) }, singleLine = true, modifier = Modifier.fillMaxWidth().testTag("task_timezone"))
                }
                error?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.testTag("task_editor_error").semantics { liveRegion = LiveRegionMode.Polite }) }
                listOfNotNull(titleError, dateError, timeError, zoneError).firstOrNull()?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.testTag("task_editor_error").semantics { liveRegion = LiveRegionMode.Polite }) }
            }
        },
        confirmButton = {
            TextButton(enabled = !saving, onClick = ::submit, modifier = Modifier.testTag("task_save")) { Text("Save") }
        },
        dismissButton = { TextButton(onClick = onCancel, modifier = Modifier.testTag("task_cancel")) { Text("Cancel") } },
    )
    if (datePickerOpen) {
        val initialMillis = runCatching { LocalDate.parse(date).toEpochDay() * 86_400_000L }.getOrNull()
        val pickerState = rememberDatePickerState(initialSelectedDateMillis = initialMillis)
        DatePickerDialog(
            onDismissRequest = { datePickerOpen = false },
            confirmButton = { TextButton(onClick = {
                pickerState.selectedDateMillis?.let { millis -> date = Instant.ofEpochMilli(millis).atZone(ZoneOffset.UTC).toLocalDate().format(dateFormatter); clearDateErrors() }
                datePickerOpen = false
            }, modifier = Modifier.testTag("task_date_picker_confirm")) { Text("Select") } },
            dismissButton = { TextButton(onClick = { datePickerOpen = false }, modifier = Modifier.testTag("task_date_picker_cancel")) { Text("Cancel") } },
        ) { DatePicker(state = pickerState) }
    }
    if (timePickerOpen) {
        val initialTime = runCatching { LocalTime.parse(time) }.getOrNull() ?: LocalTime.now().withSecond(0).withNano(0)
        val pickerState = rememberTimePickerState(initialHour = initialTime.hour, initialMinute = initialTime.minute, is24Hour = true)
        AlertDialog(
            onDismissRequest = { timePickerOpen = false },
            title = { Text("Select time (24-hour)") },
            text = { TimePicker(state = pickerState) },
            confirmButton = { TextButton(onClick = { time = "%02d:%02d".format(pickerState.hour, pickerState.minute); timeError = null; timePickerOpen = false }, modifier = Modifier.testTag("task_time_picker_confirm")) { Text("Select") } },
            dismissButton = { TextButton(onClick = { timePickerOpen = false }, modifier = Modifier.testTag("task_time_picker_cancel")) { Text("Cancel") } },
        )
    }
}
