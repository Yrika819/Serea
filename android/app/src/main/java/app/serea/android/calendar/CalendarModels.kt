package app.serea.android.calendar

import app.serea.android.DataClassification
import app.serea.android.DataOrigin
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId

/** Validated Google identifiers are scoped by the owning calendar. */
@JvmInline
value class CalendarId(val value: String) {
    init { require(value.length in 1..256 && value.matches(Regex("[A-Za-z0-9_.@+-]+"))) { "Invalid calendar ID." } }
}

@JvmInline
value class EventId(val value: String) {
    init { require(value.length in 1..1024 && value.matches(Regex("[A-Za-z0-9_.@+-]+"))) { "Invalid event ID." } }
}

data class CalendarEventKey(val calendarId: CalendarId, val eventId: EventId)

enum class CalendarVisibility { DEFAULT, PUBLIC, PRIVATE, UNKNOWN }
enum class CalendarReadPermission { UNKNOWN, NOT_CONNECTED, MOCK_ONLY, DENIED, READ_ALLOWED }
enum class EventStatus { CONFIRMED, TENTATIVE, CANCELLED, UNKNOWN }
enum class EventVisibility { DEFAULT, PUBLIC, PRIVATE, CONFIDENTIAL, UNKNOWN }
enum class CalendarEventSource { GOOGLE_CALENDAR_API, MOCK }

data class CalendarDescriptor(
    val id: CalendarId,
    val displayName: String?,
    val timeZone: ZoneId,
    val colorArgb: Int? = null,
    val visibility: CalendarVisibility = CalendarVisibility.UNKNOWN,
    val readPermission: CalendarReadPermission = CalendarReadPermission.UNKNOWN,
)

sealed interface EventTime {
    data class AllDay(val startDate: LocalDate, val endDateExclusive: LocalDate, val timeZone: ZoneId) : EventTime {
        init { require(endDateExclusive.isAfter(startDate)) { "All-day end date must be exclusive and after start." } }
    }

    data class Timed(val start: Instant, val end: Instant, val timeZone: ZoneId) : EventTime {
        init { require(end.isAfter(start)) { "Event end must be after start." } }
    }
}

sealed interface EventOriginalStart {
    data class AllDay(val date: LocalDate, val timeZone: ZoneId) : EventOriginalStart
    data class Timed(val instant: Instant, val timeZone: ZoneId) : EventOriginalStart
}

data class CalendarEvent(
    val key: CalendarEventKey,
    val title: String?,
    val description: String?,
    val location: String?,
    val time: EventTime?,
    val status: EventStatus,
    val recurringEventId: EventId? = null,
    val originalStartTime: EventOriginalStart? = null,
    val updatedAt: Instant? = null,
    val visibility: EventVisibility = EventVisibility.UNKNOWN,
    val source: CalendarEventSource,
    val dataOrigin: DataOrigin = DataOrigin.CALENDAR,
    val classification: DataClassification = DataClassification.PERSONAL,
) {
    init {
        require(dataOrigin == DataOrigin.CALENDAR) { "Calendar events must retain CALENDAR origin." }
        require(classification != DataClassification.PUBLIC) { "Calendar data cannot be classified PUBLIC automatically." }
        require(status == EventStatus.CANCELLED || time != null) { "Non-cancelled events require a start and end." }
        require(recurringEventId == null || originalStartTime != null || status == EventStatus.CANCELLED) {
            "Expanded recurring instances require their original start time."
        }
    }

    val isAllDay: Boolean get() = time is EventTime.AllDay

    /** Stable recurrence identity, independent of a rescheduled instance's current start. */
    fun stableIdentity(): String = if (recurringEventId != null && originalStartTime != null) {
        "${key.calendarId.value}:recurring:${recurringEventId.value}:${originalStartTime.identityPart()}"
    } else "${key.calendarId.value}:${key.eventId.value}"
}

private fun EventOriginalStart.identityPart(): String = when (this) {
    is EventOriginalStart.AllDay -> "D:$date@$timeZone"
    is EventOriginalStart.Timed -> "T:$instant@$timeZone"
}

data class CalendarDateRange(val start: Instant, val endExclusive: Instant) {
    init { require(endExclusive.isAfter(start)) { "Date range must be non-empty and increasing." } }

    fun overlaps(event: CalendarEvent): Boolean {
        val time = event.time ?: return false
        val (eventStart, eventEnd) = when (time) {
            is EventTime.Timed -> time.start to time.end
            is EventTime.AllDay -> time.startDate.atStartOfDay(time.timeZone).toInstant() to
                time.endDateExclusive.atStartOfDay(time.timeZone).toInstant()
        }
        return eventEnd.isAfter(start) && eventStart.isBefore(endExclusive)
    }
}

/** OAuth is deliberately not implemented. MOCK_ONLY is never an authenticated state. */
enum class CalendarAuthorizationState {
    NOT_CONFIGURED, NOT_CONNECTED, AUTHORIZATION_REQUIRED, AUTHORIZED, DENIED,
    REVOKED, EXPIRED, ERROR, MOCK_ONLY,
}

data class CalendarAuthorizationStatus(
    val state: CalendarAuthorizationState,
    val missingScopes: Set<String> = emptySet(),
    val detailCode: String? = null,
)

enum class CalendarApiFailureCode {
    NOT_FOUND, PERMISSION_DENIED, MISSING_SCOPE, RATE_LIMITED, TIMEOUT,
    CANCELLED, INVALID_RESPONSE, TOO_MANY_PAGES, TOO_MANY_EVENTS,
    PARTIAL_FAILURE, SYNC_TOKEN_EXPIRED, UNAVAILABLE,
}

class CalendarReadFailure(val code: CalendarApiFailureCode, message: String) : RuntimeException(message)
