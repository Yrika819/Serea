package app.serea.android.calendar

import app.serea.android.calendar.transport.GoogleCalendarHttpReadPort
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleCalendarAuthorizationAdapter
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import okhttp3.HttpUrl
import okhttp3.OkHttpClient
import java.util.concurrent.atomic.AtomicLong

enum class CalendarProviderKind { FAKE, GOOGLE_READ_ONLY }

/**
 * Read-only provider session for future composition roots. The constructor gate is deliberately
 * false by default; a fake gateway or MOCK_ONLY grant can never select the Google HTTP adapter.
 * Replacing or closing a session invalidates its old read port and cancels reads in flight.
 */
class CalendarProviderSessions(
    private val fakeProvider: () -> FakeCalendarProvider = { FakeCalendarProvider() },
    private val googleHttpEnabledByOwner: Boolean = false,
) {
    private val lock = Any()
    private val nextId = AtomicLong(0)
    private var current: CalendarProviderSession? = null

    fun openDefault(): CalendarProviderSession = openFake()

    fun openFake(): CalendarProviderSession {
        val provider = fakeProvider()
        return replace(CalendarProviderKind.FAKE, provider, provider)
    }

    /**
     * Only constructs the Google transport when the build's owner activation switch is on and
     * the injected authorization gateway is a real Play Services gateway. Otherwise this opens
     * a fresh fake session, preserving the demo path without touching Google APIs.
     */
    fun openGoogleReadOnly(
        gateway: GoogleAuthorizationGateway,
        client: OkHttpClient = OkHttpClient(),
        baseUrl: HttpUrl? = null,
    ): CalendarProviderSession {
        if (!googleHttpEnabledByOwner || gateway.mode != GoogleAuthorizationMode.PLAY_SERVICES) return openFake()
        val authorization = GoogleCalendarAuthorizationAdapter(gateway)
        val port = if (baseUrl == null) {
            GoogleCalendarHttpReadPort(authorization, authorization, client)
        } else {
            GoogleCalendarHttpReadPort(authorization, authorization, client, baseUrl)
        }
        return replace(CalendarProviderKind.GOOGLE_READ_ONLY, port, authorization)
    }

    private fun replace(
        kind: CalendarProviderKind,
        delegate: CalendarReadPort,
        authorization: CalendarAuthorizationPort,
    ): CalendarProviderSession = synchronized(lock) {
        current?.invalidateFromController()
        val id = nextId.incrementAndGet()
        CalendarProviderSession(
            id = id,
            kind = kind,
            delegate = delegate,
            authorizationDelegate = authorization,
            isCurrent = { synchronized(lock) { current?.id == id } },
            onClose = { closeIfCurrent(id) },
        ).also { current = it }
    }

    private fun closeIfCurrent(id: Long) = synchronized(lock) {
        if (current?.id == id) current = null
    }
}

class CalendarProviderSession internal constructor(
    internal val id: Long,
    val kind: CalendarProviderKind,
    private val delegate: CalendarReadPort,
    private val authorizationDelegate: CalendarAuthorizationPort,
    private val isCurrent: () -> Boolean,
    private val onClose: () -> Unit,
) : CalendarReadPort, CalendarAuthorizationPort, AutoCloseable {
    private val lock = Any()
    private var closed = false
    private val activeJobs = linkedSetOf<Job>()

    override suspend fun status(): CalendarAuthorizationStatus = withActiveSession {
        authorizationDelegate.status()
    }

    override suspend fun listCalendars(): List<CalendarDescriptor> = withActiveSession { delegate.listCalendars() }
    override suspend fun getCalendar(calendarId: CalendarId): CalendarDescriptor = withActiveSession { delegate.getCalendar(calendarId) }
    override suspend fun getEvent(key: CalendarEventKey): CalendarEvent = withActiveSession { delegate.getEvent(key) }
    override suspend fun listEventsPage(calendarId: CalendarId, range: CalendarDateRange, pageToken: String?): CalendarEventPage =
        withActiveSession { delegate.listEventsPage(calendarId, range, pageToken) }

    override fun close() {
        invalidate()
        onClose()
    }

    internal fun invalidateFromController() = invalidate()

    private fun invalidate() {
        val jobs = synchronized(lock) {
            if (closed) return
            closed = true
            activeJobs.toList().also { activeJobs.clear() }
        }
        jobs.forEach { it.cancel(CancellationException("Calendar provider session ended.")) }
    }

    private suspend fun <T> withActiveSession(block: suspend () -> T): T {
        val job = currentCoroutineContext()[Job]
        synchronized(lock) {
            if (closed) throw inactiveSession()
            job?.let(activeJobs::add)
        }
        try {
            if (!isCurrent()) throw inactiveSession()
            val value = block()
            if (!isActive()) throw inactiveSession()
            return value
        } finally {
            synchronized(lock) { job?.let(activeJobs::remove) }
        }
    }

    private fun isActive(): Boolean = synchronized(lock) { !closed } && isCurrent()

    private fun inactiveSession() = CalendarReadFailure(
        CalendarApiFailureCode.PERMISSION_DENIED,
        "Calendar provider session is no longer active.",
    )
}
