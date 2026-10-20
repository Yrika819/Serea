package app.serea.android.calendar.transport

import app.serea.android.calendar.CalendarApiFailureCode
import app.serea.android.calendar.CalendarAuthorizationPort
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.calendar.CalendarAuthorizationStatus
import app.serea.android.calendar.CalendarDateRange
import app.serea.android.calendar.CalendarId
import app.serea.android.calendar.CalendarReadFailure
import app.serea.android.calendar.CalendarReadLimits
import app.serea.android.calendar.CalendarAgendaService
import app.serea.android.calendar.CalendarEventKey
import app.serea.android.calendar.EventId
import app.serea.android.calendar.CalendarPermissionPolicy
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.SocketPolicy
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import java.time.Instant
import java.util.concurrent.TimeUnit

class GoogleCalendarHttpReadPortTest {
    private lateinit var server: MockWebServer
    private val calendarId = CalendarId("fixture+calendar@example.test")
    private val token = CalendarAccessToken("fixture-token-never-valid")
    private var status = CalendarAuthorizationStatus(CalendarAuthorizationState.AUTHORIZED)
    private var tokenRequests = 0
    private val authorization = object : CalendarAuthorizationPort {
        override suspend fun status() = status
    }

    @Before fun setUp() {
        server = MockWebServer()
        server.start()
        status = CalendarAuthorizationStatus(CalendarAuthorizationState.AUTHORIZED)
        tokenRequests = 0
    }

    @After fun tearDown() { server.shutdown() }

    private fun port(
        accessToken: CalendarAccessToken? = token,
        client: OkHttpClient = OkHttpClient.Builder().followRedirects(false).followSslRedirects(false)
            .retryOnConnectionFailure(false).build(),
    ) = GoogleCalendarHttpReadPort(authorization, CalendarAccessTokenPort {
        tokenRequests++
        accessToken
    }, client, server.url("/calendar/v3/"))

    @Test fun listCalendarsPaginatesEncodesTokensAndParsesMultipleTimezones() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"items":[{"id":"a@example.test","summary":"Fixture","timeZone":"Asia/Tokyo","accessRole":"reader","backgroundColor":"#123456"}],"nextPageToken":"next +/="}"""))
        server.enqueue(MockResponse().setBody("""{"items":[{"id":"b@example.test","timeZone":"America/New_York","accessRole":"freeBusyReader"}]}"""))
        val calendars = port().listCalendars()
        assertEquals(2, calendars.size)
        assertEquals("Asia/Tokyo", calendars[0].timeZone.id)
        assertEquals("America/New_York", calendars[1].timeZone.id)
        val first = server.takeRequest()
        val second = server.takeRequest()
        assertEquals("Bearer fixture-token-never-valid", first.getHeader("Authorization"))
        assertEquals("application/json", first.getHeader("Accept"))
        assertEquals("/calendar/v3/users/me/calendarList", first.requestUrl!!.encodedPath)
        assertEquals("next +/=", second.requestUrl!!.queryParameter("pageToken"))
        assertEquals(2, tokenRequests)
    }

    @Test fun eventUrlsUseEncodedPathAndHalfOpenSingleExpandedRange() = runBlocking {
        server.enqueue(MockResponse().setBody(eventJson("event-id")))
        val range = CalendarDateRange(Instant.parse("2026-10-20T00:00:00Z"), Instant.parse("2026-10-21T00:00:00Z"))
        val page = port().listEventsPage(calendarId, range, "page +/=")
        assertEquals("event-id", page.events.single().key.eventId.value)
        val request = server.takeRequest()
        assertEquals("/calendar/v3/calendars/fixture+calendar%40example.test/events", request.requestUrl!!.encodedPath)
        assertEquals("true", request.requestUrl!!.queryParameter("singleEvents"))
        assertEquals("true", request.requestUrl!!.queryParameter("showDeleted"))
        assertEquals("startTime", request.requestUrl!!.queryParameter("orderBy"))
        assertEquals("2026-10-20T00:00:00Z", request.requestUrl!!.queryParameter("timeMin"))
        assertEquals("2026-10-21T00:00:00Z", request.requestUrl!!.queryParameter("timeMax"))
        assertEquals("page +/=", request.requestUrl!!.queryParameter("pageToken"))
        assertEquals("CALENDAR", page.events.single().dataOrigin.name)
        assertEquals("PERSONAL", page.events.single().classification.name)
    }

    @Test fun getCalendarAndEventUseReadOnlyResources() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"id":"fixture+calendar@example.test","timeZone":"UTC","accessRole":"reader"}"""))
        server.enqueue(MockResponse().setBody(eventObjectJson("event")))
        port().getCalendar(calendarId)
        port().getEvent(CalendarEventKey(calendarId, EventId("event")))
        assertTrue(server.takeRequest().requestLine!!.startsWith("GET /calendar/v3/calendars/"))
        assertTrue(server.takeRequest().requestLine!!.contains("/events/event "))
    }

    @Test fun missingTokenAndInvalidAuthorizationAreRejectedBeforeNetwork() = runBlocking {
        try { port(accessToken = null).getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected missing token") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, e.code) }
        status = CalendarAuthorizationStatus(CalendarAuthorizationState.MOCK_ONLY)
        try { port().listEventsPage(calendarId, range()); fail("MOCK_ONLY must not authorize production transport") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, e.code) }
        assertEquals(0, server.requestCount)
    }

    @Test fun missingScopeDoesNotRequestToken() = runBlocking {
        status = CalendarAuthorizationStatus(CalendarAuthorizationState.AUTHORIZED,
            missingScopes = setOf(CalendarPermissionPolicy.EVENTS_READ_ONLY))
        try { port().listEventsPage(calendarId, range()); fail("Expected scope rejection") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.MISSING_SCOPE, e.code) }
        assertEquals(0, tokenRequests)
        assertEquals(0, server.requestCount)
    }

    @Test fun apiFailuresAreMappedWithoutLeakingResponseBody() = runBlocking {
        val cases = listOf(
            401 to CalendarApiFailureCode.PERMISSION_DENIED,
            403 to CalendarApiFailureCode.MISSING_SCOPE,
            404 to CalendarApiFailureCode.NOT_FOUND,
            410 to CalendarApiFailureCode.SYNC_TOKEN_EXPIRED,
            429 to CalendarApiFailureCode.RATE_LIMITED,
            500 to CalendarApiFailureCode.UNAVAILABLE,
        )
        for ((http, expected) in cases) {
            server.enqueue(MockResponse().setResponseCode(http).setBody("""{"error":{"errors":[{"reason":"insufficientPermissions"}],"message":"private fixture text"}}"""))
            try { port().getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected HTTP $http") }
            catch (e: CalendarReadFailure) {
                assertEquals(expected, e.code)
                assertFalse(e.message!!.contains("private fixture text"))
            }
        }
        assertEquals(6, server.requestCount)
    }

    @Test fun quota403IsRateLimitedAndNoAutomaticRetryOccurs() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(403).setBody("""{"error":{"errors":[{"reason":"quotaExceeded"}]}}"""))
        try { port().getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected quota failure") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.RATE_LIMITED, e.code) }
        assertEquals(1, server.requestCount)
    }

    @Test fun redirectIsNotFollowedAndMalformedOrOversizedJsonIsRejected() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(302).setHeader("Location", server.url("/elsewhere").toString()))
        try { port().getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected redirect rejection") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, e.code) }
        assertEquals(1, server.requestCount)
        server.enqueue(MockResponse().setBody("{broken"))
        try { port().getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected JSON rejection") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, e.code) }
        server.enqueue(MockResponse().setBody(" ".repeat(600_000)))
        try { port().getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected size rejection") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, e.code) }
    }

    @Test fun timeoutMapsToTimeout() = runBlocking {
        server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE))
        val client = OkHttpClient.Builder().callTimeout(100, TimeUnit.MILLISECONDS).connectTimeout(100, TimeUnit.MILLISECONDS)
            .readTimeout(100, TimeUnit.MILLISECONDS).build()
        try { port(client = client).getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected timeout") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.TIMEOUT, e.code) }
    }

    @Test fun coroutineCancellationCancelsHttpCall() = runBlocking {
        server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE))
        val deferred = async { port().getEvent(CalendarEventKey(calendarId, EventId("x"))) }
        server.takeRequest(2, TimeUnit.SECONDS)
        deferred.cancelAndJoin()
        assertTrue(deferred.isCancelled)
    }

    @Test fun agendaServiceRejectsRepeatedPageTokensAndEnforcesAggregateLimits() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"items":[{"id":"fixture+calendar@example.test","timeZone":"UTC","accessRole":"reader"}]}"""))
        server.enqueue(MockResponse().setBody("""{"items":[],"nextPageToken":"repeat"}"""))
        server.enqueue(MockResponse().setBody("""{"items":[],"nextPageToken":"repeat"}"""))
        try { CalendarAgendaService(port(), limits = CalendarReadLimits(maximumPages = 5)).eventsInRange(range()); fail("Expected repeated token rejection") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, e.code) }
        assertEquals(3, server.requestCount)
    }

    private fun range() = CalendarDateRange(Instant.parse("2026-10-20T00:00:00Z"), Instant.parse("2026-10-21T00:00:00Z"))
    private fun eventObjectJson(id: String) = """{"id":"$id","summary":"Fictional fixture","status":"confirmed","start":{"dateTime":"2026-10-20T10:00:00Z"},"end":{"dateTime":"2026-10-20T11:00:00Z"}}"""
    private fun eventJson(id: String) = """{"items":[{"id":"$id","summary":"Fictional fixture","status":"confirmed","start":{"dateTime":"2026-10-20T10:00:00Z"},"end":{"dateTime":"2026-10-20T11:00:00Z"}}]}"""
}
