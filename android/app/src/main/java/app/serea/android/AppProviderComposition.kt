package app.serea.android

import app.serea.android.calendar.CalendarProviderSession
import app.serea.android.calendar.CalendarProviderSessions
import app.serea.android.calendar.FakeCalendarProvider
import app.serea.android.gmail.FakeGmailProvider
import app.serea.android.gmail.GmailProviderSession
import app.serea.android.gmail.GmailProviderSessions
import java.time.Clock
import java.util.concurrent.atomic.AtomicBoolean

/** Activity-scoped composition root. The application factory currently selects only synthetic providers. */
internal class AppProviderComposition private constructor(
    val gmail: GmailProviderSession,
    val calendar: CalendarProviderSession,
    val calendarClock: Clock,
) : AutoCloseable {
    private val closed = AtomicBoolean(false)

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
