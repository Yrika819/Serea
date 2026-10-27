package app.serea.android.gmail

import app.serea.android.gmail.transport.GmailHttpReadPort
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleGmailAuthorizationAdapter
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.withContext
import okhttp3.HttpUrl
import okhttp3.OkHttpClient
import java.util.concurrent.atomic.AtomicLong

enum class GmailProviderKind { FAKE, GOOGLE_READ_ONLY }

/**
 * Provider composition boundary for Gmail. The production adapter is opt-in, requires a
 * Play-services gateway with a pinned account session, and is never selected by default.
 * Replacing or closing a session invalidates its ports and cancels reads in flight.
 */
class GmailProviderSessions(
    private val fakeProvider: () -> FakeGmailProvider = { FakeGmailProvider() },
    private val googleHttpEnabledByOwner: Boolean = false,
) {
    private val lock = Any()
    private val nextId = AtomicLong(0)
    private var current: GmailProviderSession? = null

    fun openDefault(): GmailProviderSession = openFake()

    fun openFake(): GmailProviderSession {
        val provider = fakeProvider()
        return replace(GmailProviderKind.FAKE, provider, provider)
    }

    /**
     * The adapter uses the same GmailHttpReadPort as its MockWebServer contract tests.
     * A Fake authorization gateway or a disabled Owner gate always stays on the Fake provider.
     */
    fun openGoogleReadOnly(
        gateway: GoogleAuthorizationGateway,
        client: OkHttpClient = OkHttpClient(),
        baseUrl: HttpUrl? = null,
    ): GmailProviderSession {
        if (!googleHttpEnabledByOwner || gateway.mode != GoogleAuthorizationMode.PLAY_SERVICES) return openFake()
        val authorization = GoogleGmailAuthorizationAdapter(gateway, googleHttpEnabledByOwner = true)
        val port = if (baseUrl == null) {
            GmailHttpReadPort(authorization, authorization, client)
        } else {
            GmailHttpReadPort(authorization, authorization, client, baseUrl)
        }
        return replace(GmailProviderKind.GOOGLE_READ_ONLY, port, authorization)
    }

    private fun replace(
        kind: GmailProviderKind,
        delegate: GmailReadPort,
        authorization: GmailAuthorizationPort,
    ): GmailProviderSession = synchronized(lock) {
        current?.invalidateFromController()
        val id = nextId.incrementAndGet()
        GmailProviderSession(
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

class GmailProviderSession internal constructor(
    internal val id: Long,
    val kind: GmailProviderKind,
    private val delegate: GmailReadPort,
    private val authorizationDelegate: GmailAuthorizationPort,
    private val isCurrent: () -> Boolean,
    private val onClose: () -> Unit,
) : GmailReadPort, GmailAuthorizationPort, AutoCloseable {
    private val lock = Any()
    private var closed = false
    private val activeJobs = linkedSetOf<Job>()

    override suspend fun status(): GmailAuthorizationStatus = withActiveSession { authorizationDelegate.status() }
    override suspend fun listInbox(pageToken: String?, pageSize: Int): GmailMessagePage =
        withActiveSession { delegate.listInbox(pageToken, pageSize) }
    override suspend fun search(query: String, pageToken: String?, pageSize: Int): GmailMessagePage =
        withActiveSession { delegate.search(query, pageToken, pageSize) }
    override suspend fun getMessage(id: GmailMessageId): GmailMessage = withActiveSession { delegate.getMessage(id) }
    override suspend fun getThread(id: GmailThreadId): GmailThread = withActiveSession { delegate.getThread(id) }

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
        jobs.forEach { it.cancel(CancellationException("Gmail provider session ended.")) }
    }

    private suspend fun <T> withActiveSession(block: suspend () -> T): T {
        val callerJob = currentCoroutineContext()[Job]
        val requestJob = SupervisorJob(callerJob)
        synchronized(lock) {
            if (closed) {
                requestJob.cancel()
                throw inactiveSession()
            }
            activeJobs += requestJob
        }
        try {
            if (!isCurrent()) throw inactiveSession()
            val value = withContext(requestJob) { block() }
            if (!isCurrent()) throw inactiveSession()
            // invalidate() marks this session closed under the same lock before a replacement
            // becomes current, making this the result-delivery linearization point.
            synchronized(lock) {
                if (closed) throw inactiveSession()
            }
            return value
        } catch (cancelled: CancellationException) {
            // A session replacement cancels only requestJob. Preserve ordinary caller
            // cancellation, but report session teardown as a typed provider denial.
            if (callerJob?.isActive != false && synchronized(lock) { closed }) throw inactiveSession()
            throw cancelled
        } finally {
            synchronized(lock) { activeJobs.remove(requestJob) }
            requestJob.cancel()
        }
    }

    private fun isActive(): Boolean = synchronized(lock) { !closed } && isCurrent()

    private fun inactiveSession() = GmailReadFailure(GmailReadFailureCode.PERMISSION_DENIED)
}
