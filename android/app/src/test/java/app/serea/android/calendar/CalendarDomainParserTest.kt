package app.serea.android.calendar

import app.serea.android.DataClassification
import app.serea.android.DataOrigin
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import org.junit.Assert.*
import org.junit.Test

class CalendarDomainParserTest {
    private val calendarId = CalendarId("calendar-a")
    private val tokyo = ZoneId.of("Asia/Tokyo")

    @Test fun eventIdIsScopedToCalendarAndTitlesMayBeMissing() {
        val one = GoogleCalendarEventParser.parseEvent("""{"id":"same","start":{"dateTime":"2026-10-20T10:00:00+09:00","timeZone":"Asia/Tokyo"},"end":{"dateTime":"2026-10-20T11:00:00+09:00","timeZone":"Asia/Tokyo"}}""", calendarId, tokyo)
        val two = GoogleCalendarEventParser.parseEvent("""{"id":"same","summary":"Other","start":{"dateTime":"2026-10-20T10:00:00+09:00"},"end":{"dateTime":"2026-10-20T11:00:00+09:00"}}""", CalendarId("calendar-b"), tokyo)
        assertNotEquals(one.key, two.key)
        assertNull(one.title)
        assertEquals(DataOrigin.CALENDAR, one.dataOrigin)
        assertEquals(DataClassification.PERSONAL, one.classification)
        assertEquals(Instant.parse("2026-10-20T01:00:00Z"), (one.time as EventTime.Timed).start)
    }

    @Test fun allDayEndIsExclusiveAndMultiDayKeepsDateCount() {
        val event = GoogleCalendarEventParser.parseEvent("""{"id":"day","start":{"date":"2026-10-20"},"end":{"date":"2026-10-21"}}""", calendarId, tokyo)
        val time = event.time as EventTime.AllDay
        assertEquals(LocalDate.parse("2026-10-20"), time.startDate)
        assertEquals(LocalDate.parse("2026-10-21"), time.endDateExclusive)
        assertTrue(CalendarDateRange(Instant.parse("2026-10-19T15:00:00Z"), Instant.parse("2026-10-20T15:00:00Z")).overlaps(event))
        assertFalse(CalendarDateRange(Instant.parse("2026-10-20T15:00:00Z"), Instant.parse("2026-10-21T15:00:00Z")).overlaps(event))
    }

    @Test fun cancelledEventMayOmitTimesAndUnknownStatusIsSafe() {
        val cancelled = GoogleCalendarEventParser.parseEvent("""{"id":"gone","status":"cancelled"}""", calendarId, tokyo)
        assertEquals(EventStatus.CANCELLED, cancelled.status)
        assertNull(cancelled.time)
        val unknown = GoogleCalendarEventParser.parseEvent("""{"id":"future","status":"futureStatus","start":{"dateTime":"2026-10-20T10:00:00Z"},"end":{"dateTime":"2026-10-20T11:00:00Z"}}""", calendarId, tokyo)
        assertEquals(EventStatus.UNKNOWN, unknown.status)
    }

    @Test fun recurrenceIdentitySurvivesReschedulingAndCancellationIsRetainedByParser() {
        val moved = GoogleCalendarEventParser.parseEvent("""{"id":"instance-new","recurringEventId":"series","originalStartTime":{"dateTime":"2026-10-20T10:00:00-04:00","timeZone":"America/New_York"},"start":{"dateTime":"2026-10-20T12:00:00-04:00","timeZone":"America/New_York"},"end":{"dateTime":"2026-10-20T13:00:00-04:00","timeZone":"America/New_York"}}""", calendarId, tokyo)
        assertEquals("calendar-a:recurring:series:T:2026-10-20T14:00:00Z@America/New_York", moved.stableIdentity())
        assertNotNull(moved.originalStartTime)
        val exception = GoogleCalendarEventParser.parseEvent("""{"id":"instance-cancelled","status":"cancelled","recurringEventId":"series","originalStartTime":{"dateTime":"2026-10-20T10:00:00-04:00","timeZone":"America/New_York"}}""", calendarId, tokyo)
        assertEquals(EventStatus.CANCELLED, exception.status)
    }

    @Test fun timezoneValidationHandlesNewYorkLondonUtcAndDateLine() {
        val dates = listOf(
            Triple("2026-03-08T01:30:00-05:00", "2026-03-08T03:30:00-04:00", "America/New_York"),
            Triple("2026-11-01T01:30:00-04:00", "2026-11-01T01:30:00-05:00", "America/New_York"),
            Triple("2026-03-29T00:30:00Z", "2026-03-29T02:30:00+01:00", "Europe/London"),
            Triple("2026-10-20T00:00:00Z", "2026-10-20T01:00:00Z", "UTC"),
            Triple("2026-10-20T23:30:00+13:00", "2026-10-21T00:30:00+13:00", "Pacific/Apia"),
        )
        dates.forEachIndexed { index, (dateTime, endDateTime, zone) ->
            val localDate = dateTime.substring(0, 10)
            val json = """{"id":"t$index","start":{"dateTime":"$dateTime","timeZone":"$zone"},"end":{"dateTime":"$endDateTime","timeZone":"$zone"}}"""
            assertNotNull(GoogleCalendarEventParser.parseEvent(json, calendarId, tokyo).time)
            assertNotNull(LocalDate.parse(localDate))
        }
    }

    @Test fun malformedInputAndOffsetZoneConflictAreRejected() {
        listOf("not json", "{}", """{"id":"x","start":{"dateTime":"y"},"end":{"dateTime":"2026-10-20T10:00:00Z"}}""",
            """{"id":"x","start":{"dateTime":"2026-10-20T10:00:00-05:00","timeZone":"Asia/Tokyo"},"end":{"dateTime":"2026-10-20T11:00:00-05:00","timeZone":"Asia/Tokyo"}}""")
            .forEach { input ->
                try { GoogleCalendarEventParser.parseEvent(input, calendarId, tokyo); fail("Expected parser rejection") }
                catch (expected: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, expected.code) }
            }
        val badZone = """{"id":"zone","start":{"dateTime":"2026-10-20T10:00:00Z","timeZone":"Mars/Olympus"},"end":{"dateTime":"2026-10-20T11:00:00Z","timeZone":"Mars/Olympus"}}"""
        try { GoogleCalendarEventParser.parseEvent(badZone, calendarId, tokyo); fail("Expected invalid timezone") }
        catch (expected: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, expected.code) }
        try { CalendarId("bad id"); fail("Expected invalid ID") } catch (_: IllegalArgumentException) { }
        try { CalendarDateRange(Instant.EPOCH, Instant.EPOCH); fail("Expected invalid range") } catch (_: IllegalArgumentException) { }
    }

    @Test fun pageParserKeepsNextTokenAndIgnoresUnknownFields() {
        val page = GoogleCalendarEventParser.parsePage("""{"items":[{"id":"x","newFutureField":{"ignored":true},"start":{"date":"2026-10-20"},"end":{"date":"2026-10-21"}}],"nextPageToken":"page-2","nextSyncToken":"future-sync-token"}""", calendarId, tokyo)
        assertEquals("page-2", page.nextPageToken)
        assertEquals(1, page.events.size)
    }

    @Test fun oversizedPayloadIsRejected() {
        val payload = " ".repeat(GoogleCalendarEventParser.MAX_PAYLOAD_BYTES + 1)
        try { GoogleCalendarEventParser.parseEvent(payload, calendarId, tokyo); fail("Expected payload rejection") }
        catch (expected: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, expected.code) }
    }
}
