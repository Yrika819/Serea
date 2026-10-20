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
import kotlin.random.Random

class OllamaCloudAdapterTest {
    private val server = MockWebServer().apply { start() }
    private val key = "test-only-not-a-real-key"
    private fun adapter(enabled: Boolean = true, reader: CredentialReader = object : CredentialReader {
        override fun readForRequest() = key
    }) = OllamaCloudAdapter(reader, CloudSettings(enabled, "gemma4:31b"), server.url("/api/chat"),
        OkHttpClient.Builder().followRedirects(false).followSslRedirects(false).build())

    @After fun close() { server.shutdown() }

    @Test fun streamsChunksAndReturnsUsage() = runBlocking {
        server.enqueue(MockResponse().setHeader("Content-Type", "application/x-ndjson")
            .setChunkedBody("""{"model":"gemma4:31b","message":{"role":"assistant","content":"Hello "},"done":false}
                {"model":"gemma4:31b","message":{"role":"assistant","content":"there"},"done":true,"prompt_eval_count":7,"eval_count":2}
            """.trimIndent(), 5))
        val deltas = mutableListOf<String>()
        val result = adapter().send(EgressApprovedPayload.fixedPublicContractProbe(), onDelta = { deltas += it })
        assertEquals("Hello there", result.text)
        assertEquals(listOf("Hello ", "there"), deltas)
        assertEquals(ProviderUsage(7, 2), result.usage)
        val request = server.takeRequest()
        assertEquals("Bearer $key", request.getHeader("Authorization"))
        val body = request.body.readUtf8()
        assertTrue(body.contains("Serea cloud contract probe"))
        assertFalse(body.contains(key))
    }

    @Test fun acceptsDocumentedNonStreamingJsonResponseAndUsage() = runBlocking {
        server.enqueue(MockResponse().setHeader("Content-Type", "application/json")
            .setBody("""{"model":"gemma4:31b","message":{"role":"assistant","content":"single response"},"done":true,"prompt_eval_count":8,"eval_count":3}"""))
        val result = adapter().send(EgressApprovedPayload.fixedPublicContractProbe())
        assertEquals("single response", result.text)
        assertEquals(ProviderUsage(8, 3), result.usage)
        assertEquals(1, server.requestCount)
    }

    @Test fun missingProviderUsageRemainsUnknown() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"message":{"content":"answer"},"done":true}"""))
        val result = adapter().send(EgressApprovedPayload.fixedPublicContractProbe())
        assertEquals("answer", result.text)
        assertNull(result.usage)
    }

    @Test fun approvedMultiTurnConversationIsSentAsOneMockHttpRequest() = runBlocking {
        server.enqueue(MockResponse().setHeader("Content-Type", "application/x-ndjson")
            .setBody("""{"message":{"content":"second answer"},"done":true,"prompt_eval_count":19,"eval_count":3}
"""))
        val now = 10_000L
        val context = listOf(ContextMessage(MessageRole.USER, "first question", DataClassification.PUBLIC,
            "source-1", DataOrigin.USER_AUTHORED, conversationId = "conversation-1"))
        val contract = EgressContract("conversation-1", "send-2", DataOrigin.USER_AUTHORED,
            DataClassification.PUBLIC, "https://ollama.com/api/chat", "second question", context,
            PayloadPreparation.RAW, now, now + 30_000)
        val request = ModelChatRequest("conversation-1", "second question", sendId = "send-2",
            classification = DataClassification.PUBLIC, context = context, egressContract = contract)
        val result = OllamaModelChatPort(adapter(), { now }).send(request)
        assertEquals("second answer", result.text)
        assertEquals(ProviderUsage(19, 3), result.usage)
        val sent = server.takeRequest()
        val body = sent.body.readUtf8()
        assertTrue(body.contains("first question"))
        assertTrue(body.contains("second question"))
        assertTrue(body.contains("\"role\":\"user\""))
        assertEquals(1, server.requestCount)
    }

    @Test fun egressRejectionDoesNotReadCredentialOrCreateHttpRequest() = runBlocking {
        var credentialReads = 0
        val port = OllamaModelChatPort(adapter(reader = object : CredentialReader {
            override fun readForRequest(): String { credentialReads++; return key }
        }), { 20_000 })
        val context = listOf(ContextMessage(MessageRole.USER, "private prior", DataClassification.PRIVATE,
            "source-private", DataOrigin.USER_AUTHORED))
        val contract = EgressContract("c", "s", DataOrigin.USER_AUTHORED, DataClassification.PUBLIC,
            "https://ollama.com/api/chat", "current", context, PayloadPreparation.RAW, 10_000, 30_000)
        try {
            port.send(ModelChatRequest("c", "current", sendId = "s", classification = DataClassification.PUBLIC,
                context = context, egressContract = contract))
            throw AssertionError("private context must be denied")
        } catch (_: ModelChatFailure) { }
        assertEquals(0, credentialReads)
        assertEquals(0, server.requestCount)
    }

    @Test fun streamsSeededManyShortDeltasAcrossFragmentedTransportChunks() = runBlocking {
        val seed = 0x5E2EA
        val chunks = (0 until 256).map { index ->
            "{\"model\":\"gemma4:31b\",\"message\":{\"role\":\"assistant\",\"content\":\"$index,\"},\"done\":false}\n"
        } + "{\"model\":\"gemma4:31b\",\"message\":{\"role\":\"assistant\",\"content\":\"done\"},\"done\":true}\n"
        val expected = (0 until 256).joinToString(separator = "") { "$it," } + "done"
        val body = chunks.joinToString(separator = "")
        val transportChunkBytes = Random(seed).nextInt(1, 12)
        server.enqueue(MockResponse().setHeader("Content-Type", "application/x-ndjson")
            .setChunkedBody(body, transportChunkBytes))

        val deltas = mutableListOf<String>()
        val result = adapter().send(EgressApprovedPayload.fixedPublicContractProbe(), onDelta = { deltas += it })
        assertEquals(expected, result.text)
        assertEquals(expected, deltas.joinToString(separator = ""))
        assertEquals(1, server.requestCount)
    }

    @Test fun fragmentedUtf8CodePointIsDecodedWithoutCorruption() = runBlocking {
        server.enqueue(MockResponse().setHeader("Content-Type", "application/x-ndjson")
            .setChunkedBody("""{"message":{"content":"🌱"},"done":true,"prompt_eval_count":1,"eval_count":1}
""", 1))
        val result = adapter().send(EgressApprovedPayload.fixedPublicContractProbe())
        assertEquals("🌱", result.text)
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

    @Test fun historicalCandidateIsNotDefaultAndExplicitSelectionIsUsed() = runBlocking {
        assertEquals("", CloudSettings().modelId)
        server.enqueue(MockResponse().setBody("""{"message":{"content":"ok"},"done":true}"""))
        val configuredWithoutDefault = OllamaCloudAdapter(object : CredentialReader { override fun readForRequest() = key },
            CloudSettings(enabled = true), server.url("/api/chat"), OkHttpClient())
        configuredWithoutDefault.send(EgressApprovedPayload.fixedPublicContractProbe(), selectedModelId = "gemma4:31b")
        val request = server.takeRequest()
        assertEquals("gemma4:31b", com.google.gson.JsonParser.parseString(request.body.readUtf8()).asJsonObject.get("model").asString)
    }

    @Test fun unlistedModelIsRejectedBeforeNetwork() = runBlocking {
        val failure = capture {
            OllamaCloudAdapter(object : CredentialReader { override fun readForRequest() = key }, CloudSettings(true, "not-a-listed-model"),
                server.url("/api/chat"), OkHttpClient()).send(EgressApprovedPayload.fixedPublicContractProbe())
        }
        assertEquals(ProviderFailureKind.MODEL_UNAVAILABLE, failure.kind)
        assertEquals(0, server.requestCount)
        val overrideFailure = capture { adapter().send(EgressApprovedPayload.fixedPublicContractProbe(), selectedModelId = "https://attacker.example") }
        assertEquals(ProviderFailureKind.MODEL_UNAVAILABLE, overrideFailure.kind)
        assertEquals(0, server.requestCount)
    }

    @Test fun arbitraryHttpsEndpointCannotBeConfigured() {
        try {
            OllamaCloudAdapter(object : CredentialReader { override fun readForRequest() = key }, CloudSettings(true, "gemma4:31b"),
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
            OllamaCloudAdapter(object : CredentialReader { override fun readForRequest() = key }, CloudSettings(true, "gemma4:31b"), endpoint,
                OkHttpClient.Builder().followRedirects(false).build()).send(EgressApprovedPayload.fixedPublicContractProbe())
        }
        assertEquals(ProviderFailureKind.NETWORK, failure.kind)
    }

    @Test fun untrustedTlsCertificateIsRejectedBeforeHttpRequest() = runBlocking {
        val certificate = HeldCertificate.Builder().addSubjectAlternativeName("localhost").build()
        val serverTrust = HandshakeCertificates.Builder().heldCertificate(certificate).build()
        server.useHttps(serverTrust.sslSocketFactory(), false)
        val failure = capture {
            OllamaCloudAdapter(object : CredentialReader { override fun readForRequest() = key }, CloudSettings(true, "gemma4:31b"),
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
