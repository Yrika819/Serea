package app.serea.android.tasks

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TaskDateTimePickerTest {
    @get:Rule val compose = createComposeRule()

    private class EmptyRepository : TaskRepository {
        override suspend fun create(draft: TaskDraft): Task = error("Unexpected save")
        override suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task = error("Unexpected save")
        override suspend fun get(id: String): Task? = null
        override suspend fun list(): List<Task> = emptyList()
        override suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task = error("Unexpected save")
        override suspend fun complete(id: String, expectedRevision: Long): Task = error("Unexpected save")
        override suspend fun reopen(id: String, expectedRevision: Long): Task = error("Unexpected save")
        override suspend fun delete(id: String, expectedRevision: Long) = error("Unexpected delete")
        override suspend fun incomplete(): List<Task> = emptyList()
        override suspend fun completed(): List<Task> = emptyList()
        override suspend fun dueToday(today: LocalDate): List<Task> = emptyList()
        override suspend fun overdue(now: Instant, fallbackZone: ZoneId): List<Task> = emptyList()
    }

    private fun show() = compose.setContent {
        app.serea.android.SereaTheme { TaskScreen(TaskViewModel(EmptyRepository())) }
    }

    @Test fun datePickerCancelPreservesEmptyDateAndAllDayChoice() {
        show()
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_date_picker_open").performClick()
        compose.onNodeWithTag("task_date_picker_cancel").performClick()
        compose.onNodeWithTag("task_due_date").assertTextEquals("")
        compose.onAllNodesWithText("日付のみの期限にもできます。").fetchSemanticsNodes().let { assertTrue(it.isNotEmpty()) }
        assertEquals(0, runBlocking { EmptyRepository().list().size })
    }

    @Test fun timeWithoutDateShowsJapaneseDateErrorAndTimePickerCanBeCancelled() {
        show()
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput("期限の確認")
        compose.onNodeWithTag("task_time_picker_open").performClick()
        compose.onNodeWithTag("task_time_picker_cancel").performClick()
        compose.onNodeWithTag("task_due_time").assertTextEquals("")
        compose.onNodeWithTag("task_due_time").performTextInput("09:30")
        compose.onNodeWithTag("task_save").performClick()
        compose.onNodeWithTag("task_editor_error").assertIsDisplayed()
        assertTrue(compose.onAllNodesWithText("時刻を設定するには期限日が必要です。").fetchSemanticsNodes().isNotEmpty())
    }
}
