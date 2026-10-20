package app.serea.android.tasks

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assert
import androidx.compose.material3.DatePickerDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.ui.test.hasClickAction
import androidx.compose.ui.test.hasScrollAction
import androidx.compose.ui.test.hasTestTag
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.semantics.Role
import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId
import java.text.NumberFormat
import java.util.Locale
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TaskDateTimePickerTest {
    @get:Rule val compose = createComposeRule()

    private fun localizedPickerNumber(number: Int, locale: Locale): SemanticsMatcher {
        val digits = NumberFormat.getIntegerInstance(locale).format(number)
        val exactNumber = Regex("(?<!\\p{N})${Regex.escape(digits)}(?!\\p{N})")
        return hasClickAction() and SemanticsMatcher("Material time picker dial choice for $number") { node ->
            SemanticsProperties.TraversalIndex in node.config &&
                SemanticsProperties.ContentDescription in node.config &&
                node.config[SemanticsProperties.ContentDescription].any { description -> exactNumber.containsMatchIn(description) }
        }
    }

    private fun timeSelectorNumber(number: Int, locale: Locale): SemanticsMatcher {
        val digits = NumberFormat.getIntegerInstance(locale).format(number)
        val exactNumber = Regex("(?<!\\p{N})${Regex.escape(digits)}(?!\\p{N})")
        return SemanticsMatcher("Material time picker hour/minute selector showing $number") { node ->
            SemanticsProperties.Role in node.config &&
                node.config[SemanticsProperties.Role] == Role.RadioButton &&
                SemanticsProperties.ContentDescription in node.config &&
                node.config[SemanticsProperties.ContentDescription].any { description -> exactNumber.containsMatchIn(description) }
        }
    }

    private val unselectedTimeSelector = SemanticsMatcher("Unselected Material time picker hour/minute selector") { node ->
        SemanticsProperties.Role in node.config &&
            node.config[SemanticsProperties.Role] == Role.RadioButton &&
            SemanticsProperties.Selected in node.config &&
            !node.config[SemanticsProperties.Selected] &&
            SemanticsProperties.ContentDescription in node.config
    }

    private fun selectedLocalizedPickerNumber(number: Int, locale: Locale): SemanticsMatcher {
        val digits = NumberFormat.getIntegerInstance(locale).format(number)
        val exactNumber = Regex("(?<!\\p{N})${Regex.escape(digits)}(?!\\p{N})")
        return SemanticsMatcher("Selected Material time picker dial choice for $number") { node ->
            SemanticsProperties.TraversalIndex in node.config &&
                SemanticsProperties.ContentDescription in node.config &&
                node.config[SemanticsProperties.ContentDescription].any { description -> exactNumber.containsMatchIn(description) } &&
                SemanticsProperties.Selected in node.config && node.config[SemanticsProperties.Selected]
        }
    }

    private class RecordingRepository : TaskRepository {
        data class UpdateCall(val id: String, val expectedRevision: Long, val draft: TaskDraft)
        val createDrafts = mutableListOf<TaskDraft>()
        val updateCalls = mutableListOf<UpdateCall>()
        private val tasks = mutableListOf<Task>()

        fun seed(task: Task) { tasks += task }

        override suspend fun create(draft: TaskDraft): Task {
            createDrafts += draft
            return Task(
                id = "00000000-0000-0000-0000-000000000001", title = draft.title,
                description = draft.description, status = TaskStatus.TODO, priority = draft.priority,
                createdAt = Instant.EPOCH, updatedAt = Instant.EPOCH, dueDate = draft.dueDate,
                dueTime = draft.dueTime, timeZoneId = draft.timeZoneId, revision = 1,
            ).also { tasks.add(it) }
        }
        override suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task = create(draft)
        override suspend fun get(id: String): Task? = tasks.firstOrNull { it.id == id }
        override suspend fun list(): List<Task> = tasks.toList()
        override suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task {
            updateCalls += UpdateCall(id, expectedRevision, draft)
            val old = tasks.first { it.id == id }
            return old.copy(title = draft.title, description = draft.description, priority = draft.priority,
                dueDate = draft.dueDate, dueTime = draft.dueTime, timeZoneId = draft.timeZoneId,
                revision = old.revision + 1).also { tasks[tasks.indexOf(old)] = it }
        }
        override suspend fun complete(id: String, expectedRevision: Long): Task = error("Unexpected complete")
        override suspend fun reopen(id: String, expectedRevision: Long): Task = error("Unexpected reopen")
        override suspend fun delete(id: String, expectedRevision: Long) { error("Unexpected delete") }
        override suspend fun incomplete(): List<Task> = tasks.filter { it.status == TaskStatus.TODO }
        override suspend fun completed(): List<Task> = tasks.filter { it.status == TaskStatus.COMPLETED }
        override suspend fun dueToday(today: LocalDate): List<Task> = tasks.filter { it.dueDate == today }
        override suspend fun overdue(now: Instant, fallbackZone: ZoneId): List<Task> = tasks.filter { it.isOverdue(now, fallbackZone) }
    }

    private fun show(repository: RecordingRepository) = compose.setContent {
        app.serea.android.SereaTheme { TaskScreen(TaskViewModel(repository)) }
    }

    @OptIn(ExperimentalMaterial3Api::class)
    @Test fun datePickerConfirmSavesDateOnlyTaskAndEditorCanScroll() {
        val repository = RecordingRepository()
        show(repository)
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput("All day from picker")
        compose.onNodeWithTag("task_due_date").performTextInput("2026-07-04")
        compose.onNodeWithTag("task_editor_scroll").assert(hasScrollAction())
        compose.onNodeWithTag("task_date_picker_open").performClick()
        val selectedDate = LocalDate.parse("2026-07-15")
        val pickerLocale = InstrumentationRegistry.getInstrumentation().targetContext.resources.configuration.locales[0]
        val selectedDateMillis = selectedDate.atStartOfDay(java.time.ZoneOffset.UTC).toInstant().toEpochMilli()
        val dateSemantics = requireNotNull(DatePickerDefaults.dateFormatter().formatDate(
            selectedDateMillis, pickerLocale, forContentDescription = true,
        ))
        compose.waitUntil(5_000) { compose.onAllNodesWithText(dateSemantics).fetchSemanticsNodes().isNotEmpty() }
        val day = compose.onNodeWithText(dateSemantics)
        day.performClick()
        day.assertIsSelected()
        compose.onNodeWithTag("task_date_picker_confirm").performClick()
        compose.onNodeWithTag("task_due_date").assertIsDisplayed()
        compose.onNodeWithTag("task_editor_scroll").performScrollToNode(hasTestTag("task_timezone"))
        compose.onNodeWithTag("task_save").performClick()
        compose.waitUntil(5_000) { repository.createDrafts.size == 1 }
        assertEquals(1, repository.createDrafts.size)
        val saved = repository.createDrafts.single()
        assertEquals(LocalDate.parse("2026-07-15"), saved.dueDate)
        assertNull(saved.dueTime)
        assertTrue(repository.updateCalls.isEmpty())
    }

    @Test fun timePickerConfirmSavesTwentyFourHourTimeWithIanaZone() {
        val repository = RecordingRepository()
        show(repository)
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput("Timed from picker")
        compose.onNodeWithTag("task_due_date").performTextInput("2026-10-20")
        compose.onNodeWithTag("task_due_time").performTextInput("00:00")
        compose.onNodeWithTag("task_timezone").performTextClearance()
        compose.onNodeWithTag("task_timezone").performTextInput("Asia/Tokyo")
        compose.onNodeWithTag("task_time_picker_open").performClick()
        val pickerLocale = InstrumentationRegistry.getInstrumentation().targetContext.resources.configuration.locales[0]
        compose.onNode(localizedPickerNumber(9, pickerLocale)).performClick()
        val hourSelector = compose.onNode(timeSelectorNumber(9, pickerLocale))
        hourSelector.assertIsDisplayed()
        hourSelector.performClick()
        hourSelector.assertIsSelected()
        compose.onNode(selectedLocalizedPickerNumber(9, pickerLocale)).assertIsSelected()
        compose.onNode(unselectedTimeSelector).performClick()
        val minute = compose.onNode(localizedPickerNumber(35, pickerLocale))
        compose.waitUntil(5_000) { compose.onAllNodes(localizedPickerNumber(35, pickerLocale)).fetchSemanticsNodes().isNotEmpty() }
        minute.performClick()
        compose.onNode(selectedLocalizedPickerNumber(35, pickerLocale)).assertIsSelected()
        compose.onNodeWithTag("task_time_picker_confirm").performClick()
        compose.onNodeWithTag("task_due_time").assertIsDisplayed()
        compose.onNodeWithTag("task_save").performClick()
        compose.waitUntil(5_000) { repository.createDrafts.size == 1 }
        val saved = repository.createDrafts.single()
        assertEquals(LocalDate.parse("2026-10-20"), saved.dueDate)
        assertEquals(LocalTime.of(9, 35), saved.dueTime)
        assertEquals("Asia/Tokyo", saved.timeZoneId)
        assertEquals(1, repository.createDrafts.size)
        assertTrue(repository.updateCalls.isEmpty())
    }

    @Test fun editingExistingTaskUpdatesExactDraftAndExpectedRevision() {
        val repository = RecordingRepository()
        val existing = Task(
            id = "00000000-0000-0000-0000-000000000042", title = "Before edit",
            description = "old notes", status = TaskStatus.TODO, priority = TaskPriority.NORMAL,
            createdAt = Instant.parse("2026-01-01T00:00:00Z"), updatedAt = Instant.parse("2026-01-02T00:00:00Z"),
            dueDate = LocalDate.parse("2026-10-20"), dueTime = LocalTime.of(9, 0),
            timeZoneId = "Asia/Tokyo", revision = 7,
        )
        repository.seed(existing)
        show(repository)
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Before edit").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("task_edit_${existing.id}").performClick()
        compose.onNodeWithTag("task_title").performTextClearance()
        compose.onNodeWithTag("task_title").performTextInput("After edit")
        compose.onNodeWithTag("task_description").performTextClearance()
        compose.onNodeWithTag("task_description").performTextInput("new notes")
        compose.onNodeWithTag("task_due_date").performTextClearance()
        compose.onNodeWithTag("task_due_date").performTextInput("2026-12-31")
        compose.onNodeWithTag("task_due_time").performTextClearance()
        compose.onNodeWithTag("task_due_time").performTextInput("18:45")
        compose.onNodeWithTag("task_timezone").performTextClearance()
        compose.onNodeWithTag("task_timezone").performTextInput("America/New_York")
        compose.onNodeWithTag("task_save").performClick()
        compose.waitUntil(5_000) { repository.updateCalls.size == 1 }

        val call = repository.updateCalls.single()
        assertEquals(existing.id, call.id)
        assertEquals(7L, call.expectedRevision)
        assertEquals(
            TaskDraft(
                title = "After edit", description = "new notes", priority = TaskPriority.NORMAL,
                dueDate = LocalDate.parse("2026-12-31"), dueTime = LocalTime.of(18, 45),
                timeZoneId = "America/New_York",
            ),
            call.draft,
        )
        assertTrue(repository.createDrafts.isEmpty())
    }

    @Test fun invalidIanaZoneIsReportedAndDoesNotCallRepository() {
        val repository = RecordingRepository()
        show(repository)
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput("Invalid zone")
        compose.onNodeWithTag("task_due_date").performTextInput("2026-12-01")
        compose.onNodeWithTag("task_timezone").performTextClearance()
        compose.onNodeWithTag("task_timezone").performTextInput("Mars/Olympus")
        compose.onNodeWithTag("task_save").performClick()
        compose.onAllNodesWithText("有効な IANA タイムゾーンを入力してください（例: Asia/Tokyo）。").fetchSemanticsNodes().also { assertTrue(it.isNotEmpty()) }
        assertTrue(repository.createDrafts.isEmpty())
        assertTrue(repository.updateCalls.isEmpty())
    }

    @Test fun daylightSavingGapAndOverlapAreReportedWithoutRepositoryCalls() {
        val repository = RecordingRepository()
        show(repository)
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput("DST boundary")
        compose.onNodeWithTag("task_due_date").performTextInput("2026-03-08")
        compose.onNodeWithTag("task_due_time").performTextInput("02:30")
        compose.onNodeWithTag("task_timezone").performTextClearance()
        compose.onNodeWithTag("task_timezone").performTextInput("America/New_York")
        compose.onNodeWithTag("task_save").performClick()
        assertTrue(compose.onAllNodesWithText("この時刻は夏時間の切り替えで存在しません。日付または時刻を変更してください。").fetchSemanticsNodes().isNotEmpty())

        compose.onNodeWithTag("task_due_date").performTextClearance()
        compose.onNodeWithTag("task_due_date").performTextInput("2026-11-01")
        compose.onNodeWithTag("task_due_time").performTextClearance()
        compose.onNodeWithTag("task_due_time").performTextInput("01:30")
        compose.onNodeWithTag("task_save").performClick()
        assertTrue(compose.onAllNodesWithText("この時刻は夏時間の切り替えで重複します。別の時刻を選んでください。").fetchSemanticsNodes().isNotEmpty())
        assertTrue(repository.createDrafts.isEmpty())
        assertTrue(repository.updateCalls.isEmpty())
    }

    @Test fun timeWithoutDateShowsJapaneseDateErrorAndDoesNotSave() {
        val repository = RecordingRepository()
        show(repository)
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput("Time needs a date")
        compose.onNodeWithTag("task_time_picker_open").performClick()
        compose.onNodeWithTag("task_time_picker_cancel").performClick()
        val dueTimeNode = compose.onNodeWithTag("task_due_time").fetchSemanticsNode()
        assertEquals("", if (SemanticsProperties.EditableText in dueTimeNode.config) dueTimeNode.config[SemanticsProperties.EditableText].text else null)
        compose.onNodeWithTag("task_due_time").performTextInput("09:30")
        compose.onNodeWithTag("task_save").performClick()
        val errorText = "時刻を設定するには期限日が必要です。"
        compose.onNodeWithTag("task_editor_scroll").performScrollToNode(SemanticsMatcher("time-needs-date inline error") { node ->
            node.config.getOrNull(SemanticsProperties.Text)?.any { it.text == errorText } == true
        })
        compose.onNodeWithText(errorText).assertIsDisplayed()
        assertTrue(repository.createDrafts.isEmpty())
    }
}
