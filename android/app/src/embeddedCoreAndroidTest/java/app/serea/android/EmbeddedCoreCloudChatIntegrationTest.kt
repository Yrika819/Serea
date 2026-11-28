package app.serea.android

import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import android.content.Context
import app.serea.android.core.EmbeddedCoreAdapter
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicReference
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class EmbeddedCoreCloudChatIntegrationTest {
    @Test
    fun fixedCloudProbeCrossesRealCoreBeforeLoopbackRequestAndSettlesUsage() =
        synchronized(app.serea.android.core.EmbeddedCoreTestLock.monitor) {
            val context = ApplicationProvider.getApplicationContext<Context>()
            val database = Room.inMemoryDatabaseBuilder(context, SereaHistoryDatabase::class.java).build()
            val usage = UsageRepository(database.usageDao())
            val server = MockWebServer().apply { start() }
            val core = EmbeddedCoreAdapter(context)
            val coreHealthyAtDispatch = AtomicBoolean(false)
            val dispatchFailure = AtomicReference<Throwable?>(null)
            var closed = false
            server.dispatcher = object : Dispatcher() {
                override fun dispatch(request: RecordedRequest): MockResponse {
                    try {
                        coreHealthyAtDispatch.set(core.getStatus().get(10, TimeUnit.SECONDS) &&
                            core.schemaVersion().get(10, TimeUnit.SECONDS) > 0u)
                    } catch (failure: Throwable) {
                        dispatchFailure.set(failure)
                    }
                    return MockResponse().setBody(
                        """{"message":{"content":"synthetic core-backed probe reply"},"done":true,"prompt_eval_count":16,"eval_count":4}""",
                    )
                }
            }
            val catalog = OllamaGemma4CloudCatalog.create()
            val cloudPort = DirectProviderChatComposition.create(
                credentials = object : CredentialReader {
                    override fun readForRequest() = "synthetic-loopback-key"
                },
                usage = usage,
                settings = CloudSettings(enabled = true, modelId = OllamaGemma4CloudCatalog.MODEL_ID),
                catalog = catalog,
                routingPolicy = {
                    ModelRoutingPolicy(
                        mode = ModelSelectionMode.MANUAL,
                        manualModelId = OllamaGemma4CloudCatalog.MODEL_ID,
                        maxInputTokens = OllamaCloudRequestLimits.INPUT_TOKENS,
                        requestedOutputTokens = OllamaCloudRequestLimits.OUTPUT_TOKENS,
                        allowedProviderIds = setOf(OllamaGemma4CloudCatalog.PROVIDER_ID),
                    )
                },
                testEndpoint = server.url("/api/chat"),
                testClient = OkHttpClient(),
                usageAttributionResolver = CatalogUsageAttributionResolver(catalog, OllamaGemma4CloudCatalog.prices()),
                maxConcurrentRequests = OllamaCloudRequestLimits.CONCURRENT_REQUESTS,
            )
            val coreChat = EmbeddedCoreCloudChatComposition.create(core, cloudPort)
            val candidate = CloudCandidatePort(coreChat, usage)

            try {
                val receipt = runBlocking { candidate.runFixedPublicContractProbe() }
                val request = server.takeRequest(10, TimeUnit.SECONDS)
                    ?: throw AssertionError("The loopback provider did not receive the fixed probe.")
                val body = request.body.readUtf8()

                assertTrue("Native Core was not healthy when provider dispatch began", coreHealthyAtDispatch.get())
                assertEquals(null, dispatchFailure.get())
                assertTrue(core.getStatus().get(10, TimeUnit.SECONDS))
                assertEquals("synthetic core-backed probe reply", receipt.response.text)
                assertEquals("Bearer synthetic-loopback-key", request.getHeader("Authorization"))
                assertEquals(1, server.requestCount)
                assertTrue(body.contains("\"model\":\"gemma4\""))
                assertTrue(body.contains("\"num_predict\":1024"))
                assertTrue(body.contains("\"messages\":[{\"role\":\"user\",\"content\":\"${CloudLiveDispatchPolicy.FIXED_PROBE_PROMPT}\"}]"))
                assertFalse(body.contains("synthetic-loopback-key"))
                assertEquals(receipt.sendId, receipt.usage?.requestId)
                assertEquals("SETTLED", receipt.usage?.usageStatus)
                assertEquals("gemma4", receipt.usage?.modelId)
                assertEquals("ollama", receipt.usage?.providerId)
                assertEquals(16L, receipt.usage?.providerInputTokens)
                assertEquals(4L, receipt.usage?.providerOutputTokens)
            } finally {
                runBlocking { coreChat.close() }
                closed = true
                server.shutdown()
                database.close()
            }
            assertTrue("Synthetic native Core lease was not closed", closed)
        }
}
