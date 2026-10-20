package app.serea.android.calendar

import com.google.gson.JsonElement
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import com.google.gson.Strictness
import com.google.gson.stream.JsonReader
import java.io.StringReader
import java.time.Instant
import java.time.LocalDate
import java.time.OffsetDateTime
import java.time.ZoneId

/** Parser for the bounded subset of Google Calendar API v3 events used by the Android read model. */
object GoogleCalendarEventParser {
    const val MAX_PAYLOAD_BYTES = 512 * 1024

    fun parseEvent(
        json: String,
        calendarId: CalendarId,
        calendarTimeZone: ZoneId,
        source: CalendarEventSource = CalendarEventSource.GOOGLE_CALENDAR_API,
    ): CalendarEvent {
        val root = parseRoot(json)
        try {
            val id = EventId(requiredString(root, "id"))
            val rawStatus = optionalString(root, "status")
            val status = when (rawStatus) {
                "confirmed" -> EventStatus.CONFIRMED
                "tentative" -> EventStatus.TENTATIVE
                "cancelled" -> EventStatus.CANCELLED
                null -> EventStatus.UNKNOWN
                else -> EventStatus.UNKNOWN
            }
            val startElement = optionalObject(root, "start")
            val endElement = optionalObject(root, "end")
            val eventTime = if (startElement == null || endElement == null) {
                if (status != EventStatus.CANCELLED) throw IllegalArgumentException("Non-cancelled event is missing start or end.")
                null
            } else parseTime(startElement, endElement, calendarTimeZone)

            val recurrenceId = optionalString(root, "recurringEventId")?.let(::EventId)
            val original = root.get("originalStartTime")?.let { element ->
                if (!element.isJsonObject) throw IllegalArgumentException("originalStartTime must be an object.")
                parseOriginalStart(element.asJsonObject, calendarTimeZone)
            }
            return CalendarEvent(
                key = CalendarEventKey(calendarId, id),
                title = optionalString(root, "summary")?.takeIf(String::isNotBlank),
                description = optionalString(root, "description"),
                location = optionalString(root, "location"),
                time = eventTime,
                status = status,
                recurringEventId = recurrenceId,
                originalStartTime = original,
                updatedAt = optionalString(root, "updated")?.let(::parseInstant),
                visibility = when (optionalString(root, "visibility")) {
                    "default" -> EventVisibility.DEFAULT
                    "public" -> EventVisibility.PUBLIC
                    "private" -> EventVisibility.PRIVATE
                    "confidential" -> EventVisibility.CONFIDENTIAL
                    null -> EventVisibility.UNKNOWN
                    else -> EventVisibility.UNKNOWN
                },
                source = source,
            )
        } catch (failure: CalendarReadFailure) {
            throw failure
        } catch (failure: Exception) {
            throw CalendarReadFailure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid Google Calendar event payload.")
        }
    }

    fun parsePage(
        json: String,
        calendarId: CalendarId,
        calendarTimeZone: ZoneId,
    ): CalendarEventPage {
        val root = parseRoot(json)
        try {
            val items = root.get("items")
            if (items != null && !items.isJsonArray) throw IllegalArgumentException("items must be an array.")
            val token = optionalString(root, "nextPageToken")?.also {
                if (it.isBlank() || it.length > 2048) throw IllegalArgumentException("Invalid nextPageToken.")
            }
            val events = items?.asJsonArray?.map { element ->
                if (!element.isJsonObject) throw IllegalArgumentException("Event item must be an object.")
                parseEvent(element.toString(), calendarId, calendarTimeZone)
            }.orEmpty()
            return CalendarEventPage(events, token)
        } catch (failure: CalendarReadFailure) {
            throw failure
        } catch (failure: Exception) {
            throw CalendarReadFailure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid Google Calendar events.list page.")
        }
    }

    private fun parseRoot(json: String): JsonObject {
        if (json.toByteArray(Charsets.UTF_8).size > MAX_PAYLOAD_BYTES) {
            throw CalendarReadFailure(CalendarApiFailureCode.INVALID_RESPONSE, "Calendar response exceeds the payload limit.")
        }
        try {
            val reader = JsonReader(StringReader(json)).apply { setStrictness(Strictness.STRICT) }
            val root = JsonParser.parseReader(reader)
            if (!root.isJsonObject) throw IllegalArgumentException("Response root must be an object.")
            return root.asJsonObject
        } catch (failure: Exception) {
            throw CalendarReadFailure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid JSON response.")
        }
    }

    private fun parseTime(start: JsonObject, end: JsonObject, calendarZone: ZoneId): EventTime {
        if ((start.has("date") && start.has("dateTime")) || (end.has("date") && end.has("dateTime"))) {
            throw IllegalArgumentException("An event time cannot contain both date and dateTime.")
        }
        val startDate = optionalString(start, "date")
        val endDate = optionalString(end, "date")
        if (startDate != null || endDate != null) {
            if (startDate == null || endDate == null || start.has("dateTime") || end.has("dateTime")) {
                throw IllegalArgumentException("Start and end must both use date or dateTime.")
            }
            val startLocal = LocalDate.parse(startDate)
            val endLocal = LocalDate.parse(endDate)
            if (!endLocal.isAfter(startLocal)) throw IllegalArgumentException("All-day end date is exclusive and must be after start.")
            val zone = readZone(start, "timeZone") ?: readZone(end, "timeZone") ?: calendarZone
            return EventTime.AllDay(startLocal, endLocal, zone)
        }
        val parsedStart = parseDateTime(start)
        val parsedEnd = parseDateTime(end)
        if (!parsedEnd.instant.isAfter(parsedStart.instant)) throw IllegalArgumentException("End must be after start.")
        return EventTime.Timed(parsedStart.instant, parsedEnd.instant, parsedStart.zone)
    }

    private fun parseOriginalStart(element: JsonObject, calendarZone: ZoneId): EventOriginalStart {
        if (element.has("date") && element.has("dateTime")) throw IllegalArgumentException("originalStartTime cannot contain both date and dateTime.")
        optionalString(element, "date")?.let { raw ->
            val date = LocalDate.parse(raw)
            val zone = readZone(element, "timeZone") ?: calendarZone
            return EventOriginalStart.AllDay(date, zone)
        }
        val parsed = parseDateTime(element)
        return EventOriginalStart.Timed(parsed.instant, parsed.zone)
    }

    private data class ParsedDateTime(val instant: Instant, val zone: ZoneId)

    private fun parseDateTime(value: JsonObject): ParsedDateTime {
        val raw = requiredString(value, "dateTime")
        val offsetDateTime = OffsetDateTime.parse(raw)
        val zone = readZone(value, "timeZone") ?: offsetDateTime.offset
        val validOffsets = zone.rules.getValidOffsets(offsetDateTime.toLocalDateTime())
        if (offsetDateTime.offset !in validOffsets) throw IllegalArgumentException("RFC3339 offset conflicts with timezone ID.")
        return ParsedDateTime(offsetDateTime.toInstant(), zone)
    }

    private fun readZone(value: JsonObject, name: String): ZoneId? = optionalString(value, name)?.let(ZoneId::of)

    private fun parseInstant(raw: String): Instant = OffsetDateTime.parse(raw).toInstant()

    private fun requiredString(obj: JsonObject, name: String): String = optionalString(obj, name)
        ?: throw IllegalArgumentException("Required string field is missing: $name")

    private fun optionalString(obj: JsonObject, name: String): String? {
        val element: JsonElement = obj.get(name) ?: return null
        if (element.isJsonNull) return null
        if (!element.isJsonPrimitive || !element.asJsonPrimitive.isString) throw IllegalArgumentException("$name must be a string or null.")
        return element.asString
    }

    private fun optionalObject(obj: JsonObject, name: String): JsonObject? {
        val element = obj.get(name) ?: return null
        if (element.isJsonNull) return null
        if (!element.isJsonObject) throw IllegalArgumentException("$name must be an object or null.")
        return element.asJsonObject
    }
}
