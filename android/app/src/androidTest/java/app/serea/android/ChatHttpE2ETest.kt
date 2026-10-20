package app.serea.android

import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import android.content.Context
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
            authorized = true, lastVerifiedAt = "2026-10-19T00:00:00Z", source = "instrumentation fixture") }
        val router = AdaptiveModelRouter(LocalModelCatalog(models))
        var policy = ModelRoutingPolicy(ModelSelectionMode.MANUAL, manualModelId = modelIds.first(), allowedProviderIds = setOf("fixture"))
        val attribution = UsageAttributionResolver { request ->
            val descriptor = models.firstOrNull { it.id == request.selectedModelId }
            ModelUsageAttribution(
                modelId = request.selectedModelId ?: ModelUsageAttribution.UNKNOWN_ATTRIBUTION,
                providerId = descriptor?.providerId ?: ModelUsageAttribution.UNKNOWN_ATTRIBUTION,
            )
        }
        val usagePort = UsageControlledModelChatPort(MockHttpChatPort(server.url("/api/chat"), clientCertificates), usage,
            attributionResolver = attribution)
        // Router must be outside accounting so its explicit selectedModelId reaches the atomic reservation.
        val routedPort = RoutedModelChatPort(usagePort, router) { policy }
        activeVm = ChatViewModel(routedPort,
            history, usage, Conversation(conversationId))
        compose.setContent { SereaTheme { ChatScreen(activeVm, usage = usage) } }

        val composer = compose.onNodeWithTag("chat-composer")
        assertEquals(conversationId, activeVm.state.value.conversation.id)
        compose.waitUntil(30_000) { runCatching { composer.assertIsEnabled() }.isSuccess }
        composer.performTextInput("first question")
        compose.onNodeWithText("送信").performClick()
        awaitAssistant("first answer")
        policy = policy.copy(manualModelId = modelIds.last())
        composer.performTextInput("second question")
        compose.onNodeWithText("送信").performClick()
        awaitAssistant("second answer")

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
        assertEquals(1L, usage.modelUsage(modelIds.first()).single().requestCount)
        assertEquals(1L, usage.modelUsage(modelIds.last()).single().requestCount)
        assertEquals("fixture", usage.providerModelUsage("fixture", modelIds.first()).single().providerId)

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
            ModelPricing(Knowledge.UNKNOWN, Knowledge.UNKNOWN, Knowledge.UNKNOWN), ModelAvailability.AVAILABLE, authorized = true, lastVerifiedAt = "2026-10-19T00:00:00Z", source = "instrumentation fixture")
        val router = AdaptiveModelRouter(LocalModelCatalog(listOf(descriptor)))
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

    private fun awaitAssistant(text: String) {
        compose.waitUntil(30_000) { !stateSnapshot().isLoading || stateSnapshot().errorMessage != null }
        val completed = stateSnapshot()
        assertFalse("Request remained active; requests=${server.requestCount}, state=$completed", completed.isLoading)
        assertNull("Request failed; requests=${server.requestCount}, state=$completed", completed.errorMessage)
        assertEquals(text, completed.conversation.messages.lastOrNull()?.text)
    }

    private fun stateSnapshot(): ChatUiState {
        // The test owns the VM and observes its StateFlow directly.
        return activeVm.state.value
    }

    private lateinit var activeVm: ChatViewModel
}
