package app.serea.android

import app.serea.android.core.EmbeddedCoreAdapter
import app.serea.android.core.SyntheticCoreNative
import java.time.Instant
import java.util.Collections
import kotlinx.coroutines.runBlocking
import okhttp3.OkHttpClient
import okhttp3.mockwebserver.Dispatcher
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import okhttp3.mockwebserver.RecordedRequest
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/** Synthetic loopback integration of Embedded Core readiness with the already gated Cloud ModelChatPort. */
class EmbeddedCoreCloudChatIntegrationTest {
    private val events = Collections.synchronizedList(mutableListOf<String>())
    private val server = MockWebServer().apply {
        dispatcher = object : Dispatcher() {
            override fun dispatch(request: RecordedRequest): MockResponse {
                events += "http-dispatch"
                return MockResponse().setHeader("Content-Type", "application/json")
                    .setBody("""{"model":"gemma4","message":{"role":"assistant","content":"synthetic loopback reply"},"done":true,"prompt_eval_count":5,"eval_count":3}""")
            }
        }
        start()
    }

    @After fun closeServer() { server.shutdown() }

    private class RecordingCore(
        private val events: MutableList<String>,
        private val failInitialize: Boolean = false,
    ) : SyntheticCoreNative {
        private var handle: ULong? = null
        override fun initialize(path: String): ULong {
            events += "core-initialize"
            check(path == "synthetic-cloud-core")
            check(handle == null)
            check(!failInitialize) { "synthetic Core initialization failure" }
            return 71uL.also { handle = it }
        }
        override fun status(handle: ULong): Boolean {
            events += "core-status"
            return this.handle == handle
        }
        override fun schema(handle: ULong): UInt {
            events += "core-schema"
            check(this.handle == handle)
            return 1u
        }
        override fun close(handle: ULong) {
            events += "core-close"
            check(this.handle == handle)
            this.handle = null
        }
    }

    private class RecordingUsageDao(private val events: MutableList<String>) : UsageDao {
        private val windows = mutableMapOf<String, UsageWindowRow>()
        private val reservations = mutableMapOf<String, UsageReservationRow>()
        private val ledgers = mutableMapOf<String, ModelUsageLedgerRow>()
        private var settings: UsageSettingsRow? = null
        var reservationAttempts = 0
            private set
        val reservationCount: Int get() = reservations.size

        override suspend fun window(key: String) = windows[key]
        override suspend fun putWindow(row: UsageWindowRow) { windows[row.windowKey] = row }
        override suspend fun reservation(id: String) = reservations[id]
        override suspend fun insertReservation(row: UsageReservationRow) {
            events += "usage-reserve"
            reservationAttempts++
            check(row.requestId !in reservations)
            reservations[row.requestId] = row
        }
        override suspend fun putReservation(row: UsageReservationRow) { reservations[row.requestId] = row }
        override suspend fun deleteInFlightReservation(id: String): Int {
            if (reservations[id]?.state != UsageLedgerState.IN_FLIGHT.name) return 0
            reservations.remove(id)
            ledgers.remove(id)
            return 1
        }
        override suspend fun inFlight() = reservations.values.filter { it.state == UsageLedgerState.IN_FLIGHT.name }
        override suspend fun markUnknown(id: String): Int {
            val old = reservations[id] ?: return 0
            if (old.state != UsageLedgerState.IN_FLIGHT.name) return 0
            reservations[id] = old.copy(state = UsageLedgerState.OUTCOME_UNKNOWN.name)
            return 1
        }
        override suspend fun settings() = settings
        override suspend fun putSettings(row: UsageSettingsRow) { settings = row }
        override suspend fun ledger(id: String) = ledgers[id]
        override suspend fun insertLedger(row: ModelUsageLedgerRow) {
            check(row.requestId !in ledgers)
            ledgers[row.requestId] = row
        }
        override suspend fun putLedger(row: ModelUsageLedgerRow) { ledgers[row.requestId] = row }
        override suspend fun allLedgerRows() = ledgers.values.toList()
        override suspend fun markRequestUnknown(id: String) = markUnknown(id)
        override suspend fun totals(key: String) = UsageWindowTotals(0, 0, 0, 0)
        override suspend fun unresolvedCount() = reservations.values.count {
            it.state in setOf(UsageLedgerState.IN_FLIGHT.name, UsageLedgerState.OUTCOME_UNKNOWN.name, UsageLedgerState.USAGE_UNKNOWN.name)
        }.toLong()
    }

    private fun request(now: Long, classification: DataClassification = DataClassification.PUBLIC): ModelChatRequest {
        val body = "synthetic: verify Embedded Core to Cloud composition"
        val contract = EgressContract(
            conversationId = "synthetic-cloud-core-conversation",
            currentMessageId = "synthetic-cloud-core-send-${classification.name.lowercase()}",
            origin = DataOrigin.USER_AUTHORED,
            classification = classification,
            destination = "https://ollama.com/api/chat",
            plannedBody = body,
            context = emptyList(),
            preparation = PayloadPreparation.RAW,
            explicitSendIntentAtMillis = now,
            expiresAtMillis = now + 30_000,
            selectedModelId = MODEL_ID,
            plannedBodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
        )
        return ModelChatRequest(
            conversationId = contract.conversationId,
            message = body,
            sendId = contract.currentMessageId,
            classification = classification,
            egressContract = contract,
            selectedModelId = null,
            bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
        )
    }

    private fun cloudPort(usageDao: UsageDao, now: Long): GatedCloudModelChatPort {
        val nowInstant = Instant.ofEpochMilli(now)
        val catalog = OllamaGemma4CloudCatalog.create { nowInstant }
        return DirectProviderChatComposition.create(
            credentials = object : CredentialReader {
                override fun readForRequest(): String {
                    events += "credential-read"
                    return "synthetic-loopback-test-key"
                }
            },
            usage = UsageRepository(usageDao) { now },
            settings = CloudSettings(enabled = true, modelId = MODEL_ID),
            catalog = catalog,
            routingPolicy = {
                ModelRoutingPolicy(
                    mode = ModelSelectionMode.MANUAL,
                    manualModelId = MODEL_ID,
                    maxInputTokens = OllamaCloudRequestLimits.INPUT_TOKENS,
                    requestedOutputTokens = OllamaCloudRequestLimits.OUTPUT_TOKENS,
                    allowedProviderIds = setOf(OllamaGemma4CloudCatalog.PROVIDER_ID),
                )
            },
            usageAttributionResolver = CatalogUsageAttributionResolver(
                catalog,
                OllamaGemma4CloudCatalog.prices(),
                clock = { nowInstant },
            ),
            maxConcurrentRequests = OllamaCloudRequestLimits.CONCURRENT_REQUESTS,
            testEndpoint = server.url("/api/chat"),
            testClient = OkHttpClient.Builder().followRedirects(false).followSslRedirects(false).build(),
        )
    }

    @Test fun coreReadinessPrecedesGatedUsageCredentialAndLoopbackDispatch() = runBlocking {
        val now = System.currentTimeMillis()
        val usageDao = RecordingUsageDao(events)
        val core = EmbeddedCoreAdapter({ "synthetic-cloud-core" }, RecordingCore(events))
        val port = EmbeddedCoreCloudChatComposition.create(core, cloudPort(usageDao, now))

        try {
            val result = port.send(request(now))
            assertEquals("synthetic loopback reply", result.text)
            assertEquals(1, server.requestCount)
            val sent = server.takeRequest()
            assertEquals("Bearer synthetic-loopback-test-key", sent.getHeader("Authorization"))
            val sentBody = sent.body.readUtf8()
            assertTrue(sentBody.contains("synthetic: verify Embedded Core to Cloud composition"))
            assertTrue(!sentBody.contains("synthetic-loopback-test-key"))
            assertTrue(sentBody.contains("\"model\":\"gemma4\""))
            val ledger = usageDao.allLedgerRows().single()
            assertEquals("gemma4", ledger.modelId)
            assertEquals("ollama", ledger.providerId)
            assertEquals(OllamaCloudRequestLimits.OUTPUT_TOKENS.toLong(), ledger.reservedOutputTokens)
            assertEquals(5L, ledger.providerInputTokens)
            assertEquals(3L, ledger.providerOutputTokens)
            assertEquals(UsageLedgerState.SETTLED.name, ledger.usageStatus)
            assertTrue("verified catalog must settle a cost", ledger.estimatedCost != null)
            assertEquals(UsageLedgerState.SETTLED.name, usageDao.reservation("synthetic-cloud-core-send-public")?.state)
            assertEquals(1, usageDao.reservationCount)
            assertOrdered(events, "core-initialize", "core-status", "core-schema", "usage-reserve", "credential-read", "http-dispatch")
        } finally {
            port.close()
        }
        assertEquals("core-close", events.last())
    }

    @Test fun deniedEgressStopsBeforeUsageCredentialAndDispatchAfterCoreReadiness() = runBlocking {
        val now = System.currentTimeMillis()
        val usageDao = RecordingUsageDao(events)
        val port = EmbeddedCoreCloudChatComposition.create(
            EmbeddedCoreAdapter({ "synthetic-cloud-core" }, RecordingCore(events)), cloudPort(usageDao, now),
        )
        try {
            val failure = runCatching { port.send(request(now, DataClassification.PERSONAL)) }.exceptionOrNull()
            assertTrue("Personal data must fail closed before provider use", failure is ModelChatFailure)
            assertEquals(0, server.requestCount)
            assertTrue("Usage must not reserve a denied request", events.none { it == "usage-reserve" })
            assertEquals(0, usageDao.reservationAttempts)
            assertTrue("Credentials must not be read for a denied request", events.none { it == "credential-read" })
            assertTrue("No HTTP dispatch is permitted", events.none { it == "http-dispatch" })
            assertTrue(events.containsAll(listOf("core-initialize", "core-status", "core-schema")))
        } finally {
            port.close()
        }
    }

    private fun assertOrdered(actual: List<String>, vararg expected: String) {
        var previous = -1
        expected.forEach { event ->
            val index = actual.indexOf(event)
            assertTrue("Missing event $event in $actual", index >= 0)
            assertTrue("Event order violated by $event in $actual", index > previous)
            previous = index
        }
    }

    @Test fun coreInitializationFailureStopsBeforeCloudUsageCredentialAndDispatch() = runBlocking {
        val now = System.currentTimeMillis()
        val usageDao = RecordingUsageDao(events)
        val core = EmbeddedCoreAdapter(
            { "synthetic-cloud-core" },
            RecordingCore(events, failInitialize = true),
        )
        val port = EmbeddedCoreCloudChatComposition.create(core, cloudPort(usageDao, now))

        try {
            val failure = runCatching { port.send(request(now)) }.exceptionOrNull()

            assertTrue("Core initialization failure must fail closed", failure is ModelChatFailure)
            assertEquals(0, server.requestCount)
            assertEquals(0, usageDao.reservationAttempts)
            assertTrue(events.none { it == "usage-reserve" })
            assertTrue(events.none { it == "credential-read" })
            assertTrue(events.none { it == "http-dispatch" })
            assertEquals(listOf("core-initialize"), events.toList())
        } finally {
            port.close()
        }
    }

    private companion object { const val MODEL_ID = "gemma4" }
}
