package app.serea.android

import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import android.content.Context
import android.util.Log
import com.google.gson.JsonParser
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.tls.HandshakeCertificates
import okhttp3.tls.HeldCertificate
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.util.UUID
import java.util.concurrent.TimeUnit

/** Compose input -> ChatViewModel -> local HTTP provider -> encrypted Room history. */
@RunWith(AndroidJUnit4::class)
class ChatHttpE2ETest {
    @get:Rule val compose = createComposeRule()
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private val loopbackCertificate = HeldCertificate.Builder().addSubjectAlternativeName("localhost").build()
    private val serverCertificates = HandshakeCertificates.Builder().heldCertificate(loopbackCertificate).build()
    private val clientCertificates = HandshakeCertificates.Builder()
        .addTrustedCertificate(loopbackCertificate.certificate).build()
    private val server = MockWebServer().apply {
        useHttps(serverCertificates.sslSocketFactory(), false)
        start()
    }
    private val db = Room.inMemoryDatabaseBuilder(context, SereaHistoryDatabase::class.java).build()
    private val alias = "http-e2e-${UUID.randomUUID()}"
    private val history = ConversationRepository(db.historyDao(), KeystoreAesGcm(alias))
    private val usage = UsageRepository(db.usageDao())
    private companion object { const val DIAG_TAG = "SereaChatInputRegression" }

    @After fun close() {
        db.close()
        server.shutdown()
        java.security.KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry(alias) }
    }

    @Test fun twoTurnsIncludePersistedHistoryAndUsageThroughLoopbackHttp() {
        server.enqueue(MockResponse().setBody("""{"message":{"content":"first answer"},"done":true,"prompt_eval_count":4,"eval_count":2}"""))
        server.enqueue(MockResponse().setBody("""{"message":{"content":"second answer"},"done":true,"prompt_eval_count":11,"eval_count":3}"""))
        val conversationId = "http-e2e-conversation"
        runBlocking { history.create(conversationId, "HTTP E2E") }
        val modelIds = listOf("fixture/first-model", "fixture/second-model")
        val models = modelIds.map { id -> ModelDescriptor(id, "fixture", id, Knowledge.Known(8192), Knowledge.Known(512),
            ModelCapabilities(setOf("text/plain")), ModelPricing(Knowledge.UNKNOWN, Knowledge.UNKNOWN), ModelAvailability.AVAILABLE,
            authorized = true, lastVerifiedAt = "2026-10-19T00:00:00Z", source = "instrumentation fixture (fictional)", verificationStatus = ModelVerificationStatus.TEST_FIXTURE) }
        val catalog = FakeModelCatalog(models)
        val router = AdaptiveModelRouter(catalog)
        var policy = ModelRoutingPolicy(ModelSelectionMode.MANUAL, manualModelId = modelIds.first(), allowedProviderIds = setOf("fixture"))
        val usagePort = UsageControlledModelChatPort(MockHttpChatPort(server.url("/api/chat"), clientCertificates), usage,
            attributionResolver = CatalogUsageAttributionResolver(catalog))
        // Router must be outside accounting so its explicit selectedModelId reaches the atomic reservation.
        val routedPort = RoutedModelChatPort(usagePort, router) { policy }
        activeVm = ChatViewModel(routedPort,
            history, usage, Conversation(conversationId))
        compose.setContent { SereaTheme { ChatScreen(activeVm, usage = usage) } }

        val composer = compose.onNodeWithTag("chat-composer")
        assertEquals(conversationId, activeVm.state.value.conversation.id)
        compose.waitUntil(30_000) { runCatching { composer.assertIsEnabled() }.isSuccess }
        sendAndAwait(composer, "first question", "first answer")
        policy = policy.copy(manualModelId = modelIds.last())
        sendAndAwait(composer, "second question", "second answer")

        val firstRequest = server.takeRequest(5, TimeUnit.SECONDS)!!
        val firstPayload = JsonParser.parseString(firstRequest.body.readUtf8()).asJsonObject
        val firstMessages = firstPayload.getAsJsonArray("messages")
        assertEquals(1, firstMessages.size())
        assertTrue(firstMessages[0].asJsonObject.get("content").asString == "first question")
        assertEquals(modelIds.first(), firstPayload.get("model").asString)

        val secondRequest = server.takeRequest(5, TimeUnit.SECONDS)!!
        val secondPayload = JsonParser.parseString(secondRequest.body.readUtf8()).asJsonObject
        val secondMessages = secondPayload.getAsJsonArray("messages")
        assertEquals(3, secondMessages.size())
        assertEquals(listOf("first question", "first answer", "second question"),
            (0 until secondMessages.size()).map { secondMessages[it].asJsonObject.get("content").asString })
        assertEquals(modelIds.last(), secondPayload.get("model").asString)
        assertTrue(secondMessages[0].asJsonObject.has("source_message_id"))
        assertEquals(2, server.requestCount)
        runBlocking {
            assertEquals(1L, usage.modelUsage(modelIds.first()).single().requestCount)
            assertEquals(1L, usage.modelUsage(modelIds.last()).single().requestCount)
            assertEquals("fixture", usage.providerModelUsage("fixture", modelIds.first()).single().providerId)
        }

        val saved = runBlocking { history.get(conversationId)!! }
        assertEquals(listOf("COMPLETED", "COMPLETED", "COMPLETED", "COMPLETED"), saved.messages.map { it.status })
        assertEquals(listOf(DataOrigin.USER_AUTHORED, DataOrigin.MODEL_OUTPUT, DataOrigin.USER_AUTHORED, DataOrigin.MODEL_OUTPUT),
            saved.messages.map { it.origin })
        val total = runBlocking { usage.snapshot() }
        assertEquals(2L, total.dailyRequests)
        assertEquals(0L, total.dailyEstimatedInputTokens + total.dailyEstimatedOutputTokens)
        assertEquals(20L, total.dailyProviderInputTokens + total.dailyProviderOutputTokens)
    }

    @Test fun manualRouterSelectionIsPassedToMockHttpRequestAndNeverFallsBack() = runBlocking {
        server.enqueue(MockResponse().setBody("""{"message":{"content":"routed answer"},"done":true,"prompt_eval_count":3,"eval_count":2}"""))
        val descriptor = ModelDescriptor("fixture/manual-v1", "fixture", "Fixture Manual",
            Knowledge.Known(8192), Knowledge.Known(512), ModelCapabilities(setOf("text/plain")),
            ModelPricing(Knowledge.UNKNOWN, Knowledge.UNKNOWN, Knowledge.UNKNOWN), ModelAvailability.AVAILABLE, authorized = true, lastVerifiedAt = "2026-10-19T00:00:00Z", source = "instrumentation fixture (fictional)", verificationStatus = ModelVerificationStatus.TEST_FIXTURE)
        val router = AdaptiveModelRouter(FakeModelCatalog(listOf(descriptor)))
        val routed = RoutedModelChatPort(MockHttpChatPort(server.url("/api/chat"), clientCertificates), router) {
            ModelRoutingPolicy(mode = ModelSelectionMode.MANUAL, manualModelId = "fixture/manual-v1", allowedProviderIds = setOf("fixture"))
        }
        val result = routed.send(ModelChatRequest("route-conversation", "hello"))
        assertEquals("routed answer", result.text)
        val sent = JsonParser.parseString(server.takeRequest(5, TimeUnit.SECONDS)!!.body.readUtf8()).asJsonObject
        assertEquals("fixture/manual-v1", sent.get("model").asString)

        val unavailable = RoutedModelChatPort(MockHttpChatPort(server.url("/api/chat"), clientCertificates), router) {
            ModelRoutingPolicy(mode = ModelSelectionMode.MANUAL, manualModelId = "missing", allowedProviderIds = setOf("fixture"))
        }
        runCatching { unavailable.send(ModelChatRequest("route-conversation", "hello")) }
            .onSuccess { throw AssertionError("missing manual model must fail") }
        assertEquals("unavailable manual selection made no HTTP request", 1, server.requestCount)
    }

    @Test fun directCompositionRoutesCatalogModelAndDenialPrecedesUsageReservation() = runBlocking {
        val ids = listOf("fixture/ollama-a", "fixture/ollama-b")
        val responseUsage = listOf(10 to 2, 7 to 3, 1 to 1)
        ids.plus("fixture/ollama-no-price").forEachIndexed { index, id ->
            val (inputTokens, outputTokens) = responseUsage[index]
            server.enqueue(MockResponse().setBody("""{"message":{"content":"$id answer"},"done":true,"prompt_eval_count":$inputTokens,"eval_count":$outputTokens}"""))
        }
        val verifiedAt = java.time.Instant.now().minusSeconds(30).toString()
        val modelIds = ids + "fixture/ollama-no-price"
        val models = modelIds.map { id -> ModelDescriptor(id, "ollama", id, Knowledge.Known(8192), Knowledge.Known(512),
            ModelCapabilities(setOf("text/plain")), ModelPricing(Knowledge.UNKNOWN, Knowledge.UNKNOWN),
            ModelAvailability.AVAILABLE, authorized = true, lastVerifiedAt = verifiedAt,
            source = "fictional local instrumentation fixture", verificationStatus = ModelVerificationStatus.TEST_FIXTURE) }
        val catalog = FakeModelCatalog(models)
        val priceCatalog = VerifiedModelPriceCatalog { id ->
            val terms = when (id) {
                ids.first() -> "2.00" to "8.00"
                ids.last() -> "1.50" to "4.00"
                else -> null
            }
            terms?.let { (input, output) ->
                VerifiedModelPriceSnapshot(id, VerifiedPriceSnapshot(
                    inputPricePerMillion = input, outputPricePerMillion = output, currency = "USD",
                    source = "fictional MockWebServer test price fixture", verifiedAt = verifiedAt,
                    effectiveAt = verifiedAt, revision = "fixture-$id-v1",
                ), java.time.Instant.now().plusSeconds(3600).toString())
            }
        }
        var selected = ids.first()
        val port = DirectProviderChatComposition.create(
            credentials = object : CredentialReader { override fun readForRequest() = "test-only-fake" },
            usage = usage,
            settings = CloudSettings(enabled = true),
            catalog = catalog,
            routingPolicy = { ModelRoutingPolicy(ModelSelectionMode.MANUAL, manualModelId = selected, allowedProviderIds = setOf("ollama")) },
            testEndpoint = server.url("/api/chat"),
            testClient = okhttp3.OkHttpClient.Builder().sslSocketFactory(
                clientCertificates.sslSocketFactory(), clientCertificates.trustManager).build(),
            usageAttributionResolver = CatalogUsageAttributionResolver(catalog, priceCatalog),
        )
        val now = System.currentTimeMillis()
        suspend fun send(id: String, message: String, context: List<ContextMessage> = emptyList(), classification: DataClassification = DataClassification.PUBLIC) =
            port.send(ModelChatRequest("routed-conversation", message, sendId = id, classification = classification,
                context = context, egressContract = EgressContract("routed-conversation", id, DataOrigin.USER_AUTHORED,
                    classification, "https://ollama.com/api/chat", message, context, PayloadPreparation.RAW, now, now + 30_000)))

        val first = send("routed-send-1", "first public question")
        assertEquals("${ids.first()} answer", first.text)
        val prior = listOf(ContextMessage(MessageRole.USER, "first public question", DataClassification.PUBLIC,
            "routed-send-1", DataOrigin.USER_AUTHORED, conversationId = "routed-conversation"))
        selected = ids.last()
        val second = send("routed-send-2", "second public question", prior)
        assertEquals("${ids.last()} answer", second.text)
        selected = modelIds.last()
        val noPrice = send("routed-send-no-price", "third public question")
        assertEquals("${modelIds.last()} answer", noPrice.text)
        val firstBody = JsonParser.parseString(server.takeRequest(5, TimeUnit.SECONDS)!!.body.readUtf8()).asJsonObject
        val secondBody = JsonParser.parseString(server.takeRequest(5, TimeUnit.SECONDS)!!.body.readUtf8()).asJsonObject
        val thirdBody = JsonParser.parseString(server.takeRequest(5, TimeUnit.SECONDS)!!.body.readUtf8()).asJsonObject
        assertEquals(ids.first(), firstBody.get("model").asString)
        assertEquals(ids.last(), secondBody.get("model").asString)
        assertEquals(modelIds.last(), thirdBody.get("model").asString)
        assertEquals("first public question", secondBody.getAsJsonArray("messages")[0].asJsonObject.get("content").asString)
        assertEquals(3, server.requestCount)
        assertEquals(3L, usage.snapshot().dailyRequests)
        val firstLedger = db.usageDao().ledger("routed-send-1")!!
        val secondLedger = db.usageDao().ledger("routed-send-2")!!
        val noPriceLedger = db.usageDao().ledger("routed-send-no-price")!!
        assertEquals(ids.first(), firstLedger.modelId)
        assertEquals(ids.last(), secondLedger.modelId)
        assertEquals("ollama", firstLedger.providerId)
        assertEquals("ollama", secondLedger.providerId)
        assertEquals("SETTLED", firstLedger.usageStatus)
        assertEquals(10L, firstLedger.providerInputTokens)
        assertEquals(2L, firstLedger.providerOutputTokens)
        assertEquals("0.000036", firstLedger.estimatedCost)
        assertEquals("fixture-${ids.first()}-v1", firstLedger.pricingRevision)
        assertEquals("SETTLED", secondLedger.usageStatus)
        assertEquals(7L, secondLedger.providerInputTokens)
        assertEquals(3L, secondLedger.providerOutputTokens)
        assertEquals("0.0000225", secondLedger.estimatedCost)
        assertEquals("UNKNOWN", noPriceLedger.costStatus)
        assertNull(noPriceLedger.estimatedCost)
        assertNull(noPriceLedger.currency)

        selected = ids.first()
        runCatching { send("routed-send-1", "first public question") }
            .onSuccess { throw AssertionError("a duplicate send ID must not dispatch twice") }
        assertEquals(3, server.requestCount)
        assertEquals(3L, usage.snapshot().dailyRequests)
        assertEquals(1L, usage.modelUsage(ids.first()).single().requestCount)

        selected = "fixture/unavailable-manual-model"
        runCatching { send("unavailable-manual-send", "must not fall back") }
            .onSuccess { throw AssertionError("an unavailable manual model must fail without fallback") }
        assertEquals(3, server.requestCount)
        assertEquals(3L, usage.snapshot().dailyRequests)
        assertNull(db.usageDao().reservation("unavailable-manual-send"))
        assertNull(db.usageDao().ledger("unavailable-manual-send"))

        val beforeDenied = usage.snapshot().dailyRequests
        try { send("routed-send-denied", "private question", classification = DataClassification.PRIVATE); throw AssertionError("egress must deny") }
        catch (_: ModelChatFailure) { }
        assertEquals(3, server.requestCount)
        assertEquals(beforeDenied, usage.snapshot().dailyRequests)
        assertNull(db.usageDao().reservation("routed-send-denied"))
        assertNull(db.usageDao().ledger("routed-send-denied"))
    }

    @Test fun innerEgressExpiryReleasesReservationBeforeAnyHttpOrCredentialAccess() = runBlocking {
        val modelId = "fixture/inner-gate"
        val now = java.time.Instant.now()
        val descriptor = ModelDescriptor(modelId, "ollama", "Fictional inner gate", Knowledge.Known(8192), Knowledge.Known(512),
            ModelCapabilities(setOf("text/plain")), ModelPricing(Knowledge.UNKNOWN, Knowledge.UNKNOWN),
            ModelAvailability.AVAILABLE, authorized = true, lastVerifiedAt = now.minusSeconds(30).toString(),
            source = "fictional inner gate fixture", verificationStatus = ModelVerificationStatus.TEST_FIXTURE)
        val catalog = FakeModelCatalog(listOf(descriptor))
        var credentialReads = 0
        val adapter = OllamaCloudAdapter(
            object : CredentialReader { override fun readForRequest(): String { credentialReads++; return "test-only-fake" } },
            CloudSettings(enabled = true), server.url("/api/chat"),
            okhttp3.OkHttpClient.Builder().sslSocketFactory(clientCertificates.sslSocketFactory(), clientCertificates.trustManager).build(),
            catalog,
        )
        var innerNow = 1000L
        val inner = OllamaModelChatPort(adapter) { innerNow }
        val usagePort = UsageControlledModelChatPort(
            delegate = object : ModelChatPort {
                override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                    innerNow = 1600L
                    return inner.send(request)
                }
            },
            usage = usage,
            attributionResolver = CatalogUsageAttributionResolver(catalog),
        )
        val outer = EgressCheckedModelChatPort(
            usagePort,
            clock = { 1000L },
            expectedTarget = EgressTarget.CLOUD,
        )
        val router = AdaptiveModelRouter(catalog)
        val routed = RoutedModelChatPort(outer, router) {
            ModelRoutingPolicy(ModelSelectionMode.MANUAL, manualModelId = modelId, allowedProviderIds = setOf("ollama"))
        }
        val request = ModelChatRequest(
            conversationId = "inner-gate-conversation", message = "public question", sendId = "inner-gate-denied",
            classification = DataClassification.PUBLIC,
            egressContract = EgressContract(
                "inner-gate-conversation", "inner-gate-denied", DataOrigin.USER_AUTHORED, DataClassification.PUBLIC,
                "https://ollama.com/api/chat", "public question", emptyList(), PayloadPreparation.RAW,
                explicitSendIntentAtMillis = 900L, expiresAtMillis = 1500L,
            ),
        )
        runCatching { routed.send(request) }.onSuccess { throw AssertionError("the independent inner egress check must reject the expired intent") }
        assertEquals(0, server.requestCount)
        assertEquals(0, credentialReads)
        assertEquals(0L, usage.snapshot().dailyRequests)
        assertNull(db.usageDao().reservation("inner-gate-denied"))
        assertNull(db.usageDao().ledger("inner-gate-denied"))

        val disabledPort = DirectProviderChatComposition.create(
            credentials = object : CredentialReader {
                override fun readForRequest(): String { credentialReads++; return "test-only-fake" }
            },
            usage = usage,
            settings = CloudSettings(enabled = false),
            catalog = catalog,
            routingPolicy = { ModelRoutingPolicy(ModelSelectionMode.MANUAL, manualModelId = modelId, allowedProviderIds = setOf("ollama")) },
            testEndpoint = server.url("/api/chat"),
            testClient = okhttp3.OkHttpClient.Builder().sslSocketFactory(clientCertificates.sslSocketFactory(), clientCertificates.trustManager).build(),
        )
        val disabledNow = System.currentTimeMillis()
        val disabledRequestId = "disabled-cloud-send"
        val disabledRequest = ModelChatRequest(
            conversationId = "disabled-cloud-conversation", message = "must remain local", sendId = disabledRequestId,
            classification = DataClassification.PUBLIC,
            egressContract = EgressContract(
                "disabled-cloud-conversation", disabledRequestId, DataOrigin.USER_AUTHORED, DataClassification.PUBLIC,
                "https://ollama.com/api/chat", "must remain local", emptyList(), PayloadPreparation.RAW,
                explicitSendIntentAtMillis = disabledNow, expiresAtMillis = disabledNow + 30_000,
            ),
        )
        runCatching { disabledPort.send(disabledRequest) }
            .onSuccess { throw AssertionError("disabled cloud must fail before dispatch") }
        assertEquals(0, server.requestCount)
        assertEquals(0, credentialReads)
        assertEquals(0L, usage.snapshot().dailyRequests)
        assertNull(db.usageDao().reservation(disabledRequestId))
        assertNull(db.usageDao().ledger(disabledRequestId))
    }

    private fun sendAndAwait(composer: androidx.compose.ui.test.SemanticsNodeInteraction, prompt: String, response: String) {
        val before = stateSnapshot()
        val previousMessageCount = before.conversation.messages.size
        val previousMessageIds = before.conversation.messages.mapTo(mutableSetOf()) { it.id }
        val expectedRequestCount = server.requestCount + 1
        logInputState("BEFORE_TYPE:$prompt")
        composer.performTextInput(prompt)
        logInputState("AFTER_TYPE:$prompt")
        compose.onNodeWithText("送信").performClick()
        logInputState("AFTER_SEND_CLICK:$prompt")
        try {
            compose.waitUntil(30_000) {
                val current = stateSnapshot()
                val turn = current.conversation.messages.drop(previousMessageCount)
                val sendId = turn.firstOrNull()?.id
                current.conversation.id == before.conversation.id &&
                    turn.size == 2 &&
                    !sendId.isNullOrBlank() && sendId !in previousMessageIds &&
                    turn[0].role == MessageRole.USER && turn[0].text == prompt && turn[0].status == "COMPLETED" &&
                    turn[1].role == MessageRole.ASSISTANT && turn[1].text == response && turn[1].status == "COMPLETED" &&
                    !current.isLoading && current.errorMessage == null && server.requestCount == expectedRequestCount
            }
        } catch (failure: Throwable) {
            val pendingRequests = buildList {
                repeat(server.requestCount) {
                    server.takeRequest(100, TimeUnit.MILLISECONDS)?.let { add(it.method + " " + it.path + " " + it.body.readUtf8()) }
                }
            }
            logInputState("WAIT_FAILURE:$prompt pendingRequests=$pendingRequests failure=$failure")
            throw failure
        }
        val completed = stateSnapshot()
        logInputState("COMPLETED:$prompt")
        val turn = completed.conversation.messages.drop(previousMessageCount)
        assertEquals("The new send did not produce exactly one user/assistant turn", 2, turn.size)
        assertEquals(prompt, turn[0].text)
        assertTrue("The current send ID was not persisted as a new message", turn[0].id.isNotBlank() && turn[0].id !in previousMessageIds)
        assertEquals("COMPLETED", turn[0].status)
        assertEquals(response, turn[1].text)
        assertEquals("COMPLETED", turn[1].status)
        assertEquals(expectedRequestCount, server.requestCount)
        assertFalse("Request remained active; state=$completed", completed.isLoading)
        assertNull("Request failed; state=$completed", completed.errorMessage)
    }

    private fun stateSnapshot(): ChatUiState {
        // The test owns the VM and observes its StateFlow directly.
        return activeVm.state.value
    }

    private fun logInputState(stage: String) {
        val state = stateSnapshot()
        val composer = runCatching {
            val nodes = compose.onAllNodesWithTag("chat-composer", useUnmergedTree = true).fetchSemanticsNodes()
            val text = nodes.singleOrNull()?.config?.getOrElseNullable(SemanticsProperties.EditableText) { null }?.text
            "count=${nodes.size};composerValueText=$text;matchesStateInput=${text == state.input};" + nodes.joinToString(";") {
                "id=${it.id},editable=${it.config.getOrElseNullable(SemanticsProperties.EditableText) { null }},bounds=${it.boundsInWindow}"
            }
        }.getOrElse { "unavailable:$it" }
        val saved = runCatching { runBlocking { history.get(state.conversation.id)?.messages?.map { it.role to it.status } } }
            .getOrElse { "failed:$it" }
        val usageState = runCatching { runBlocking { usage.snapshot() } }.getOrElse { "failed:$it" }
        Log.i(DIAG_TAG, "$stage|stateInput=${state.input}|composerEditable=$composer|loading=${state.isLoading}|error=${state.errorMessage}|messages=${state.conversation.messages.map { Triple(it.role, it.text, it.status) }}|httpRequestCount=${server.requestCount}|history=$saved|usage=$usageState")
    }

    private lateinit var activeVm: ChatViewModel
}
