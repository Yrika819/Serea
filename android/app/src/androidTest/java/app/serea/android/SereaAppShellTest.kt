package app.serea.android

import android.view.WindowManager
import android.os.ParcelFileDescriptor
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Before
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

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
        compose.onNodeWithTag("gmail_search_input").performTextInput("Garden plan")
        compose.onNodeWithTag("gmail_search_input").assertTextContains("Garden plan")
        compose.onNodeWithTag("gmail_search_input").performImeAction()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("gmail_message_subject").fetchSemanticsNodes().size == 1
        }
        compose.onNodeWithTag("gmail_message_subject").assertTextContains("Garden plan")
        compose.activityRule.scenario.recreate()
        compose.onNodeWithTag("gmail_search_input").assertTextContains("Garden plan").assertIsDisplayed()
        compose.onNodeWithText("検索を取消").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("受信箱", substring = true).fetchSemanticsNodes().isNotEmpty() &&
                compose.onAllNodesWithTag("gmail_message_subject").fetchSemanticsNodes().size == 7
        }
        compose.onNodeWithTag("gmail_search_input").performTextInput("Garden plan")
        compose.onNodeWithTag("gmail_search_input").assertTextContains("Garden plan")
        compose.onNodeWithTag("gmail_search_submit").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("gmail_message_subject").fetchSemanticsNodes().size == 1
        }
        compose.onNodeWithTag("gmail_message_subject").assertTextContains("Garden plan")
        compose.onNodeWithTag("gmail_search_input").assertIsNotFocused()
        compose.onNodeWithTag("gmail_message_subject").performClick()
        compose.onNodeWithText("The imaginary basil bed is ready.").assertIsDisplayed()
        assertEquals(chatConversationBefore, compose.activity.chatViewModel.state.value.conversation)
    }

    @Test fun chatCalendarBackShowsFictionalAgendaAndDetail() {
        select(SereaDestination.CALENDAR)
        compose.onNodeWithText("DEMO / 架空の予定").assertIsDisplayed()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("表示するカレンダー").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Demo Global").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Date-line test").fetchSemanticsNodes().isNotEmpty()
        }
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

    @Test fun taskProposalDemoDoesNotWriteUntilExplicitApproval() {
        compose.waitUntil(10_000) { !compose.activity.shellTaskViewModel.state.value.loading }
        val taskIdsBefore = compose.activity.shellTaskViewModel.state.value.tasks.map { it.id }
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("明示的に承認するまで、タスクは変更されません。").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_close").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("chat-composer").fetchSemanticsNodes().isNotEmpty()
        }
        assertEquals(taskIdsBefore, compose.activity.shellTaskViewModel.state.value.tasks.map { it.id })
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
        compose.waitUntil(5_000) {
            runCatching { compose.onNodeWithTag("gmail_search_input").assertIsNotFocused() }.isSuccess
        }
        pressBack()
        assertEquals("draft survives navigation", compose.activity.chatViewModel.state.value.input)
        compose.onNodeWithTag("chat-composer").assertIsNotFocused()
        select(SereaDestination.CALENDAR)
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
        select(SereaDestination.CALENDAR)
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("表示するカレンダー").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Demo Global").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Date-line test").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Date-line test").performClick()
        compose.onNodeWithText("Date-line test").assertIsDisplayed()

        select(SereaDestination.GMAIL)
        pressBack()

        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("開始・終了").fetchSemanticsNodes().isNotEmpty()
        }
        compose.waitUntil(10_000) {
            runCatching { compose.onNodeWithText("開始・終了").assertIsDisplayed() }.isSuccess
        }
        compose.onNodeWithText("Date-line test").assertIsDisplayed()
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
        compose.onNodeWithText("Date-line test").performClick()
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("Date-line test").assertIsDisplayed()
    }

    @Test fun fontScaleTwoKeepsTaskProposalReviewAndFooterReachable() {
        val originalScale = compose.activity.resources.configuration.fontScale
        shell("wm size 320x640")
        shell("wm density 160")
        shell("settings put system font_scale 2.0")
        try {
            compose.waitUntil(15_000) {
                var fontScale = 1f
                compose.activityRule.scenario.onActivity { fontScale = it.resources.configuration.fontScale }
                fontScale >= 1.99f
            }
            DestinationContract.all.forEach { contract ->
                compose.onNodeWithTag("destination_${contract.destination.name.lowercase()}")
                    .assertIsDisplayed()
                    .assertTextContains(contract.title)
            }
            select(SereaDestination.TASK_PROPOSAL_DEMO)
            compose.onNodeWithText("DEMO / 架空のタスク").assertIsDisplayed()
            compose.waitUntil(10_000) {
                compose.onAllNodesWithTag("task_proposal_approve").fetchSemanticsNodes().isNotEmpty()
            }
            compose.waitUntil(10_000) {
                runCatching { compose.onNodeWithTag("task_proposal_approve").performScrollTo().assertIsDisplayed() }.isSuccess
            }
            compose.onNodeWithText("デモをリセット").performScrollTo().assertIsDisplayed().performClick()
            compose.waitUntil(10_000) {
                runCatching { compose.onNodeWithTag("task_proposal_close").performScrollTo().assertIsDisplayed() }.isSuccess
            }
        } finally {
            shell("settings put system font_scale $originalScale")
            shell("wm size reset")
            shell("wm density reset")
            compose.waitUntil(15_000) {
                var width = 0
                var fontScale = 0f
                compose.activityRule.scenario.onActivity {
                    width = it.resources.configuration.screenWidthDp
                    fontScale = it.resources.configuration.fontScale
                }
                width > 340 && kotlin.math.abs(fontScale - originalScale) < 0.02f
            }
        }
    }
}
