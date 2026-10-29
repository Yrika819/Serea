package app.serea.android.googleauth

import app.serea.android.calendar.CalendarDateRange
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.calendar.CalendarProviderSession
import app.serea.android.calendar.CalendarProviderSessions
import app.serea.android.calendar.CalendarReadFailure
import app.serea.android.calendar.CalendarMockWebServerTls
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import app.serea.android.calendar.EventTime
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.async
import kotlinx.coroutines.supervisorScope
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestDispatcher
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.withContext
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.SocketPolicy
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.Description
import org.junit.runners.model.Statement
import java.time.Instant
import java.util.concurrent.TimeUnit

@OptIn(ExperimentalCoroutinesApi::class)
class GoogleCalendarDemoLoopbackIntegrationTest {
    class MainDispatcherRule(
        val dispatcher: TestDispatcher = StandardTestDispatcher(),
    ) : org.junit.rules.TestRule {
        override fun apply(base: Statement, description: Description): Statement = object : Statement() {
            override fun evaluate() {
                Dispatchers.setMain(dispatcher)
                try {
                    base.evaluate()
                } finally {
                    Dispatchers.resetMain()
                }
            }
        }
    }

    @get:Rule val mainDispatcher = MainDispatcherRule()
    private val server = MockWebServer().apply { CalendarMockWebServerTls.start(this) }

    @After fun stopServer() = server.shutdown()

    private class SyntheticAuthorizedCalendarGateway(
        account: GoogleDemoAccount,
        accountGeneration: Int,
    ) : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        override val authorizationSessionId = "mock-${account.name.lowercase()}-$accountGeneration"
        private val grantedScopes = GoogleFeature.CALENDAR.scopes
        val requestedScopeSets = mutableListOf<Set<GoogleScope>>()
        var tokenRequests = 0
            private set

        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            requestedScopeSets += scopes.toSet()
            if (feature != GoogleFeature.CALENDAR || scopes.isEmpty() || !grantedScopes.containsAll(scopes)) {
                return TokenSupplyResult.Unavailable(AuthorizationFailure.UNAVAILABLE)
            }
            tokenRequests++
            val accountSession = authorizationSessionId
            return TokenSupplyResult.Available(
                AccessTokenLease(
                    token = "synthetic-loopback-token-${accountSession}",
                    grantedScopes = grantedScopes,
                    expiresAtElapsedRealtimeMillis = null,
                    authorizationSessionId = accountSession,
                    validityCheck = { authorizationSessionId == accountSession },
                ),
            )
        }

        override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
            service = GoogleServiceState.AVAILABLE,
            permissions = scopes.associateWith { scope ->
                if (scope in grantedScopes) ScopePermission.GRANTED else ScopePermission.NEEDS_CONSENT
            },
        )
    }

    private class LoopbackSessionFactory(private val server: MockWebServer) : CalendarDemoSessionFactory {
        private val sessions = CalendarProviderSessions(googleHttpEnabledByOwner = true)
        val opened = mutableListOf<Pair<Pair<GoogleDemoAccount, Int>, Pair<CalendarProviderSession, SyntheticAuthorizedCalendarGateway>>>()

        override fun openAfterSyntheticConsent(
            account: GoogleDemoAccount,
            accountGeneration: Int,
        ): CalendarProviderSession {
            val gateway = SyntheticAuthorizedCalendarGateway(account, accountGeneration)
            val session = sessions.openGoogleReadOnly(
                gateway = gateway,
                client = CalendarMockWebServerTls.client(server),
            )
            opened += (account to accountGeneration) to (session to gateway)
            return session
        }
    }

    @Test
    fun defaultProductDemoSessionIsFakeAndHasNoHttpDispatchCapability() = runTest(mainDispatcher.dispatcher) {
        val viewModel = GoogleAuthorizationDemoViewModel()
        viewModel.requestConsent(GoogleFeature.CALENDAR)
        runCurrent()
        assertEquals(GoogleFeature.CALENDAR, viewModel.state.value.pendingFeature)
        viewModel.grantPendingConsent(checkNotNull(viewModel.state.value.pendingConsentPreview))
        runCurrent()

        val session = checkNotNull(viewModel.calendarDemoSession.value)
        assertEquals(CalendarProviderKind.FAKE, session.kind)
        assertEquals(3, session.listCalendars().size)
        assertEquals(0, server.requestCount)
    }

    @Test
    fun viewModelStoreRetainsDemoSessionForRecreationAndClearsOnOwnerTeardown() =
        runTest(mainDispatcher.dispatcher) {
            var opens = 0
            val factory = CalendarDemoSessionFactory { _, _ ->
                opens++
                CalendarProviderSessions().openFake()
            }
            val activityStore = ViewModelStore()
            val firstProvider = viewModelProvider(activityStore, factory)
            val beforeRecreation = firstProvider[GoogleAuthorizationDemoViewModel::class.java]
            beforeRecreation.requestConsent(GoogleFeature.CALENDAR)
            runCurrent()
            beforeRecreation.grantPendingConsent(checkNotNull(beforeRecreation.state.value.pendingConsentPreview))
            runCurrent()
            val beforeRecreationSession = checkNotNull(beforeRecreation.calendarDemoSession.value)

            // Android retains the Activity ViewModelStore through a configuration recreation.
            val afterRecreation = viewModelProvider(activityStore, factory)[GoogleAuthorizationDemoViewModel::class.java]
            assertSame(beforeRecreation, afterRecreation)
            assertSame(beforeRecreationSession, afterRecreation.calendarDemoSession.value)
            assertEquals(1, opens)

            // Clearing that store models ViewModel owner teardown; it is not an OS process-death run.
            activityStore.clear()
            assertNull(beforeRecreation.calendarDemoSession.value)
            assertTrue(runCatching { beforeRecreationSession.listCalendars() }.exceptionOrNull() is CalendarReadFailure)

            val relaunchedProcessStore = ViewModelStore()
            val afterProcessDeath = viewModelProvider(relaunchedProcessStore, factory)[GoogleAuthorizationDemoViewModel::class.java]
            assertEquals(GoogleDemoAccount.ACCOUNT_A, afterProcessDeath.state.value.selectedAccount)
            assertEquals(GoogleDemoFeatureState.DISCONNECTED, afterProcessDeath.state.value.calendar)
            assertNull(afterProcessDeath.calendarDemoSession.value)
            assertEquals(1, opens)
            relaunchedProcessStore.clear()
        }

    @Test
    fun consentConfirmOpensLoopbackListAndDetailThenAccountSwitchAndRevokeSuppressOldSession() =
        runTest(mainDispatcher.dispatcher) {
            val factory = LoopbackSessionFactory(server)
            val viewModel = GoogleAuthorizationDemoViewModel(
                gateway = FakeGoogleAuthorizationGateway(),
                calendarSessionFactory = factory,
            )

            // The same ViewModel request/confirm actions exposed by the Settings panel drive the
            // factory; before explicit confirmation there is no session and no HTTP dispatch.
            viewModel.requestConsent(GoogleFeature.CALENDAR)
            runCurrent()
            assertEquals(GoogleDemoAccount.ACCOUNT_A, viewModel.state.value.selectedAccount)
            assertEquals(GoogleFeature.CALENDAR, viewModel.state.value.pendingFeature)
            assertEquals(GoogleFeature.CALENDAR.scopes, viewModel.state.value.pendingScopes)
            assertTrue(factory.opened.isEmpty())
            assertEquals(0, server.requestCount)

            viewModel.grantPendingConsent(checkNotNull(viewModel.state.value.pendingConsentPreview))
            runCurrent()
            assertEquals(1, factory.opened.size)
            assertEquals(GoogleDemoAccount.ACCOUNT_A to 1, factory.opened.single().first)
            val (accountASession, accountAGateway) = factory.opened.single().second
            assertEquals(CalendarProviderKind.GOOGLE_READ_ONLY, accountASession.kind)

            val calendarId = app.serea.android.calendar.CalendarId("primary-fixture")
            val range = CalendarDateRange(
                Instant.parse("2026-10-28T00:00:00Z"),
                Instant.parse("2026-10-29T00:00:00Z"),
            )
            server.enqueue(
                MockResponse().setBody(
                    """{"timeZone":"America/Los_Angeles","items":[{"id":"fixture-event-1","summary":"Synthetic review","status":"confirmed","start":{"dateTime":"2026-10-28T09:00:00-07:00","timeZone":"America/Los_Angeles"},"end":{"dateTime":"2026-10-28T09:30:00-07:00","timeZone":"America/Los_Angeles"}}]}""",
                ),
            )
            server.enqueue(
                MockResponse().setBody(
                    """{"id":"fixture-event-1","summary":"Synthetic review detail","description":"Test-only event detail","status":"confirmed","start":{"dateTime":"2026-10-28T09:00:00-07:00","timeZone":"America/Los_Angeles"},"end":{"dateTime":"2026-10-28T09:30:00-07:00","timeZone":"America/Los_Angeles"}}""",
                ),
            )

            val page = accountASession.listEventsPage(calendarId, range)
            assertEquals("fixture-event-1", page.events.single().key.eventId.value)
            assertEquals("Synthetic review", page.events.single().title)
            assertEquals("America/Los_Angeles", (page.events.single().time as EventTime.Timed).timeZone.id)
            val detail = accountASession.getEvent(page.events.single().key)
            assertEquals("Synthetic review detail", detail.title)
            assertEquals("Test-only event detail", detail.description)
            assertEquals(2, accountAGateway.tokenRequests)
            assertEquals(
                listOf(
                    setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY),
                    setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY),
                ),
                accountAGateway.requestedScopeSets,
            )
            repeat(2) {
                val request = server.takeRequest()
                assertEquals("GET", request.method)
                assertEquals("localhost", request.requestUrl!!.host)
                assertTrue(request.requestLine.contains("/calendar/v3/"))
                assertEquals("Bearer synthetic-loopback-token-mock-account_a-1", request.getHeader("Authorization"))
            }
            assertEquals(2, server.requestCount)

            // Explicit account selection closes the Account A session and blocks future dispatch.
            viewModel.selectDemoAccount(GoogleDemoAccount.ACCOUNT_B)
            assertEquals(GoogleDemoFeatureState.REVOKED, viewModel.state.value.calendar)
            assertNull(viewModel.calendarDemoSession.value)
            assertTrue(runCatching { accountASession.getEvent(page.events.single().key) }.exceptionOrNull() is CalendarReadFailure)
            assertEquals(2, server.requestCount)

            // Reauthorize B, begin a delayed loopback request, and revoke while it is pending.
            viewModel.requestConsent(GoogleFeature.CALENDAR)
            runCurrent()
            viewModel.grantPendingConsent(checkNotNull(viewModel.state.value.pendingConsentPreview))
            runCurrent()
            assertEquals(2, factory.opened.size)
            val (accountBSession, _) = factory.opened.last().second
            assertEquals(GoogleDemoAccount.ACCOUNT_B to 2, factory.opened.last().first)
            server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE))
            supervisorScope {
                // Supervise the expected failed read so its CalendarReadFailure cannot cancel the
                // runTest parent before await() observes it. Undispatched startup ensures the
                // session enters its read path before we wait for MockWebServer's request record.
                val pendingRead = async(Dispatchers.IO, start = CoroutineStart.UNDISPATCHED) {
                    accountBSession.listEventsPage(calendarId, range)
                }
                val delayedRequest = withContext(Dispatchers.IO) { server.takeRequest(5, TimeUnit.SECONDS) }
                assertNotNull(delayedRequest)
                assertEquals("GET", delayedRequest!!.method)
                assertEquals("localhost", delayedRequest.requestUrl!!.host)

                // Revoke only after MockWebServer has parsed the pending request, then observe the
                // expected fail-closed result before leaving the supervised scope.
                viewModel.revokeDemoGrant()
                assertNull(viewModel.calendarDemoSession.value)
                val delayedResult = runCatching { pendingRead.await() }
                assertFalse(delayedResult.isSuccess)
                assertTrue(delayedResult.exceptionOrNull() is CalendarReadFailure)
                assertEquals(3, server.requestCount)
            }
        }

    private fun viewModelProvider(
        store: ViewModelStore,
        calendarFactory: CalendarDemoSessionFactory,
    ) = ViewModelProvider(store, object : ViewModelProvider.Factory {
        override fun <T : ViewModel> create(modelClass: Class<T>): T {
            require(modelClass == GoogleAuthorizationDemoViewModel::class.java)
            @Suppress("UNCHECKED_CAST")
            return GoogleAuthorizationDemoViewModel(
                gateway = FakeGoogleAuthorizationGateway(),
                calendarSessionFactory = calendarFactory,
            ) as T
        }
    })
}
