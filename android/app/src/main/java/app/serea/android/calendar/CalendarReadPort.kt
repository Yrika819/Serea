package app.serea.android.calendar

/** Read-only boundary for a Calendar API adapter or the deterministic fake. */
interface CalendarReadPort {
    suspend fun listCalendars(): List<CalendarDescriptor>
    suspend fun getCalendar(calendarId: CalendarId): CalendarDescriptor
    suspend fun getEvent(key: CalendarEventKey): CalendarEvent
    suspend fun listEventsPage(
        calendarId: CalendarId,
        range: CalendarDateRange,
        pageToken: String? = null,
    ): CalendarEventPage
}

data class CalendarEventPage(
    val events: List<CalendarEvent>,
    val nextPageToken: String? = null,
)

/** Future adapter seam. It describes authorization but has no token or sign-in methods. */
interface CalendarAuthorizationPort {
    suspend fun status(): CalendarAuthorizationStatus
}

data class CalendarReadLimits(
    val maximumPages: Int = 20,
    val maximumEvents: Int = 2_000,
) {
    init { require(maximumPages in 1..100 && maximumEvents in 1..10_000) }
}

/** Aggregates every page or fails the whole request; a partial page set is never returned as complete. */
class CalendarAgendaService(
    private val port: CalendarReadPort,
    private val clock: java.time.Clock = java.time.Clock.systemUTC(),
    private val deviceTimeZone: java.time.ZoneId = java.time.ZoneId.systemDefault(),
    private val limits: CalendarReadLimits = CalendarReadLimits(),
) {
    suspend fun eventsToday(): List<CalendarEvent> {
        val today = clock.instant().atZone(deviceTimeZone).toLocalDate()
        return eventsOnDate(today)
    }

    suspend fun eventsTomorrow(): List<CalendarEvent> {
        val tomorrow = clock.instant().atZone(deviceTimeZone).toLocalDate().plusDays(1)
        return eventsOnDate(tomorrow)
    }

    suspend fun eventsOnDate(date: java.time.LocalDate): List<CalendarEvent> {
        val start = date.atStartOfDay(deviceTimeZone).toInstant()
        val end = date.plusDays(1).atStartOfDay(deviceTimeZone).toInstant()
        return eventsInRange(CalendarDateRange(start, end))
    }

    suspend fun eventsInRange(range: CalendarDateRange): List<CalendarEvent> {
        val calendars = port.listCalendars()
        val collected = LinkedHashMap<String, CalendarEvent>()
        for (calendar in calendars.sortedBy { it.id.value }) {
            val pagesSeen = HashSet<String>()
            var token: String? = null
            var pageCount = 0
            do {
                if (++pageCount > limits.maximumPages) throw CalendarReadFailure(
                    CalendarApiFailureCode.TOO_MANY_PAGES, "Calendar page limit exceeded.")
                if (token != null && !pagesSeen.add(token)) throw CalendarReadFailure(
                    CalendarApiFailureCode.INVALID_RESPONSE, "Calendar page token repeated.")
                val page = port.listEventsPage(calendar.id, range, token)
                if (page.events.size > limits.maximumEvents - collected.size) throw CalendarReadFailure(
                    CalendarApiFailureCode.TOO_MANY_EVENTS, "Calendar event limit exceeded.")
                page.events.forEach { event ->
                    if (event.key.calendarId != calendar.id) throw CalendarReadFailure(
                        CalendarApiFailureCode.INVALID_RESPONSE, "Provider returned an event for another calendar.")
                    if (event.status != EventStatus.CANCELLED && rangeOverlaps(range, event)) {
                        collected.putIfAbsent(event.stableIdentity(), event)
                    }
                }
                token = page.nextPageToken?.also {
                    if (it.isBlank() || it.length > 2048) throw CalendarReadFailure(
                        CalendarApiFailureCode.INVALID_RESPONSE, "Invalid next page token.")
                }
            } while (token != null)
        }
        return collected.values.sortedWith(EVENT_ORDER)
    }

    suspend fun nextUpcomingEvents(limit: Int = 10, horizon: java.time.Duration = java.time.Duration.ofDays(30)): List<CalendarEvent> {
        require(limit in 1..100 && !horizon.isNegative && !horizon.isZero && horizon <= java.time.Duration.ofDays(366))
        val now = clock.instant()
        return eventsInRange(CalendarDateRange(now, now.plus(horizon)))
            .filter { it.time is EventTime.Timed && (it.time as EventTime.Timed).start >= now }
            .take(limit)
    }

    private fun rangeOverlaps(range: CalendarDateRange, event: CalendarEvent): Boolean {
        val time = event.time ?: return false
        return when (time) {
            is EventTime.Timed -> range.overlaps(event)
            is EventTime.AllDay -> range.overlaps(event)
        }
    }

    companion object {
        val EVENT_ORDER: Comparator<CalendarEvent> = Comparator { left, right ->
            val leftTime = left.time.orderInstant()
            val rightTime = right.time.orderInstant()
            val byInstant = compareValues(leftTime, rightTime)
            if (byInstant != 0) byInstant else compareValues(left.stableIdentity(), right.stableIdentity())
        }

        private fun EventTime?.orderInstant(): java.time.Instant? = when (this) {
            is EventTime.Timed -> start
            is EventTime.AllDay -> startDate.atStartOfDay(timeZone).toInstant()
            null -> null
        }
    }
}
