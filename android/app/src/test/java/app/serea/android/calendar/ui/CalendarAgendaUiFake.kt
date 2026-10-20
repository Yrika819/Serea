package app.serea.android.calendar.ui

import app.serea.android.calendar.CalendarApiFailureCode
import app.serea.android.calendar.CalendarAuthorizationPort
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.calendar.CalendarAuthorizationStatus
import app.serea.android.calendar.CalendarDateRange
import app.serea.android.calendar.CalendarDescriptor
import app.serea.android.calendar.CalendarEvent
import app.serea.android.calendar.CalendarEventKey
import app.serea.android.calendar.CalendarId
import app.serea.android.calendar.CalendarReadFailure
import app.serea.android.calendar.CalendarReadPort
import app.serea.android.calendar.CalendarEventPage
import app.serea.android.calendar.EventId
import app.serea.android.calendar.EventStatus
import app.serea.android.calendar.EventTime
import app.serea.android.calendar.FakeCalendarProvider
import java.time.Clock
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

internal val UI_TEST_INSTANT: Instant = Instant.parse("2026-10-20T12:00:00Z")
internal val UI_TEST_CLOCK: Clock = Clock.fixed(UI_TEST_INSTANT, ZoneId.of("UTC"))

internal fun calendar(id: String, name: String = id, zone: ZoneId = ZoneId.of("UTC")) =
    CalendarDescriptor(CalendarId(id), name, zone)

internal fun timedEvent(
    calendarId: String,
    eventId: String,
    start: String,
    end: String,
    zone: ZoneId = ZoneId.of("UTC"),
    title: String? = eventId,
    description: String? = null,
    location: String? = null,
    status: EventStatus = EventStatus.CONFIRMED,
) = CalendarEvent(
    key = CalendarEventKey(CalendarId(calendarId), EventId(eventId)),
    title = title,
    description = description,
    location = location,
    time = if (status == EventStatus.CANCELLED) null else EventTime.Timed(
        Instant.parse(start), Instant.parse(end), zone,
    ),
    status = status,
    source = app.serea.android.calendar.CalendarEventSource.MOCK,
)

internal fun allDayEvent(
    calendarId: String,
    eventId: String,
    start: String,
    endExclusive: String,
    zone: ZoneId,
    title: String? = eventId,
) = CalendarEvent(
    key = CalendarEventKey(CalendarId(calendarId), EventId(eventId)),
    title = title,
    description = null,
    location = null,
    time = EventTime.AllDay(LocalDate.parse(start), LocalDate.parse(endExclusive), zone),
    status = EventStatus.CONFIRMED,
    source = app.serea.android.calendar.CalendarEventSource.MOCK,
)

internal class CalendarAgendaUiFake(
    private val delegate: FakeCalendarProvider,
    var authorization: CalendarAuthorizationStatus = CalendarAuthorizationStatus(CalendarAuthorizationState.MOCK_ONLY),
    private val beforePage: suspend (callIndex: Int, calendarId: CalendarId, pageToken: String?) -> Unit = { _, _, _ -> },
    private val beforeGetEvent: suspend (CalendarEventKey) -> Unit = {},
    private val getEventFailure: CalendarApiFailureCode? = null,
    private var firstFailure: CalendarApiFailureCode? = null,
) : CalendarReadPort, CalendarAuthorizationPort {
    private var pageCalls = 0
    val pageCalendarIds = mutableListOf<String>()
    val getEventKeys = mutableListOf<CalendarEventKey>()

    override suspend fun status(): CalendarAuthorizationStatus = authorization
    override suspend fun listCalendars() = delegate.listCalendars()
    override suspend fun getCalendar(calendarId: CalendarId) = delegate.getCalendar(calendarId)
    override suspend fun getEvent(key: CalendarEventKey): CalendarEvent {
        getEventKeys += key
        beforeGetEvent(key)
        getEventFailure?.let { throw CalendarReadFailure(it, "Fictional detail failure.") }
        return delegate.getEvent(key)
    }

    override suspend fun listEventsPage(
        calendarId: CalendarId,
        range: CalendarDateRange,
        pageToken: String?,
    ): CalendarEventPage {
        val callIndex = pageCalls++
        pageCalendarIds += calendarId.value
        beforePage(callIndex, calendarId, pageToken)
        firstFailure?.let { code ->
            firstFailure = null
            throw CalendarReadFailure(code, "Fictional private event details must not be surfaced.")
        }
        return delegate.listEventsPage(calendarId, range, pageToken)
    }
}

internal fun fakeProvider(
    calendars: List<CalendarDescriptor>,
    events: List<CalendarEvent>,
    pageSize: Int = 100,
    options: FakeCalendarProvider.Options = FakeCalendarProvider.Options(),
    clock: Clock = UI_TEST_CLOCK,
) = FakeCalendarProvider(
    calendars = calendars,
    events = events,
    pageSize = pageSize,
    options = options,
    clock = clock,
)
