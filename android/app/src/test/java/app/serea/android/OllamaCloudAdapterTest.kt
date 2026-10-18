package app.serea.android

import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.TimeoutCancellationException
import okhttp3.OkHttpClient
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.SocketPolicy
import okhttp3.tls.HandshakeCertificates
import okhttp3.tls.HeldCertificate
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class OllamaCloudAdapterTest {
    private val server = MockWebServer().apply { start() }
    private val key = "test-only-not-a-real-key"
    private fun adapter(enabled: Boolean = true, reader: CredentialReader = object : CredentialReader {
        override fun readForRequest() = key
    }) = OllamaCloudAdapter(reader, CloudSettings(enabled), server.url("/api/chat"),
        OkHttpClient.Builder().followRedirects(false).followSslRedirects(false).build())

    @After fun close() { server.shutdown() }

    @Test fun streamsChunksAndReturnsUsage() = runBlocking {
        server.enqueue(MockResponse().setHeader("Content-Type", "application/x-ndjson")
            .setChunkedBody("""{"model":"gemma4:31b","message":{"role":"assistant","content":"Hello "},"done":false}
                {"model":"gemma4:31b","message":{"role":"assistant","content":"there"},"done":true,"prompt_eval_count":7,"eval_count":2}
            """.trimIndent(), 5))
        val deltas = mutableListOf<String>()
        val result = adapter().send(EgressApprovedPayload.fixedPublicContractProbe()) { deltas += it }
        assertEquals("Hello there", result.text)
        assertEquals(listOf("Hello ", "there"), deltas)
        assertEquals(ProviderUsage(7, 2), result.usage)
        val request = server.takeRequest()
        assertEquals("Bearer $key", request.getHeader("Authorization"))
        val body = request.body.readUtf8()
        assertTrue(body.contains("Serea cloud contract probe"))
        assertFalse(body.contains(key))
    }

    @Test fun missingCredentialDoesNotCreateHttpRequest() = runBlocking {
        var reads = 0
        val error = capture { adapter(reader = object : CredentialReader { override fun readForRequest(): String { reads++; throw IllegalStateException() } })
            .send(EgressApprovedPayload.fixedPublicContractProbe()) }
        assertEquals(ProviderFailureKind.CREDENTIAL_MISSING, error.kind)
        assertEquals(1, reads)
        assertEquals(0, server.requestCount)
    }

    @Test fun invalidatedCredentialFailsBeforeHttp() = runBlocking {
        val failure = capture {
            adapter(reader = object : CredentialReader {
                override fun readForRequest(): String = throw HistoryUnavailableException("ciphertext is not available")
            }).send(EgressApprovedPayload.fixedPublicContractProbe())
        }
        assertEquals(ProviderFailureKind.CREDENTIAL_INVALID, failure.kind)
        assertEquals(0, server.requestCount)
    }

    @Test fun disabledCloudDoesNotReadCredentialOrConnect() = runBlocking {
        var reads = 0
        val error = capture { adapter(false, object : CredentialReader { override fun readForRequest(): String { reads++; return key } })
            .send(EgressApprovedPayload.fixedPublicContractProbe()) }
        assertEquals(ProviderFailureKind.DISABLED, error.kind)
        assertEquals(0, reads)
        assertEquals(0, server.requestCount)
    }

    @Test fun unlistedModelIsRejectedBeforeNetwork() = runBlocking {
        val failure = capture {
            OllamaCloudAdapter(object : CredentialReader { override fun readForRequest() = key }, CloudSettings(true, "not-a-listed-model"),
                server.url("/api/chat"), OkHttpClient()).send(EgressApprovedPayload.fixedPublicContractProbe())
        }
        assertEquals(ProviderFailureKind.MODEL_UNAVAILABLE, failure.kind)
        assertEquals(0, server.requestCount)
    }

    @Test fun arbitraryHttpsEndpointCannotBeConfigured() {
        try {
            OllamaCloudAdapter(object : CredentialReader { override fun readForRequest() = key }, CloudSettings(true),
                "https://attacker.example/api/chat".toHttpUrl(), OkHttpClient())
            throw AssertionError("arbitrary host must be rejected")
        } catch (_: IllegalArgumentException) { }
        assertEquals(0, server.requestCount)
    }

    @Test fun mapsAuthenticationRateLimitAndServerFailuresWithoutDisplayingBody() = runBlocking {
        listOf(401 to ProviderFailureKind.AUTHENTICATION, 403 to ProviderFailureKind.FORBIDDEN,
            404 to ProviderFailureKind.MODEL_UNAVAILABLE, 429 to ProviderFailureKind.RATE_LIMITED,
            500 to ProviderFailureKind.SERVER, 503 to ProviderFailureKind.SERVER).forEach { (code, kind) ->
            server.enqueue(MockResponse().setResponseCode(code).setBody("private provider diagnostic"))
            val error = capture { adapter().send(EgressApprovedPayload.fixedPublicContractProbe()) }
            assertEquals(kind, error.kind)
            assertFalse(error.message.orEmpty().contains("private provider diagnostic"))
        }
    }

    @Test fun redirectsAreNotFollowed() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(302).setHeader("Location", server.url("/steal")))
        val error = capture { adapter().send(EgressApprovedPayload.fixedPublicContractProbe()) }
        assertEquals(ProviderFailureKind.INVALID_RESPONSE, error.kind)
        assertEquals(1, server.requestCount)
        assertNull(server.takeRequest().getHeader("X-Unexpected"))
    }

    @Test fun malformedAndOversizedStreamsFailClosed() = runBlocking {
        server.enqueue(MockResponse().setBody("not-json\n"))
        assertEquals(ProviderFailureKind.INVALID_RESPONSE, capture { adapter().send(EgressApprovedPayload.fixedPublicContractProbe()) }.kind)
        server.enqueue(MockResponse().setBody("x".repeat(70_000) + "\n"))
        assertEquals(ProviderFailureKind.LIMIT_EXCEEDED, capture { adapter().send(EgressApprovedPayload.fixedPublicContractProbe()) }.kind)
        server.enqueue(MockResponse().setBody(" ".repeat(1024 * 1024 + 1)))
        assertEquals(ProviderFailureKind.LIMIT_EXCEEDED, capture { adapter().send(EgressApprovedPayload.fixedPublicContractProbe()) }.kind)
        server.enqueue(MockResponse().setBody(""))
        assertEquals(ProviderFailureKind.INVALID_RESPONSE, capture { adapter().send(EgressApprovedPayload.fixedPublicContractProbe()) }.kind)
        server.enqueue(MockResponse().setBody("{\"message\":{\"content\":\"partial\"},\"done\":false}\n"))
        assertEquals(ProviderFailureKind.INVALID_RESPONSE, capture { adapter().send(EgressApprovedPayload.fixedPublicContractProbe()) }.kind)
    }

    @Test fun timeoutCancellationAndStreamCutoffDoNotRetry() = runBlocking {
        server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.NO_RESPONSE))
        try { withTimeout(250) { adapter().send(EgressApprovedPayload.fixedPublicContractProbe()) }; throw AssertionError("expected timeout") }
        catch (_: TimeoutCancellationException) { }
        assertEquals(1, server.requestCount)

        server.enqueue(MockResponse().setSocketPolicy(SocketPolicy.DISCONNECT_DURING_RESPONSE_BODY)
            .setBody("{".repeat(80_000)))
        assertEquals(ProviderFailureKind.NETWORK, capture { adapter().send(EgressApprovedPayload.fixedPublicContractProbe()) }.kind)
        assertEquals(2, server.requestCount)
    }

    @Test fun connectionFailureIsMappedWithoutRetry() = runBlocking {
        val endpoint = server.url("/api/chat")
        server.shutdown()
        val failure = capture {
            OllamaCloudAdapter(object : CredentialReader { override fun readForRequest() = key }, CloudSettings(true), endpoint,
                OkHttpClient.Builder().followRedirects(false).build()).send(EgressApprovedPayload.fixedPublicContractProbe())
        }
        assertEquals(ProviderFailureKind.NETWORK, failure.kind)
    }

    @Test fun untrustedTlsCertificateIsRejectedBeforeHttpRequest() = runBlocking {
        val certificate = HeldCertificate.Builder().addSubjectAlternativeName("localhost").build()
        val serverTrust = HandshakeCertificates.Builder().heldCertificate(certificate).build()
        server.useHttps(serverTrust.sslSocketFactory(), false)
        val failure = capture {
            OllamaCloudAdapter(object : CredentialReader { override fun readForRequest() = key }, CloudSettings(true),
                server.url("/api/chat"), OkHttpClient.Builder().followRedirects(false).followSslRedirects(false).build())
                .send(EgressApprovedPayload.fixedPublicContractProbe())
        }
        assertEquals(ProviderFailureKind.NETWORK, failure.kind)
        assertEquals(0, server.requestCount)
    }

    private suspend fun capture(block: suspend () -> CloudReply): ProviderFailure = try {
        block(); throw AssertionError("Expected ProviderFailure")
    } catch (failure: ProviderFailure) { failure }
}
