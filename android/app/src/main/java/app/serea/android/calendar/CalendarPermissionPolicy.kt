package app.serea.android.calendar

import app.serea.android.DataClassification
import app.serea.android.DataOrigin

/** Least-privilege OAuth plan only; this module does not initiate OAuth or hold credentials. */
object CalendarPermissionPolicy {
    const val EVENTS_READ_ONLY = "https://www.googleapis.com/auth/calendar.events.readonly"
    const val CALENDAR_LIST_READ_ONLY = "https://www.googleapis.com/auth/calendar.calendarlist.readonly"

    fun requiredScopes(listCalendars: Boolean): Set<String> = buildSet {
        add(EVENTS_READ_ONLY)
        if (listCalendars) add(CALENDAR_LIST_READ_ONLY)
    }

    fun mayReadEvents(status: CalendarAuthorizationStatus): Boolean =
        (status.state == CalendarAuthorizationState.AUTHORIZED || status.state == CalendarAuthorizationState.MOCK_ONLY) &&
            EVENTS_READ_ONLY !in status.missingScopes
    fun mayListCalendars(status: CalendarAuthorizationStatus): Boolean =
        mayReadEvents(status) && CALENDAR_LIST_READ_ONLY !in status.missingScopes

    fun missingScopes(grantedScopes: Set<String>, listCalendars: Boolean): Set<String> =
        requiredScopes(listCalendars) - grantedScopes

    /** A calendar read grant never conveys AI egress authority. */
    fun canSendToExternalModel(origin: DataOrigin, classification: DataClassification): Boolean =
        origin != DataOrigin.CALENDAR && classification == DataClassification.PUBLIC

    fun assertCalendarEventSafe(event: CalendarEvent) {
        require(event.dataOrigin == DataOrigin.CALENDAR)
        require(event.classification != DataClassification.PUBLIC)
        require(event.source == CalendarEventSource.MOCK || event.source == CalendarEventSource.GOOGLE_CALENDAR_API)
    }
}

/** Enforces the read scope boundary around any future transport adapter. */
class PermissionCheckedCalendarReadPort(
    private val delegate: CalendarReadPort,
    private val authorization: CalendarAuthorizationPort,
) : CalendarReadPort {
    override suspend fun listCalendars(): List<CalendarDescriptor> {
        check(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY)
        return delegate.listCalendars()
    }

    override suspend fun getCalendar(calendarId: CalendarId): CalendarDescriptor {
        check(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY)
        return delegate.getCalendar(calendarId)
    }

    override suspend fun getEvent(key: CalendarEventKey): CalendarEvent {
        check(CalendarPermissionPolicy.EVENTS_READ_ONLY)
        return delegate.getEvent(key)
    }

    override suspend fun listEventsPage(calendarId: CalendarId, range: CalendarDateRange, pageToken: String?): CalendarEventPage {
        check(CalendarPermissionPolicy.EVENTS_READ_ONLY)
        return delegate.listEventsPage(calendarId, range, pageToken)
    }

    private suspend fun check(scope: String) {
        val status = authorization.status()
        if (status.state != CalendarAuthorizationState.AUTHORIZED && status.state != CalendarAuthorizationState.MOCK_ONLY) {
            throw CalendarReadFailure(CalendarApiFailureCode.PERMISSION_DENIED, "Calendar read permission is unavailable.")
        }
        if (scope in status.missingScopes) {
            throw CalendarReadFailure(CalendarApiFailureCode.MISSING_SCOPE, "Required Calendar read scope is missing.")
        }
    }
}
