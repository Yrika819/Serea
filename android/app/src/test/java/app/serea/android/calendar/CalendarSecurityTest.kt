package app.serea.android.calendar

import app.serea.android.ChatContextBuilder
import app.serea.android.ChatEgressGate
import app.serea.android.ContextMessage
import app.serea.android.DataClassification
import app.serea.android.DataOrigin
import app.serea.android.EgressContract
import app.serea.android.EgressContractDecision
import app.serea.android.MessageRole
import app.serea.android.PayloadPreparation
import java.time.Instant
import java.time.ZoneId
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test

class CalendarSecurityTest {
    @Test fun calendarCannotBecomePublicOrLeaveForModelAndRetainsOrigin() {
        val event = FakeCalendarProvider.Fixtures.events.first()
        CalendarPermissionPolicy.assertCalendarEventSafe(event)
        assertEquals(DataOrigin.CALENDAR, event.dataOrigin)
        assertNotEquals(DataClassification.PUBLIC, event.classification)
        assertFalse(CalendarPermissionPolicy.canSendToExternalModel(event.dataOrigin, DataClassification.PUBLIC))
        assertFalse(CalendarPermissionPolicy.canSendToExternalModel(DataOrigin.UNKNOWN, DataClassification.PUBLIC))

        val context = listOf(ContextMessage(MessageRole.USER, "Fictional event title", DataClassification.PUBLIC, "calendar-event",
            origin = DataOrigin.CALENDAR, conversationId = "conversation"))
        val contract = EgressContract("conversation", "send", DataOrigin.CALENDAR, DataClassification.PUBLIC,
            "https://ollama.com/api/chat", "Fictional event title", context, PayloadPreparation.RAW,
            explicitSendIntentAtMillis = 1_000, expiresAtMillis = 2_000)
        val result = ChatEgressGate.evaluate(contract, 1_500)
        assertEquals(EgressContractDecision.ORIGIN_DENIED, result.decision)
        assertNull(result.payload)
        assertTrue(ChatContextBuilder.build(emptyList()).isEmpty())
    }

    @Test fun fakeAuthorizationIsClearlyMockOnlyAndNeverReportsAuthorized() {
        val fake = FakeCalendarProvider()
        assertEquals(CalendarAuthorizationState.MOCK_ONLY, kotlinx.coroutines.runBlocking { fake.status().state })
        assertFalse(fake.containsCredentialState())
        assertTrue(CalendarPermissionPolicy.mayReadEvents(CalendarAuthorizationStatus(CalendarAuthorizationState.MOCK_ONLY)))
        assertFalse(CalendarPermissionPolicy.mayReadEvents(CalendarAuthorizationStatus(CalendarAuthorizationState.NOT_CONNECTED)))
        assertFalse(CalendarPermissionPolicy.mayReadEvents(CalendarAuthorizationStatus(CalendarAuthorizationState.DENIED)))
        assertFalse(CalendarPermissionPolicy.mayReadEvents(CalendarAuthorizationStatus(CalendarAuthorizationState.REVOKED)))
        assertFalse(CalendarPermissionPolicy.mayReadEvents(CalendarAuthorizationStatus(CalendarAuthorizationState.MOCK_ONLY,
            missingScopes = setOf(CalendarPermissionPolicy.EVENTS_READ_ONLY))))
    }

    @Test fun onlyNarrowReadScopesAreRequestedAndCalendarListIsOptional() {
        assertEquals(setOf(CalendarPermissionPolicy.EVENTS_READ_ONLY), CalendarPermissionPolicy.requiredScopes(false))
        assertEquals(setOf(CalendarPermissionPolicy.EVENTS_READ_ONLY, CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY),
            CalendarPermissionPolicy.requiredScopes(true))
        assertEquals(setOf(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY),
            CalendarPermissionPolicy.missingScopes(setOf(CalendarPermissionPolicy.EVENTS_READ_ONLY), true))
    }

    @Test fun permissionCheckedPortRejectsNotConnectedAndAllowsMockOnly() = runBlocking {
        val fake = FakeCalendarProvider()
        val noPermission = object : CalendarAuthorizationPort {
            override suspend fun status() = CalendarAuthorizationStatus(CalendarAuthorizationState.NOT_CONNECTED)
        }
        try { PermissionCheckedCalendarReadPort(fake, noPermission).listEventsPage(
            CalendarId("demo-primary"), CalendarDateRange(Instant.EPOCH, Instant.EPOCH.plusSeconds(60)))
            fail("Expected not-connected rejection")
        } catch (expected: CalendarReadFailure) { assertEquals(CalendarApiFailureCode.PERMISSION_DENIED, expected.code) }
        val mock = PermissionCheckedCalendarReadPort(fake, fake)
        assertNotNull(mock.getEvent(FakeCalendarProvider.Fixtures.events.first().key))
    }

    @Test fun noCredentialTypesOrMutationMethodsExistInCalendarReadPortContract() {
        val methods = CalendarReadPort::class.java.methods.map { it.name }.toSet()
        assertTrue(methods.containsAll(setOf("listCalendars", "getCalendar", "getEvent", "listEventsPage")))
        assertFalse(methods.any { it.contains("create", true) || it.contains("update", true) || it.contains("delete", true) || it.contains("write", true) })
        assertTrue(CalendarEvent::class.java.declaredFields.none { it.name.contains("token", true) || it.name.contains("credential", true) })
    }
}
