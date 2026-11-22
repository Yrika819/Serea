package app.serea.android

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.calendar.CalendarEvent
import app.serea.android.calendar.CalendarEventKey
import app.serea.android.calendar.CalendarId
import app.serea.android.calendar.CalendarApiFailureCode
import app.serea.android.calendar.CalendarAuthorizationPort
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.calendar.CalendarAuthorizationStatus
import app.serea.android.calendar.CalendarDescriptor
import app.serea.android.calendar.CalendarEventSource
import app.serea.android.calendar.CalendarProviderSessions
import app.serea.android.calendar.CalendarReadFailure
import app.serea.android.calendar.CalendarReadPort
import app.serea.android.calendar.CalendarReadPermission
import app.serea.android.calendar.CalendarVisibility
import app.serea.android.calendar.EventId
import app.serea.android.calendar.EventStatus
import app.serea.android.calendar.EventTime
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
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.time.Clock
import java.time.Duration
import java.time.Instant
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

    @Test fun replacingCalendarSessionReplacesAgendaWithoutOldSessionEvents() {
        val firstSessions = CalendarProviderSessions(fakeProvider = { sessionProvider("Session A event") })
        val secondSessions = CalendarProviderSessions(fakeProvider = { sessionProvider("Session B event") })
        val first = firstSessions.openPrimaryFake()
        val second = secondSessions.openPrimaryFake()
        var selected by mutableStateOf(first)
        val clock = Clock.fixed(Instant.parse("2026-10-20T12:00:00Z"), ZoneId.of("UTC"))
        compose.setContent {
            SereaTheme {
                CalendarAgendaScreen(
                    onBack = {}, provider = selected, authorizationPort = selected,
                    clock = clock, displayZone = ZoneId.of("UTC"),
                )
            }
        }
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Session A event").fetchSemanticsNodes().isNotEmpty()
        }
        selected = second
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Session B event").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Session B event").assertIsDisplayed()
        assertTrue(compose.onAllNodesWithText("Session A event").fetchSemanticsNodes().isEmpty())
        compose.setContent { }
        first.close()
        second.close()
    }

    @Test fun authorizedProviderFailureIsNotReplacedByDemoAgenda() {
        val provider = AuthorizedFailureCalendarProvider()
        compose.setContent {
            SereaTheme {
                CalendarAgendaScreen(onBack = {}, provider = provider, authorizationPort = provider, clock = provider.fixture.clock)
            }
        }
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Google Calendar予定取得失敗 · Demoへの代替なし").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Google Calendar予定取得失敗 · Demoへの代替なし").assertIsDisplayed()
        assertTrue(compose.onAllNodesWithText("DEMO / 架空の予定").fetchSemanticsNodes().isEmpty())
    }

    @Test fun calendarSettingsActionIsVisibleAndInvoked() {
        var opened = false
        val provider = FakeCalendarProvider()
        compose.setContent {
            SereaTheme {
                CalendarAgendaScreen(
                    onBack = {}, provider = provider, clock = provider.clock,
                    displayZone = ZoneId.of("UTC"), onOpenGoogleSettings = { opened = true },
                )
            }
        }
        compose.onNodeWithTag("calendar_google_auth_settings").assertIsDisplayed().performClick()
        assertTrue(opened)
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

    private class AuthorizedFailureCalendarProvider(
        val fixture: FakeCalendarProvider = FakeCalendarProvider(),
    ) : CalendarReadPort by fixture, CalendarAuthorizationPort {
        override suspend fun status() = CalendarAuthorizationStatus(CalendarAuthorizationState.AUTHORIZED)
        override suspend fun status(requiredScopes: Set<String>) = status()
        override suspend fun listCalendars(): List<CalendarDescriptor> =
            throw CalendarReadFailure(CalendarApiFailureCode.UNAVAILABLE, "Synthetic provider failure.")
    }

    private fun sessionProvider(title: String): FakeCalendarProvider {
        val primaryId = CalendarId("primary")
        val zone = ZoneId.of("UTC")
        val calendar = CalendarDescriptor(
            id = primaryId,
            displayName = "Private Calendar",
            timeZone = zone,
            visibility = CalendarVisibility.PRIVATE,
            readPermission = CalendarReadPermission.MOCK_ONLY,
        )
        val event = CalendarEvent(
            key = CalendarEventKey(primaryId, EventId(title.replace(' ', '-'))),
            title = title,
            description = null,
            location = null,
            time = EventTime.Timed(
                Instant.parse("2026-10-20T10:00:00Z"),
                Instant.parse("2026-10-20T10:30:00Z"),
                zone,
            ),
            status = EventStatus.CONFIRMED,
            source = CalendarEventSource.MOCK,
        )
        return FakeCalendarProvider(calendars = listOf(calendar), events = listOf(event))
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
