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
    }

    @Test fun nextUpcomingExcludesAllDayAndPastTimedEvents() = runBlocking {
        val service = CalendarAgendaService(FakeCalendarProvider(pageSize = 500), fixed, ZoneId.of("UTC"))
        assertTrue(service.nextUpcomingEvents(5, Duration.ofDays(2)).size <= 5)
    }
}
