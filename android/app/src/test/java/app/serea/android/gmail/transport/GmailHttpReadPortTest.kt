package app.serea.android.gmail.transport

import app.serea.android.gmail.GmailAuthorizationPort
import app.serea.android.gmail.GmailAuthorizationState
import app.serea.android.gmail.GmailAuthorizationStatus
import app.serea.android.gmail.GmailMessageId
import app.serea.android.gmail.GmailReadFailure
import app.serea.android.gmail.GmailReadFailureCode
import app.serea.android.gmail.GmailThreadId
import app.serea.android.googleauth.AccessTokenLease
import app.serea.android.googleauth.FakeGoogleAuthorizationGateway
import app.serea.android.googleauth.AuthorizationStatus
import app.serea.android.googleauth.GoogleAuthorizationGateway
import app.serea.android.googleauth.GoogleAuthorizationMode
import app.serea.android.googleauth.GoogleFeature
import app.serea.android.googleauth.GoogleServiceState
import app.serea.android.googleauth.ScopePermission
import app.serea.android.googleauth.GoogleScope
import app.serea.android.googleauth.GoogleGmailAuthorizationAdapter
import app.serea.android.googleauth.TokenSupplyResult
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.SocketPolicy
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.util.concurrent.TimeUnit

class GmailHttpReadPortTest {
    private lateinit var server: MockWebServer
    private var authorizationStatus = GmailAuthorizationStatus(GmailAuthorizationState.AUTHORIZED)
    private var tokenRequests = 0

    @Before fun setUp() {
        server = MockWebServer().apply { start() }
        authorizationStatus = GmailAuthorizationStatus(GmailAuthorizationState.AUTHORIZED)
        tokenRequests = 0
    }

    @After fun close() = server.shutdown()

    private fun port(
        invalidatedToken: Boolean = false,
        client: OkHttpClient = OkHttpClient.Builder().followRedirects(false).followSslRedirects(false)
            .retryOnConnectionFailure(false).build(),
    ) = GmailHttpReadPort(
        object : GmailAuthorizationPort {
            override suspend fun status() = authorizationStatus
        },
        GmailAccessTokenPort { scope ->
            tokenRequests++
            if (scope != GMAIL_SCOPE) return@GmailAccessTokenPort null
            val lease = AccessTokenLease("fixture-token-never-valid", setOf(GoogleScope.GMAIL_READ_ONLY), null) {
                !invalidatedToken
            }
            GmailAccessToken(lease)
        },
        client,
        server.url("/gmail/v1/"),
    )

    @Test fun inboxListsPageAndFetchesOnlyReadOnlyFullMessages() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"messages":[{"id":"message_1","threadId":"thread_1"}],"nextPageToken":"next +/="}"""))
        server.enqueue(MockResponse().setBody(messageJson("message_1", "thread_1")))

        val page = port().listInbox(pageToken = "previous +/=", pageSize = 25)

        assertEquals("Hello from fixture", page.messages.single().plainTextBody)
        assertEquals("thread_1", page.messages.single().threadId.value)
        assertEquals("next +/=", page.nextPageToken)
        val list = server.takeRequest()
        assertEquals("GET", list.method)
        assertEquals("INBOX", list.requestUrl!!.queryParameter("labelIds"))
        assertEquals("25", list.requestUrl!!.queryParameter("maxResults"))
        assertEquals("previous +/=", list.requestUrl!!.queryParameter("pageToken"))
        assertEquals(listOf("gmail", "v1", "users", "me", "messages"), list.requestUrl!!.pathSegments)
        val message = server.takeRequest()
        assertEquals("GET", message.method)
        assertEquals("full", message.requestUrl!!.queryParameter("format"))
        assertEquals(listOf("gmail", "v1", "users", "me", "messages", "message_1"), message.requestUrl!!.pathSegments)
        assertEquals("Bearer fixture-token-never-valid", message.getHeader("Authorization"))
        assertEquals(2, tokenRequests)
    }

    @Test fun searchQueryIsEncodedAndThreadFetchVerifiesMessageOwnership() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"messages":[{"id":"message_2","threadId":"thread_2"}]}"""))
        server.enqueue(MockResponse().setBody(messageJson("message_2", "thread_2")))
        val searched = port().search("from:person@example.invalid subject:hello", pageSize = 10)
        assertEquals("message_2", searched.messages.single().id.value)
        assertEquals("from:person@example.invalid subject:hello", server.takeRequest().requestUrl!!.queryParameter("q"))
        server.takeRequest()

        server.enqueue(MockResponse().setBody("""{"id":"thread_2","messages":[${messageJson("message_2", "thread_2")},${messageJson("message_3", "thread_2")}] }"""))
        val thread = port().getThread(GmailThreadId("thread_2"))
        assertEquals(listOf("message_2", "message_3"), thread.messages.map { it.id.value })
        val request = server.takeRequest()
        assertEquals("GET", request.method)
        assertEquals("full", request.requestUrl!!.queryParameter("format"))
        assertEquals(listOf("gmail", "v1", "users", "me", "threads", "thread_2"), request.requestUrl!!.pathSegments)
    }

    @Test fun messageLookupRejectsAResponseForAnotherId() = runBlocking {
        server.enqueue(MockResponse().setBody(messageJson("other_message", "thread_1")))
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { port().getMessage(GmailMessageId("requested")) }.code)
    }

    @Test fun deniedMissingAndExpiredAuthorizationFailBeforeTokensOrHttp() = runBlocking {
        val invalidStates = listOf(
            GmailAuthorizationStatus(GmailAuthorizationState.DENIED),
            GmailAuthorizationStatus(GmailAuthorizationState.EXPIRED),
            GmailAuthorizationStatus(GmailAuthorizationState.AUTHORIZED, setOf("https://www.googleapis.com/auth/gmail.readonly")),
        )
        for (status in invalidStates) {
            authorizationStatus = status
            val error = failure { port().listInbox() }
            assertTrue(error.code in setOf(GmailReadFailureCode.PERMISSION_DENIED, GmailReadFailureCode.MISSING_SCOPE))
            assertEquals(0, tokenRequests)
            assertEquals(0, server.requestCount)
        }
    }

    @Test fun mockGoogleAuthorizationCannotAuthorizeTheHttpAdapter() = runBlocking {
        val mockGateway = FakeGoogleAuthorizationGateway(setOf(GoogleScope.GMAIL_READ_ONLY))
        val bridge = GoogleGmailAuthorizationAdapter(mockGateway)
        val realHttp = GmailHttpReadPort(bridge, bridge, OkHttpClient(), server.url("/gmail/v1/"))
        assertEquals(GmailAuthorizationState.MOCK_ONLY, bridge.status().state)
        assertEquals(GmailReadFailureCode.PERMISSION_DENIED, failure { realHttp.listInbox() }.code)
        assertEquals(0, server.requestCount)
    }

    @Test fun googleBridgeRequestsOnlyGmailReadOnlyAndRejectsLeasesWithoutIt() = runBlocking {
        val acquisitions = mutableListOf<Pair<GoogleFeature, Set<GoogleScope>>>()
        val gateway = object : GoogleAuthorizationGateway {
            override val mode = GoogleAuthorizationMode.PLAY_SERVICES
            override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { ScopePermission.GRANTED },
            )
            override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
                acquisitions += feature to scopes
                return TokenSupplyResult.Available(AccessTokenLease("fixture-gmail-token", scopes, null))
            }
        }
        val disabled = GoogleGmailAuthorizationAdapter(gateway)
        assertEquals(GmailAuthorizationState.NOT_CONFIGURED, disabled.status().state)
        assertEquals(null, disabled.accessToken(GMAIL_SCOPE))
        assertTrue(acquisitions.isEmpty())

        val bridge = GoogleGmailAuthorizationAdapter(gateway, googleHttpEnabledByOwner = true)
        val broadScope = bridge.accessToken("https://www.googleapis.com/auth/gmail.modify")
        assertEquals(null, broadScope)
        assertTrue(acquisitions.isEmpty())

        server.enqueue(MockResponse().setBody("""{"messages":[]}"""))
        val page = GmailHttpReadPort(bridge, bridge, OkHttpClient(), server.url("/gmail/v1/")).listInbox()
        assertTrue(page.messages.isEmpty())
        assertEquals(listOf(GoogleFeature.GMAIL to setOf(GoogleScope.GMAIL_READ_ONLY)), acquisitions)
        assertEquals("Bearer fixture-gmail-token", server.takeRequest().getHeader("Authorization"))

        val insufficient = GoogleGmailAuthorizationAdapter(object : GoogleAuthorizationGateway {
            override val mode = GoogleAuthorizationMode.PLAY_SERVICES
            override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { ScopePermission.GRANTED },
            )
            override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>) =
                TokenSupplyResult.Available(AccessTokenLease("fixture-wrong-scope-token", emptySet(), null))
        }, googleHttpEnabledByOwner = true)
        assertEquals(null, insufficient.accessToken(GMAIL_SCOPE))
        assertEquals(1, server.requestCount)
    }

    @Test fun invalidatedTokenLeaseDoesNotReachTheServer() = runBlocking {
        assertEquals(GmailReadFailureCode.PERMISSION_DENIED, failure { port(invalidatedToken = true).listInbox() }.code)
        assertEquals(1, tokenRequests)
        assertEquals(0, server.requestCount)
    }

    @Test fun httpErrorsAreTypedAndNeverExposeProviderBody() = runBlocking {
        val cases = listOf(
            401 to GmailReadFailureCode.PERMISSION_DENIED,
            403 to GmailReadFailureCode.MISSING_SCOPE,
            403 to GmailReadFailureCode.RATE_LIMITED,
            404 to GmailReadFailureCode.NOT_FOUND,
            429 to GmailReadFailureCode.RATE_LIMITED,
            503 to GmailReadFailureCode.UNAVAILABLE,
        )
        for ((http, expected) in cases) {
            val reason = when (http to expected) {
                403 to GmailReadFailureCode.RATE_LIMITED -> "quotaExceeded"
                403 to GmailReadFailureCode.MISSING_SCOPE -> "insufficientPermissions"
                else -> "private-fixture-reason"
            }
            server.enqueue(MockResponse().setResponseCode(http).setBody(
                """{"error":{"errors":[{"reason":"$reason"}],"message":"private fixture text"}}""",
            ))
            val error = failure { port().getMessage(GmailMessageId("message_1")) }
            assertEquals(expected, error.code)
            assertFalse(error.message.orEmpty().contains("private fixture"))
        }
        assertEquals(6, server.requestCount)
    }

    @Test fun oversizedAndMalformedResponsesFailWithStableError() = runBlocking {
        server.enqueue(MockResponse().setBody(" ".repeat(2 * 1024 * 1024 + 1)))
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { port().getMessage(GmailMessageId("message_1")) }.code)
        server.enqueue(MockResponse().setBody("{broken"))
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { port().listInbox() }.code)
    }

    @Test fun communicationFailureTimeoutAndCancellationAreBounded() = runBlocking {
        server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.DISCONNECT_AT_START))
        assertEquals(GmailReadFailureCode.UNAVAILABLE, failure { port().getMessage(GmailMessageId("message_1")) }.code)

        server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE))
        val timeout = OkHttpClient.Builder().callTimeout(100, TimeUnit.MILLISECONDS)
            .connectTimeout(100, TimeUnit.MILLISECONDS).readTimeout(100, TimeUnit.MILLISECONDS).build()
        assertEquals(GmailReadFailureCode.TIMEOUT, failure { port(client = timeout).getMessage(GmailMessageId("message_1")) }.code)

        server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE))
        val deferred = async { port().getMessage(GmailMessageId("message_1")) }
        server.takeRequest(2, TimeUnit.SECONDS)
        deferred.cancelAndJoin()
        assertTrue(deferred.isCancelled)
    }

    private suspend fun failure(block: suspend () -> Any?): GmailReadFailure = try {
        block()
        throw AssertionError("Expected a typed Gmail failure")
    } catch (failure: GmailReadFailure) { failure }

    private fun messageJson(id: String, threadId: String) = """{
        "id":"$id","threadId":"$threadId","internalDate":"1792454400000","snippet":"fixture",
        "payload":{"mimeType":"text/plain","headers":[
          {"name":"From","value":"Person <person@example.invalid>"},
          {"name":"Subject","value":"Fictional subject"}
        ],"body":{"size":18,"data":"SGVsbG8gZnJvbSBmaXh0dXJl"}}
    }""".trimIndent()

    companion object {
        private const val GMAIL_SCOPE = "https://www.googleapis.com/auth/gmail.readonly"
    }
}
