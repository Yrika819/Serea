package app.serea.android

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.calendar.FakeCalendarProvider
import app.serea.android.gmail.FakeGmailFailure
import app.serea.android.gmail.FakeGmailProvider
import app.serea.android.gmail.GmailAuthorizationPort
import app.serea.android.gmail.GmailAuthorizationState
import app.serea.android.gmail.GmailAuthorizationStatus
import app.serea.android.gmail.GmailMessage
import app.serea.android.gmail.GmailMessageId
import app.serea.android.gmail.GmailMessagePage
import app.serea.android.gmail.GmailReadPort
import app.serea.android.gmail.GmailReadFailure
import app.serea.android.gmail.GmailReadFailureCode
import app.serea.android.gmail.GmailThread
import app.serea.android.gmail.GmailThreadId
import app.serea.android.gmail.ui.GmailScreen
import app.serea.android.gmail.ui.GmailViewModel
import app.serea.android.calendar.ui.CalendarAgendaScreen
import kotlinx.coroutines.CompletableDeferred
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.time.Duration
import java.time.ZoneId

@RunWith(AndroidJUnit4::class)
class MockUiStateIntegrationTest {
    @get:Rule val compose = createComposeRule()

    @Test fun gmailLoadingTransitionsToEmptyAgenda() {
        val provider = HeldGmailProvider()
        val viewModel = GmailViewModel(provider, provider)
        compose.setContent { SereaTheme { GmailScreen(viewModel, onBack = {}) } }

        compose.waitUntil(5_000) { provider.readStarted.isCompleted }
        compose.onNodeWithText("読み込み中…").assertIsDisplayed()
        provider.page.complete(GmailMessagePage(emptyList()))
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("受信トレイは空です").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("受信トレイは空です").assertIsDisplayed()
    }

    @Test fun gmailFailureStateIsVisibleAndRetryable() {
        val retrying = RetryGmailProvider()
        compose.setContent { SereaTheme { GmailScreen(GmailViewModel(retrying, retrying), onBack = {}) } }
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("メールを読み込めませんでした。再試行してください").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("メールを読み込めませんでした。再試行してください").assertIsDisplayed()
        compose.onNodeWithText("再試行").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Project Finch schedule").fetchSemanticsNodes().isNotEmpty()
        }
        assert(retrying.inboxCalls >= 2)
        compose.onNodeWithText("Project Finch schedule").assertIsDisplayed()
    }

    @Test fun calendarLoadingTransitionsToEmptyAgenda() {
        val provider = FakeCalendarProvider(
            events = emptyList(),
            options = FakeCalendarProvider.Options(pageDelay = Duration.ofSeconds(2)),
        )
        compose.setContent {
            SereaTheme { CalendarAgendaScreen(onBack = {}, provider = provider, clock = provider.clock, displayZone = ZoneId.of("UTC")) }
        }
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("予定を読み込み中").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("予定を読み込み中").assertIsDisplayed()
        compose.waitUntil(8_000) {
            compose.onAllNodesWithText("このカレンダーに予定はありません。").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("このカレンダーに予定はありません。").assertIsDisplayed()
    }

    @Test fun calendarFailureStateIsVisibleAndRetryable() {
        val denied = FakeCalendarProvider(options = FakeCalendarProvider.Options(failureOnPageIndex = 0))
        compose.setContent {
            SereaTheme { CalendarAgendaScreen(onBack = {}, provider = denied, clock = denied.clock, displayZone = ZoneId.of("UTC")) }
        }
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("予定をすべて読み込めませんでした。再試行してください。").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("予定をすべて読み込めませんでした。再試行してください。").assertIsDisplayed()
        compose.onNodeWithText("再試行").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Design review").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Design review").assertIsDisplayed()
    }

    private class RetryGmailProvider : GmailReadPort, GmailAuthorizationPort {
        private val fixture = FakeGmailProvider()
        var inboxCalls = 0
            private set

        override suspend fun status() = fixture.status()
        override suspend fun listInbox(pageToken: String?, pageSize: Int): GmailMessagePage {
            inboxCalls++
            if (inboxCalls == 1) throw GmailReadFailure(GmailReadFailureCode.UNAVAILABLE)
            return fixture.listInbox(pageToken, pageSize)
        }
        override suspend fun search(query: String, pageToken: String?, pageSize: Int) = fixture.search(query, pageToken, pageSize)
        override suspend fun getMessage(id: GmailMessageId) = fixture.getMessage(id)
        override suspend fun getThread(id: GmailThreadId) = fixture.getThread(id)
    }

    private class HeldGmailProvider : GmailReadPort, GmailAuthorizationPort {
        val readStarted = CompletableDeferred<Unit>()
        val page = CompletableDeferred<GmailMessagePage>()

        override suspend fun status() = GmailAuthorizationStatus(GmailAuthorizationState.MOCK_ONLY)
        override suspend fun listInbox(pageToken: String?, pageSize: Int): GmailMessagePage {
            readStarted.complete(Unit)
            return page.await()
        }
        override suspend fun search(query: String, pageToken: String?, pageSize: Int) = listInbox(pageToken, pageSize)
        override suspend fun getMessage(id: GmailMessageId): GmailMessage = error("Unused Gmail detail read")
        override suspend fun getThread(id: GmailThreadId): GmailThread = error("Unused Gmail thread read")
    }
}
