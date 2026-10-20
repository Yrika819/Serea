package app.serea.android.calendar.transport

import app.serea.android.calendar.CalendarApiFailureCode
import app.serea.android.calendar.CalendarAuthorizationPort
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.calendar.CalendarDateRange
import app.serea.android.calendar.CalendarDescriptor
import app.serea.android.calendar.CalendarEvent
import app.serea.android.calendar.CalendarEventKey
import app.serea.android.calendar.CalendarEventPage
import app.serea.android.calendar.CalendarId
import app.serea.android.calendar.CalendarReadFailure
import app.serea.android.calendar.CalendarReadPort
import app.serea.android.calendar.CalendarPermissionPolicy
import app.serea.android.calendar.CalendarReadLimits
import app.serea.android.calendar.CalendarReadPermission
import app.serea.android.calendar.CalendarVisibility
import app.serea.android.calendar.EventId
import app.serea.android.calendar.GoogleCalendarEventParser
import com.google.gson.JsonParser
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.suspendCancellableCoroutine
import okhttp3.Call
import okhttp3.Callback
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.time.ZoneId
import java.util.concurrent.TimeUnit
import kotlin.coroutines.resume

/** Google Calendar v3 read-only transport. It has no write verb and never follows redirects. */
class GoogleCalendarHttpReadPort internal constructor(
    private val authorization: CalendarAuthorizationPort,
    private val tokens: CalendarAccessTokenPort,
    private val client: OkHttpClient = productionClient(),
    private val baseUrl: HttpUrl = GOOGLE_API_BASE,
) : CalendarReadPort {
    private val safeClient = client.newBuilder()
        .followRedirects(false)
        .followSslRedirects(false)
        .retryOnConnectionFailure(false)
        .build()

    init {
        val production = baseUrl.scheme == "https" && baseUrl.host == GOOGLE_API_HOST
        val loopbackTest = baseUrl.scheme == "http" && baseUrl.host in TEST_HOSTS
        require((production || loopbackTest) && baseUrl.encodedPath == "/calendar/v3/")
    }

    override suspend fun listCalendars(): List<CalendarDescriptor> {
        requireAuthorized(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY)
        val calendars = ArrayList<CalendarDescriptor>()
        val seen = HashSet<String>()
        var token: String? = null
        var pageCount = 0
        do {
            if (++pageCount > MAX_PAGES) throw failure(CalendarApiFailureCode.TOO_MANY_PAGES, "Calendar page limit exceeded.")
            if (token != null && !seen.add(token)) throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Calendar page token repeated.")
            val url = baseUrl.newBuilder().addPathSegment("users").addPathSegment("me")
                .addPathSegment("calendarList").addQueryParameter("maxResults", CALENDAR_PAGE_SIZE.toString())
                .apply { token?.let { addQueryParameter("pageToken", it) } }.build()
            val json = get(url, CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY)
            val root = parseJson(json)
            val items = root.arrayOrEmpty("items")
            if (items.size() > CALENDAR_PAGE_SIZE || items.size() > MAX_CALENDARS - calendars.size) {
                throw failure(CalendarApiFailureCode.TOO_MANY_EVENTS, "Calendar list limit exceeded.")
            }
            items.forEach { calendars += parseCalendar(it.toString()) }
            token = root.stringOrNull("nextPageToken")?.also(::validatePageToken)
        } while (token != null)
        return calendars.distinctBy { it.id }.also {
            if (it.size != calendars.size) throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Duplicate calendar identifier.")
        }
    }

    override suspend fun getCalendar(calendarId: CalendarId): CalendarDescriptor {
        requireAuthorized(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY)
        val url = baseUrl.newBuilder().addPathSegment("users").addPathSegment("me")
            .addPathSegment("calendarList").addPathSegment(calendarId.value).build()
        return parseCalendar(get(url, CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY))
    }

    override suspend fun getEvent(key: CalendarEventKey): CalendarEvent {
        requireAuthorized(CalendarPermissionPolicy.EVENTS_READ_ONLY)
        val url = baseUrl.newBuilder().addPathSegment("calendars").addPathSegment(key.calendarId.value)
            .addPathSegment("events").addPathSegment(key.eventId.value).build()
        return parseEvent(get(url, CalendarPermissionPolicy.EVENTS_READ_ONLY), key.calendarId)
    }

    override suspend fun listEventsPage(
        calendarId: CalendarId,
        range: CalendarDateRange,
        pageToken: String?,
    ): CalendarEventPage {
        requireAuthorized(CalendarPermissionPolicy.EVENTS_READ_ONLY)
        pageToken?.let(::validatePageToken)
        val url = baseUrl.newBuilder().addPathSegment("calendars").addPathSegment(calendarId.value)
            .addPathSegment("events")
            .addQueryParameter("timeMin", range.start.toString())
            .addQueryParameter("timeMax", range.endExclusive.toString())
            .addQueryParameter("singleEvents", "true")
            .addQueryParameter("showDeleted", "true")
            .addQueryParameter("orderBy", "startTime")
            .addQueryParameter("maxResults", EVENT_PAGE_SIZE.toString())
            .addQueryParameter("fields", EVENT_FIELDS)
            .apply { pageToken?.let { addQueryParameter("pageToken", it) } }.build()
        return parsePage(get(url, CalendarPermissionPolicy.EVENTS_READ_ONLY), calendarId)
    }

    private suspend fun requireAuthorized(scope: String) {
        val status = authorization.status()
        if (status.state != CalendarAuthorizationState.AUTHORIZED) {
            throw failure(CalendarApiFailureCode.PERMISSION_DENIED, "Calendar read authorization is unavailable.")
        }
        if (scope in status.missingScopes) throw failure(CalendarApiFailureCode.MISSING_SCOPE, "Required Calendar read scope is missing.")
    }

    private suspend fun get(url: HttpUrl, scope: String): String {
        val status = authorization.status()
        if (status.state != CalendarAuthorizationState.AUTHORIZED) {
            throw failure(CalendarApiFailureCode.PERMISSION_DENIED, "Calendar read authorization is unavailable.")
        }
        if (scope in status.missingScopes) throw failure(CalendarApiFailureCode.MISSING_SCOPE, "Required Calendar read scope is missing.")
        val token = try { tokens.accessToken() }
        catch (cancelled: CancellationException) { throw cancelled }
        catch (_: Exception) { throw failure(CalendarApiFailureCode.PERMISSION_DENIED, "Calendar access token is unavailable.") }
        if (token == null) throw failure(CalendarApiFailureCode.PERMISSION_DENIED, "Calendar access token is unavailable.")
        val request = Request.Builder().url(url).header("Authorization", "Bearer ${token.value}")
            .header("Accept", "application/json").get().build()
        val response = execute(safeClient.newCall(request))
        response.use {
            if (it.code !in 200..299) throw responseFailure(it)
            val body = it.body ?: throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Calendar response body is missing.")
            val bytes = try { body.byteStream().use { stream -> readBoundedResponse(stream, MAX_RESPONSE_BYTES) } }
            catch (cancelled: CancellationException) { throw cancelled }
            catch (_: ResponseTooLargeException) { throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Calendar response exceeds the payload limit.") }
            catch (_: java.io.InterruptedIOException) { throw failure(CalendarApiFailureCode.TIMEOUT, "Calendar response read timed out.") }
            catch (_: IOException) { throw failure(CalendarApiFailureCode.UNAVAILABLE, "Calendar response could not be read.") }
            return bytes.toString(Charsets.UTF_8)
        }
    }

    private suspend fun execute(call: Call): Response = suspendCancellableCoroutine { continuation ->
        continuation.invokeOnCancellation { call.cancel() }
        call.enqueue(object : Callback {
            override fun onFailure(call: Call, e: java.io.IOException) {
                if (continuation.isActive) {
                    val mapped = if (call.isCanceled()) failure(CalendarApiFailureCode.CANCELLED, "Calendar request was cancelled.")
                    else if (e is java.io.InterruptedIOException) failure(CalendarApiFailureCode.TIMEOUT, "Calendar request timed out.")
                    else failure(CalendarApiFailureCode.UNAVAILABLE, "Calendar service is unavailable.")
                    runCatching { continuation.resumeWith(Result.failure(mapped)) }
                }
            }
            override fun onResponse(call: Call, response: Response) {
                if (continuation.isActive) runCatching { continuation.resume(response) }.onFailure { response.close() }
                else response.close()
            }
        })
    }

    private fun responseFailure(response: Response): CalendarReadFailure {
        val code = when (response.code) {
            401 -> CalendarApiFailureCode.PERMISSION_DENIED
            403 -> when (errorReason(response)) {
                "insufficientPermissions", "forbidden" -> CalendarApiFailureCode.MISSING_SCOPE
                "rateLimitExceeded", "userRateLimitExceeded", "quotaExceeded" -> CalendarApiFailureCode.RATE_LIMITED
                else -> CalendarApiFailureCode.PERMISSION_DENIED
            }
            404 -> CalendarApiFailureCode.NOT_FOUND
            410 -> CalendarApiFailureCode.SYNC_TOKEN_EXPIRED
            429 -> CalendarApiFailureCode.RATE_LIMITED
            in 500..599 -> CalendarApiFailureCode.UNAVAILABLE
            else -> CalendarApiFailureCode.INVALID_RESPONSE
        }
        return failure(code, "Calendar API returned HTTP ${response.code}.")
    }

    private fun errorReason(response: Response): String? = try {
        val bytes = response.peekBody(MAX_ERROR_BYTES.toLong()).bytes()
        JsonParser.parseString(bytes.toString(Charsets.UTF_8)).asJsonObject
            .getAsJsonObject("error")?.getAsJsonArray("errors")?.firstOrNull()?.asJsonObject?.get("reason")?.asString
    } catch (_: Exception) { null }

    private fun parseCalendar(json: String): CalendarDescriptor = try {
        val item = JsonParser.parseString(json).asJsonObject
        val id = item.stringOrNull("id") ?: invalid()
        val zone = item.stringOrNull("timeZone")?.let(ZoneId::of) ?: ZoneId.of("UTC")
        val role = item.stringOrNull("accessRole")
        val permission = when (role) {
            "owner", "writer", "reader", "freeBusyReader" -> CalendarReadPermission.READ_ALLOWED
            else -> CalendarReadPermission.UNKNOWN
        }
        CalendarDescriptor(
            id = CalendarId(id), displayName = item.stringOrNull("summary"), timeZone = zone,
            colorArgb = parseColor(item.stringOrNull("backgroundColor")),
            visibility = CalendarVisibility.UNKNOWN,
            readPermission = permission,
        )
    } catch (failure: CalendarReadFailure) { throw failure }
    catch (_: Exception) { throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid Google Calendar calendar payload.") }

    private fun parseColor(value: String?): Int? = value?.let {
        try {
            val hex = it.removePrefix("#")
            when (hex.length) {
                6 -> (0xff shl 24) or hex.toInt(16)
                8 -> hex.toLong(16).toInt()
                else -> null
            }
        } catch (_: Exception) { null }
    }

    private fun parseEvent(json: String, calendarId: CalendarId): CalendarEvent =
        try { GoogleCalendarEventParser.parseEvent(json, calendarId, eventFallbackTimeZone(json)) }
        catch (failure: CalendarReadFailure) { throw failure }
        catch (_: Exception) { throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid Google Calendar event payload.") }

    private fun parsePage(json: String, calendarId: CalendarId): CalendarEventPage =
        try {
            val root = JsonParser.parseString(json).asJsonObject
            val zone = root.stringOrNull("timeZone")?.let(ZoneId::of) ?: ZoneId.of("UTC")
            GoogleCalendarEventParser.parsePage(json, calendarId, zone)
        }
        catch (failure: CalendarReadFailure) { throw failure }
        catch (_: Exception) { throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid Google Calendar events page.") }

    /** events.get can carry the calendar zone on its start/end; its RFC3339 offset remains authoritative otherwise. */
    private fun eventFallbackTimeZone(json: String): ZoneId {
        val root = parseJson(json)
        for (field in listOf("start", "end")) {
            val time = root.get(field)?.takeUnless { it.isJsonNull }?.asJsonObject ?: continue
            time.stringOrNull("timeZone")?.let { return ZoneId.of(it) }
        }
        return ZoneId.of("UTC")
    }

    private fun parseJson(json: String) = try { JsonParser.parseString(json).asJsonObject }
    catch (_: Exception) { throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid Google Calendar response.") }

    private fun validatePageToken(token: String) {
        if (token.isBlank() || token.length > 2048) throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid Calendar page token.")
    }

    private fun failure(code: CalendarApiFailureCode, message: String) = CalendarReadFailure(code, message)
    private fun invalid(): Nothing = throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid Google Calendar response.")

    private fun com.google.gson.JsonObject.arrayOrEmpty(name: String) = try {
        get(name)?.takeUnless { it.isJsonNull }?.asJsonArray ?: com.google.gson.JsonArray()
    } catch (_: Exception) { throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid Google Calendar response.") }
    private fun com.google.gson.JsonObject.stringOrNull(name: String): String? = try {
        get(name)?.takeUnless { it.isJsonNull }?.asString
    } catch (_: Exception) { throw failure(CalendarApiFailureCode.INVALID_RESPONSE, "Invalid Google Calendar response.") }

    companion object {
        private const val GOOGLE_API_HOST = "www.googleapis.com"
        private val TEST_HOSTS = setOf("127.0.0.1", "localhost", "[::1]")
        private val GOOGLE_API_BASE = "https://www.googleapis.com/calendar/v3/".toHttpUrl()
        private const val MAX_RESPONSE_BYTES = GoogleCalendarEventParser.MAX_PAYLOAD_BYTES
        private const val MAX_ERROR_BYTES = 16 * 1024
        private const val MAX_PAGES = 20
        private const val CALENDAR_PAGE_SIZE = 250
        private const val EVENT_PAGE_SIZE = 100
        private const val MAX_CALENDARS = 2_000
        private const val EVENT_FIELDS = "items(id,summary,description,location,start,end,status,recurringEventId,originalStartTime,updated,visibility),nextPageToken,timeZone"

        private fun productionClient() = OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS).readTimeout(10, TimeUnit.SECONDS)
            .callTimeout(20, TimeUnit.SECONDS).followRedirects(false).followSslRedirects(false)
            .retryOnConnectionFailure(false).build()
    }
}

internal class ResponseTooLargeException : IOException("Response exceeds configured byte limit.")

/** Reads with bounded chunks and at most one byte beyond the accepted limit. */
internal fun readBoundedResponse(input: InputStream, maximumBytes: Int): ByteArray {
    require(maximumBytes >= 0)
    val output = ByteArrayOutputStream(minOf(maximumBytes, 8 * 1024))
    val chunk = ByteArray(8 * 1024)
    while (true) {
        val remainingWithSentinel = maximumBytes.toLong() + 1L - output.size()
        if (remainingWithSentinel <= 0L) throw ResponseTooLargeException()
        val count = input.read(chunk, 0, minOf(chunk.size.toLong(), remainingWithSentinel).toInt())
        if (count < 0) return output.toByteArray()
        if (count == 0) {
            val one = input.read()
            if (one < 0) return output.toByteArray()
            output.write(one)
        } else {
            output.write(chunk, 0, count)
        }
        if (output.size() > maximumBytes) throw ResponseTooLargeException()
    }
}
