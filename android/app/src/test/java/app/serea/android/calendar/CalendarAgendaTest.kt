package app.serea.android.calendar

import java.time.Clock
import java.time.Duration
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test

class CalendarAgendaTest {
    private val fixed = Clock.fixed(Instant.parse("2026-10-20T12:00:00Z"), ZoneId.of("UTC"))

    @Test fun todayTomorrowUseInjectedDeviceTimezoneAndCalendarDays() = runBlocking {
        val provider = FakeCalendarProvider(pageSize = 500)
        val tokyo = CalendarAgendaService(provider, fixed, ZoneId.of("Asia/Tokyo"))
        val today = tokyo.eventsToday()
        assertTrue(today.any { it.key.eventId.value == "evt-all-day" })
        assertFalse(today.any { it.key.eventId.value == "bulk-0" })
        val tomorrow = tokyo.eventsTomorrow()
        assertTrue(tomorrow.any { it.key.eventId.value == "bulk-0" })
        val apia = CalendarAgendaService(provider, fixed, ZoneId.of("Pacific/Kiritimati"))
        assertEquals(LocalDate.parse("2026-10-21"), fixed.instant().atZone(ZoneId.of("Pacific/Kiritimati")).toLocalDate())
        assertTrue(apia.eventsToday().any { it.key.eventId.value == "bulk-0" })
    }

    @Test fun rangeIsHalfOpenIncludesOverlapsSortsByInstantAndSkipsCancelled() = runBlocking {
        val service = CalendarAgendaService(FakeCalendarProvider(pageSize = 500), fixed, ZoneId.of("UTC"))
        val events = service.eventsInRange(CalendarDateRange(Instant.parse("2026-10-20T00:00:00Z"), Instant.parse("2026-10-20T05:00:00Z")))
        assertFalse(events.any { it.status == EventStatus.CANCELLED })
        assertTrue(events.any { it.key.eventId.value == "evt-multi-day" })
        assertTrue(events.zipWithNext().all { (a, b) -> CalendarAgendaService.EVENT_ORDER.compare(a, b) <= 0 })
        val sameInstant = events.filter { it.time is EventTime.Timed && (it.time as EventTime.Timed).start == Instant.parse("2026-10-20T00:00:00Z") }
        assertEquals(2, sameInstant.size)
        assertNotEquals(sameInstant[0].key.calendarId, sameInstant[1].key.calendarId)
    }

    @Test fun pageAggregationDeduplicatesInstancesAndHandlesEmptyIntermediatePages() = runBlocking {
        val provider = FakeCalendarProvider(pageSize = 20, options = FakeCalendarProvider.Options(
            duplicateFirstEventAcrossPages = true, emptyPageIndexes = setOf(1), reverseEventOrder = true,
        ))
        val service = CalendarAgendaService(provider, fixed, ZoneId.of("UTC"))
        val events = service.eventsInRange(CalendarDateRange(Instant.parse("2026-10-19T00:00:00Z"), Instant.parse("2026-11-10T00:00:00Z")))
        val ids = events.map { it.stableIdentity() }
        assertEquals(ids.size, ids.toSet().size)
        assertTrue(events.size > 200)
    }

    @Test fun partialPageFailureTimeoutCancellationAndLimitsNeverReturnPartialAgenda() = runBlocking {
        val range = CalendarDateRange(Instant.parse("2026-10-19T00:00:00Z"), Instant.parse("2026-11-10T00:00:00Z"))
        val failures = listOf(
            FakeCalendarProvider.Options(failureOnPageIndex = 1),
            FakeCalendarProvider.Options(timeoutOnPageIndex = 0),
        )
        failures.forEach { options ->
            try { CalendarAgendaService(FakeCalendarProvider(pageSize = 10, options = options), fixed).eventsInRange(range); fail("Expected injected failure") }
            catch (expected: CalendarReadFailure) { assertTrue(expected.code in setOf(CalendarApiFailureCode.PARTIAL_FAILURE, CalendarApiFailureCode.TIMEOUT)) }
        }
        try { CalendarAgendaService(FakeCalendarProvider(pageSize = 10, options = FakeCalendarProvider.Options(cancellationOnPageIndex = 0)), fixed).eventsInRange(range); fail("Expected cancellation") }
        catch (_: CancellationException) { }
        try { CalendarAgendaService(FakeCalendarProvider(pageSize = 10), fixed, limits = CalendarReadLimits(maximumPages = 1, maximumEvents = 1000)).eventsInRange(range); fail("Expected page limit") }
        catch (expected: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.TOO_MANY_PAGES, expected.code) }
        try { CalendarAgendaService(FakeCalendarProvider(pageSize = 10), fixed, limits = CalendarReadLimits(maximumPages = 100, maximumEvents = 5)).eventsInRange(range); fail("Expected event limit") }
        catch (expected: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.TOO_MANY_EVENTS, expected.code) }
    }

    @Test fun permissionAndEmptyAgendaAreExplicit() = runBlocking {
        val denied = FakeCalendarProvider(options = FakeCalendarProvider.Options(permissionDenied = true))
        try { CalendarAgendaService(denied, fixed).eventsToday(); fail("Expected permission denial") }
        catch (expected: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, expected.code) }
        val empty = FakeCalendarProvider(calendars = emptyList(), events = emptyList())
        assertTrue(CalendarAgendaService(empty, fixed).eventsToday().isEmpty())
        try { empty.getCalendar(CalendarId("missing")); fail("Expected missing calendar") }
        catch (expected: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.NOT_FOUND, expected.code) }
        try { empty.getEvent(CalendarEventKey(CalendarId("missing"), EventId("missing"))); fail("Expected missing event") }
        catch (expected: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.NOT_FOUND, expected.code) }
    }

    @Test fun malformedPageResponseIsRejectedAsIncomplete() = runBlocking {
        val provider = FakeCalendarProvider(options = FakeCalendarProvider.Options(malformedResponseOnPageIndex = 0))
        try { CalendarAgendaService(provider, fixed).eventsToday(); fail("Expected malformed page rejection") }
        catch (expected: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.INVALID_RESPONSE, expected.code) }
    }

    @Test fun nextUpcomingExcludesAllDayAndPastTimedEvents() = runBlocking {
        val service = CalendarAgendaService(FakeCalendarProvider(pageSize = 500), fixed, ZoneId.of("UTC"))
        assertTrue(service.nextUpcomingEvents(5, Duration.ofDays(2)).size <= 5)
    }

    @Test fun localDateWindowsUseDstBoundariesInsteadOfTwentyFourHourAddition() = runBlocking {
        val zone = ZoneId.of("America/New_York")
        val start = LocalDate.parse("2026-03-08").atStartOfDay(zone).toInstant()
        val end = LocalDate.parse("2026-03-09").atStartOfDay(zone).toInstant()
        assertEquals(Duration.ofHours(23), Duration.between(start, end))
        val descriptor = CalendarDescriptor(CalendarId("dst-calendar"), "Fictional DST", zone,
            readPermission = CalendarReadPermission.MOCK_ONLY)
        val event = CalendarEvent(CalendarEventKey(descriptor.id, EventId("dst-after-spring-forward")), "After DST jump", null, null,
            EventTime.Timed(Instant.parse("2026-03-08T07:15:00Z"), Instant.parse("2026-03-08T07:45:00Z"), zone),
            EventStatus.CONFIRMED, source = CalendarEventSource.MOCK)
        val service = CalendarAgendaService(FakeCalendarProvider(calendars = listOf(descriptor), events = listOf(event)), fixed, zone)
        assertEquals(listOf(event.key), service.eventsOnDate(LocalDate.parse("2026-03-08")).map { it.key })
        assertTrue(service.eventsOnDate(LocalDate.parse("2026-03-09")).isEmpty())
    }

    @Test fun dstFallBackDateWindowCanBeTwentyFiveHours() {
        val zone = ZoneId.of("America/New_York")
        val start = LocalDate.parse("2026-11-01").atStartOfDay(zone).toInstant()
        val end = LocalDate.parse("2026-11-02").atStartOfDay(zone).toInstant()
        assertEquals(Duration.ofHours(25), Duration.between(start, end))
    }
}
