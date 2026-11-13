package app.serea.android.googleauth

import app.serea.android.calendar.CalendarProviderSession
import app.serea.android.calendar.CalendarProviderSessions
import app.serea.android.calendar.FakeCalendarProvider

/** Private authorization preview composition. Live Google HTTP is intentionally not selectable. */
internal class PrivateGoogleCalendarComposition : AutoCloseable {
    private val sessions = CalendarProviderSessions(
        fakeProvider = { FakeCalendarProvider.Fixtures.privatePrimaryProvider() },
        googleHttpEnabledByOwner = false,
    )
    private var current: CalendarProviderSession? = null

    fun openAuthorizedPreview(gateway: GoogleAuthorizationGateway): CalendarProviderSession {
        check(gateway.status(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY))
            .permits(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY))) {
            "Primary Calendar read-only authorization is not active."
        }
        current?.close()
        return sessions.openPrimaryFake().also { session ->
            check(session.kind == app.serea.android.calendar.CalendarProviderKind.FAKE) {
                "Private preview must stay on the Fake Calendar provider."
            }
            check(session.primaryCalendarOnly) { "Private preview must be primary-only." }
            current = session
        }
    }

    fun closeCurrent() {
        current?.close()
        current = null
    }

    override fun close() = closeCurrent()
}
