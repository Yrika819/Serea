package app.serea.android.gmail

import app.serea.android.googleauth.AccessTokenLease
import app.serea.android.googleauth.AuthorizationFailure
import app.serea.android.googleauth.AuthorizationStatus
import app.serea.android.googleauth.FakeGoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleFeature
import app.serea.android.googleauth.GoogleGrantLeaseState
import app.serea.android.googleauth.GoogleScope
import app.serea.android.googleauth.GoogleServiceState
import app.serea.android.googleauth.ScopePermission
import app.serea.android.googleauth.TokenSupplyResult
import app.serea.android.googleauth.awaitGoogleIdentityTaskToTerminal
import app.serea.android.googleauth.invalidateGoogleGrantAtRevokeStart
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withContext
import kotlinx.coroutines.yield
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.TimeUnit
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.CountDownLatch

class GmailProviderSessionsTest {
    private val server = MockWebServer().apply { start() }

    private enum class RevokeOutcome { SUCCESS, FAILURE, CANCELLATION }

    private class GatedResponseDispatcher(private val body: String) : Dispatcher() {
        val requestArrived = CountDownLatch(1)
        val releaseResponse = CountDownLatch(1)
        @Volatile var recordedRequest: RecordedRequest? = null

        override fun dispatch(request: RecordedRequest): MockResponse {
            recordedRequest = request
            requestArrived.countDown()
            return if (releaseResponse.await(30, TimeUnit.SECONDS)) {
                MockResponse().setBody(body)
            } else {
                MockResponse().setResponseCode(500).setBody("{}")
            }
        }
    }

    @After fun close() = server.shutdown()

    private class PlayServicesFixture(
        permission: ScopePermission = ScopePermission.GRANTED,
        override var authorizationSessionId: String? = "gmail-session-first",
    ) : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        var tokenRequests = 0
        var rotateAfterNextToken = false
        var onLeaseUse: (() -> Unit)? = null
        private val stateLock = Any()
        private val grantState = GoogleGrantLeaseState()
        private val knownPermissions = mutableMapOf(GoogleScope.GMAIL_READ_ONLY to permission)
        private var failure: AuthorizationFailure? = null
        lateinit var lastLease: AccessTokenLease
        var lastLeaseIdentity: Int = -1
        var capturedGmailVersion: Long = -1
        var gmailVersionAtRevoke: Long = -1
        val validityTrace = CopyOnWriteArrayList<Pair<Long, Boolean>>()

        private fun beginRevoke() {
            synchronized(stateLock) {
                check(grantState.tryBeginAuthorizationOperation())
                invalidateGoogleGrantAtRevokeStart(grantState, knownPermissions)
                gmailVersionAtRevoke = grantState.version(GoogleFeature.GMAIL)
            }
        }

        fun disableGmail() {
            synchronized(stateLock) { grantState.disableInApp(GoogleFeature.GMAIL) }
        }

        suspend fun revokeWithRemote(remoteRevoke: suspend () -> Unit) {
            beginRevoke()
            try {
                awaitGoogleIdentityTaskToTerminal(remoteRevoke)
            } finally {
                synchronized(stateLock) { grantState.finishAuthorizationOperation() }
            }
        }

        fun authorizationOperationInFlight(): Boolean = synchronized(stateLock) {
            grantState.isAuthorizationOperationInFlight()
        }
        fun tryBeginAuthorizationOperation(): Boolean = synchronized(stateLock) {
            grantState.tryBeginAuthorizationOperation()
        }
        fun finishAuthorizationOperation() = synchronized(stateLock) { grantState.finishAuthorizationOperation() }

        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            if (feature != GoogleFeature.GMAIL || scopes != setOf(GoogleScope.GMAIL_READ_ONLY)) {
                return TokenSupplyResult.UserActionRequired(scopes)
            }
            if (!grantState.isEnabledInApp(GoogleFeature.GMAIL)) return TokenSupplyResult.Unavailable(AuthorizationFailure.FEATURE_DISABLED)
            if (authorizationSessionId == null) return TokenSupplyResult.Unavailable(AuthorizationFailure.ACCOUNT_REQUIRED)
            val permission = synchronized(stateLock) {
                if (grantState.isAuthorizationOperationInFlight()) {
                    return TokenSupplyResult.Unavailable(AuthorizationFailure.REQUEST_IN_PROGRESS)
                }
                knownPermissions[GoogleScope.GMAIL_READ_ONLY]
            }
            if (permission != ScopePermission.GRANTED) return TokenSupplyResult.UserActionRequired(setOf(GoogleScope.GMAIL_READ_ONLY))
            tokenRequests++
            val boundSession = authorizationSessionId
            val boundVersions = synchronized(stateLock) { grantState.leaseVersions(setOf(GoogleFeature.GMAIL)) }
            capturedGmailVersion = boundVersions.getValue(GoogleFeature.GMAIL)
            val lease = AccessTokenLease(
                "fixture-token-$boundSession",
                setOf(GoogleScope.GMAIL_READ_ONLY),
                null,
                authorizationSessionId = boundSession,
                validityCheck = {
                    onLeaseUse?.invoke()
                    val current = synchronized(stateLock) { grantState.isLeaseCurrent(boundVersions) }
                    validityTrace += grantState.version(GoogleFeature.GMAIL) to current
                    current
                },
            )
            lastLease = lease
            lastLeaseIdentity = System.identityHashCode(lease)
            if (rotateAfterNextToken) {
                rotateAfterNextToken = false
                authorizationSessionId = "gmail-session-second"
                failure = AuthorizationFailure.ACCOUNT_CHANGED
            }
            return TokenSupplyResult.Available(lease)
        }

        override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
            GoogleServiceState.AVAILABLE,
            scopes.associateWith { scope -> synchronized(stateLock) { knownPermissions[scope] } ?: ScopePermission.NOT_REQUESTED },
            failure,
        )

        fun switchAccount() {
            authorizationSessionId = "gmail-session-second"
            failure = AuthorizationFailure.ACCOUNT_CHANGED
        }
    }

    @Test fun defaultAndMockAuthorizationStayOnClearlyFakeProvider() = runBlocking {
        val default = GmailProviderSessions().openDefault()
        assertEquals(GmailProviderKind.FAKE, default.kind)
        assertEquals(GmailAuthorizationState.MOCK_ONLY, default.status().state)

        val mockGateway = FakeGoogleAuthorizationGateway(setOf(GoogleScope.GMAIL_READ_ONLY))
        val stillFake = GmailProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(mockGateway, baseUrl = server.url("/gmail/v1/"))
        assertEquals(GmailProviderKind.FAKE, stillFake.kind)
        assertTrue(stillFake.listInbox().messages.isNotEmpty())
        assertEquals(0, server.requestCount)
    }

    @Test fun ownerEnabledSessionUsesTheProductionAdapterAndReplacementInvalidatesIt() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"messages":[{"id":"message_1","threadId":"thread_1"}]}"""))
        server.enqueue(MockResponse().setBody(messageJson))
        val sessions = GmailProviderSessions(googleHttpEnabledByOwner = true)
        val real = sessions.openGoogleReadOnly(
            PlayServicesFixture(),
            OkHttpClient(),
            server.url("/gmail/v1/"),
        )
        assertEquals(GmailProviderKind.GOOGLE_READ_ONLY, real.kind)
        assertEquals("Hello", real.listInbox().messages.single().plainTextBody)
        assertEquals("Bearer fixture-token-gmail-session-first", server.takeRequest().getHeader("Authorization"))
        assertEquals("Bearer fixture-token-gmail-session-first", server.takeRequest().getHeader("Authorization"))

        val fake = sessions.openFake()
        assertEquals(GmailProviderKind.FAKE, fake.kind)
        assertEquals(GmailReadFailureCode.PERMISSION_DENIED, failure { real.listInbox() }.code)
        assertEquals(2, server.requestCount)
    }

    @Test fun unknownAccountBindingAndExpiredAuthorizationFailBeforeNetwork() = runBlocking {
        val sessions = GmailProviderSessions(googleHttpEnabledByOwner = true)
        val unknown = PlayServicesFixture(authorizationSessionId = null)
        val unknownSession = sessions.openGoogleReadOnly(unknown, baseUrl = server.url("/gmail/v1/"))
        assertEquals(GmailAuthorizationState.NOT_CONNECTED, unknownSession.status().state)
        assertEquals(GmailReadFailureCode.PERMISSION_DENIED, failure { unknownSession.listInbox() }.code)
        assertEquals(0, unknown.tokenRequests)
        assertEquals(0, server.requestCount)

        val expired = GoogleGmailAuthorizationAdapterFixture(
            statusFailure = AuthorizationFailure.EXPIRED,
        )
        val expiredSession = sessions.openGoogleReadOnly(expired, baseUrl = server.url("/gmail/v1/"))
        assertEquals(GmailAuthorizationState.EXPIRED, expiredSession.status().state)
        assertEquals(GmailReadFailureCode.PERMISSION_DENIED, failure { expiredSession.listInbox() }.code)
        assertEquals(0, server.requestCount)
    }

    @Test fun accountSwitchDuringTokenLeaseAcquisitionCannotDispatchAGmailRequest() = runBlocking {
        val gateway = PlayServicesFixture().apply { rotateAfterNextToken = true }
        val session = GmailProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(gateway, baseUrl = server.url("/gmail/v1/"))

        assertEquals(GmailReadFailureCode.PERMISSION_DENIED, failure { session.listInbox() }.code)
        assertEquals(GmailAuthorizationState.REVOKED, session.status().state)
        assertEquals(1, gateway.tokenRequests)
        assertEquals(0, server.requestCount)
    }

    @Test fun delayedGmailResponseIsDeniedWhileRevocationIsPendingAndAfterEveryOutcome() = runBlocking {
        suspend fun assertInvalidatedRead(outcome: RevokeOutcome) {
            val gateway = PlayServicesFixture()
            val previousRequestCount = server.requestCount
            val response = GatedResponseDispatcher(messageJson)
            server.dispatcher = response
            val session = GmailProviderSessions(googleHttpEnabledByOwner = true)
                .openGoogleReadOnly(gateway, baseUrl = server.url("/gmail/v1/"))
            val read = async { runCatching { session.getMessage(GmailMessageId("message_1")) } }

            assertTrue(withContext(Dispatchers.IO) { response.requestArrived.await(5, TimeUnit.SECONDS) })
            val request = requireNotNull(response.recordedRequest)
            assertEquals("GET", request.method)
            assertEquals("Bearer fixture-token-gmail-session-first", request.getHeader("Authorization"))
            assertFalse("the delayed response body is still pending at request receipt", read.isCompleted)
            assertEquals(1, gateway.tokenRequests)
            assertTrue(gateway.lastLeaseIdentity != -1)

            try {
                val remoteRevokeStarted = CompletableDeferred<Unit>()
                val remoteRevokeGate = CompletableDeferred<Unit>()
                val revoke = async {
                    try {
                        gateway.revokeWithRemote {
                            remoteRevokeStarted.complete(Unit)
                            remoteRevokeGate.await()
                        }
                        null
                    } catch (cancelled: kotlinx.coroutines.CancellationException) {
                        throw cancelled
                    } catch (exception: Exception) {
                        exception
                    }
                }
                remoteRevokeStarted.await()

                assertFalse("the lease is invalid before remote revoke completes", runCatching {
                    gateway.lastLease.useToken { it }
                }.isSuccess)
                assertEquals(ScopePermission.NOT_REQUESTED,
                    gateway.status(setOf(GoogleScope.GMAIL_READ_ONLY)).permissions[GoogleScope.GMAIL_READ_ONLY])
                assertEquals(gateway.capturedGmailVersion + 1, gateway.gmailVersionAtRevoke)
                assertFalse("the dispatcher has not released the HTTP response", read.isCompleted)
                val checksBeforeResponseBodyCompletion = gateway.validityTrace.size

                // Release the real local HTTP response while the synthetic remote revoke remains gated.
                response.releaseResponse.countDown()
                val readOutcome = read.await()
                assertTrue(
                    "response must be denied by the active lease callback; identity=${gateway.lastLeaseIdentity}, " +
                        "capturedVersion=${gateway.capturedGmailVersion}, revokedVersion=${gateway.gmailVersionAtRevoke}, " +
                        "checks=${gateway.validityTrace.toList()}, tokenRequests=${gateway.tokenRequests}, outcome=${readOutcome.exceptionOrNull()}",
                    readOutcome.exceptionOrNull() is GmailReadFailure,
                )
                assertEquals(GmailReadFailureCode.PERMISSION_DENIED, (readOutcome.exceptionOrNull() as GmailReadFailure).code)
                assertTrue("response-body validation must observe the invalidated production lease state",
                    gateway.validityTrace.drop(checksBeforeResponseBodyCompletion)
                        .any { (version, valid) -> version == gateway.gmailVersionAtRevoke && !valid })
                assertFalse("remote revoke must still be gated while the read is rejected", revoke.isCompleted)

                when (outcome) {
                    RevokeOutcome.SUCCESS -> {
                        remoteRevokeGate.complete(Unit)
                        assertEquals(null, revoke.await())
                    }
                    RevokeOutcome.FAILURE -> {
                        remoteRevokeGate.completeExceptionally(IllegalStateException("synthetic revoke failure"))
                        assertTrue(revoke.await() is IllegalStateException)
                    }
                    RevokeOutcome.CANCELLATION -> {
                        revoke.cancel()
                        yield()
                        assertFalse("caller cancellation must not complete before the remote revoke task", revoke.isCompleted)
                        assertTrue(gateway.authorizationOperationInFlight())
                        assertFalse("new authorization is blocked until remote revoke is terminal",
                            gateway.tryBeginAuthorizationOperation())
                        assertEquals(
                            TokenSupplyResult.Unavailable(AuthorizationFailure.REQUEST_IN_PROGRESS),
                            gateway.acquire(GoogleFeature.GMAIL, setOf(GoogleScope.GMAIL_READ_ONLY)),
                        )
                        remoteRevokeGate.complete(Unit)
                        revoke.join()
                        assertFalse(gateway.authorizationOperationInFlight())
                        assertTrue("new authorization may start after the remote task is terminal",
                            gateway.tryBeginAuthorizationOperation())
                        gateway.finishAuthorizationOperation()
                        assertTrue(gateway.acquire(GoogleFeature.GMAIL, setOf(GoogleScope.GMAIL_READ_ONLY))
                            is TokenSupplyResult.UserActionRequired)
                    }
                }

                assertFalse("no remote outcome restores the invalidated lease", runCatching {
                    gateway.lastLease.useToken { it }
                }.isSuccess)
                assertEquals(ScopePermission.NOT_REQUESTED,
                    gateway.status(setOf(GoogleScope.GMAIL_READ_ONLY)).permissions[GoogleScope.GMAIL_READ_ONLY])
                assertEquals(1, gateway.tokenRequests)
                assertEquals(previousRequestCount + 1, server.requestCount)
            } finally {
                response.releaseResponse.countDown()
            }
        }

        RevokeOutcome.entries.forEach { assertInvalidatedRead(it) }
    }

    @Test fun delayedGmailResponseIsDeniedAfterAppLocalDisable() = runBlocking {
        val gateway = PlayServicesFixture()
        val response = GatedResponseDispatcher(messageJson)
        server.dispatcher = response
        val session = GmailProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(gateway, baseUrl = server.url("/gmail/v1/"))
        val read = async { failure { session.getMessage(GmailMessageId("message_1")) } }
        assertTrue(withContext(Dispatchers.IO) { response.requestArrived.await(5, TimeUnit.SECONDS) })
        assertFalse("the dispatcher is holding the response before app-local disable", read.isCompleted)

        try {
            gateway.disableGmail()
            assertFalse("the existing lease is invalid before the response is released",
                runCatching { gateway.lastLease.useToken { it } }.isSuccess)
            response.releaseResponse.countDown()

            assertEquals(GmailReadFailureCode.PERMISSION_DENIED, read.await().code)
            assertTrue("post-body validity check observes the disabled feature",
                gateway.validityTrace.any { (_, current) -> !current })
            assertFalse(runCatching { gateway.lastLease.useToken { it } }.isSuccess)
        } finally {
            response.releaseResponse.countDown()
        }
    }

    @Test fun replacingSessionImmediatelyAfterLeaseAcquisitionPreventsHttpDispatch() = runBlocking {
        lateinit var sessions: GmailProviderSessions
        var replacement: GmailProviderSession? = null
        val gateway = PlayServicesFixture().apply {
            onLeaseUse = { replacement = sessions.openFake() }
        }
        sessions = GmailProviderSessions(googleHttpEnabledByOwner = true)
        val oldSession = sessions.openGoogleReadOnly(gateway, baseUrl = server.url("/gmail/v1/"))

        assertEquals(
            GmailReadFailureCode.PERMISSION_DENIED,
            failure { oldSession.getMessage(GmailMessageId("message_1")) }.code,
        )
        assertTrue(currentCoroutineContext().isActive)
        assertEquals(GmailProviderKind.FAKE, replacement?.kind)
        assertEquals(0, server.requestCount)
    }

    @Test fun replacingSessionCancelsConcurrentReadsWithoutCancellingTheirParentOrSiblingCommunication() = runBlocking {
        val message = FakeGmailProvider.fixtures.messages.first()
        val delegate = GatedGmailReadPort(message)
        val session = testSession(delegate)
        val unrelatedResult = CompletableDeferred<String>()

        val caller = launch {
            coroutineScope {
                val unrelated = async { unrelatedResult.await() }
                val firstRead = async { failure { session.getMessage(message.id) }.code }
                val secondRead = async { failure { session.getMessage(message.id) }.code }

                delegate.started.receive()
                delegate.started.receive()
                session.invalidateFromController()

                assertEquals(GmailReadFailureCode.PERMISSION_DENIED, firstRead.await())
                assertEquals(GmailReadFailureCode.PERMISSION_DENIED, secondRead.await())
                assertTrue(currentCoroutineContext().isActive)
                assertTrue("unrelated sibling communication must remain active", unrelated.isActive)
                unrelatedResult.complete("unrelated communication survived")
                assertEquals("unrelated communication survived", unrelated.await())
            }
        }

        caller.join()
        assertTrue("provider replacement must not cancel the caller job", caller.isCompleted && !caller.isCancelled)
        assertEquals(0, server.requestCount)
    }

    @Test fun responseAlreadyProducedByOldSessionIsDeniedWhenAccountSwitchWinsBeforeDelivery() = runBlocking {
        val message = FakeGmailProvider.fixtures.messages.first()
        val delegate = GatedGmailReadPort(message, finishResponseAfterCancellation = true)
        val session = testSession(delegate)
        val result = async { failure { session.getMessage(message.id) }.code }

        delegate.started.receive()
        // Model a parsed HTTP response buffered by the adapter while its caller is suspended.
        session.invalidateFromController()
        delegate.releaseResponse.complete(Unit)

        assertEquals(GmailReadFailureCode.PERMISSION_DENIED, result.await())
        assertEquals(0, server.requestCount)
    }

    @Test fun callerCancellationStillCancelsItsOwnGmailRead() = runBlocking {
        val message = FakeGmailProvider.fixtures.messages.first()
        val delegate = GatedGmailReadPort(message)
        val session = testSession(delegate)
        val read = async { session.getMessage(message.id) }

        delegate.started.receive()
        read.cancelAndJoin()

        assertTrue(read.isCancelled)
        assertTrue("caller cancellation must not close the provider session", currentCoroutineContext().isActive)
        assertEquals(GmailAuthorizationState.MOCK_ONLY, session.status().state)
        assertEquals(0, server.requestCount)
    }

    private fun testSession(delegate: GmailReadPort) = GmailProviderSession(
        id = 7,
        kind = GmailProviderKind.FAKE,
        delegate = delegate,
        authorizationDelegate = FakeGmailProvider(),
        isCurrent = { true },
        onClose = {},
    )

    private class GatedGmailReadPort(
        private val response: GmailMessage,
        private val finishResponseAfterCancellation: Boolean = false,
    ) : GmailReadPort {
        val started = Channel<Unit>(Channel.UNLIMITED)
        val releaseResponse = CompletableDeferred<Unit>()

        override suspend fun listInbox(pageToken: String?, pageSize: Int): GmailMessagePage {
            awaitResponseDelivery()
            return GmailMessagePage(listOf(response))
        }
        override suspend fun search(query: String, pageToken: String?, pageSize: Int): GmailMessagePage {
            awaitResponseDelivery()
            return GmailMessagePage(listOf(response))
        }
        override suspend fun getMessage(id: GmailMessageId): GmailMessage {
            val parsedResponse = response
            awaitResponseDelivery()
            return parsedResponse
        }
        override suspend fun getThread(id: GmailThreadId): GmailThread = error("not used by this contract test")

        private suspend fun awaitResponseDelivery() {
            started.send(Unit)
            if (finishResponseAfterCancellation) {
                withContext(NonCancellable) { releaseResponse.await() }
            } else {
                releaseResponse.await()
            }
        }
    }

    private suspend fun failure(block: suspend () -> Any): GmailReadFailure = try {
        block()
        throw AssertionError("Expected GmailReadFailure")
    } catch (failure: GmailReadFailure) {
        failure
    }

    private val messageJson = """{"id":"message_1","threadId":"thread_1","snippet":"fixture","payload":{"mimeType":"text/plain","body":{"data":"SGVsbG8="}}}"""

    private class GoogleGmailAuthorizationAdapterFixture(
        private val statusFailure: AuthorizationFailure,
    ) : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        override val authorizationSessionId = "expired-gmail-session"
        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>) =
            TokenSupplyResult.Unavailable(statusFailure)
        override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
            GoogleServiceState.AVAILABLE,
            scopes.associateWith { ScopePermission.EXPIRED },
            statusFailure,
        )
    }
}
