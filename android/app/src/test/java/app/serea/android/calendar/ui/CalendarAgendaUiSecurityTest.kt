package app.serea.android.calendar.ui

import app.serea.android.DataClassification
import app.serea.android.DataOrigin
import app.serea.android.AppProviderComposition
import app.serea.android.calendar.CalendarAuthorizationPort
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.calendar.CalendarReadPort
import app.serea.android.calendar.CalendarPermissionPolicy
import app.serea.android.calendar.FakeCalendarProvider
import app.serea.android.gmail.GmailProviderKind
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CalendarAgendaUiSecurityTest {
    @Test fun screenUsesInjectedReadOnlyPortsAndCompositionRootDefaultsToFakes() {
        val screenClass = Class.forName("app.serea.android.calendar.ui.CalendarAgendaScreenKt")
        val screen = screenClass.declaredMethods.first { it.name == "CalendarAgendaScreen" }
        assertTrue(screen.parameterTypes.any { it == CalendarReadPort::class.java })
        assertTrue(screen.parameterTypes.any { it == CalendarAuthorizationPort::class.java })

        val methods = CalendarAgendaUiController::class.java.declaredMethods.map { it.name.lowercase() }
        assertFalse(methods.any { it.contains("create") || it.contains("update") || it.contains("delete") || it.contains("write") })

        val providers = AppProviderComposition.fakeDefault()
        try {
            assertTrue(providers.gmail.kind == GmailProviderKind.FAKE)
            assertTrue(providers.calendar.kind == CalendarProviderKind.FAKE)
        } finally {
            providers.close()
        }
    }

    @Test fun calendarContentRetainsPrivateCalendarOriginAndCannotEnterModelEgressPolicy() {
        val event = FakeCalendarProvider.Fixtures.events.first()
        assertTrue(event.dataOrigin == DataOrigin.CALENDAR)
        assertTrue(event.classification != DataClassification.PUBLIC)
        assertFalse(CalendarPermissionPolicy.canSendToExternalModel(event.dataOrigin, DataClassification.PUBLIC))
    }
}
