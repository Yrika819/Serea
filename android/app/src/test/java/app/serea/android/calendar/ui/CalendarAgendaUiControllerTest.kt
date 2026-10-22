package app.serea.android.calendar.ui

import app.serea.android.DataClassification
import app.serea.android.calendar.CalendarApiFailureCode
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.calendar.CalendarAuthorizationStatus
import app.serea.android.calendar.CalendarPermissionPolicy
import app.serea.android.calendar.EventOriginalStart
import app.serea.android.calendar.EventStatus
import app.serea.android.calendar.FakeCalendarProvider
import app.serea.android.calendar.CalendarAgendaService
import app.serea.android.calendar.CalendarId
import app.serea.android.calendar.EventId
import app.serea.android.calendar.CalendarEventKey
import java.time.Clock
import java.time.Duration
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withContext
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class CalendarAgendaUiControllerTest {
    @Test fun providerCancellationEndsLoadingAndCanBeRetried() = runTest {
        val provider = fakeProvider(
            listOf(calendar("demo")),
            listOf(timedEvent("demo", "event", "2026-10-20T13:00:00Z", "2026-10-20T14:00:00Z")),
        )
        var cancelFirstPage = true
        val fake = CalendarAgendaUiFake(provider, beforePage = { _, _, _ ->
            if (cancelFirstPage) {
                cancelFirstPage = false
                throw CancellationException("Fictional provider cancellation")
            }
        })
        val controller = CalendarAgendaUiController(fake, fake, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        runCurrent()

        assertFailure(controller, CalendarAgendaFailure.UNAVAILABLE)
        controller.retry()
        advanceUntilIdle()
        assertEquals(listOf("event"), agenda(controller).events.map { it.key.eventId.value })
        controller.close()
    }

    @Test fun detailProviderCancellationEndsLoadingAndCanBeRetried() = runTest {
        val event = timedEvent("demo", "detail-retry", "2026-10-20T13:00:00Z", "2026-10-20T14:00:00Z")
        val provider = fakeProvider(listOf(calendar("demo")), listOf(event))
        var cancelFirstDetail = true
        val fake = CalendarAgendaUiFake(provider, beforeGetEvent = {
            if (cancelFirstDetail) {
                cancelFirstDetail = false
                throw CancellationException("Fictional detail provider cancellation")
            }
        })
        val controller = CalendarAgendaUiController(fake, fake, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()

        controller.openEvent(event.key)
        advanceUntilIdle()
        val failure = assertFailure(controller, CalendarAgendaFailure.UNAVAILABLE)
        assertEquals(event.key, failure.detailKey)

        controller.retry()
        advanceUntilIdle()
        val detail = controller.state.value as CalendarAgendaUiState.EventDetail
        assertEquals(event.key, detail.event.key)
        controller.close()
    }

    @Test fun choosingSpecifiedDateSeedsAnEmptyDateFieldFromInjectedClockAndZone() {
        val beforeTokyoMidnight = Clock.fixed(Instant.parse("2026-10-20T15:30:00Z"), ZoneId.of("UTC"))
        assertEquals("2026-10-21", normalizeSpecifiedDateInput("", beforeTokyoMidnight, ZoneId.of("Asia/Tokyo")))
        assertEquals("2026-11-02", normalizeSpecifiedDateInput("2026-11-02", beforeTokyoMidnight, ZoneId.of("UTC")))
        assertEquals("2026-10-20", normalizeSpecifiedDateInput("not-a-date", beforeTokyoMidnight, ZoneId.of("UTC")))
    }

    @Test fun todayTomorrowUpcomingAndSelectedDateUseCalendarDayQueries() = runTest {
        val descriptor = calendar("demo")
        val events = listOf(
            timedEvent("demo", "today-past", "2026-10-20T10:00:00Z", "2026-10-20T10:30:00Z"),
            timedEvent("demo", "today-later", "2026-10-20T13:00:00Z", "2026-10-20T13:30:00Z"),
            timedEvent("demo", "tomorrow", "2026-10-21T09:00:00Z", "2026-10-21T09:30:00Z"),
            timedEvent("demo", "specified-date", "2026-10-22T09:00:00Z", "2026-10-22T09:30:00Z"),
        )
        val provider = fakeProvider(listOf(descriptor), events)
        val uiFake = CalendarAgendaUiFake(provider)
        val controller = CalendarAgendaUiController(uiFake, uiFake, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertEquals(setOf("today-past", "today-later"), agenda(controller).events.map { it.key.eventId.value }.toSet())

        controller.selectMode(CalendarAgendaMode.TOMORROW)
        advanceUntilIdle()
        assertEquals(listOf("tomorrow"), agenda(controller).events.map { it.key.eventId.value })

        controller.selectMode(CalendarAgendaMode.UPCOMING)
        advanceUntilIdle()
        assertEquals(setOf("today-later", "tomorrow", "specified-date"), agenda(controller).events.map { it.key.eventId.value }.toSet())

        controller.selectDate(LocalDate.parse("2026-10-22"))
        advanceUntilIdle()
        assertEquals(CalendarAgendaMode.DATE, agenda(controller).query.mode)
        assertEquals(listOf("specified-date"), agenda(controller).events.map { it.key.eventId.value })
        controller.close()
    }

    @Test fun selectingCalendarThenUpcomingKeepsLatestScopeAndLoadsPagedEventFifty() = runTest {
        val provider = FakeCalendarProvider(
            events = FakeCalendarProvider.Fixtures.appShellEvents,
            pageSize = 50,
        )
        val uiFake = CalendarAgendaUiFake(provider)
        val controller = CalendarAgendaUiController(
            uiFake,
            uiFake,
            UI_TEST_CLOCK,
            ZoneId.of("UTC"),
            this,
        )
        advanceUntilIdle()

        controller.selectCalendar("demo-work")
        controller.selectMode(CalendarAgendaMode.UPCOMING)
        advanceUntilIdle()

        val state = agenda(controller)
        assertEquals(CalendarAgendaMode.UPCOMING, state.query.mode)
        assertEquals("demo-work", state.selectedCalendarId)
        assertEquals(72, state.events.count { it.key.eventId.value.startsWith("bulk-") })
        assertEquals(
            "bulk-50",
            state.events.single { it.title == "Fictional paged event 50" }.key.eventId.value,
        )
        assertTrue(state.events.zipWithNext().all { (first, second) ->
            CalendarAgendaService.EVENT_ORDER.compare(first, second) <= 0
        })
        controller.close()
    }

    @Test fun emptyAgendaAndEmptyCalendarListAreDistinctSuccessfulStates() = runTest {
        val emptyAgendaProvider = fakeProvider(listOf(calendar("empty")), emptyList())
        val emptyAgendaFake = CalendarAgendaUiFake(emptyAgendaProvider)
        val emptyAgenda = CalendarAgendaUiController(emptyAgendaFake, emptyAgendaFake, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertTrue(agenda(emptyAgenda).events.isEmpty())
        assertEquals(1, agenda(emptyAgenda).calendars.size)

        val noCalendarsProvider = fakeProvider(emptyList(), emptyList())
        val noCalendarsFake = CalendarAgendaUiFake(noCalendarsProvider)
        val noCalendars = CalendarAgendaUiController(noCalendarsFake, noCalendarsFake, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertTrue(agenda(noCalendars).calendars.isEmpty())
        assertTrue(agenda(noCalendars).events.isEmpty())
        emptyAgenda.close()
        noCalendars.close()
    }

    @Test fun selectedCalendarFiltersResultsAndPaginationDeduplicatesInReverseOrder() = runTest {
        val calendars = listOf(calendar("a", "予定A"), calendar("b", "予定B"))
        val events = listOf(
            timedEvent("a", "a-1", "2026-10-20T13:00:00Z", "2026-10-20T13:15:00Z"),
            timedEvent("a", "a-2", "2026-10-20T14:00:00Z", "2026-10-20T14:15:00Z"),
            timedEvent("a", "a-3", "2026-10-20T15:00:00Z", "2026-10-20T15:15:00Z"),
            timedEvent("b", "b-1", "2026-10-20T13:05:00Z", "2026-10-20T13:20:00Z"),
        )
        val provider = fakeProvider(
            calendars, events, pageSize = 1,
            options = FakeCalendarProvider.Options(duplicateFirstEventAcrossPages = true, reverseEventOrder = true),
        )
        val uiFake = CalendarAgendaUiFake(provider)
        val controller = CalendarAgendaUiController(
            uiFake, uiFake, UI_TEST_CLOCK, ZoneId.of("UTC"), this,
            initialCalendarId = "a",
        )
        advanceUntilIdle()
        val a = agenda(controller)
        assertEquals("a", a.selectedCalendarId)
        assertEquals(setOf("a-1", "a-2", "a-3"), a.events.map { it.key.eventId.value }.toSet())
        assertEquals(a.events.size, a.events.map { it.stableIdentity() }.toSet().size)
        assertTrue(uiFake.pageCalendarIds.isNotEmpty())
        assertTrue(uiFake.pageCalendarIds.all { it == "a" })
        assertTrue(a.events.zipWithNext().all { (left, right) -> CalendarAgendaService.EVENT_ORDER.compare(left, right) <= 0 })

        controller.selectCalendar("b")
        advanceUntilIdle()
        assertEquals("b", agenda(controller).selectedCalendarId)
        assertEquals(listOf("b-1"), agenda(controller).events.map { it.key.eventId.value })
        assertTrue(uiFake.pageCalendarIds.all { it == "a" || it == "b" })
        controller.close()
    }

    @Test fun calendarAThenBDiscardsLateResultFromCancelledRequest() = runTest {
        val firstPageStarted = CompletableDeferred<Unit>()
        val releaseOldPage = CompletableDeferred<Unit>()
        val provider = fakeProvider(
            listOf(calendar("a"), calendar("b")),
            listOf(
                timedEvent("a", "from-a", "2026-10-20T13:00:00Z", "2026-10-20T13:30:00Z"),
                timedEvent("b", "from-b", "2026-10-20T14:00:00Z", "2026-10-20T14:30:00Z"),
            ),
        )
        val uiFake = CalendarAgendaUiFake(provider, beforePage = { index, _, _ ->
            if (index == 0) {
                firstPageStarted.complete(Unit)
                withContext(NonCancellable) { releaseOldPage.await() }
            }
        })
        val controller = CalendarAgendaUiController(
            uiFake, uiFake, UI_TEST_CLOCK, ZoneId.of("UTC"), this,
            initialCalendarId = "a",
        )
        runCurrent()
        firstPageStarted.await()
        controller.selectCalendar("b")
        runCurrent()
        assertEquals("b", agenda(controller).selectedCalendarId)
        assertEquals(listOf("from-b"), agenda(controller).events.map { it.key.eventId.value })

        releaseOldPage.complete(Unit)
        runCurrent()
        assertEquals("b", agenda(controller).selectedCalendarId)
        assertEquals(listOf("from-b"), agenda(controller).events.map { it.key.eventId.value })
        controller.close()
    }

    @Test fun mockOnlyIsVisibleAndPermissionFailuresNeverBecomeSuccess() = runTest {
        val descriptor = calendar("demo")
        val provider = fakeProvider(listOf(descriptor), listOf(timedEvent("demo", "private-title", "2026-10-20T13:00:00Z", "2026-10-20T14:00:00Z")))
        val mock = CalendarAgendaUiFake(provider)
        val mockController = CalendarAgendaUiController(mock, mock, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertTrue(agenda(mockController).mockOnly)
        assertTrue(agenda(mockController).events.single().dataOrigin == app.serea.android.DataOrigin.CALENDAR)

        val denied = CalendarAgendaUiFake(provider, CalendarAuthorizationStatus(CalendarAuthorizationState.DENIED))
        val deniedController = CalendarAgendaUiController(denied, denied, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertFailure(deniedController, CalendarAgendaFailure.DENIED)

        val expired = CalendarAgendaUiFake(provider, CalendarAuthorizationStatus(CalendarAuthorizationState.EXPIRED))
        val expiredController = CalendarAgendaUiController(expired, expired, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertFailure(expiredController, CalendarAgendaFailure.EXPIRED)

        val disconnected = CalendarAgendaUiFake(provider, CalendarAuthorizationStatus(CalendarAuthorizationState.NOT_CONNECTED))
        val disconnectedController = CalendarAgendaUiController(disconnected, disconnected, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertFailure(disconnectedController, CalendarAgendaFailure.PERMISSION_REQUIRED)

        val missingScope = CalendarAgendaUiFake(
            provider,
            CalendarAuthorizationStatus(
                CalendarAuthorizationState.MOCK_ONLY,
                setOf(CalendarPermissionPolicy.EVENTS_READ_ONLY),
            ),
        )
        val scopeController = CalendarAgendaUiController(missingScope, missingScope, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertFailure(scopeController, CalendarAgendaFailure.MISSING_SCOPE)

        val missingCalendarListScope = CalendarAgendaUiFake(
            provider,
            CalendarAuthorizationStatus(
                CalendarAuthorizationState.MOCK_ONLY,
                setOf(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY),
            ),
        )
        val listScopeController = CalendarAgendaUiController(
            missingCalendarListScope, missingCalendarListScope, UI_TEST_CLOCK, ZoneId.of("UTC"), this,
        )
        advanceUntilIdle()
        assertFailure(listScopeController, CalendarAgendaFailure.MISSING_SCOPE)

        val deniedProvider = fakeProvider(
            listOf(descriptor), emptyList(), options = FakeCalendarProvider.Options(permissionDenied = true),
        )
        val deniedByPort = CalendarAgendaUiFake(deniedProvider)
        val deniedByPortController = CalendarAgendaUiController(
            deniedByPort, deniedByPort, UI_TEST_CLOCK, ZoneId.of("UTC"), this,
        )
        advanceUntilIdle()
        assertFailure(deniedByPortController, CalendarAgendaFailure.DENIED)

        listOf(
            mockController, deniedController, expiredController, disconnectedController, scopeController,
            listScopeController, deniedByPortController,
        ).forEach { it.close() }
    }

    @Test fun realAuthorizationOrRealSourcedRecordsAreRejectedByTheMockScreen() = runTest {
        val sourceEvent = timedEvent("demo", "source-check", "2026-10-20T13:00:00Z", "2026-10-20T14:00:00Z")
        val provider = fakeProvider(listOf(calendar("demo")), listOf(sourceEvent))
        val authorized = CalendarAgendaUiFake(
            provider,
            CalendarAuthorizationStatus(CalendarAuthorizationState.AUTHORIZED),
        )
        val authorizedController = CalendarAgendaUiController(authorized, authorized, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertFailure(authorizedController, CalendarAgendaFailure.MOCK_ONLY_REQUIRED)

        val realSourcedEvent = sourceEvent.copy(source = app.serea.android.calendar.CalendarEventSource.GOOGLE_CALENDAR_API)
        val realSourceProvider = fakeProvider(listOf(calendar("demo")), listOf(realSourcedEvent))
        val mockAuthorization = CalendarAgendaUiFake(realSourceProvider)
        val realSourceController = CalendarAgendaUiController(
            mockAuthorization, mockAuthorization, UI_TEST_CLOCK, ZoneId.of("UTC"), this,
        )
        advanceUntilIdle()
        assertFailure(realSourceController, CalendarAgendaFailure.MOCK_ONLY_REQUIRED)
        realSourceController.openEvent(realSourcedEvent.key)
        advanceUntilIdle()
        assertFailure(realSourceController, CalendarAgendaFailure.MOCK_ONLY_REQUIRED)
        authorizedController.close()
        realSourceController.close()
    }

    @Test fun timeoutPartialFailureRateLimitAndUnknownTimezoneMapToSafeStates() = runTest {
        val provider = fakeProvider(listOf(calendar("demo")), listOf(timedEvent("demo", "title-must-not-leak", "2026-10-20T13:00:00Z", "2026-10-20T14:00:00Z")))
        val cases = listOf(
            CalendarApiFailureCode.TIMEOUT to CalendarAgendaFailure.TIMEOUT,
            CalendarApiFailureCode.PARTIAL_FAILURE to CalendarAgendaFailure.PARTIAL_FAILURE,
            CalendarApiFailureCode.RATE_LIMITED to CalendarAgendaFailure.RATE_LIMITED,
            CalendarApiFailureCode.TIME_ZONE_UNKNOWN to CalendarAgendaFailure.TIME_ZONE_UNKNOWN,
        )
        cases.forEach { (code, expected) ->
            val uiFake = CalendarAgendaUiFake(provider, firstFailure = code)
            val controller = CalendarAgendaUiController(uiFake, uiFake, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
            advanceUntilIdle()
            val failure = assertFailure(controller, expected)
            val message = CalendarAgendaJapaneseText.safeMessage(failure.reason)
            assertFalse(message.contains("title-must-not-leak"))
            controller.close()
        }
    }

    @Test fun retryRecoversAfterTransientTimeoutAndCloseCancelsWithoutPublishingError() = runTest {
        val provider = fakeProvider(listOf(calendar("demo")), listOf(timedEvent("demo", "event", "2026-10-20T13:00:00Z", "2026-10-20T14:00:00Z")))
        val retryFake = CalendarAgendaUiFake(provider, firstFailure = CalendarApiFailureCode.TIMEOUT)
        val retryController = CalendarAgendaUiController(retryFake, retryFake, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertFailure(retryController, CalendarAgendaFailure.TIMEOUT)
        retryController.retry()
        advanceUntilIdle()
        assertEquals(listOf("event"), agenda(retryController).events.map { it.key.eventId.value })

        val started = CompletableDeferred<Unit>()
        val neverProvider = fakeProvider(listOf(calendar("demo")), listOf(timedEvent("demo", "event", "2026-10-20T13:00:00Z", "2026-10-20T14:00:00Z")))
        val cancellationFake = CalendarAgendaUiFake(neverProvider, beforePage = { _, _, _ ->
            started.complete(Unit)
            awaitCancellation()
        })
        val cancelledController = CalendarAgendaUiController(cancellationFake, cancellationFake, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        runCurrent()
        started.await()
        cancelledController.close()
        runCurrent()
        assertTrue(cancelledController.state.value is CalendarAgendaUiState.Loading)
        retryController.close()
    }

    @Test fun returningToCachedAgendaRejectsLateFailureFromCancelledDetailRequest() = runTest {
        val event = timedEvent("demo", "late-detail", "2026-10-20T13:00:00Z", "2026-10-20T14:00:00Z")
        val provider = fakeProvider(listOf(calendar("demo")), listOf(event))
        val getStarted = CompletableDeferred<Unit>()
        val releaseGet = CompletableDeferred<Unit>()
        val fake = CalendarAgendaUiFake(
            provider,
            beforeGetEvent = {
                getStarted.complete(Unit)
                withContext(NonCancellable) { releaseGet.await() }
            },
            getEventFailure = CalendarApiFailureCode.UNAVAILABLE,
        )
        val controller = CalendarAgendaUiController(fake, fake, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        val cachedAgenda = agenda(controller)

        controller.openEvent(event.key)
        runCurrent()
        getStarted.await()
        controller.backToAgenda()
        assertEquals(cachedAgenda, controller.state.value)

        releaseGet.complete(Unit)
        advanceUntilIdle()
        assertEquals(cachedAgenda, controller.state.value)
        controller.close()
    }

    @Test fun todayAndTomorrowChangeAtExplicitUtcTokyoAndDstLocalDayBoundaries() = runTest {
        val transitionClock = Clock.fixed(Instant.parse("2026-10-20T15:30:00Z"), ZoneId.of("UTC"))
        val provider = fakeProvider(
            listOf(calendar("demo")),
            listOf(
                timedEvent("demo", "near-midnight", "2026-10-21T00:00:00Z", "2026-10-21T00:30:00Z"),
                timedEvent("demo", "next-local-day", "2026-10-22T00:00:00Z", "2026-10-22T00:30:00Z"),
            ),
        )
        val utcFake = CalendarAgendaUiFake(provider)
        val utc = CalendarAgendaUiController(utcFake, utcFake, transitionClock, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertTrue(agenda(utc).events.isEmpty())
        utc.selectMode(CalendarAgendaMode.TOMORROW)
        advanceUntilIdle()
        assertEquals(listOf("near-midnight"), agenda(utc).events.map { it.key.eventId.value })

        val tokyoFake = CalendarAgendaUiFake(provider)
        val tokyo = CalendarAgendaUiController(tokyoFake, tokyoFake, transitionClock, ZoneId.of("Asia/Tokyo"), this)
        advanceUntilIdle()
        assertEquals(listOf("near-midnight"), agenda(tokyo).events.map { it.key.eventId.value })
        tokyo.selectMode(CalendarAgendaMode.TOMORROW)
        advanceUntilIdle()
        assertEquals(listOf("next-local-day"), agenda(tokyo).events.map { it.key.eventId.value })

        val nyZone = ZoneId.of("America/New_York")
        val springRange = calendarAgendaRange(CalendarAgendaQuery(CalendarAgendaMode.DATE, LocalDate.parse("2026-03-08")), UI_TEST_CLOCK, nyZone)
        val fallRange = calendarAgendaRange(CalendarAgendaQuery(CalendarAgendaMode.DATE, LocalDate.parse("2026-11-01")), UI_TEST_CLOCK, nyZone)
        assertEquals(Duration.ofHours(23), Duration.between(springRange.start, springRange.endExclusive))
        assertEquals(Duration.ofHours(25), Duration.between(fallRange.start, fallRange.endExclusive))

        val springProvider = fakeProvider(
            listOf(calendar("ny", zone = nyZone)),
            listOf(
                timedEvent("ny", "after-gap", "2026-03-08T07:15:00Z", "2026-03-08T07:45:00Z", nyZone),
                timedEvent("ny", "next-day", "2026-03-09T04:15:00Z", "2026-03-09T04:45:00Z", nyZone),
            ),
        )
        val springFake = CalendarAgendaUiFake(springProvider)
        val spring = CalendarAgendaUiController(
            springFake, springFake, UI_TEST_CLOCK, nyZone, this,
            initialQuery = CalendarAgendaQuery(CalendarAgendaMode.DATE, LocalDate.parse("2026-03-08")),
        )
        advanceUntilIdle()
        assertEquals(listOf("after-gap"), agenda(spring).events.map { it.key.eventId.value })
        listOf(utc, tokyo, spring).forEach { it.close() }
    }

    @Test fun recurringIdentityIsStableAndCancelledEventsRemainDetailOnly() = runTest {
        val moved = FakeCalendarProvider.Fixtures.events.first { it.key.eventId.value == "evt-series-moved" }
        val sameInstanceWithNewTransportId = moved.copy(key = moved.key.copy(eventId = EventId("transport-id-changed")))
        assertEquals(moved.stableIdentity(), sameInstanceWithNewTransportId.stableIdentity())

        val cancelled = timedEvent(
            "demo", "cancelled-event", "2026-10-20T13:00:00Z", "2026-10-20T14:00:00Z",
            title = null, status = EventStatus.CANCELLED,
        )
        val provider = fakeProvider(listOf(calendar("demo")), listOf(cancelled))
        val uiFake = CalendarAgendaUiFake(provider)
        val controller = CalendarAgendaUiController(uiFake, uiFake, UI_TEST_CLOCK, ZoneId.of("UTC"), this)
        advanceUntilIdle()
        assertTrue(agenda(controller).events.isEmpty())
        controller.openEvent(cancelled.key)
        advanceUntilIdle()
        val detail = controller.state.value as CalendarAgendaUiState.EventDetail
        assertEquals(EventStatus.CANCELLED, detail.event.status)
        assertEquals("（タイトルなし）", CalendarAgendaJapaneseText.title(detail.event.title))
        assertEquals("説明はありません", CalendarAgendaJapaneseText.description(detail.event.description))
        assertEquals("場所はありません", CalendarAgendaJapaneseText.location(detail.event.location))
        assertEquals(app.serea.android.DataOrigin.CALENDAR, detail.event.dataOrigin)
        assertFalse(CalendarPermissionPolicy.canSendToExternalModel(detail.event.dataOrigin, DataClassification.PUBLIC))
        controller.backToAgenda()
        assertTrue(controller.state.value is CalendarAgendaUiState.Agenda)
        controller.close()
    }

    @Test fun detailRejectsAKeyOutsideTheSelectedCalendarBeforeProviderRead() = runTest {
        val eventInOtherCalendar = timedEvent(
            "calendar-b", "private-b", "2026-10-20T13:00:00Z", "2026-10-20T13:30:00Z",
        )
        val provider = fakeProvider(
            listOf(calendar("calendar-a"), calendar("calendar-b")), listOf(eventInOtherCalendar),
        )
        val uiFake = CalendarAgendaUiFake(provider)
        val controller = CalendarAgendaUiController(
            uiFake, uiFake, UI_TEST_CLOCK, ZoneId.of("UTC"), this,
            initialCalendarId = "calendar-a",
        )
        advanceUntilIdle()

        controller.openEvent(eventInOtherCalendar.key)
        advanceUntilIdle()

        assertTrue(uiFake.getEventKeys.isEmpty())
        assertFailure(controller, CalendarAgendaFailure.EVENT_UNAVAILABLE)
        controller.close()
    }

    @Test fun allDayExclusiveEndAndNullTimeDetailsStayExplicit() = runTest {
        val tokyo = ZoneId.of("Asia/Tokyo")
        val event = allDayEvent("demo", "multi-day", "2026-10-20", "2026-10-22", tokyo, title = null)
        assertEquals("終日 · 2026年10月20日(火) ～ 2026年10月21日(水)", CalendarAgendaJapaneseText.eventTime(event, ZoneId.of("UTC")))
        assertEquals("2026年10月22日(木)", CalendarAgendaJapaneseText.allDayEndBoundary(event)!!)
        val cancelledWithoutTime = timedEvent(
            "demo", "cancelled-no-time", "2026-10-20T13:00:00Z", "2026-10-20T13:30:00Z",
            status = EventStatus.CANCELLED,
        )
        assertTrue(CalendarAgendaJapaneseText.eventTime(cancelledWithoutTime, ZoneId.of("UTC")).contains("日時"))
    }

    private fun agenda(controller: CalendarAgendaUiController): CalendarAgendaUiState.Agenda =
        controller.state.value as? CalendarAgendaUiState.Agenda
            ?: error("Expected Agenda state; safe UI failure was ${controller.state.value}")

    private fun assertFailure(
        controller: CalendarAgendaUiController,
        reason: CalendarAgendaFailure,
    ): CalendarAgendaUiState.Failure {
        val state = controller.state.value as? CalendarAgendaUiState.Failure
            ?: error("Expected Failure($reason), found ${controller.state.value}")
        assertEquals(reason, state.reason)
        return state
    }
}
