package app.serea.android

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.lifecycle.ViewModelProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.googleauth.GoogleAuthorizationDemoViewModel
import app.serea.android.googleauth.GoogleDemoAccount
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.time.Instant
import java.time.ZoneId

/** Actual MainActivity path through consent-owned Fake Calendar handoff and Chat send validation. */
@RunWith(AndroidJUnit4::class)
class GoogleCalendarHandoffRevocationUiTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test
    fun confirmedCalendarHandoffCannotSendAfterSyntheticAccountSwitch() {
        val authViewModel = ViewModelProvider(compose.activity)
            .get(GoogleAuthorizationDemoViewModel::class.java)
        compose.waitUntil(10_000) { !compose.activity.chatViewModel.state.value.isRestoring }
        compose.activity.chatViewModel.newConversation()

        compose.onNodeWithTag("destination_settings").performClick()
        compose.onNodeWithTag("google_auth_demo_heading").performScrollTo()
        compose.onNodeWithTag("google_auth_demo_account_account_b").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().performClick()
        compose.onNodeWithText("Calendarの認可プレビュー").assertTextContains("Calendar", substring = true)
        compose.onNodeWithText("選択中: 架空アカウント B").assertTextContains("B", substring = true)
        compose.onNodeWithTag("google_auth_demo_consent_grant").performClick()
        compose.waitUntil(5_000) { authViewModel.calendarDemoSession.value != null }
        val authorizedSession = checkNotNull(authViewModel.calendarDemoSession.value)
        assertEquals(CalendarProviderKind.FAKE, authorizedSession.kind)
        assertEquals(GoogleDemoAccount.ACCOUNT_B, authViewModel.state.value.selectedAccount)
        assertEquals(2, authViewModel.state.value.accountGeneration)
        assertNotSame(compose.activity.shellProviders.calendar, authorizedSession)

        compose.onNodeWithTag("google_auth_demo_open_calendar").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("google_auth_demo_calendar_destination").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("指定日").performClick()
        compose.onNodeWithTag("calendar_date_input").performScrollTo()
        compose.onNodeWithTag("calendar_date_input").performTextClearance()
        val reviewDate = Instant.parse("2026-10-20T00:00:00Z")
            .atZone(ZoneId.systemDefault()).toLocalDate().toString()
        compose.onNodeWithTag("calendar_date_input").performTextInput(reviewDate)
        try {
            compose.waitUntil(5_000) {
                compose.onAllNodesWithText("Design review").fetchSemanticsNodes().isNotEmpty()
            }
        } catch (failure: AssertionError) {
            throw AssertionError(
                "Design review missing for fixture instant 2026-10-20T00:00:00Z; " +
                    "deviceZone=${ZoneId.systemDefault().id}, queryDate=$reviewDate, selectedAccount=ACCOUNT_B",
                failure,
            )
        }
        compose.onNodeWithText("Design review").performClick()
        compose.onNodeWithText("この予定をChatへ渡す").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Chatへ渡す内容を確認").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("送信先: この端末内のローカルChat")
            .assertTextContains("ローカルChat", substring = true)
        compose.onNodeWithText("端末内で処理してChatへ渡す").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("chat-composer").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("chat-composer").assertTextContains("Design review", substring = true)
        assertEquals(setOf(DataOrigin.CALENDAR), compose.activity.chatViewModel.state.value.editorSourceLineage)

        val usageBefore = runBlocking { compose.activity.shellUsage.snapshot() }
        authViewModel.switchDemoAccount()
        compose.waitUntil(5_000) { authViewModel.calendarDemoSession.value == null }
        assertEquals(GoogleDemoAccount.ACCOUNT_A, authViewModel.state.value.selectedAccount)
        assertTrue(runCatching { runBlocking { authorizedSession.listCalendars() } }.isFailure)

        compose.onNodeWithText("送信").performClick()
        compose.waitUntil(5_000) {
            !compose.activity.chatViewModel.state.value.isLoading &&
                compose.activity.chatViewModel.state.value.errorMessage != null
        }
        val chatState = compose.activity.chatViewModel.state.value
        assertTrue(chatState.errorMessage.orEmpty().contains("Google handoff is stale"))
        assertTrue(chatState.conversation.messages.any { it.role == MessageRole.USER && it.status == "FAILED" })
        assertFalse(chatState.conversation.messages.any { it.role == MessageRole.ASSISTANT })
        assertEquals(usageBefore.dailyRequests, runBlocking { compose.activity.shellUsage.snapshot().dailyRequests })
        assertEquals(usageBefore.uncertainRequests, runBlocking { compose.activity.shellUsage.snapshot().uncertainRequests })
        compose.onAllNodesWithText("Demo response:", substring = true).assertCountEquals(0)
    }
}
