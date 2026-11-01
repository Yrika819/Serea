package app.serea.android.calendar.ui

import app.serea.android.calendar.ui.CalendarAgendaJapaneseText
import app.serea.android.calendar.EventTime
import java.time.Instant
import java.time.ZoneId
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class CalendarAgendaFormattingTest {
    @Test fun timedEventsUseInjectedDisplayZoneAndRetainOriginalZone() {
        val originalZone = ZoneId.of("America/New_York")
        val event = timedEvent(
            "demo", "zone-check", "2026-10-20T15:30:00Z", "2026-10-20T16:00:00Z", originalZone,
        )
        val time = event.time as EventTime.Timed
        assertEquals(Instant.parse("2026-10-20T15:30:00Z"), time.start)
        assertEquals(originalZone, time.timeZone)
        assertTrue(CalendarAgendaJapaneseText.eventTime(event, ZoneId.of("UTC")).contains("2026年10月20日(火) 15:30 Z"))
        assertTrue(CalendarAgendaJapaneseText.eventTime(event, ZoneId.of("Asia/Tokyo")).contains("2026年10月21日(水) 00:30 +09:00"))
    }

    @Test fun springGapAndFallOverlapFormatFromInstantsWithoutInventingLocalTimes() {
        val newYork = ZoneId.of("America/New_York")
        val afterSpringGap = timedEvent(
            "demo", "spring", "2026-03-08T07:15:00Z", "2026-03-08T07:45:00Z", newYork,
        )
        val springText = CalendarAgendaJapaneseText.eventTime(afterSpringGap, newYork)
        assertTrue(springText.contains("2026年3月8日(日) 03:15 -04:00"))
        assertTrue(springText.contains("03:45 -04:00"))
        assertTrue(!springText.contains("02:"))

        val firstOverlapOccurrence = timedEvent(
            "demo", "fall-before", "2026-11-01T05:30:00Z", "2026-11-01T05:45:00Z", newYork,
        )
        val secondOverlapOccurrence = timedEvent(
            "demo", "fall-after", "2026-11-01T06:30:00Z", "2026-11-01T06:45:00Z", newYork,
        )
        val firstText = CalendarAgendaJapaneseText.eventTime(firstOverlapOccurrence, newYork)
        val secondText = CalendarAgendaJapaneseText.eventTime(secondOverlapOccurrence, newYork)
        assertTrue(firstText.contains("01:30 -04:00"))
        assertTrue(secondText.contains("01:30 -05:00"))
        assertTrue(firstText.startsWith("2026年11月1日(日)"))
        assertTrue(secondText.startsWith("2026年11月1日(日)"))
    }

    @Test fun allDayDateAcrossDstRemainsItsCalendarLocalDateInsteadOfUtcConvertedDate() {
        val newYork = ZoneId.of("America/New_York")
        val event = allDayEvent("demo", "dst-all-day", "2026-03-08", "2026-03-09", newYork)
        val time = event.time as EventTime.AllDay
        assertEquals("2026-03-08", time.startDate.toString())
        assertEquals("2026-03-09", time.endDateExclusive.toString())
        assertEquals("終日 · 2026年3月8日(日)", CalendarAgendaJapaneseText.eventTime(event, ZoneId.of("America/Los_Angeles")))
    }
}
