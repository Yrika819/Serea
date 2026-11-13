package app.serea.android.calendar

import app.serea.android.calendar.transport.GoogleCalendarHttpReadPort
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleCalendarAuthorizationAdapter
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlin.coroutines.AbstractCoroutineContextElement
import kotlin.coroutines.CoroutineContext
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.withContext
import okhttp3.HttpUrl
import okhttp3.OkHttpClient
import java.util.concurrent.atomic.AtomicLong

enum class CalendarProviderKind { FAKE, GOOGLE_READ_ONLY }

/** Stable identity for one tracked provider request across nested withContext scopes. */
internal class CalendarSessionRequestJob(val job: Job) : AbstractCoroutineContextElement(Key) {
    companion object Key : CoroutineContext.Key<CalendarSessionRequestJob>
}

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

    /** Opens a provider session that can only expose a single synthetic primary calendar. */
    fun openPrimaryFake(): CalendarProviderSession {
        val provider = fakeProvider()
        return replace(
            CalendarProviderKind.FAKE,
            PrimaryOnlyCalendarReadPort(provider),
            provider,
            primaryCalendarOnly = true,
        )
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
        return openGoogleReadOnly(gateway, client, baseUrl, primaryOnly = false)
    }

    /** Defense-in-depth Google path; direct transport calls also enforce this same mode. */
    fun openPrimaryGoogleReadOnly(
        gateway: GoogleAuthorizationGateway,
        client: OkHttpClient = OkHttpClient(),
        baseUrl: HttpUrl? = null,
    ): CalendarProviderSession {
        if (!googleHttpEnabledByOwner || gateway.mode != GoogleAuthorizationMode.PLAY_SERVICES) return openPrimaryFake()
        return openGoogleReadOnly(gateway, client, baseUrl, primaryOnly = true)
    }

    private fun openGoogleReadOnly(
        gateway: GoogleAuthorizationGateway,
        client: OkHttpClient,
        baseUrl: HttpUrl?,
        primaryOnly: Boolean,
    ): CalendarProviderSession {
        val authorization = GoogleCalendarAuthorizationAdapter(gateway, primaryOnly = primaryOnly)
        val port = if (baseUrl == null) {
            GoogleCalendarHttpReadPort(
                authorization, authorization, client,
                accessMode = if (primaryOnly) GoogleCalendarHttpReadPort.AccessMode.PRIMARY_ONLY
                    else GoogleCalendarHttpReadPort.AccessMode.GENERAL_READ_ONLY,
            )
        } else {
            GoogleCalendarHttpReadPort(
                authorization, authorization, client, baseUrl,
                if (primaryOnly) GoogleCalendarHttpReadPort.AccessMode.PRIMARY_ONLY
                    else GoogleCalendarHttpReadPort.AccessMode.GENERAL_READ_ONLY,
            )
        }
        val session = replace(
            CalendarProviderKind.GOOGLE_READ_ONLY,
            port,
            authorization,
            currentGrantInvalidationGeneration = { gateway.googleGrantInvalidationGeneration },
            primaryCalendarOnly = primaryOnly,
        )
        session.attachGrantInvalidationObserver(gateway.observeGoogleGrantInvalidation {
            session.cancelInFlightReadsForGrantInvalidation()
        })
        session.attachAuthenticationInvalidationObserver(gateway.observeGoogleAuthenticationInvalidation { sourceJob, generation ->
            session.cancelInFlightReadsForAuthenticationInvalidation(sourceJob, generation)
        })
        return session
    }

    private fun replace(
        kind: CalendarProviderKind,
        delegate: CalendarReadPort,
        authorization: CalendarAuthorizationPort,
        currentGrantInvalidationGeneration: () -> Long = { 0L },
        primaryCalendarOnly: Boolean = false,
    ): CalendarProviderSession = synchronized(lock) {
        current?.invalidateFromController()
        val id = nextId.incrementAndGet()
        CalendarProviderSession(
            id = id,
            kind = kind,
            delegate = delegate,
            authorizationDelegate = authorization,
            primaryCalendarOnly = primaryCalendarOnly,
            currentGrantInvalidationGeneration = currentGrantInvalidationGeneration,
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
    val primaryCalendarOnly: Boolean = false,
    private val currentGrantInvalidationGeneration: () -> Long = { 0L },
    private val isCurrent: () -> Boolean,
    private val onClose: () -> Unit,
) : CalendarReadPort, CalendarAuthorizationPort, AutoCloseable {
    private val lock = Any()
    private var closed = false
    private val activeJobs = linkedMapOf<Job, Long>()
    private var grantInvalidationObserver: AutoCloseable? = null
    private var authenticationInvalidationObserver: AutoCloseable? = null
    private var grantInvalidationGeneration = 0L

    override suspend fun status(): CalendarAuthorizationStatus = withActiveSession {
        authorizationDelegate.status()
    }

    override suspend fun status(requiredScopes: Set<String>): CalendarAuthorizationStatus = withActiveSession {
        authorizationDelegate.status(requiredScopes)
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

    internal fun attachGrantInvalidationObserver(observer: AutoCloseable) {
        val closeImmediately = synchronized(lock) {
            if (closed) true else {
                grantInvalidationObserver = observer
                false
            }
        }
        if (closeImmediately) observer.close()
    }

    internal fun attachAuthenticationInvalidationObserver(observer: AutoCloseable) {
        val closeImmediately = synchronized(lock) {
            if (closed) true else {
                authenticationInvalidationObserver = observer
                false
            }
        }
        if (closeImmediately) observer.close()
    }

    internal fun cancelInFlightReadsForGrantInvalidation() {
        val jobs = synchronized(lock) {
            if (closed) return
            grantInvalidationGeneration++
            activeJobs.keys.toList()
        }
        jobs.forEach { it.cancel(CancellationException("Google authorization was revoked.")) }
    }

    internal fun cancelInFlightReadsForAuthenticationInvalidation(sourceJob: Job?, invalidationGeneration: Long) {
        val jobs = synchronized(lock) {
            if (closed) return
            activeJobs.filter { (job, requestGeneration) ->
                job !== sourceJob && requestGeneration < invalidationGeneration
            }.keys
        }
        jobs.forEach { it.cancel(CancellationException("Google authorization expired.")) }
    }

    private fun invalidate() {
        val (jobs, grantObserver, authenticationObserver) = synchronized(lock) {
            if (closed) return
            closed = true
            val active = activeJobs.keys.toList().also { activeJobs.clear() }
            Triple(
                active,
                grantInvalidationObserver.also { grantInvalidationObserver = null },
                authenticationInvalidationObserver.also { authenticationInvalidationObserver = null },
            )
        }
        grantObserver?.close()
        authenticationObserver?.close()
        jobs.forEach { it.cancel(CancellationException("Calendar provider session ended.")) }
    }

    private suspend fun <T> withActiveSession(block: suspend () -> T): T {
        val callerJob = currentCoroutineContext()[Job]
        val requestJob = SupervisorJob(callerJob)
        val (requestSessionGrantGeneration, requestGatewayGrantGeneration) = synchronized(lock) {
            if (closed) {
                requestJob.cancel()
                throw inactiveSession()
            }
            val gatewayGeneration = currentGrantInvalidationGeneration()
            activeJobs[requestJob] = gatewayGeneration
            grantInvalidationGeneration to gatewayGeneration
        }
        try {
            if (!isCurrent()) throw inactiveSession()
            val value = withContext(requestJob + CalendarSessionRequestJob(requestJob)) { block() }
            if (!isCurrent()) throw inactiveSession()
            // invalidate() closes this session under the same lock before replacement, so a
            // response racing account/session replacement cannot cross this delivery gate.
            synchronized(lock) {
                if (closed) throw inactiveSession()
                // The revoke callback can arrive after the child request Job has completed but
                // before result delivery; the generation closes that cancellation gap.
                if (grantInvalidationGeneration != requestSessionGrantGeneration) throw inactiveSession()
                // This gateway-scoped generation advances inside revoke-start state mutation,
                // before the observer callback can cancel this request Job.
                if (currentGrantInvalidationGeneration() != requestGatewayGrantGeneration) throw inactiveSession()
            }
            return value
        } catch (cancelled: CancellationException) {
            if (callerJob?.isActive != false && synchronized(lock) { closed }) throw inactiveSession()
            throw cancelled
        } finally {
            synchronized(lock) { activeJobs.remove(requestJob) }
            requestJob.cancel()
        }
    }

    private fun isActive(): Boolean = synchronized(lock) { !closed } && isCurrent()

    private fun inactiveSession() = CalendarReadFailure(
        CalendarApiFailureCode.PERMISSION_DENIED,
        "Calendar provider session is no longer active.",
    )
}

/** Provider-layer guard used for private synthetic previews and as a second Google boundary. */
private class PrimaryOnlyCalendarReadPort(private val delegate: CalendarReadPort) : CalendarReadPort {
    override suspend fun listCalendars(): List<CalendarDescriptor> = delegate.listCalendars()
        .filter { it.id.value == "primary" }
        .take(1)
    override suspend fun getCalendar(calendarId: CalendarId): CalendarDescriptor = reject()
    override suspend fun getEvent(key: CalendarEventKey): CalendarEvent {
        requirePrimary(key.calendarId)
        return delegate.getEvent(key).also { event ->
            if (event.key != key || event.key.calendarId.value != "primary") reject()
        }
    }
    override suspend fun listEventsPage(calendarId: CalendarId, range: CalendarDateRange, pageToken: String?): CalendarEventPage {
        requirePrimary(calendarId)
        return delegate.listEventsPage(calendarId, range, pageToken).also { page ->
            if (page.events.any { it.key.calendarId.value != "primary" }) reject()
        }
    }
    private fun requirePrimary(calendarId: CalendarId) {
        if (calendarId.value != "primary") reject()
    }
    private fun reject(): Nothing = throw CalendarReadFailure(
        CalendarApiFailureCode.PERMISSION_DENIED, "Only the primary calendar is available.",
    )
}
