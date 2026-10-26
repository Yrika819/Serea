package app.serea.android.calendar

import app.serea.android.googleauth.AccessTokenLease
import app.serea.android.googleauth.AuthorizationFailure
import app.serea.android.googleauth.AuthorizationStatus
import app.serea.android.googleauth.FakeGoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleFeature
import app.serea.android.googleauth.GoogleScope
import app.serea.android.googleauth.GoogleServiceState
import app.serea.android.googleauth.ScopePermission
import app.serea.android.googleauth.TokenSupplyResult
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CalendarProviderSessionsTest {
    private val server = MockWebServer().apply { start() }

    @After fun close() = server.shutdown()

    private class PlayServicesFixture(
        private val granted: Set<GoogleScope> = setOf(
            GoogleScope.CALENDAR_EVENTS_READ_ONLY,
            GoogleScope.CALENDAR_LIST_READ_ONLY,
        ),
        private val permission: ScopePermission = ScopePermission.GRANTED,
    ) : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        override var authorizationSessionId: String? = "calendar-session-fixture"
        var tokenRequests = 0
        var rotateAfterNextToken = false
        private var accountChanged = false
        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            if (feature != GoogleFeature.CALENDAR || !granted.containsAll(scopes)) {
                return TokenSupplyResult.UserActionRequired(scopes - granted)
            }
            tokenRequests++
            val boundSession = authorizationSessionId
            val lease = AccessTokenLease("fixture-never-valid", granted, null,
                authorizationSessionId = boundSession)
            if (rotateAfterNextToken) {
                rotateAfterNextToken = false
                authorizationSessionId = "calendar-session-second"
                accountChanged = true
            }
            return TokenSupplyResult.Available(lease)
        }
        override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
            GoogleServiceState.AVAILABLE,
            scopes.associateWith { scope -> if (scope in granted) permission else ScopePermission.NEEDS_CONSENT },
            when {
                accountChanged -> AuthorizationFailure.ACCOUNT_CHANGED
                permission == ScopePermission.DENIED -> AuthorizationFailure.DENIED
                else -> null
            },
        )
    }

    @Test fun defaultAndMockAuthorizationStayOnFakeProvider() = runBlocking {
        val disabled = CalendarProviderSessions(googleHttpEnabledByOwner = false)
        val default = disabled.openDefault()
        assertEquals(CalendarProviderKind.FAKE, default.kind)
        assertEquals(CalendarAuthorizationState.MOCK_ONLY, default.status().state)

        val mockGateway = FakeGoogleAuthorizationGateway(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY))
        val alsoFake = CalendarProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(mockGateway, baseUrl = server.url("/calendar/v3/"))
        assertEquals(CalendarProviderKind.FAKE, alsoFake.kind)
        assertEquals(0, server.requestCount)
        assertTrue(alsoFake.listCalendars().isNotEmpty())
    }

    @Test fun ownerEnabledPlayServicesSessionUsesOnlyMockServerAndRevokesOldSession() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"items":[{"id":"fixture@example.test","timeZone":"UTC","accessRole":"reader"}]}"""))
        val sessions = CalendarProviderSessions(googleHttpEnabledByOwner = true)
        val first = sessions.openGoogleReadOnly(
            PlayServicesFixture(),
            OkHttpClient(),
            server.url("/calendar/v3/"),
        )
        assertEquals(CalendarProviderKind.GOOGLE_READ_ONLY, first.kind)
        assertEquals("fixture@example.test", first.listCalendars().single().id.value)
        assertEquals("Bearer fixture-never-valid", server.takeRequest().getHeader("Authorization"))

        val replacement = sessions.openFake()
        assertEquals(CalendarProviderKind.FAKE, replacement.kind)
        val failure = runCatching { first.listCalendars() }.exceptionOrNull()
        assertTrue(failure is CalendarReadFailure)
        assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, (failure as CalendarReadFailure).code)
        assertEquals(1, server.requestCount)
        assertFalse(replacement.status().state == CalendarAuthorizationState.AUTHORIZED)
    }

    @Test fun deniedAndExpiredAuthorizationDoNotReachTheHttpServer() = runBlocking {
        val sessions = CalendarProviderSessions(googleHttpEnabledByOwner = true)
        for (gateway in listOf(
            PlayServicesFixture(permission = ScopePermission.DENIED),
            object : GoogleAuthorizationGateway {
                override val mode = GoogleAuthorizationMode.PLAY_SERVICES
                override val authorizationSessionId = "expired-calendar-session-fixture"
                override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>) =
                    TokenSupplyResult.Unavailable(AuthorizationFailure.EXPIRED)
                override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                    GoogleServiceState.AVAILABLE,
                    scopes.associateWith { ScopePermission.EXPIRED },
                    AuthorizationFailure.EXPIRED,
                )
            },
        )) {
            val session = sessions.openGoogleReadOnly(gateway, baseUrl = server.url("/calendar/v3/"))
            try {
                session.getEvent(CalendarEventKey(CalendarId("fixture@example.test"), EventId("fixture")))
                throw AssertionError("authorization must fail closed")
            } catch (failure: CalendarReadFailure) {
                assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, failure.code)
            }
            assertEquals(0, server.requestCount)
        }
    }

    @Test fun accountSwitchDuringTokenLeaseAcquisitionCannotDispatchACalendarRequest() = runBlocking {
        val gateway = PlayServicesFixture().apply { rotateAfterNextToken = true }
        val session = CalendarProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(gateway, baseUrl = server.url("/calendar/v3/"))

        val failure = runCatching { session.listCalendars() }.exceptionOrNull()
        assertTrue(failure is CalendarReadFailure)
        assertEquals(CalendarAuthorizationState.REVOKED, session.status().state)
        assertEquals(1, gateway.tokenRequests)
        assertEquals(0, server.requestCount)
    }

    @Test fun replacingSessionImmediatelyAfterLeaseAcquisitionPreventsCalendarHttpDispatch() = runBlocking {
        lateinit var sessions: CalendarProviderSessions
        var replacement: CalendarProviderSession? = null
        val gateway = object : GoogleAuthorizationGateway {
            override val mode = GoogleAuthorizationMode.PLAY_SERVICES
            override val authorizationSessionId = "calendar-session-bound-to-first-account"
            override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { ScopePermission.GRANTED },
            )
            override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
                val lease = AccessTokenLease(
                    "fixture-calendar-token-before-replacement",
                    scopes,
                    null,
                    authorizationSessionId = authorizationSessionId,
                    validityCheck = {
                        replacement = sessions.openFake()
                        true
                    },
                )
                return TokenSupplyResult.Available(lease)
            }
        }
        sessions = CalendarProviderSessions(googleHttpEnabledByOwner = true)
        val oldSession = sessions.openGoogleReadOnly(gateway, baseUrl = server.url("/calendar/v3/"))

        val failure = runCatching { oldSession.getCalendar(CalendarId("fixture@example.test")) }.exceptionOrNull()
        assertTrue(failure is CalendarReadFailure)
        assertTrue(currentCoroutineContext().isActive)
        assertEquals(CalendarProviderKind.FAKE, replacement?.kind)
        assertEquals(0, server.requestCount)
    }

    @Test fun unknownAccountBindingIsNotAuthorizedAndCannotReachCalendarHttp() = runBlocking {
        var tokenRequests = 0
        val unboundGateway = object : GoogleAuthorizationGateway {
            override val mode = GoogleAuthorizationMode.PLAY_SERVICES
            override val authorizationSessionId: String? = null
            override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
                tokenRequests++
                return TokenSupplyResult.Available(AccessTokenLease("must-not-be-used", scopes, null))
            }
            override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { ScopePermission.GRANTED },
            )
        }
        val session = CalendarProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(unboundGateway, baseUrl = server.url("/calendar/v3/"))

        assertEquals(CalendarAuthorizationState.NOT_CONNECTED, session.status().state)
        val failure = runCatching {
            session.listEventsPage(CalendarId("fixture@example.test"), range())
        }.exceptionOrNull()
        assertTrue(failure is CalendarReadFailure)
        assertEquals(0, tokenRequests)
        assertEquals(0, server.requestCount)
    }

    @Test fun replacingSessionCancelsItsReadWithoutCancellingParentOrSiblingCommunication() = runBlocking {
        val started = Channel<Unit>(Channel.UNLIMITED)
        val pendingResponse = CompletableDeferred<List<CalendarDescriptor>>()
        val delegate = object : CalendarReadPort {
            override suspend fun listCalendars(): List<CalendarDescriptor> {
                started.send(Unit)
                return pendingResponse.await()
            }
            override suspend fun getCalendar(calendarId: CalendarId): CalendarDescriptor = error("not used")
            override suspend fun getEvent(key: CalendarEventKey): CalendarEvent = error("not used")
            override suspend fun listEventsPage(
                calendarId: CalendarId,
                range: CalendarDateRange,
                pageToken: String?,
            ): CalendarEventPage = error("not used")
        }
        val session = CalendarProviderSession(
            id = 8,
            kind = CalendarProviderKind.FAKE,
            delegate = delegate,
            authorizationDelegate = FakeCalendarProvider(),
            isCurrent = { true },
            onClose = {},
        )
        val unrelatedResult = CompletableDeferred<String>()
        val caller = launch {
            coroutineScope {
                val unrelated = async { unrelatedResult.await() }
                val read = async { calendarFailure { session.listCalendars() }.code }

                started.receive()
                session.invalidateFromController()

                assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, read.await())
                assertTrue(currentCoroutineContext().isActive)
                assertTrue("unrelated sibling communication must remain active", unrelated.isActive)
                unrelatedResult.complete("unrelated communication survived")
                assertEquals("unrelated communication survived", unrelated.await())
            }
        }

        caller.join()
        assertTrue("provider replacement must not cancel the caller job", caller.isCompleted && !caller.isCancelled)
        assertEquals(0, server.requestCount)
    }

    private suspend fun calendarFailure(block: suspend () -> Any): CalendarReadFailure = try {
        block()
        throw AssertionError("Expected CalendarReadFailure")
    } catch (failure: CalendarReadFailure) {
        failure
    }

    private fun range() = CalendarDateRange(
        java.time.Instant.parse("2026-10-20T00:00:00Z"),
        java.time.Instant.parse("2026-10-21T00:00:00Z"),
    )
}
