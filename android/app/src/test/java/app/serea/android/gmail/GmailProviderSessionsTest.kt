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
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

class GmailProviderSessionsTest {
    private val server = MockWebServer().apply { start() }

    @After fun close() = server.shutdown()

    private class PlayServicesFixture(
        private val permission: ScopePermission = ScopePermission.GRANTED,
        override var authorizationSessionId: String? = "gmail-session-first",
    ) : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        var tokenRequests = 0
        var rotateAfterNextToken = false
        private var failure: AuthorizationFailure? = null

        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            if (feature != GoogleFeature.GMAIL || scopes != setOf(GoogleScope.GMAIL_READ_ONLY)) {
                return TokenSupplyResult.UserActionRequired(scopes)
            }
            if (permission != ScopePermission.GRANTED || authorizationSessionId == null) {
                return TokenSupplyResult.Unavailable(failure ?: AuthorizationFailure.ACCOUNT_REQUIRED)
            }
            tokenRequests++
            val boundSession = authorizationSessionId
            val lease = AccessTokenLease(
                "fixture-token-$boundSession",
                setOf(GoogleScope.GMAIL_READ_ONLY),
                null,
                authorizationSessionId = boundSession,
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

    @Test fun accountSwitchBetweenListAndMessageFetchCannotReturnMixedOrPartialMailboxData() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"messages":[{"id":"message_1","threadId":"thread_1"}]}"""))
        val gateway = PlayServicesFixture().apply { rotateAfterNextToken = true }
        val session = GmailProviderSessions(googleHttpEnabledByOwner = true)
            .openGoogleReadOnly(gateway, baseUrl = server.url("/gmail/v1/"))

        assertEquals(GmailReadFailureCode.PERMISSION_DENIED, failure { session.listInbox() }.code)
        assertEquals(GmailAuthorizationState.REVOKED, session.status().state)
        assertEquals(1, gateway.tokenRequests)
        assertEquals(1, server.requestCount)
        assertEquals("Bearer fixture-token-gmail-session-first", server.takeRequest().getHeader("Authorization"))
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
