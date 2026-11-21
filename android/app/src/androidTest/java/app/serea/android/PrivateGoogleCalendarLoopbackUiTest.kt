package app.serea.android

import android.security.NetworkSecurityPolicy
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.runtime.mutableStateOf
import androidx.compose.ui.Modifier
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToNode
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.calendar.CalendarProviderSession
import app.serea.android.calendar.ui.CalendarAgendaLoadedEventCountKey
import app.serea.android.googleauth.AccessTokenLease
import app.serea.android.googleauth.AuthorizationFailure
import app.serea.android.googleauth.AuthorizationStatus
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleFeature
import app.serea.android.googleauth.GoogleScope
import app.serea.android.googleauth.GoogleServiceState
import app.serea.android.googleauth.PrivateGoogleAuthorizationViewModel
import app.serea.android.googleauth.PrivateGoogleAuthStatus
import app.serea.android.googleauth.IsolatedGoogleCalendarTestComposition
import app.serea.android.googleauth.ScopePermission
import app.serea.android.googleauth.TokenSupplyResult
import java.time.Clock
import java.time.Instant
import java.time.ZoneId
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.runBlocking
import okhttp3.Call
import okhttp3.EventListener
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.SocketPolicy
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PrivateGoogleCalendarLoopbackUiTest {
    @get:Rule val compose = createComposeRule()
    private val server = MockWebServer().apply { start() }
    private val openedSession = mutableStateOf<CalendarProviderSession?>(null)
    private var viewModel: PrivateGoogleAuthorizationViewModel? = null

    @After fun close() {
        viewModel?.disconnectCalendarInApp()
        server.shutdown()
    }

    private class SyntheticPrimaryGateway : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        override val authorizationSessionId = "synthetic-private-calendar-session"
        val statusScopeRequests = mutableListOf<Set<GoogleScope>>()
        val tokenScopeRequests = mutableListOf<Set<GoogleScope>>()
        private var enabled = true

        override fun isEnabledInApp(feature: GoogleFeature): Boolean =
            enabled && feature == GoogleFeature.CALENDAR

        override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
            statusScopeRequests += scopes.toSet()
            return AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { scope ->
                    if (scope == GoogleScope.CALENDAR_EVENTS_READ_ONLY) ScopePermission.GRANTED
                    else ScopePermission.DENIED
                },
            )
        }

        override suspend fun acquire(
            feature: GoogleFeature,
            scopes: Set<GoogleScope>,
        ): TokenSupplyResult {
            tokenScopeRequests += scopes.toSet()
            if (!isEnabledInApp(feature) || feature != GoogleFeature.CALENDAR ||
                scopes != setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)
            ) {
                return TokenSupplyResult.Unavailable(AuthorizationFailure.SCOPE_MISMATCH)
            }
            return TokenSupplyResult.Available(
                AccessTokenLease(
                    token = "synth-token",
                    grantedScopes = scopes,
                    expiresAtElapsedRealtimeMillis = null,
                    authorizationSessionId = authorizationSessionId,
                    validityCheck = { enabled },
                ),
            )
        }
    }

    @Test
    fun privateAuthorizationUiReadsPagedPrimaryEventsDetailsRefreshAndClosesSession() {
        val networkPolicy = NetworkSecurityPolicy.getInstance()
        assertTrue(networkPolicy.isCleartextTrafficPermitted("localhost"))
        assertTrue(networkPolicy.isCleartextTrafficPermitted("127.0.0.1"))
        assertTrue(networkPolicy.isCleartextTrafficPermitted("::1"))
        assertFalse(networkPolicy.isCleartextTrafficPermitted("example.com"))
        assertTrue(server.url("/").isLoopbackHost())

        val gateway = SyntheticPrimaryGateway()
        val vm = PrivateGoogleAuthorizationViewModel(
            context = null,
            enabledInThisVariant = true,
            realOAuthEnabled = false,
            calendarGatewayForTests = gateway,
            calendarCompositionFactoryForTests = {
                IsolatedGoogleCalendarTestComposition(
                    client = OkHttpClient(),
                    baseUrl = server.url("/calendar/v3/"),
                )
            },
        )
        viewModel = vm
        server.enqueue(jsonResponse(
            """{"timeZone":"America/New_York","items":[{"id":"synthetic-timed","summary":"Synthetic timed event","start":{"dateTime":"2026-11-18T14:00:00-05:00","timeZone":"America/New_York"},"end":{"dateTime":"2026-11-18T14:30:00-05:00","timeZone":"America/New_York"}},{"id":"synthetic-all-day","summary":"Synthetic all-day event","start":{"date":"2026-11-18"},"end":{"date":"2026-11-19"}}],"nextPageToken":"page-2"}""",
        ).setBodyDelay(1, TimeUnit.SECONDS))
        server.enqueue(jsonResponse(
            """{"timeZone":"America/New_York","items":[{"id":"synthetic-detail","summary":"Synthetic detail event","description":"Synthetic detail text","start":{"dateTime":"2026-11-18T16:00:00-05:00","timeZone":"America/New_York"},"end":{"dateTime":"2026-11-18T16:30:00-05:00","timeZone":"America/New_York"}}]}""",
        ))
        server.enqueue(jsonResponse(
            """{"id":"synthetic-timed","summary":"Synthetic timed event","description":"Synthetic detail text","start":{"dateTime":"2026-11-18T14:00:00-05:00","timeZone":"America/New_York"},"end":{"dateTime":"2026-11-18T14:30:00-05:00","timeZone":"America/New_York"}}""",
        ))
        server.enqueue(jsonResponse("""{"timeZone":"America/New_York","items":[]}"""))
        renderPrivatePanel(vm)

        authorizeMockAccountFromUi()
        assertEquals(0, server.requestCount)
        compose.onNodeWithText("Google CalendarのPrimary予定を表示").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("予定を読み込み中").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Google Calendar · 読み取り専用").assertIsDisplayed()
        compose.onNodeWithText("Google Calendarから取得 · 変更はできません").assertIsDisplayed()
        assertTrue(compose.onAllNodesWithText("DEMO / 架空の予定").fetchSemanticsNodes().isEmpty())
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().singleOrNull()
                ?.config?.getOrElseNullable(CalendarAgendaLoadedEventCountKey) { 0 } == 3
        }
        assertEquals(CalendarProviderKind.GOOGLE_READ_ONLY, vm.calendarSession.value?.kind)
        assertTrue(vm.calendarSession.value?.primaryCalendarOnly == true)
        compose.onNodeWithText("Google Calendarから取得 · 変更はできません").assertIsDisplayed()
        compose.onNodeWithTag("calendar_agenda_list").performScrollToNode(hasText("Synthetic all-day event"))
        compose.onNodeWithText("Synthetic all-day event").assertIsDisplayed()
        compose.onNodeWithText("終日", substring = true).assertIsDisplayed()

        compose.onNodeWithTag("calendar_agenda_list").performScrollToNode(hasText("Synthetic timed event"))
        compose.onNodeWithText("Synthetic timed event").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_event_detail_list").fetchSemanticsNodes().isNotEmpty() &&
                server.requestCount == 3
        }
        compose.onNodeWithTag("calendar_event_detail_item_4").performScrollTo()
        compose.onAllNodesWithText("America/New_York", substring = true).assertCountEquals(2)
        compose.onNodeWithText("表示タイムゾーン", substring = true).assertIsDisplayed()
        compose.onNodeWithText("2026年11月18日(水) 14:00 -05:00", substring = true).assertIsDisplayed()

        compose.onNodeWithText("戻る").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("calendar_agenda_refresh").performClick()
        compose.waitUntil(10_000) {
            server.requestCount == 4 &&
                compose.onAllNodesWithText("このカレンダーに予定はありません。").fetchSemanticsNodes().isNotEmpty()
        }
        assertTrue(compose.onAllNodesWithText("Synthetic timed event").fetchSemanticsNodes().isEmpty())

        val session = checkNotNull(vm.calendarSession.value)
        compose.onNodeWithText("戻る").performClick()
        compose.onNodeWithTag("private_google_disconnect_calendar").performScrollTo().performClick()
        compose.waitUntil(5_000) { vm.calendarSession.value == null }
        assertTrue(runCatching { runBlocking { session.status() } }.isFailure)
        assertEquals(4, server.requestCount)

        val requests = (0 until 4).map {
            checkNotNull(server.takeRequest(5, TimeUnit.SECONDS))
        }
        assertEquals(listOf("GET", "GET", "GET", "GET"), requests.map { it.method })
        assertEquals(
            listOf(
                "/calendar/v3/calendars/primary/events",
                "/calendar/v3/calendars/primary/events",
                "/calendar/v3/calendars/primary/events/synthetic-timed",
                "/calendar/v3/calendars/primary/events",
            ),
            requests.map { it.requestUrl!!.encodedPath },
        )
        assertEquals(listOf(null, "page-2", null, null), requests.map { it.requestUrl!!.queryParameter("pageToken") })
        requests.forEach { request ->
            assertEquals("Bearer synth-token", request.getHeader("Authorization"))
            assertFalse(request.requestLine.contains("calendarList", ignoreCase = true))
            assertFalse(request.requestLine.contains("gmail", ignoreCase = true))
            assertTrue(request.requestUrl!!.host in setOf("localhost", "127.0.0.1", "::1"))
            assertTrue(request.requestUrl!!.isLoopbackHost())
        }
        assertTrue(gateway.statusScopeRequests.isNotEmpty())
        assertTrue(gateway.statusScopeRequests.all { it == setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY) })
        assertEquals(4, gateway.tokenScopeRequests.size)
        assertTrue(gateway.tokenScopeRequests.all { it == setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY) })
    }

    @Test
    fun selectedLiveTransportFailureStaysAnErrorWithoutFakeFallback() {
        val gateway = SyntheticPrimaryGateway()
        val vm = liveViewModel(gateway)
        viewModel = vm
        server.enqueue(jsonResponse("""{"error":{"message":"synthetic unavailable"}}""").setResponseCode(503))
        renderPrivatePanel(vm)

        authorizeMockAccountFromUi()
        compose.onNodeWithText("Google CalendarのPrimary予定を表示").performScrollTo().performClick()
        compose.waitUntil(10_000) {
            server.requestCount == 1 &&
                compose.onAllNodesWithText("予定を読み込めませんでした。再試行してください。")
                    .fetchSemanticsNodes().isNotEmpty()
        }

        assertEquals(CalendarProviderKind.GOOGLE_READ_ONLY, vm.calendarSession.value?.kind)
        compose.onNodeWithText("Google Calendarから取得 · 変更はできません").assertIsDisplayed()
        assertTrue(compose.onAllNodesWithText("DEMO / 架空の予定").fetchSemanticsNodes().isEmpty())
        assertTrue(compose.onAllNodesWithText("このカレンダーに予定はありません。").fetchSemanticsNodes().isEmpty())
        assertEquals(
            "/calendar/v3/calendars/primary/events",
            checkNotNull(server.takeRequest(5, TimeUnit.SECONDS)).requestUrl!!.encodedPath,
        )
        assertEquals(listOf(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)), gateway.tokenScopeRequests)
        vm.disconnectCalendarInApp()
        assertTrue(vm.calendarSession.value == null)
    }

    @Test
    fun leavingDuringLiveReadCancelsTheHttpCallWithoutShowingFakeSuccess() {
        val cancellationObserved = CountDownLatch(1)
        val client = OkHttpClient.Builder().eventListenerFactory {
            object : EventListener() {
                override fun canceled(call: Call) { cancellationObserved.countDown() }
            }
        }.build()
        val gateway = SyntheticPrimaryGateway()
        val vm = liveViewModel(gateway, client)
        viewModel = vm
        server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE))
        renderPrivatePanel(vm)

        authorizeMockAccountFromUi()
        compose.onNodeWithText("Google CalendarのPrimary予定を表示").performScrollTo().performClick()
        compose.waitUntil(10_000) {
            server.requestCount == 1 &&
                compose.onAllNodesWithText("予定を読み込み中").fetchSemanticsNodes().isNotEmpty()
        }
        assertEquals(
            "/calendar/v3/calendars/primary/events",
            checkNotNull(server.takeRequest(5, TimeUnit.SECONDS)).requestUrl!!.encodedPath,
        )

        compose.onNodeWithText("戻る").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("private_google_auth_section").fetchSemanticsNodes().isNotEmpty()
        }
        assertTrue(cancellationObserved.await(5, TimeUnit.SECONDS))
        assertTrue(compose.onAllNodesWithText("DEMO / 架空の予定").fetchSemanticsNodes().isEmpty())
        assertEquals(1, server.requestCount)
        assertEquals(listOf(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)), gateway.tokenScopeRequests)
        vm.disconnectCalendarInApp()
        assertTrue(vm.calendarSession.value == null)
    }

    @Test
    fun productDefaultCompositionKeepsPrivateUiAndAgendaExplicitlyFictional() {
        val vm = PrivateGoogleAuthorizationViewModel(
            context = null,
            enabledInThisVariant = true,
            realOAuthEnabled = false,
        )
        viewModel = vm
        renderPrivatePanel(vm)

        authorizeMockAccountFromUi()
        compose.onNodeWithText("架空のPrimary予定を表示").performScrollTo().performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
        }

        assertEquals(CalendarProviderKind.FAKE, vm.calendarSession.value?.kind)
        compose.onNodeWithText("DEMO / 架空の予定").assertIsDisplayed()
        compose.onNodeWithText("デモのみ · Google Calendar未接続").assertIsDisplayed()
        assertEquals(0, server.requestCount)

        val session = checkNotNull(vm.calendarSession.value)
        compose.onNodeWithText("戻る").performClick()
        compose.onNodeWithTag("private_google_disconnect_calendar").performScrollTo().performClick()
        compose.waitUntil(5_000) { vm.calendarSession.value == null }
        assertTrue(runCatching { runBlocking { session.status() } }.isFailure)
        assertEquals(0, server.requestCount)
    }

    private fun liveViewModel(
        gateway: SyntheticPrimaryGateway,
        client: OkHttpClient = OkHttpClient(),
    ) = PrivateGoogleAuthorizationViewModel(
        context = null,
        enabledInThisVariant = true,
        realOAuthEnabled = false,
        calendarGatewayForTests = gateway,
        calendarCompositionFactoryForTests = {
            IsolatedGoogleCalendarTestComposition(
                client = client,
                baseUrl = server.url("/calendar/v3/"),
            )
        },
    )

    private fun renderPrivatePanel(vm: PrivateGoogleAuthorizationViewModel) {
        compose.setContent {
            SereaTheme {
                val session = openedSession.value
                if (session == null) {
                    Column(Modifier.fillMaxSize().verticalScroll(rememberScrollState())) {
                        app.serea.android.googleauth.PrivateGoogleAuthorizationPanel(
                            viewModel = vm,
                            onOpenCalendar = { openedSession.value = it },
                        )
                    }
                } else {
                    app.serea.android.calendar.ui.CalendarAgendaScreen(
                        onBack = { openedSession.value = null },
                        provider = session,
                        authorizationPort = session,
                        clock = Clock.fixed(Instant.parse("2026-11-18T12:00:00Z"), ZoneId.of("America/New_York")),
                        displayZone = ZoneId.of("America/New_York"),
                        primaryCalendarOnly = session.primaryCalendarOnly,
                    )
                }
            }
        }
    }

    private fun authorizeMockAccountFromUi() {
        compose.onNodeWithTag("private_google_synthetic_account_a").performScrollTo().performClick()
        compose.onNodeWithTag("private_google_request_calendar_consent").performScrollTo().performClick()
        compose.onNodeWithText("Calendar認可のMock確認").assertIsDisplayed()
        compose.onNodeWithTag("private_google_mock_consent_approve").performClick()
        val vm = checkNotNull(viewModel)
        compose.waitUntil(5_000) { vm.state.value.status == PrivateGoogleAuthStatus.AUTHORIZED }
    }

    private fun jsonResponse(body: String) = MockResponse()
        .setHeader("Content-Type", "application/json; charset=utf-8")
        .setBody(body)

    private fun okhttp3.HttpUrl.isLoopbackHost(): Boolean = host in setOf("localhost", "127.0.0.1", "::1")
}
