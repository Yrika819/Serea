package app.serea.android.googleauth

import app.serea.android.calendar.CalendarAuthorizationPort
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.calendar.CalendarAuthorizationStatus
import app.serea.android.calendar.CalendarPermissionPolicy
import app.serea.android.calendar.transport.CalendarAccessToken
import app.serea.android.calendar.transport.CalendarAccessTokenPort

/**
 * Bridges the shared Google authorization foundation to Calendar's read-only ports.
 * CalendarList scope stays optional: event reads ask only for `calendar.events.readonly`.
 * Mock gateways remain MOCK_ONLY and cannot authorize the real Google HTTP transport.
 */
class GoogleCalendarAuthorizationAdapter(
    private val gateway: GoogleAuthorizationGateway,
) : CalendarAuthorizationPort, CalendarAccessTokenPort {
    private val calendarScopes = setOf(
        GoogleScope.CALENDAR_EVENTS_READ_ONLY,
        GoogleScope.CALENDAR_LIST_READ_ONLY,
    )

    override suspend fun status(): CalendarAuthorizationStatus {
        val googleStatus = gateway.status(calendarScopes)
        val missingScopes = calendarScopes
            .filter { googleStatus.permissions[it] != ScopePermission.GRANTED }
            .mapTo(linkedSetOf()) { it.value }
        val eventsPermission = googleStatus.permissions[GoogleScope.CALENDAR_EVENTS_READ_ONLY]
            ?: ScopePermission.NOT_REQUESTED
        val state = when {
            gateway.mode == GoogleAuthorizationMode.MOCK_ONLY -> CalendarAuthorizationState.MOCK_ONLY
            googleStatus.failure == AuthorizationFailure.NOT_CONFIGURED -> CalendarAuthorizationState.NOT_CONFIGURED
            !gateway.isEnabledInApp(GoogleFeature.CALENDAR) -> CalendarAuthorizationState.FEATURE_DISABLED
            googleStatus.service == GoogleServiceState.MISSING ||
                googleStatus.failure == AuthorizationFailure.SERVICE_MISSING -> CalendarAuthorizationState.NOT_CONNECTED
            googleStatus.service == GoogleServiceState.OUTDATED ||
                googleStatus.failure == AuthorizationFailure.SERVICE_OUTDATED -> CalendarAuthorizationState.ERROR
            eventsPermission == ScopePermission.GRANTED -> CalendarAuthorizationState.AUTHORIZED
            eventsPermission == ScopePermission.DENIED || googleStatus.failure == AuthorizationFailure.DENIED -> CalendarAuthorizationState.DENIED
            eventsPermission == ScopePermission.EXPIRED || googleStatus.failure == AuthorizationFailure.EXPIRED -> CalendarAuthorizationState.EXPIRED
            else -> CalendarAuthorizationState.AUTHORIZATION_REQUIRED
        }
        return CalendarAuthorizationStatus(state, missingScopes, googleStatus.failure?.name)
    }

    override suspend fun accessToken(scope: String): CalendarAccessToken? {
        if (gateway.mode != GoogleAuthorizationMode.PLAY_SERVICES || !gateway.isEnabledInApp(GoogleFeature.CALENDAR)) return null
        val googleScope = when (scope) {
            CalendarPermissionPolicy.EVENTS_READ_ONLY -> GoogleScope.CALENDAR_EVENTS_READ_ONLY
            CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY -> GoogleScope.CALENDAR_LIST_READ_ONLY
            else -> return null
        }
        val result = gateway.acquire(GoogleFeature.CALENDAR, setOf(googleScope))
        val lease = (result as? TokenSupplyResult.Available)?.lease ?: return null
        // Keep the authorization lease alive until the transport has built and sent its
        // request. Copying the bearer value here would let a disabled feature retain it.
        return CalendarAccessToken(lease)
    }
}
