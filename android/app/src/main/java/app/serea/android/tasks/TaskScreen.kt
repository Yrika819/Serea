package app.serea.android.tasks

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
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
import androidx.compose.runtime.CompositionLocalProvider
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.key
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.runtime.saveable.LocalSaveableStateRegistry
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.contentDescription
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
                Text("タスク", style = MaterialTheme.typography.headlineMedium,
                    modifier = Modifier.semantics { heading() }.testTag("tasks_heading"))
                Button(onClick = viewModel::addTask, modifier = Modifier.testTag("task_add")) { Text("タスクを追加") }
            }
            state.error?.let { Text(it, color = MaterialTheme.colorScheme.error,
                modifier = Modifier.testTag("task_error").semantics { liveRegion = LiveRegionMode.Assertive }) }
            state.message?.let { Text(it, color = MaterialTheme.colorScheme.primary,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }) }
            if (!state.editorVisible) state.operationNotice?.let { notice ->
                Text(
                    notice,
                    color = if (state.operationNoticeIsError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                    modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }.testTag("task_operation_notice"),
                )
            }
            when {
                state.loading -> Text("タスクを読み込み中…", modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
                state.tasks.isEmpty() -> Text("タスクはありません。", modifier = Modifier.testTag("task_empty")
                    .semantics { liveRegion = LiveRegionMode.Polite })
                else -> LazyColumn(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(state.tasks, key = { it.id }) { task ->
                        Column(Modifier.fillMaxWidth().padding(8.dp)) {
                            Row(verticalAlignment = androidx.compose.ui.Alignment.CenterVertically) {
                                Checkbox(checked = task.status == TaskStatus.COMPLETED,
                                    onCheckedChange = { viewModel.toggleComplete(task.id) },
                                    enabled = !state.saving,
                                    modifier = Modifier.semantics {
                                        contentDescription = if (task.status == TaskStatus.COMPLETED) {
                                            "${task.title} を未完了に戻す"
                                        } else {
                                            "${task.title} を完了にする"
                                        }
                                    }.testTag("task_complete_${task.id}"))
                                Column(Modifier.weight(1f)) {
                                    Text(task.title, style = MaterialTheme.typography.titleMedium)
                                    if (task.description.isNotBlank()) Text(task.description, style = MaterialTheme.typography.bodyMedium)
                                    val due = task.dueDate?.let { date ->
                                        if (task.dueTime == null) "期限 $date（終日）" else "期限 $date ${task.dueTime} ${task.timeZoneId.orEmpty()}"
                                    }
                                    Text(listOfNotNull(priorityLabel(task.priority), due, if (task.isOverdue(state.now)) "期限超過" else null).joinToString(" · "), style = MaterialTheme.typography.bodySmall)
                                }
                            }
                            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                                TextButton(onClick = { viewModel.editTask(task.id) }, enabled = state.savingTaskId != task.id,
                                    modifier = Modifier.semantics { contentDescription = "${task.title} を編集" }.testTag("task_edit_${task.id}")) { Text("編集") }
                                TextButton(onClick = { viewModel.requestDelete(task.id) }, enabled = !state.saving,
                                    modifier = Modifier.semantics { contentDescription = "${task.title} を削除" }.testTag("task_delete_${task.id}")) { Text("削除") }
                            }
                        }
                    }
                }
            }
        }
    }
    if (state.editorVisible) key(state.editing?.id ?: "new") {
        TaskEditorDialog(
            task = state.editing,
            draft = requireNotNull(state.editorInput),
            error = state.error,
            editorSaving = state.editorSaving,
            mutationInFlight = state.saving,
            operationNotice = state.operationNotice,
            operationNoticeIsError = state.operationNoticeIsError,
            onDraftChange = viewModel::updateEditorInput,
            onSave = viewModel::save,
            onCancel = viewModel::cancelEditor,
        )
    }
    val deleteId = state.deleteTargetId
    if (deleteId != null) {
        val target = state.tasks.firstOrNull { it.id == deleteId }
        AlertDialog(
            onDismissRequest = viewModel::cancelDelete,
            title = { Text("タスクを削除しますか？") },
            text = { Text(target?.title ?: "このタスクは利用できません。") },
            confirmButton = { TextButton(enabled = target != null && !state.saving, onClick = viewModel::confirmDelete, modifier = Modifier.testTag("task_delete_confirm")) { Text("削除") } },
            dismissButton = { TextButton(onClick = viewModel::cancelDelete, modifier = Modifier.testTag("task_delete_cancel")) { Text("キャンセル") } },
        )
    }
}

@Composable
@OptIn(ExperimentalMaterial3Api::class)
private fun TaskEditorDialog(
    task: Task?,
    draft: TaskEditorInput,
    error: String?,
    editorSaving: Boolean,
    mutationInFlight: Boolean,
    operationNotice: String?,
    operationNoticeIsError: Boolean,
    onDraftChange: (TaskEditorInput) -> Unit,
    onSave: (TaskDraft) -> Unit,
    onCancel: () -> Unit,
) {
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
        titleError = if (draft.title.isBlank()) "タイトルを入力してください。" else if (draft.title.codePointCount(0, draft.title.length) > TaskValidation.MAX_TITLE_CODE_POINTS) "タイトルは200文字以内で入力してください。" else null
        dateError = null; timeError = null; zoneError = null
        val parsedDate = if (draft.dueDate.isBlank()) null else try { LocalDate.parse(draft.dueDate.trim(), dateFormatter) }
            catch (_: DateTimeException) { dateError = "期限日は YYYY-MM-DD 形式で入力してください。"; null }
        val parsedTime = if (draft.dueTime.isBlank()) null else try {
            val value = draft.dueTime.trim()
            if (!value.matches(Regex("(?:[01][0-9]|2[0-3]):[0-5][0-9]"))) throw DateTimeException("not HH:mm")
            LocalTime.parse(value, DateTimeFormatter.ISO_LOCAL_TIME)
        }
            catch (_: DateTimeException) { timeError = "時刻は24時間制の HH:MM 形式で入力してください。"; null }
        if (parsedTime != null && parsedDate == null && dateError == null) dateError = "時刻を設定するには期限日が必要です。"
        val zoneId = if (parsedDate != null) try {
            val value = draft.timeZoneId.trim()
            if (value !in ZoneId.getAvailableZoneIds()) throw DateTimeException("not IANA")
            ZoneId.of(value)
        } catch (_: Exception) { zoneError = "有効な IANA タイムゾーンを入力してください（例: Asia/Tokyo）。"; null } else null
        if (parsedTime != null && parsedDate != null && zoneId != null) {
            val offsetCount = zoneId.rules.getValidOffsets(parsedDate.atTime(parsedTime)).size
            if (offsetCount == 0) timeError = "この時刻は夏時間の切り替えで存在しません。日付または時刻を変更してください。"
            else if (offsetCount > 1) timeError = "この時刻は夏時間の切り替えで重複します。別の時刻を選んでください。"
        }
        if (titleError == null && dateError == null && timeError == null && zoneError == null) {
            try { onSave(parseTaskDraft(draft.title, draft.description, TaskPriority.valueOf(draft.priorityName), draft.dueDate, draft.dueTime, draft.timeZoneId)) }
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
        onDismissRequest = { if (!editorSaving) onCancel() },
        title = { Text(if (task == null) "新しいタスク" else "タスクを編集") },
        text = {
            Column(
                Modifier.fillMaxWidth().heightIn(max = 420.dp).imePadding()
                    .verticalScroll(rememberScrollState()).testTag("task_editor_scroll"),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                OutlinedTextField(draft.title, { onDraftChange(draft.copy(title = it)); titleError = null }, enabled = !editorSaving, label = { Text("タイトル") }, isError = titleError != null, supportingText = titleError?.let { msg -> { Text(msg, Modifier.semantics { liveRegion = LiveRegionMode.Polite }) } }, singleLine = true, modifier = Modifier.fillMaxWidth().testTag("task_title"))
                OutlinedTextField(draft.description, { onDraftChange(draft.copy(description = it)) }, enabled = !editorSaving, label = { Text("説明・メモ") }, modifier = Modifier.fillMaxWidth().testTag("task_description"))
                TextButton(onClick = { priorityExpanded = true }, enabled = !editorSaving, modifier = Modifier.testTag("task_priority")) { Text("優先度: ${priorityLabel(TaskPriority.valueOf(draft.priorityName))}") }
                DropdownMenu(expanded = priorityExpanded, onDismissRequest = { priorityExpanded = false }) {
                    TaskPriority.entries.forEach { item -> DropdownMenuItem(text = { Text(priorityLabel(item)) }, enabled = !editorSaving, onClick = { onDraftChange(draft.copy(priorityName = item.name)); priorityExpanded = false }) }
                }
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    OutlinedTextField(draft.dueDate, { onDraftChange(draft.copy(dueDate = it)); clearDateErrors() }, enabled = !editorSaving, label = { Text("期限日（YYYY-MM-DD）") }, isError = dateError != null, supportingText = dateError?.let { msg -> { Text(msg, Modifier.semantics { liveRegion = LiveRegionMode.Polite }) } }, singleLine = true, modifier = Modifier.weight(1f).testTag("task_due_date"))
                    TextButton(onClick = { datePickerOpen = true }, enabled = !editorSaving, modifier = Modifier.testTag("task_date_picker_open")) { Text("日付を選択") }
                }
                Text(if (draft.dueDate.isBlank()) "日付のみの期限にもできます。" else "時刻を空欄にすると、この日付の終日タスクになります。", style = MaterialTheme.typography.bodySmall)
                Row(horizontalArrangement = Arrangement.spacedBy(6.dp)) {
                    OutlinedTextField(draft.dueTime, { onDraftChange(draft.copy(dueTime = it)); timeError = null }, enabled = !editorSaving, label = { Text("期限時刻（24時間制・任意）") }, isError = timeError != null, supportingText = timeError?.let { msg -> { Text(msg, Modifier.semantics { liveRegion = LiveRegionMode.Polite }) } }, singleLine = true, modifier = Modifier.weight(1f).testTag("task_due_time"))
                    TextButton(onClick = { timePickerOpen = true }, enabled = !editorSaving, modifier = Modifier.testTag("task_time_picker_open")) { Text("時刻を選択") }
                }
                if (draft.dueDate.isNotBlank()) {
                    OutlinedTextField(draft.timeZoneId, { onDraftChange(draft.copy(timeZoneId = it)); zoneError = null }, enabled = !editorSaving, label = { Text("タイムゾーン（IANA）") }, isError = zoneError != null, supportingText = { Text(zoneError ?: "例: Asia/Tokyo。現在の候補: ${ZoneId.systemDefault().id}", Modifier.semantics { if (zoneError != null) liveRegion = LiveRegionMode.Polite }) }, singleLine = true, modifier = Modifier.fillMaxWidth().testTag("task_timezone"))
                }
                when {
                    editorSaving -> Text("保存中です。画面を閉じても処理は続行します。", modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }.testTag("task_editor_saving"))
                    mutationInFlight -> Text("別のタスク操作が完了するまで保存できません。", modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }.testTag("task_editor_waiting"))
                }
                operationNotice?.let { notice ->
                    Text(
                        notice,
                        color = if (operationNoticeIsError) MaterialTheme.colorScheme.error else MaterialTheme.colorScheme.primary,
                        modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }.testTag("task_operation_notice"),
                    )
                }
                error?.let { Text(it, color = MaterialTheme.colorScheme.error, modifier = Modifier.testTag("task_editor_error").semantics { liveRegion = LiveRegionMode.Polite }) }
            }
        },
        confirmButton = {
            TextButton(enabled = !editorSaving && !mutationInFlight, onClick = ::submit, modifier = Modifier.testTag("task_save")) { Text("保存") }
        },
        dismissButton = { TextButton(onClick = onCancel, modifier = Modifier.testTag("task_cancel")) { Text(if (editorSaving) "閉じる" else "キャンセル") } },
    )
    if (datePickerOpen) {
        CompositionLocalProvider(LocalSaveableStateRegistry provides null) {
            val initialMillis = runCatching { LocalDate.parse(draft.dueDate).toEpochDay() * 86_400_000L }.getOrNull()
            val pickerState = rememberDatePickerState(initialSelectedDateMillis = initialMillis)
            DatePickerDialog(
                onDismissRequest = { datePickerOpen = false },
                confirmButton = { TextButton(onClick = {
                    pickerState.selectedDateMillis?.let { millis -> onDraftChange(draft.copy(dueDate = Instant.ofEpochMilli(millis).atZone(ZoneOffset.UTC).toLocalDate().format(dateFormatter))); clearDateErrors() }
                    datePickerOpen = false
                }, modifier = Modifier.testTag("task_date_picker_confirm")) { Text("選択") } },
                dismissButton = { TextButton(onClick = { datePickerOpen = false }, modifier = Modifier.testTag("task_date_picker_cancel")) { Text("キャンセル") } },
            ) { DatePicker(state = pickerState) }
        }
    }
    if (timePickerOpen) {
        CompositionLocalProvider(LocalSaveableStateRegistry provides null) {
            val initialTime = runCatching { LocalTime.parse(draft.dueTime) }.getOrNull() ?: LocalTime.now().withSecond(0).withNano(0)
            val pickerState = rememberTimePickerState(initialHour = initialTime.hour, initialMinute = initialTime.minute, is24Hour = true)
            AlertDialog(
                onDismissRequest = { timePickerOpen = false },
                title = { Text("時刻を選択（24時間制）") },
                text = { TimePicker(state = pickerState) },
                confirmButton = { TextButton(onClick = { onDraftChange(draft.copy(dueTime = "%02d:%02d".format(pickerState.hour, pickerState.minute))); timeError = null; timePickerOpen = false }, modifier = Modifier.testTag("task_time_picker_confirm")) { Text("選択") } },
                dismissButton = { TextButton(onClick = { timePickerOpen = false }, modifier = Modifier.testTag("task_time_picker_cancel")) { Text("キャンセル") } },
            )
        }
    }
}

private fun priorityLabel(priority: TaskPriority): String = when (priority) {
    TaskPriority.LOW -> "低"
    TaskPriority.NORMAL -> "標準"
    TaskPriority.HIGH -> "高"
}
