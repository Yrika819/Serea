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
import app.serea.android.googleauth.AccessTokenLease
import app.serea.android.googleauth.AuthorizationStatus
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleCalendarAuthorizationAdapter
import app.serea.android.googleauth.GoogleFeature
import app.serea.android.googleauth.GoogleScope
import app.serea.android.googleauth.GoogleServiceState
import app.serea.android.googleauth.ScopePermission
import app.serea.android.googleauth.TokenSupplyResult
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
import java.io.ByteArrayInputStream
import java.io.IOException
import java.io.InputStream
import java.time.Instant
import java.util.concurrent.TimeUnit

class GoogleCalendarHttpReadPortTest {
    private lateinit var server: MockWebServer
    private val calendarId = CalendarId("fixture+calendar@example.test")
    private fun freshToken() = CalendarAccessToken(
        AccessTokenLease("fixture-token-never-valid", setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY), null),
    )
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
        tokenProvider: () -> CalendarAccessToken? = { freshToken() },
        client: OkHttpClient = OkHttpClient.Builder().followRedirects(false).followSslRedirects(false)
            .retryOnConnectionFailure(false).build(),
    ) = GoogleCalendarHttpReadPort(authorization, CalendarAccessTokenPort { _ ->
        tokenRequests++
        tokenProvider()
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
        assertEquals(listOf("calendar", "v3", "calendars", calendarId.value, "events"), request.requestUrl!!.pathSegments)
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
        val calendarRequest = server.takeRequest()
        assertEquals(listOf("calendar", "v3", "users", "me", "calendarList", calendarId.value), calendarRequest.requestUrl!!.pathSegments)
        assertEquals("Bearer fixture-token-never-valid", calendarRequest.getHeader("Authorization"))
        assertTrue(server.takeRequest().requestLine!!.contains("/events/event "))
    }

    @Test fun missingTokenAndInvalidAuthorizationAreRejectedBeforeNetwork() = runBlocking {
        try { port(tokenProvider = { null }).getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected missing token") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, e.code) }
        status = CalendarAuthorizationStatus(CalendarAuthorizationState.MOCK_ONLY)
        try { port().listEventsPage(calendarId, range()); fail("MOCK_ONLY must not authorize production transport") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, e.code) }
        assertEquals(0, server.requestCount)
    }

    @Test fun accountSwitchDuringTokenLeaseAcquisitionRejectsBeforeHttpDispatch() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"id":"fixture+calendar@example.test","timeZone":"UTC","accessRole":"reader"}"""))
        var selectedSession = "calendar-session-before-switch"
        val gateway = object : GoogleAuthorizationGateway {
            override val mode = GoogleAuthorizationMode.PLAY_SERVICES
            override val authorizationSessionId: String get() = selectedSession
            override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { ScopePermission.GRANTED },
            )
            override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
                val lease = AccessTokenLease(
                    "fixture-calendar-token-before-switch",
                    scopes,
                    null,
                    authorizationSessionId = selectedSession,
                )
                selectedSession = "calendar-session-after-switch"
                return TokenSupplyResult.Available(lease)
            }
        }
        val bridge = GoogleCalendarAuthorizationAdapter(gateway)
        val realHttp = GoogleCalendarHttpReadPort(bridge, bridge, OkHttpClient(), server.url("/calendar/v3/"))

        try {
            realHttp.getCalendar(calendarId)
            fail("an account switch during lease acquisition must deny the read")
        } catch (failure: CalendarReadFailure) {
            assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, failure.code)
        }
        assertEquals(0, server.requestCount)
    }

    @Test fun missingScopeDoesNotRequestToken() = runBlocking {
        status = CalendarAuthorizationStatus(CalendarAuthorizationState.AUTHORIZED,
            missingScopes = setOf(CalendarPermissionPolicy.EVENTS_READ_ONLY))
        try { port().listEventsPage(calendarId, range()); fail("Expected scope rejection") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.MISSING_SCOPE, e.code) }
        assertEquals(0, tokenRequests)
        assertEquals(0, server.requestCount)

        status = CalendarAuthorizationStatus(CalendarAuthorizationState.AUTHORIZED,
            missingScopes = setOf(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY))
        try { port().getCalendar(calendarId); fail("Expected calendar-list scope rejection") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.MISSING_SCOPE, e.code) }
        assertEquals(0, tokenRequests)
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

    @Test fun injectedClientCannotForwardBearerTokenThroughRedirect() = runBlocking {
        val destination = MockWebServer().apply { start() }
        try {
            server.enqueue(MockResponse().setResponseCode(302).setHeader("Location", destination.url("/steal").toString()))
            val redirectingClient = OkHttpClient.Builder().followRedirects(true).followSslRedirects(true).build()
            try { port(client = redirectingClient).getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected redirect rejection") }
            catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, e.code) }
            assertEquals(1, server.requestCount)
            assertEquals(0, destination.requestCount)
            assertEquals("Bearer fixture-token-never-valid", server.takeRequest().getHeader("Authorization"))
        } finally { destination.shutdown() }
    }

    @Test fun malformedOrOversizedJsonIsRejected() = runBlocking {
        server.enqueue(MockResponse().setBody("{broken"))
        try { port().getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected JSON rejection") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, e.code) }
        server.enqueue(MockResponse().setBody(" ".repeat(600_000)))
        try { port().getEvent(CalendarEventKey(calendarId, EventId("x"))); fail("Expected size rejection") }
        catch (e: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, e.code) }
    }

    @Test fun boundedChunkReaderAcceptsExactLimitAndRejectsOneByteOver() {
        val limit = 512 * 1024
        val exact = ByteArray(limit) { (it % 251).toByte() }
        assertArrayEquals(exact, readBoundedResponse(ByteArrayInputStream(exact), limit))
        try { readBoundedResponse(ByteArrayInputStream(ByteArray(limit + 1)), limit); fail("Expected size limit") }
        catch (_: ResponseTooLargeException) { }
    }

    @Test fun boundedChunkReaderMapsMidstreamIoFailure() {
        val input = object : InputStream() {
            private var first = true
            override fun read(): Int = error("single-byte path should not be used")
            override fun read(buffer: ByteArray, offset: Int, length: Int): Int {
                if (first) {
                    first = false
                    buffer[offset] = 1
                    return 1
                }
                throw IOException("fixture read failure")
            }
        }
        try { readBoundedResponse(input, 32); fail("Expected I/O failure") }
        catch (e: IOException) { assertEquals("fixture read failure", e.message) }
    }

    @Test fun listEventsUsesSafePageSizeAndResponseTimezone() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"timeZone":"Asia/Tokyo","items":[{"id":"all-day","summary":"Fixture","status":"confirmed","start":{"date":"2026-10-20"},"end":{"date":"2026-10-21"}}]}"""))
        val page = port().listEventsPage(calendarId, range())
        val time = page.events.single().time as app.serea.android.calendar.EventTime.AllDay
        assertEquals("Asia/Tokyo", time.timeZone.id)
        val request = server.takeRequest()
        assertEquals("100", request.requestUrl!!.queryParameter("maxResults"))
        assertTrue(request.requestUrl!!.queryParameter("fields")!!.contains("timeZone"))
    }

    @Test fun getEventUsesPayloadTimezoneForAllDayFallback() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"id":"all-day","summary":"Fixture","start":{"date":"2026-10-20","timeZone":"America/Los_Angeles"},"end":{"date":"2026-10-21","timeZone":"America/Los_Angeles"}}"""))
        val event = port().getEvent(CalendarEventKey(calendarId, EventId("all-day")))
        val time = event.time as app.serea.android.calendar.EventTime.AllDay
        assertEquals("America/Los_Angeles", time.timeZone.id)
    }

    @Test fun getEventUsesCalendarListTimezoneOnlyWhenThatScopeIsAlreadyGranted() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"id":"all-day","start":{"date":"2026-10-20"},"end":{"date":"2026-10-21"}}"""))
        server.enqueue(MockResponse().setBody("""{"id":"fixture+calendar@example.test","timeZone":"Asia/Tokyo","accessRole":"reader"}"""))
        val event = port().getEvent(CalendarEventKey(calendarId, EventId("all-day")))
        assertEquals("Asia/Tokyo", (event.time as app.serea.android.calendar.EventTime.AllDay).timeZone.id)
        assertEquals(
            listOf("calendar", "v3", "calendars", calendarId.value, "events", "all-day"),
            server.takeRequest().requestUrl!!.pathSegments,
        )
        assertEquals(
            listOf("calendar", "v3", "users", "me", "calendarList", calendarId.value),
            server.takeRequest().requestUrl!!.pathSegments,
        )
        assertEquals(2, tokenRequests)
    }

    @Test fun getEventWithUnknownAllDayTimezoneFailsWithoutRequestingAnUnapprovedScope() = runBlocking {
        status = CalendarAuthorizationStatus(
            CalendarAuthorizationState.AUTHORIZED,
            missingScopes = setOf(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY),
        )
        server.enqueue(MockResponse().setBody("""{"id":"all-day","start":{"date":"2026-10-20"},"end":{"date":"2026-10-21"}}"""))
        try {
            port().getEvent(CalendarEventKey(calendarId, EventId("all-day")))
            fail("Expected incomplete all-day timezone result")
        } catch (failure: CalendarReadFailure) {
            assertEquals(CalendarApiFailureCode.TIME_ZONE_UNKNOWN, failure.code)
        }
        assertEquals(1, tokenRequests)
        assertEquals(1, server.requestCount)
        assertTrue(server.takeRequest().requestLine!!.contains("/events/all-day"))
    }

    @Test fun calendarListResponseWithoutTimezoneIsNotSilentlyTreatedAsUtc() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"id":"fixture+calendar@example.test","accessRole":"reader"}"""))
        try {
            port().getCalendar(calendarId)
            fail("Expected missing timezone failure")
        } catch (failure: CalendarReadFailure) {
            assertEquals(CalendarApiFailureCode.TIME_ZONE_UNKNOWN, failure.code)
        }
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
