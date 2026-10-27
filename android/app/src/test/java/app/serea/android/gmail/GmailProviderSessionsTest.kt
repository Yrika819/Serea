package app.serea.android.gmail

import app.serea.android.googleauth.AccessTokenLease
import app.serea.android.googleauth.AuthorizationFailure
import app.serea.android.googleauth.AuthorizationStatus
import app.serea.android.googleauth.FakeGoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleFeature
import app.serea.android.googleauth.GoogleScope
import app.serea.android.googleauth.GoogleServiceState
import app.serea.android.googleauth.ScopePermission
import app.serea.android.googleauth.TokenSupplyResult
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
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.concurrent.TimeUnit

class GmailProviderSessionsTest {
    private val server = MockWebServer().apply { start() }

    @After fun close() = server.shutdown()

    private class PlayServicesFixture(
        private var permission: ScopePermission = ScopePermission.GRANTED,
        override var authorizationSessionId: String? = "gmail-session-first",
    ) : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        var tokenRequests = 0
        var rotateAfterNextToken = false
        var onLeaseUse: (() -> Unit)? = null
        private var leaseGeneration = 0L
        private var disabled = false
        private var failure: AuthorizationFailure? = null

        fun revokeGrant() {
            permission = ScopePermission.NEEDS_CONSENT
            leaseGeneration++
        }

        fun disableGmail() {
            disabled = true
            leaseGeneration++
        }

        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            if (feature != GoogleFeature.GMAIL || scopes != setOf(GoogleScope.GMAIL_READ_ONLY)) {
                return TokenSupplyResult.UserActionRequired(scopes)
            }
            if (disabled) return TokenSupplyResult.Unavailable(AuthorizationFailure.FEATURE_DISABLED)
            if (permission != ScopePermission.GRANTED || authorizationSessionId == null) {
                return TokenSupplyResult.Unavailable(failure ?: AuthorizationFailure.ACCOUNT_REQUIRED)
            }
            tokenRequests++
            val boundSession = authorizationSessionId
            val boundGeneration = leaseGeneration
            val lease = AccessTokenLease(
                "fixture-token-$boundSession",
                setOf(GoogleScope.GMAIL_READ_ONLY),
                null,
                authorizationSessionId = boundSession,
                validityCheck = {
                    onLeaseUse?.invoke()
                    !disabled && leaseGeneration == boundGeneration
                },
            )
            if (rotateAfterNextToken) {
                rotateAfterNextToken = false
                authorizationSessionId = "gmail-session-second"
                failure = AuthorizationFailure.ACCOUNT_CHANGED
            }
            return TokenSupplyResult.Available(lease)
        }

        override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
            GoogleServiceState.AVAILABLE,
            scopes.associateWith { if (permission == ScopePermission.GRANTED) permission else ScopePermission.NOT_REQUESTED },
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

    @Test fun delayedGmailResponseIsRejectedAfterGrantRevocationOrFeatureDisable() = runBlocking {
        suspend fun assertInvalidatedRead(action: (PlayServicesFixture) -> Unit) {
            val gateway = PlayServicesFixture()
            val previousRequestCount = server.requestCount
            server.enqueue(
                MockResponse()
                    .setBody(messageJson)
                    .setBodyDelay(2, TimeUnit.SECONDS),
            )
            val session = GmailProviderSessions(googleHttpEnabledByOwner = true)
                .openGoogleReadOnly(gateway, baseUrl = server.url("/gmail/v1/"))
            val read = async { failure { session.getMessage(GmailMessageId("message_1")) } }

            val request = requireNotNull(withContext(Dispatchers.IO) { server.takeRequest(5, TimeUnit.SECONDS) })
            assertEquals("GET", request.method)
            assertEquals("Bearer fixture-token-gmail-session-first", request.getHeader("Authorization"))
            action(gateway)

            assertEquals(GmailReadFailureCode.PERMISSION_DENIED, read.await().code)
            assertEquals(1, gateway.tokenRequests)
            assertEquals(previousRequestCount + 1, server.requestCount)
        }

        assertInvalidatedRead(PlayServicesFixture::revokeGrant)
        assertInvalidatedRead(PlayServicesFixture::disableGmail)
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
