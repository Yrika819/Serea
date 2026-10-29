package app.serea.android.calendar

import app.serea.android.googleauth.AccessTokenLease
import app.serea.android.googleauth.AuthorizationFailure
import app.serea.android.googleauth.AuthorizationStatus
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleFeature
import app.serea.android.googleauth.GoogleScope
import app.serea.android.googleauth.GoogleServiceState
import app.serea.android.googleauth.ScopePermission
import app.serea.android.googleauth.TokenSupplyResult
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.time.Instant

/**
 * Calendar-first OAuth-to-read contract using synthetic authorization and loopback HTTP only.
 * This test does not instantiate Play Services or authorize a real Google account.
 */
class CalendarOAuthMockE2ETest {
    private val server = MockWebServer().apply { start() }

    @After fun close() = server.shutdown()

    private class SyntheticCalendarAuthorization : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        override var authorizationSessionId: String? = "synthetic-account-a-session-1"

        private val grantedScopes = linkedSetOf<GoogleScope>()
        val requestedScopeSets = mutableListOf<Set<GoogleScope>>()
        var tokenRequests = 0
            private set

        /** Represents a local test-fixture user action; it never calls Google's consent UI. */
        fun approveSyntheticCalendarEventsScope() {
            grantedScopes += GoogleScope.CALENDAR_EVENTS_READ_ONLY
        }

        fun switchSyntheticAccount() {
            authorizationSessionId = "synthetic-account-b-session-1"
            grantedScopes.clear()
        }

        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            requestedScopeSets += scopes.toSet()
            if (feature != GoogleFeature.CALENDAR || scopes != setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)) {
                return TokenSupplyResult.Unavailable(AuthorizationFailure.UNAVAILABLE)
            }
            val accountSession = authorizationSessionId
                ?: return TokenSupplyResult.Unavailable(AuthorizationFailure.ACCOUNT_REQUIRED)
            if (!grantedScopes.containsAll(scopes)) {
                return TokenSupplyResult.UserActionRequired(scopes - grantedScopes)
            }

            tokenRequests++
            return TokenSupplyResult.Available(
                AccessTokenLease(
                    token = "synthetic-never-valid-token",
                    grantedScopes = grantedScopes.toSet(),
                    expiresAtElapsedRealtimeMillis = null,
                    authorizationSessionId = accountSession,
                    validityCheck = { authorizationSessionId == accountSession && grantedScopes.containsAll(scopes) },
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

    @Test fun syntheticConsentAllowsOnlyCalendarReadAndAccountSwitchStopsFurtherDispatch() = runBlocking {
        val gateway = SyntheticCalendarAuthorization()
        val session = CalendarProviderSessions(googleHttpEnabledByOwner = true).openGoogleReadOnly(
            gateway = gateway,
            client = OkHttpClient(),
            baseUrl = server.url("/calendar/v3/"),
        )
        val calendarId = CalendarId("primary-fixture")
        val range = CalendarDateRange(
            Instant.parse("2026-10-28T00:00:00Z"),
            Instant.parse("2026-10-29T00:00:00Z"),
        )

        assertEquals(CalendarProviderKind.GOOGLE_READ_ONLY, session.kind)
        assertEquals(CalendarAuthorizationState.AUTHORIZATION_REQUIRED, session.status().state)
        val deniedBeforeConsent = runCatching { session.listEventsPage(calendarId, range) }.exceptionOrNull()
        assertTrue(deniedBeforeConsent is CalendarReadFailure)
        assertEquals(0, gateway.tokenRequests)
        assertEquals(0, server.requestCount)

        gateway.approveSyntheticCalendarEventsScope()
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

        val page = session.listEventsPage(calendarId, range)
        assertEquals("fixture-event-1", page.events.single().key.eventId.value)
        assertEquals("Synthetic review", page.events.single().title)
        assertEquals(CalendarEventSource.GOOGLE_CALENDAR_API, page.events.single().source)
        assertEquals("America/Los_Angeles", (page.events.single().time as EventTime.Timed).timeZone.id)
        val detail = session.getEvent(page.events.single().key)
        assertEquals("fixture-event-1", detail.key.eventId.value)
        assertEquals("Synthetic review detail", detail.title)
        assertEquals("Test-only event detail", detail.description)
        assertEquals(CalendarEventSource.GOOGLE_CALENDAR_API, detail.source)
        assertEquals(2, gateway.tokenRequests)
        assertEquals(
            listOf(
                setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY),
                setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY),
            ),
            gateway.requestedScopeSets,
        )

        val request = server.takeRequest()
        assertEquals("GET", request.method)
        assertEquals(
            listOf("calendar", "v3", "calendars", calendarId.value, "events"),
            request.requestUrl!!.pathSegments,
        )
        assertEquals("localhost", request.requestUrl!!.host)
        assertEquals("Bearer synthetic-never-valid-token", request.getHeader("Authorization"))
        assertFalse(request.requestLine.contains("gmail"))
        val detailRequest = server.takeRequest()
        assertEquals("GET", detailRequest.method)
        assertEquals(
            listOf("calendar", "v3", "calendars", calendarId.value, "events", "fixture-event-1"),
            detailRequest.requestUrl!!.pathSegments,
        )
        assertEquals("localhost", detailRequest.requestUrl!!.host)
        assertFalse(detailRequest.requestLine.contains("gmail"))

        gateway.switchSyntheticAccount()
        val staleAccountRead = runCatching { session.listEventsPage(calendarId, range) }.exceptionOrNull()
        assertTrue(staleAccountRead is CalendarReadFailure)
        assertEquals(2, server.requestCount)
        assertEquals(2, gateway.tokenRequests)
    }
}
