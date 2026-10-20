package app.serea.android.calendar.ui

import app.serea.android.DataClassification
import app.serea.android.DataOrigin
import app.serea.android.calendar.CalendarPermissionPolicy
import app.serea.android.calendar.FakeCalendarProvider
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class CalendarAgendaUiSecurityTest {
    @Test fun screenEntryPointIsBoundToTheFakeProviderAndAddsNoCalendarWrites() {
        val screenClass = Class.forName("app.serea.android.calendar.ui.CalendarAgendaScreenKt")
        val screen = screenClass.declaredMethods.first { it.name == "CalendarAgendaScreen" }
        assertTrue(screen.parameterTypes.any { it == FakeCalendarProvider::class.java })

        val methods = CalendarAgendaUiController::class.java.declaredMethods.map { it.name.lowercase() }
        assertFalse(methods.any { it.contains("create") || it.contains("update") || it.contains("delete") || it.contains("write") })
    }

    @Test fun calendarContentRetainsPrivateCalendarOriginAndCannotEnterModelEgressPolicy() {
        val event = FakeCalendarProvider.Fixtures.events.first()
        assertTrue(event.dataOrigin == DataOrigin.CALENDAR)
        assertTrue(event.classification != DataClassification.PUBLIC)
        assertFalse(CalendarPermissionPolicy.canSendToExternalModel(event.dataOrigin, DataClassification.PUBLIC))
    }
}
