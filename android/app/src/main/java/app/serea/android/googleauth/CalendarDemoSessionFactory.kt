package app.serea.android.googleauth

import app.serea.android.calendar.CalendarProviderSession
import app.serea.android.calendar.CalendarProviderSessions

/** Test seam invoked only after the synthetic Calendar consent action succeeds. */
internal fun interface CalendarDemoSessionFactory {
    fun openAfterSyntheticConsent(account: GoogleDemoAccount, accountGeneration: Int): CalendarProviderSession
}

/** Shipped Settings composition is permanently Fake-backed and cannot construct an HTTP port. */
internal class FakeCalendarDemoSessionFactory(
    private val sessions: CalendarProviderSessions = CalendarProviderSessions(),
) : CalendarDemoSessionFactory {
    override fun openAfterSyntheticConsent(
        account: GoogleDemoAccount,
        accountGeneration: Int,
    ): CalendarProviderSession {
        // These bindings are consumed by test factories; the Fake session carries MOCK_ONLY and
        // has no credential or network capability, regardless of the selected synthetic account.
        require(account.displayName.isNotBlank() && accountGeneration > 0)
        return sessions.openFake()
    }
}
