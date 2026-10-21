package app.serea.android

import android.content.pm.ActivityInfo
import android.view.WindowManager
import android.os.ParcelFileDescriptor
import androidx.activity.compose.setContent
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.printToLog
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.test.hasText
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.serea.android.assistant.taskproposal.DemoTaskProposalRepository
import app.serea.android.assistant.taskproposal.TaskProposalDemoScreenForTest
import kotlinx.coroutines.CompletableDeferred
import org.junit.Before
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.time.Instant
import java.time.ZoneId

@RunWith(AndroidJUnit4::class)
class SereaAppShellTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Before fun waitUntilChatCanAcceptInput() {
        compose.waitUntil(30_000) {
            runCatching { compose.onNodeWithTag("chat-composer").assertIsEnabled() }.isSuccess
        }
    }

    private fun select(destination: SereaDestination) = compose.onNodeWithTag(
        "destination_${destination.name.lowercase()}",
    ).performClick()

    private fun pressBack() {
        compose.activityRule.scenario.onActivity { it.onBackPressedDispatcher.onBackPressed() }
    }

    private fun shell(command: String): String {
        val descriptor = InstrumentationRegistry.getInstrumentation().uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(descriptor).bufferedReader().use { it.readText() }.trim()
    }

    @Test fun chatIsInitialDestinationAndAllRoutesAreAccessible() {
        compose.onNodeWithText("Serea").assertIsDisplayed()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
        compose.onAllNodesWithTag("destination_chat").assertCountEquals(1)
        DestinationContract.all.forEach { contract ->
            compose.onAllNodesWithTag("destination_${contract.destination.name.lowercase()}").assertCountEquals(1)
        }
    }

    @Test fun chatTasksBackReturnsToTheSameChatViewModel() {
        val viewModel = compose.activity.chatViewModel
        compose.onNodeWithTag("chat-composer").performTextInput("unsent task transition draft")
        compose.onNodeWithTag("open_tasks").performClick()
        compose.onNodeWithTag("task_add").assertIsDisplayed()
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
        assertEquals("unsent task transition draft", compose.activity.chatViewModel.state.value.input)
        assertTrue(viewModel === compose.activity.chatViewModel)
        compose.activityRule.scenario.recreate()
        compose.onNodeWithTag("chat-composer").assertTextContains("unsent task transition draft")
        assertEquals("unsent task transition draft", compose.activity.chatViewModel.state.value.input)
        assertTrue(viewModel === compose.activity.chatViewModel)
    }

    @Test fun chatGmailBackShowsOnlyFictionalMailboxData() {
        select(SereaDestination.GMAIL)
        compose.onNodeWithText("DEMO / 架空のメール — 実アカウントには接続していません").assertIsDisplayed()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Project Finch schedule").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Project Finch schedule").assertIsDisplayed()
        compose.onNodeWithText("Project Finch schedule").performClick()
        compose.onNodeWithText("The made-up Finch review starts at noon.").assertIsDisplayed()
        pressBack() // Gmail detail -> Gmail list.
        compose.onNodeWithTag("gmail_list_heading").assertTextContains("受信箱").assertIsDisplayed()
        pressBack() // Gmail destination -> Chat.
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun gmailSearchAndMessageDetailUseTheFakeProviderFixtures() {
        compose.waitUntil(10_000) { !compose.activity.chatViewModel.state.value.isRestoring }
        val chatConversationBefore = compose.activity.chatViewModel.state.value.conversation
        select(SereaDestination.GMAIL)
        compose.waitUntil(10_000) {
            runCatching { compose.onNodeWithTag("gmail_result_count").assertTextContains("7 件") }.isSuccess
        }
        compose.onNodeWithText("7 件").assertIsDisplayed()
        compose.onNodeWithTag("gmail_search_input").performTextInput("Garden plan")
        compose.onNodeWithTag("gmail_search_input").assertTextContains("Garden plan")
        compose.onNodeWithTag("gmail_search_input").performImeAction()
        val firstSearchLoaded = runCatching { compose.waitUntil(10_000) {
            runCatching {
                compose.onNodeWithTag("gmail_result_count").assertTextEquals("1 件")
                compose.onNodeWithTag("gmail_search_input").assertIsNotFocused()
            }.isSuccess
        } }.isSuccess
        if (!firstSearchLoaded) compose.onRoot(useUnmergedTree = true).printToLog("gmail-search-result-timeout")
        assertTrue("Gmail IME search must load its single fictional Garden plan message and clear field focus.", firstSearchLoaded)
        compose.onNodeWithTag("gmail_message_subject").assertTextContains("Garden plan")
        compose.activityRule.scenario.recreate()
        compose.onNodeWithTag("gmail_search_input").assertTextContains("Garden plan").assertIsDisplayed()
        compose.onNodeWithText("検索を取消").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("受信箱").fetchSemanticsNodes().isNotEmpty() &&
                runCatching { compose.onNodeWithTag("gmail_result_count").assertTextEquals("7 件") }.isSuccess
        }
        compose.onNodeWithTag("gmail_result_count").assertTextEquals("7 件").assertIsDisplayed()
        compose.onNodeWithTag("gmail_list_heading").assertTextEquals("受信箱")
        compose.onAllNodesWithTag("gmail_message_subject").onFirst().assertTextContains("Project Finch schedule")
        compose.onNodeWithTag("gmail_search_input").performTextInput("Garden plan")
        compose.onNodeWithTag("gmail_search_input").assertTextContains("Garden plan")
        Espresso.closeSoftKeyboard()
        compose.onNodeWithTag("gmail_search_submit").assertIsDisplayed().assertIsEnabled()
        compose.onNodeWithTag("gmail_search_submit").performClick()
        val repeatedSearchLoaded = runCatching { compose.waitUntil(10_000) {
            runCatching { compose.onNodeWithTag("gmail_result_count").assertTextEquals("1 件") }.isSuccess
        } }.isSuccess
        if (!repeatedSearchLoaded) compose.onRoot(useUnmergedTree = true).printToLog("gmail-repeat-search-result-timeout")
        assertTrue("Gmail search after cancellation must settle to exactly one fictional Garden plan message.", repeatedSearchLoaded)
        compose.onNodeWithTag("gmail_message_subject").assertTextContains("Garden plan")
        compose.onNodeWithTag("gmail_search_input").assertIsNotFocused()
        compose.onNodeWithTag("gmail_message_subject").performClick()
        compose.onNodeWithText("The imaginary basil bed is ready.").assertIsDisplayed()
        assertEquals(chatConversationBefore, compose.activity.chatViewModel.state.value.conversation)
    }

    @Test fun chatCalendarBackShowsFictionalAgendaAndDetail() {
        select(SereaDestination.CALENDAR)
        compose.onNodeWithTag("calendar_destination").assertIsDisplayed()
        compose.onNodeWithText("DEMO / 架空の予定").assertIsDisplayed()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("表示するカレンダー").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Demo Global").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Date-line test").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("calendar_agenda_list").performScrollToNode(hasText("Date-line test"))
        compose.onNodeWithText("Date-line test").performClick()
        compose.onNodeWithText("Date-line test").assertIsDisplayed()
        pressBack() // Event detail -> agenda.
        compose.onNodeWithText("表示するカレンダー").assertIsDisplayed()
        pressBack() // Calendar destination -> Chat.
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun chatSettingsBackKeepsCredentialScreenshotProtectionScoped() {
        compose.onNodeWithTag("open_settings").performClick()
        compose.onNodeWithText("Provider候補: Ollama Cloud").performScrollTo().assertIsDisplayed()
        compose.waitUntil(5_000) {
            compose.activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE != 0
        }
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
        compose.waitUntil(5_000) {
            compose.activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE == 0
        }
    }

    @Test fun taskProposalDemoIsFakeOnlyUnapprovedAndCanClose() {
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        compose.onNodeWithText("DEMO / 架空のタスク").assertIsDisplayed()
        compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_approve").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_close").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("chat-composer").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun taskProposalApprovalBlocksBackAndNavigationUntilResultIsKnown() {
        val repository = DemoTaskProposalRepository()
        val mutationStarted = CompletableDeferred<Unit>()
        val allowMutation = CompletableDeferred<Unit>()
        repository.beforeMutation = {
            mutationStarted.complete(Unit)
            allowMutation.await()
        }
        compose.activityRule.scenario.onActivity { activity ->
            activity.setContent {
                SereaTheme {
                    SereaAppShell(
                        chatViewModel = activity.chatViewModel,
                        credentials = activity.shellCredentials,
                        usage = activity.shellUsage,
                        taskViewModel = activity.shellTaskViewModel,
                        taskProposalDemoContent = { modifier, onClose, onPendingChanged ->
                            TaskProposalDemoScreenForTest(repository, modifier, onClose, onPendingChanged)
                        },
                    )
                }
            }
        }
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        compose.onNodeWithTag("task_proposal_approve").performScrollTo().performClick()
        compose.waitUntil(5_000) { mutationStarted.isCompleted }
        compose.waitUntil(5_000) {
            runCatching { compose.onNodeWithTag("destination_chat").assertIsNotEnabled() }.isSuccess
        }

        pressBack()
        compose.onNodeWithTag("task_proposal_demo").assertIsDisplayed()
        compose.waitUntil(5_000) {
            runCatching { compose.onNodeWithTag("task_proposal_status").assertTextContains("処理しています") }.isSuccess
        }
        compose.onNodeWithTag("task_proposal_status").assertTextContains("処理しています")
        compose.onNodeWithTag("destination_chat").assertIsNotEnabled()
        assertEquals(0, repository.writeCount())

        allowMutation.complete(Unit)
        compose.waitUntil(5_000) { repository.writeCount() == 1 }
        compose.waitUntil(5_000) {
            runCatching { compose.onNodeWithTag("task_proposal_status").assertTextContains("承認済み") }.isSuccess
        }
        compose.waitUntil(5_000) {
            runCatching { compose.onNodeWithTag("destination_chat").assertIsEnabled() }.isSuccess
        }
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun taskProposalDemoDoesNotWriteUntilExplicitApproval() {
        compose.waitUntil(10_000) { !compose.activity.shellTaskViewModel.state.value.loading }
        val tasksBefore = compose.activity.shellTaskViewModel.state.value.tasks.toList()
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("明示的に承認するまで、タスクは変更されません。").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_close").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("chat-composer").fetchSemanticsNodes().isNotEmpty()
        }
        compose.activityRule.scenario.onActivity { it.shellTaskViewModel.refresh() }
        compose.waitUntil(10_000) { !compose.activity.shellTaskViewModel.state.value.loading }
        assertEquals(null, compose.activity.shellTaskViewModel.state.value.error)
        assertEquals(tasksBefore, compose.activity.shellTaskViewModel.state.value.tasks)
    }

    @Test fun backStackReturnsThroughPriorDestinations() {
        select(SereaDestination.GMAIL)
        select(SereaDestination.CALENDAR)
        compose.onNodeWithText("DEMO / 架空の予定").assertIsDisplayed()
        pressBack()
        compose.onNodeWithTag("gmail_list_heading").assertTextContains("受信箱").assertIsDisplayed()
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun activityRecreationRestoresRouteAndBackStackButNoProposalSession() {
        select(SereaDestination.GMAIL)
        select(SereaDestination.SETTINGS)
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("Provider候補: Ollama Cloud").performScrollTo().assertIsDisplayed()
        pressBack()
        compose.onNodeWithText("DEMO / 架空のメール — 実アカウントには接続していません").assertIsDisplayed()
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun chatDraftSurvivesGmailAndCalendarDestinationChanges() {
        compose.onNodeWithTag("chat-composer").performTextInput("draft survives navigation")
        compose.waitUntil(5_000) { compose.activity.chatViewModel.state.value.input == "draft survives navigation" }
        compose.onNodeWithTag("serea_navigation_bar").assertIsDisplayed()
        select(SereaDestination.GMAIL)
        compose.onNodeWithTag("gmail_search_input").performTextInput("back clears focus")
        compose.onNodeWithTag("gmail_search_input").performImeAction()
        val emptySearchSettled = runCatching { compose.waitUntil(10_000) {
            runCatching {
                compose.onNodeWithTag("gmail_result_count").assertTextEquals("0 件")
                compose.onNodeWithTag("gmail_search_input").assertIsNotFocused()
            }.isSuccess
        } }.isSuccess
        if (!emptySearchSettled) compose.onRoot(useUnmergedTree = true).printToLog("chat-draft-gmail-search-timeout")
        assertTrue("Gmail IME search must settle before navigation and release TextField focus.", emptySearchSettled)
        pressBack()
        compose.onNodeWithTag("gmail_list_heading").assertTextContains("受信箱").assertIsDisplayed()
        pressBack()
        assertEquals("draft survives navigation", compose.activity.chatViewModel.state.value.input)
        compose.onNodeWithTag("chat-composer").assertIsNotFocused()
        select(SereaDestination.CALENDAR)
        compose.onNodeWithTag("calendar_destination").assertIsDisplayed()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("DEMO / 架空の予定").fetchSemanticsNodes().isNotEmpty()
        }
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
        compose.onNodeWithTag("chat-composer").assertTextContains("draft survives navigation")
    }

    @Test fun navigationDestinationsRemainSemanticallyAvailable() {
        val navigation = compose.onNodeWithTag("serea_navigation_bar")
        navigation.assertIsDisplayed()
        DestinationContract.all.forEach { contract ->
            compose.onNodeWithTag("destination_${contract.destination.name.lowercase()}")
                .assertIsDisplayed()
                .assertHasClickAction()
        }
        compose.onNodeWithTag("destination_chat").assertIsSelected()
        DestinationContract.all.drop(1).forEach { contract ->
            select(contract.destination)
            compose.onNodeWithTag("serea_navigation_bar").assertIsDisplayed()
            compose.onNodeWithTag("destination_${contract.destination.name.lowercase()}")
                .assertIsDisplayed()
                .assertIsSelected()
        }
    }

    @Test fun screenHeadingsAreExposedToScreenReaders() {
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithText("Serea").fetchSemanticsNode().config)

        select(SereaDestination.TASKS)
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithTag("tasks_heading").fetchSemanticsNode().config)
        select(SereaDestination.GMAIL)
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithTag("gmail_list_heading").fetchSemanticsNode().config)
        select(SereaDestination.CALENDAR)
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithText("DEMO / 架空の予定", substring = true).fetchSemanticsNode().config)
        select(SereaDestination.SETTINGS)
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithTag("destination_title").fetchSemanticsNode().config)
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithText("DEMO / 架空のタスク").fetchSemanticsNode().config)
    }

    @Test fun calendarDetailAndSelectedCalendarSurviveAnotherDestination() {
        compose.activityRule.scenario.onActivity { activity ->
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        }
        compose.waitUntil(10_000) {
            val size = compose.onRoot().fetchSemanticsNode().size
            size.width > size.height
        }

        select(SereaDestination.CALENDAR)
        compose.onNodeWithTag("calendar_destination").assertIsDisplayed()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("表示するカレンダー").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Demo Work").performClick()
        compose.waitUntil(10_000) {
            runCatching { compose.onNodeWithText("今後").assertIsEnabled() }.isSuccess
        }
        compose.onNodeWithText("今後").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("calendar_agenda_list").performScrollToNode(hasText("Fictional paged event 50"))
        compose.onNodeWithText("Fictional paged event 50").assertIsDisplayed()
        val agendaOffsetBeforeDetail = compose.onNodeWithTag("calendar_agenda_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertTrue("Calendar agenda must have several scroll positions", agendaOffsetBeforeDetail.maxValue() > 0f)
        assertTrue("The long agenda test must begin below its first event", agendaOffsetBeforeDetail.value() > 0f)
        compose.onNodeWithText("Fictional paged event 50").performClick()
        compose.onNodeWithText("Fictional paged event 50").assertIsDisplayed()
        pressBack()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
        }
        val agendaOffsetAfterDetail = compose.onNodeWithTag("calendar_agenda_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertEquals(agendaOffsetBeforeDetail.value(), agendaOffsetAfterDetail.value(), 1f)

        select(SereaDestination.GMAIL)
        pressBack()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
        }
        val agendaOffsetAfterDestination = compose.onNodeWithTag("calendar_agenda_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertEquals(agendaOffsetBeforeDetail.value(), agendaOffsetAfterDestination.value(), 1f)

        compose.activityRule.scenario.recreate()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
        }
        val agendaOffsetAfterRecreation = compose.onNodeWithTag("calendar_agenda_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertEquals(agendaOffsetBeforeDetail.value(), agendaOffsetAfterRecreation.value(), 1f)

        // Both query and calendar changes are checked against long lists, avoiding a false pass
        // caused by a short list clamping a stale scroll offset to zero.
        compose.onNodeWithText("指定日").performClick()
        compose.onNodeWithTag("calendar_date_input").performTextClearance()
        val displayZone = ZoneId.systemDefault()
        val firstBulkDate = Instant.parse("2026-10-22T16:00:00Z").atZone(displayZone).toLocalDate().toString()
        val secondBulkDate = Instant.parse("2026-10-23T16:00:00Z").atZone(displayZone).toLocalDate().toString()
        compose.onNodeWithTag("calendar_date_input").performTextInput(firstBulkDate)
        assertEquals(
            firstBulkDate,
            compose.onNodeWithTag("calendar_date_input").fetchSemanticsNode()
                .config[SemanticsProperties.EditableText].text,
        )
        compose.activityRule.scenario.onActivity { activity ->
            val manager = activity.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                as android.view.inputmethod.InputMethodManager
            manager.hideSoftInputFromWindow(activity.window.decorView.windowToken, 0)
        }
        compose.waitUntil(10_000) {
            runCatching {
                val range = compose.onNodeWithTag("calendar_agenda_list")
                    .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
                range.maxValue() > 0f && range.value() == 0f
            }.getOrDefault(false)
        }
        val agendaOffsetAfterQueryChange = compose.onNodeWithTag("calendar_agenda_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertTrue("date query must retain a scrollable Work agenda", agendaOffsetAfterQueryChange.maxValue() > 0f)
        assertEquals(0f, agendaOffsetAfterQueryChange.value(), 1f)

        compose.onNodeWithTag("calendar_agenda_list").performScrollToNode(hasText("Fictional paged event 40"))
        compose.onNodeWithText("Fictional paged event 40").assertIsDisplayed()
        val workOffsetBeforeDateChange = compose.onNodeWithTag("calendar_agenda_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertTrue("date-change test must start below the first Work event", workOffsetBeforeDateChange.value() > 0f)

        compose.onNodeWithTag("calendar_date_input").performTextClearance()
        compose.onNodeWithTag("calendar_date_input").performTextInput(secondBulkDate)
        assertEquals(
            secondBulkDate,
            compose.onNodeWithTag("calendar_date_input").fetchSemanticsNode()
                .config[SemanticsProperties.EditableText].text,
        )
        compose.activityRule.scenario.onActivity { activity ->
            val manager = activity.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                as android.view.inputmethod.InputMethodManager
            manager.hideSoftInputFromWindow(activity.window.decorView.windowToken, 0)
        }
        compose.waitUntil(10_000) {
            runCatching {
                val range = compose.onNodeWithTag("calendar_agenda_list")
                    .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
                range.maxValue() > 0f && range.value() == 0f
            }.getOrDefault(false)
        }
        val agendaOffsetAfterDateChange = compose.onNodeWithTag("calendar_agenda_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertTrue("changing the selected date must keep a scrollable Work agenda", agendaOffsetAfterDateChange.maxValue() > 0f)
        assertEquals(0f, agendaOffsetAfterDateChange.value(), 1f)

        compose.onNodeWithTag("calendar_agenda_list").performScrollToNode(hasText("Fictional paged event 64"))
        compose.onNodeWithText("Fictional paged event 64").assertIsDisplayed()
        val workOffsetBeforeCalendarChange = compose.onNodeWithTag("calendar_agenda_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertTrue("calendar switch test must start below the first Work event", workOffsetBeforeCalendarChange.value() > 0f)
        compose.onNodeWithText("今後").performClick()
        compose.onNodeWithText("Demo Global").performClick()
        compose.waitUntil(10_000) {
            runCatching {
                val range = compose.onNodeWithTag("calendar_agenda_list")
                    .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
                range.maxValue() > 0f && range.value() == 0f
            }.getOrDefault(false)
        }
        val agendaOffsetAfterCalendarChange = compose.onNodeWithTag("calendar_agenda_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertTrue("Demo Global must also provide a long agenda for reset verification", agendaOffsetAfterCalendarChange.maxValue() > 0f)
        assertEquals(0f, agendaOffsetAfterCalendarChange.value(), 1f)
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Date-line test").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("calendar_agenda_list").performScrollToNode(hasText("Date-line test"))
        compose.onNodeWithText("Date-line test").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_event_detail_list").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("calendar_event_detail_list").performScrollToNode(hasText("説明"))
        compose.onNodeWithText("説明").assertIsDisplayed()
        val detailScrollBefore = compose.onNodeWithTag("calendar_event_detail_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertTrue("calendar detail must be scrollable in this viewport", detailScrollBefore.maxValue() > 0f)
        assertTrue("calendar detail must have a non-zero saved offset", detailScrollBefore.value() > 0f)

        select(SereaDestination.GMAIL)
        compose.onNodeWithTag("gmail_list_heading").assertIsDisplayed()
        pressBack()

        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_event_detail_list").fetchSemanticsNodes().isNotEmpty()
        }
        val detailScrollAfter = compose.onNodeWithTag("calendar_event_detail_list")
            .fetchSemanticsNode().config[SemanticsProperties.VerticalScrollAxisRange]
        assertTrue(detailScrollAfter.maxValue() > 0f)
        assertEquals(detailScrollBefore.value(), detailScrollAfter.value(), 1f)
        compose.onNodeWithTag("calendar_event_detail_list").performScrollToNode(hasText("Date-line test"))
        compose.onNodeWithText("Date-line test").assertIsDisplayed()
        compose.onNodeWithText("説明").assertIsDisplayed()
        compose.onNodeWithTag("calendar_event_detail_list").performScrollToNode(hasText("開始・終了"))
        compose.onNodeWithText("開始・終了").assertIsDisplayed()
        pressBack()
        compose.onNodeWithText("Demo Global").assertIsSelected()
    }

    @Test fun taskEditorDraftSurvivesActivityRecreation() {
        compose.waitUntil(10_000) { !compose.activity.shellTaskViewModel.state.value.loading }
        select(SereaDestination.TASKS)
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput("unsaved task draft")
        compose.activityRule.scenario.onActivity { activity ->
            val manager = activity.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                as android.view.inputmethod.InputMethodManager
            manager.hideSoftInputFromWindow(activity.window.decorView.windowToken, 0)
        }
        compose.waitForIdle()

        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("新しいタスク").assertIsDisplayed()
        compose.onNodeWithTag("task_title").assertTextContains("unsaved task draft")
    }

    @Test fun taskProposalRotationStartsANewUnapprovedFakeReview() {
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_approve").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Fake Repository 書き込み回数: 1").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Fake Repository 書き込み回数: 1").performScrollTo().assertIsDisplayed()
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("DEMO / 架空のタスク").assertIsDisplayed()
        compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("明示的に承認するまで、タスクは変更されません。").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_approve").performScrollTo().assertIsEnabled()
    }

    @Test fun gmailAndCalendarDetailsRestoreAcrossActivityRecreation() {
        select(SereaDestination.GMAIL)
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Project Finch schedule").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Project Finch schedule").performClick()
        compose.onNodeWithText("The made-up Finch review starts at noon.").assertIsDisplayed()
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("The made-up Finch review starts at noon.").assertIsDisplayed()
        pressBack()
        compose.onNodeWithTag("gmail_list_heading").assertTextContains("受信箱").assertIsDisplayed()
        pressBack()

        select(SereaDestination.CALENDAR)
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("表示するカレンダー").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Demo Global").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Date-line test").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("calendar_agenda_list").performScrollToNode(hasText("Date-line test"))
        compose.onNodeWithText("Date-line test").performClick()
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("Date-line test").assertIsDisplayed()
    }

    @Test fun fontScaleTwoKeepsTaskProposalReviewAndFooterReachable() {
        val originalScale = compose.activity.resources.configuration.fontScale
        val originalWidthDp = compose.activity.resources.configuration.screenWidthDp
        val originalDensityDpi = compose.activity.resources.configuration.densityDpi
        val originalFontScaleSetting = shell("settings get system font_scale")
        check(originalFontScaleSetting == "null" || originalFontScaleSetting.toFloatOrNull() != null) {
            "Unable to read the original system font scale setting"
        }
        val originalSizeOverride = shell("wm size").lineSequence()
            .firstOrNull { it.startsWith("Override size:") }
            ?.substringAfter(':')?.trim()?.takeIf { it.isNotEmpty() }
        val originalDensityOverride = shell("wm density").lineSequence()
            .firstOrNull { it.startsWith("Override density:") }
            ?.substringAfter(':')?.trim()?.takeIf { it.isNotEmpty() }
        try {
            shell("wm size 320x640")
            shell("wm density 160")
            shell("settings put system font_scale 2.0")
            compose.waitUntil(15_000) {
                var fontScale = 1f
                var width = 0
                var densityDpi = 0
                compose.activityRule.scenario.onActivity {
                    fontScale = it.resources.configuration.fontScale
                    width = it.resources.configuration.screenWidthDp
                    densityDpi = it.resources.configuration.densityDpi
                }
                fontScale >= 1.99f && width == 320 && densityDpi == 160
            }
            DestinationContract.all.forEach { contract ->
                compose.onNodeWithTag("destination_${contract.destination.name.lowercase()}")
                    .assertIsDisplayed()
                    .assertTextContains(contract.title)
            }
            compose.onNodeWithTag("chat-composer").assertIsDisplayed()

            select(SereaDestination.TASKS)
            compose.waitUntil(10_000) { !compose.activity.shellTaskViewModel.state.value.loading }
            compose.onNodeWithTag("tasks_heading").assertIsDisplayed()
            compose.onNodeWithTag("task_add").assertIsDisplayed()

            select(SereaDestination.SETTINGS)
            compose.onNodeWithTag("destination_title").assertTextContains("設定").assertIsDisplayed()
            compose.onNodeWithText("Provider候補: Ollama Cloud").performScrollTo().assertIsDisplayed()
            compose.onNodeWithText("DEMO · Cloud送信: 無効", substring = false).performScrollTo().assertIsDisplayed()
            pressBack()
            compose.onNodeWithTag("tasks_heading").assertIsDisplayed()
            pressBack()
            compose.onNodeWithTag("chat-composer").assertIsDisplayed()

            select(SereaDestination.GMAIL)
            val gmailHeadingDisplayed = runCatching { compose.waitUntil(10_000) {
                runCatching { compose.onNodeWithTag("gmail_list_heading").assertIsDisplayed() }.isSuccess
            } }.isSuccess
            if (!gmailHeadingDisplayed) compose.onRoot(useUnmergedTree = true).printToLog("font-scale-gmail-heading")
            assertTrue("The Gmail heading must remain displayed after returning through Tasks and Settings at font scale 2.0.", gmailHeadingDisplayed)
            compose.onNodeWithTag("gmail_list_heading").assertIsDisplayed()
            compose.onNodeWithTag("gmail_search_input").assertIsDisplayed()
            compose.waitUntil(10_000) {
                compose.onAllNodesWithText("7 件").fetchSemanticsNodes().isNotEmpty()
            }
            compose.onAllNodesWithTag("gmail_message_subject").onFirst().performScrollTo().assertIsDisplayed()

            select(SereaDestination.CALENDAR)
            compose.onNodeWithText("DEMO / 架空の予定").assertIsDisplayed()
            compose.waitUntil(10_000) {
                compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
            }
            compose.onNodeWithTag("calendar_agenda_list").performScrollToNode(hasText("Design review"))
            compose.onNodeWithText("Design review").assertIsDisplayed()

            select(SereaDestination.TASK_PROPOSAL_DEMO)
            compose.onNodeWithText("DEMO / 架空のタスク").assertIsDisplayed()
            listOf("create", "update", "complete", "delete").forEach { operation ->
                compose.onNodeWithTag("task_proposal_operation_$operation").performScrollTo().assertIsDisplayed()
            }
            compose.waitUntil(10_000) {
                compose.onAllNodesWithTag("task_proposal_approve").fetchSemanticsNodes().isNotEmpty()
            }
            compose.waitUntil(10_000) {
                runCatching { compose.onNodeWithTag("task_proposal_approve").performScrollTo().assertIsDisplayed() }.isSuccess
            }
            compose.onNodeWithTag("task_proposal_close").performScrollTo().assertIsDisplayed()
            compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
            compose.onNodeWithTag("task_proposal_approve").performScrollTo().assertIsDisplayed()
            compose.onNodeWithText("デモをリセット").performScrollTo().assertIsDisplayed().performClick()
            compose.waitUntil(10_000) {
                runCatching { compose.onNodeWithTag("task_proposal_close").performScrollTo().assertIsDisplayed() }.isSuccess
            }
        } finally {
            runCatching {
                if (originalFontScaleSetting == "null") {
                    shell("settings delete system font_scale")
                } else {
                    shell("settings put system font_scale $originalFontScaleSetting")
                }
            }
            runCatching {
                if (originalSizeOverride == null) shell("wm size reset") else shell("wm size $originalSizeOverride")
            }
            runCatching {
                if (originalDensityOverride == null) shell("wm density reset") else shell("wm density $originalDensityOverride")
            }
            val restoredSizeOverride = shell("wm size").lineSequence()
                .firstOrNull { it.startsWith("Override size:") }
                ?.substringAfter(':')?.trim()?.takeIf { it.isNotEmpty() }
            val restoredDensityOverride = shell("wm density").lineSequence()
                .firstOrNull { it.startsWith("Override density:") }
                ?.substringAfter(':')?.trim()?.takeIf { it.isNotEmpty() }
            assertEquals(originalFontScaleSetting, shell("settings get system font_scale"))
            assertEquals(originalSizeOverride, restoredSizeOverride)
            assertEquals(originalDensityOverride, restoredDensityOverride)
            compose.waitUntil(15_000) {
                var width = 0
                var densityDpi = 0
                var fontScale = 0f
                compose.activityRule.scenario.onActivity {
                    width = it.resources.configuration.screenWidthDp
                    densityDpi = it.resources.configuration.densityDpi
                    fontScale = it.resources.configuration.fontScale
                }
                width == originalWidthDp && densityDpi == originalDensityDpi &&
                    kotlin.math.abs(fontScale - originalScale) < 0.02f
            }
        }
    }
}
