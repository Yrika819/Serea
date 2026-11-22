package app.serea.android

import app.serea.android.calendar.CalendarProviderSession
import app.serea.android.calendar.CalendarProviderSessions
import app.serea.android.calendar.FakeCalendarProvider
import app.serea.android.gmail.FakeGmailProvider
import app.serea.android.gmail.GmailProviderSession
import app.serea.android.gmail.GmailProviderSessions
import app.serea.android.handoff.GoogleTrustedHandoffCoordinator
import app.serea.android.handoff.trustedHandoffSource
import androidx.lifecycle.ViewModel
import java.time.Clock
import java.util.concurrent.atomic.AtomicBoolean

/** Activity configuration-retained composition root. The application factory selects only synthetic providers. */
internal class AppProviderComposition private constructor(
    val gmail: GmailProviderSession,
    val calendar: CalendarProviderSession,
    val calendarClock: Clock,
) : AutoCloseable {
    private val closed = AtomicBoolean(false)
    /** Session-scoped, in-memory capabilities; previews are not saved across Activity recreation. */
    val gmailHandoff = GoogleTrustedHandoffCoordinator(gmail.trustedHandoffSource())
    val calendarHandoff = GoogleTrustedHandoffCoordinator(calendar.trustedHandoffSource())

    override fun close() {
        if (!closed.compareAndSet(false, true)) return
        gmail.close()
        calendar.close()
    }

    companion object {
        fun fakeDefault(): AppProviderComposition {
            val calendarFake = FakeCalendarProvider(
                events = FakeCalendarProvider.Fixtures.appShellEvents,
                pageSize = 50,
            )
            val gmailSessions = GmailProviderSessions(fakeProvider = { FakeGmailProvider() })
            val calendarSessions = CalendarProviderSessions(fakeProvider = { calendarFake })
            return AppProviderComposition(
                gmail = gmailSessions.openDefault(),
                calendar = calendarSessions.openDefault(),
                calendarClock = calendarFake.clock,
            )
        }
    }
}

/** The screen's provider and handoff capability always come from the same calendar session. */
internal data class CalendarRouteComposition(
    val session: CalendarProviderSession,
    val clock: Clock,
    val handoffCoordinator: GoogleTrustedHandoffCoordinator,
)

internal fun AppProviderComposition.calendarRoute(
    privateCalendarSession: CalendarProviderSession?,
): CalendarRouteComposition = if (privateCalendarSession == null) {
    CalendarRouteComposition(calendar, calendarClock, calendarHandoff)
} else {
    CalendarRouteComposition(
        session = privateCalendarSession,
        clock = privateCalendarSession.clock,
        handoffCoordinator = GoogleTrustedHandoffCoordinator(privateCalendarSession.trustedHandoffSource()),
    )
}

/**
 * Owns provider sessions for the Activity ViewModelStore lifetime. Activity instances are recreated
 * during configuration changes while their ViewModels survive; keeping the sessions at that same
 * lifetime prevents retained Gmail/Calendar ViewModels from calling a session closed by the old Activity.
 */
internal class AppProviderCompositionViewModel : ViewModel() {
    val providers: AppProviderComposition = AppProviderComposition.fakeDefault()

    override fun onCleared() {
        providers.close()
        super.onCleared()
    }
}
