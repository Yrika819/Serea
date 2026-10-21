package app.serea.android.calendar

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.delay
import java.time.Clock
import java.time.Duration
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

/** All records in this provider are fictional test/demo data; it performs no network access. */
class FakeCalendarProvider(
    private val calendars: List<CalendarDescriptor> = Fixtures.calendars,
    private val events: List<CalendarEvent> = Fixtures.events,
    private val pageSize: Int = 100,
    private val options: Options = Options(),
    val clock: Clock = Clock.fixed(Instant.parse("2026-10-20T12:00:00Z"), ZoneId.of("UTC")),
) : CalendarReadPort, CalendarAuthorizationPort {
    data class Options(
        val emptyPageIndexes: Set<Int> = emptySet(),
        val duplicateFirstEventAcrossPages: Boolean = false,
        val reverseEventOrder: Boolean = false,
        val failureOnPageIndex: Int? = null,
        val malformedResponseOnPageIndex: Int? = null,
        val timeoutOnPageIndex: Int? = null,
        val cancellationOnPageIndex: Int? = null,
        val permissionDenied: Boolean = false,
        val pageDelay: Duration = Duration.ZERO,
    )

    init { require(pageSize > 0) }
    private val pageCalls = mutableMapOf<CalendarId, Int>()
    private val ordered = if (options.reverseEventOrder) events.reversed() else events

    override suspend fun listCalendars(): List<CalendarDescriptor> {
        if (options.permissionDenied) throw CalendarReadFailure(CalendarApiFailureCode.PERMISSION_DENIED, "Mock calendar permission denied.")
        return calendars.toList()
    }

    override suspend fun getCalendar(calendarId: CalendarId): CalendarDescriptor {
        if (options.permissionDenied) throw CalendarReadFailure(CalendarApiFailureCode.PERMISSION_DENIED, "Mock calendar permission denied.")
        return calendars.firstOrNull { it.id == calendarId }
            ?: throw CalendarReadFailure(CalendarApiFailureCode.NOT_FOUND, "Mock calendar not found.")
    }

    override suspend fun getEvent(key: CalendarEventKey): CalendarEvent {
        getCalendar(key.calendarId)
        return events.firstOrNull { it.key == key }
            ?: throw CalendarReadFailure(CalendarApiFailureCode.NOT_FOUND, "Mock event not found.")
    }

    override suspend fun listEventsPage(
        calendarId: CalendarId,
        range: CalendarDateRange,
        pageToken: String?,
    ): CalendarEventPage {
        getCalendar(calendarId)
        if (options.permissionDenied) throw CalendarReadFailure(CalendarApiFailureCode.PERMISSION_DENIED, "Mock calendar permission denied.")
        val page = pageToken?.removePrefix("page-")?.toIntOrNull() ?: 0
        if (pageToken != null && !pageToken.matches(Regex("page-[0-9]+"))) {
            throw CalendarReadFailure(CalendarApiFailureCode.INVALID_RESPONSE, "Malformed mock page token.")
        }
        val callIndex = pageCalls.getOrDefault(calendarId, 0)
        pageCalls[calendarId] = callIndex + 1
        if (!options.pageDelay.isZero) delay(options.pageDelay.toMillis())
        if (options.timeoutOnPageIndex == callIndex) throw CalendarReadFailure(CalendarApiFailureCode.TIMEOUT, "Injected mock timeout.")
        if (options.cancellationOnPageIndex == callIndex) throw CancellationException("Injected mock cancellation.")
        if (options.failureOnPageIndex == callIndex) throw CalendarReadFailure(CalendarApiFailureCode.PARTIAL_FAILURE, "Injected mock page failure.")
        if (options.malformedResponseOnPageIndex == callIndex) return CalendarEventPage(emptyList(), " ")

        val matching = ordered.filter { it.key.calendarId == calendarId && overlaps(it, range) }
        val start = page * pageSize
        val end = minOf(start + pageSize, matching.size)
        val pageEvents: MutableList<CalendarEvent> = if (page in options.emptyPageIndexes) {
            mutableListOf()
        } else {
            matching.subList(minOf(start, matching.size), end).toMutableList()
        }
        if (options.duplicateFirstEventAcrossPages && page > 0 && matching.isNotEmpty()) pageEvents.add(0, matching.first())
        return CalendarEventPage(pageEvents, if (end < matching.size) "page-${page + 1}" else null)
    }

    override suspend fun status() = CalendarAuthorizationStatus(CalendarAuthorizationState.MOCK_ONLY)

    private fun overlaps(event: CalendarEvent, range: CalendarDateRange): Boolean {
        if (event.status != EventStatus.CANCELLED) return range.overlaps(event)
        return when (val original = event.originalStartTime) {
            is EventOriginalStart.Timed -> original.instant >= range.start && original.instant < range.endExclusive
            is EventOriginalStart.AllDay -> {
                val start = original.date.atStartOfDay(original.timeZone).toInstant()
                val end = original.date.plusDays(1).atStartOfDay(original.timeZone).toInstant()
                end.isAfter(range.start) && start.isBefore(range.endExclusive)
            }
            null -> false
        }
    }

    /** Small probe for tests asserting no API credential was supplied or retained. */
    fun containsCredentialState(): Boolean = false

    object Fixtures {
        private val tokyo = ZoneId.of("Asia/Tokyo")
        private val newYork = ZoneId.of("America/New_York")
        private val london = ZoneId.of("Europe/London")
        private val apia = ZoneId.of("Pacific/Apia")

        val calendars = listOf(
            CalendarDescriptor(CalendarId("demo-primary"), "Demo Personal", tokyo, 0xff4285f4.toInt(), CalendarVisibility.PRIVATE, CalendarReadPermission.MOCK_ONLY),
            CalendarDescriptor(CalendarId("demo-work"), "Demo Work", newYork, 0xff34a853.toInt(), CalendarVisibility.DEFAULT, CalendarReadPermission.MOCK_ONLY),
            CalendarDescriptor(CalendarId("demo-global"), "Demo Global", apia, null, CalendarVisibility.UNKNOWN, CalendarReadPermission.MOCK_ONLY),
        )

        private fun timed(
            calendar: String, id: String, title: String?, start: String, end: String,
            zone: ZoneId, status: EventStatus = EventStatus.CONFIRMED,
            recurring: String? = null, originalStart: String? = null,
            visibility: EventVisibility = EventVisibility.DEFAULT,
        ) = CalendarEvent(
            key = CalendarEventKey(CalendarId(calendar), EventId(id)), title = title,
            description = null, location = null,
            time = if (status == EventStatus.CANCELLED && start.isBlank()) null else EventTime.Timed(Instant.parse(start), Instant.parse(end), zone),
            status = status, recurringEventId = recurring?.let(::EventId),
            originalStartTime = originalStart?.let { EventOriginalStart.Timed(Instant.parse(it), zone) },
            updatedAt = Instant.parse("2026-10-19T08:00:00Z"), visibility = visibility,
            source = CalendarEventSource.MOCK,
        )

        val events: List<CalendarEvent> = buildList {
            add(timed("demo-primary", "evt-meeting", "Design review", "2026-10-20T00:00:00Z", "2026-10-20T01:00:00Z", tokyo))
            add(CalendarEvent(CalendarEventKey(CalendarId("demo-primary"), EventId("evt-all-day")), "Autumn holiday", null, null,
                EventTime.AllDay(LocalDate.parse("2026-10-20"), LocalDate.parse("2026-10-21"), tokyo), EventStatus.CONFIRMED,
                updatedAt = Instant.parse("2026-10-18T00:00:00Z"), source = CalendarEventSource.MOCK))
            add(CalendarEvent(CalendarEventKey(CalendarId("demo-primary"), EventId("evt-multi-day")), "Demo conference", null, null,
                EventTime.AllDay(LocalDate.parse("2026-10-19"), LocalDate.parse("2026-10-22"), tokyo), EventStatus.CONFIRMED,
                source = CalendarEventSource.MOCK))
            add(timed("demo-work", "evt-work-same-instant", "Cross-calendar sync", "2026-10-20T00:00:00Z", "2026-10-20T00:30:00Z", newYork))
            add(timed("demo-primary", "evt-series-20261020", "Recurring stand-up", "2026-10-20T02:00:00Z", "2026-10-20T02:30:00Z", tokyo,
                recurring = "series-standup", originalStart = "2026-10-20T02:00:00Z"))
            add(timed("demo-primary", "evt-series-moved", "Recurring stand-up (moved)", "2026-10-20T04:00:00Z", "2026-10-20T04:30:00Z", tokyo,
                recurring = "series-standup", originalStart = "2026-10-20T03:00:00Z"))
            add(timed("demo-primary", "evt-series-cancelled", null, "", "", tokyo, EventStatus.CANCELLED,
                recurring = "series-standup", originalStart = "2026-10-20T05:00:00Z"))
            add(timed("demo-work", "evt-private", "Confidential demo block", "2026-10-20T10:00:00Z", "2026-10-20T11:00:00Z", newYork,
                visibility = EventVisibility.PRIVATE))
            add(timed("demo-primary", "evt-no-summary", null, "2026-10-20T12:00:00Z", "2026-10-20T12:15:00Z", tokyo))
            add(timed("demo-global", "evt-apia", "Date-line test", "2026-10-20T13:00:00Z", "2026-10-20T14:00:00Z", apia))
            // Keep a long fictional list in a second calendar so UI tests can verify that
            // changing the selected calendar resets scroll position without short-list clamping.
            for (index in 0 until 96) {
                val start = Instant.parse("2026-10-24T00:00:00Z").plusSeconds(index * 3600L)
                add(timed("demo-global", "global-bulk-$index", "Fictional global event $index", start.toString(), start.plusSeconds(900).toString(), apia))
            }
            add(timed("demo-work", "evt-dst-start", "DST start sample", "2026-03-08T06:30:00Z", "2026-03-08T07:30:00Z", newYork))
            add(timed("demo-work", "evt-dst-end", "DST end sample", "2026-11-01T05:30:00Z", "2026-11-01T06:30:00Z", newYork))
            for (index in 0 until 245) {
                val start = Instant.parse("2026-10-21T00:00:00Z").plusSeconds(index * 3600L)
                add(timed("demo-work", "bulk-$index", "Fictional paged event $index", start.toString(), start.plusSeconds(900).toString(), newYork))
            }
        }

        /** Bounded app-shell fixture crosses two pages without rendering a test-only 245-row demo. */
        val appShellEvents: List<CalendarEvent> = events.filter { event ->
            val index = event.key.eventId.value.removePrefix("bulk-").toIntOrNull()
            index == null || index < 72
        }
    }
}
