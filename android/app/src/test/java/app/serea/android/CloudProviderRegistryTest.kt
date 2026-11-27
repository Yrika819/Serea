package app.serea.android

import java.time.Instant
import kotlinx.coroutines.runBlocking
import okhttp3.HttpUrl.Companion.toHttpUrl
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

private const val SYNTHETIC_DEFAULT_DESTINATION = "https://ollama.com/api/chat"

class CloudProviderRegistryTest {
    private val ollamaDestination = SYNTHETIC_DEFAULT_DESTINATION
    private val now = System.currentTimeMillis()

    private fun descriptor(id: String, providerId: String) = ModelDescriptor(
        id = id,
        providerId = providerId,
        displayName = "Synthetic $providerId",
        contextWindow = Knowledge.Known(1024),
        maxOutput = Knowledge.Known(64),
        capabilities = ModelCapabilities(setOf("text/plain")),
        availability = ModelAvailability.AVAILABLE,
        authorized = true,
        lastVerifiedAt = Instant.ofEpochMilli(now - 1_000).toString(),
        source = "synthetic provider registry test",
        verificationStatus = ModelVerificationStatus.TEST_FIXTURE,
    )

    private fun catalog() = FakeModelCatalog(listOf(
        descriptor("synthetic-a/model-v1", "synthetic-a"),
        descriptor("synthetic-b/model-v1", "synthetic-b"),
        descriptor("synthetic-c/model-v1", "synthetic-c"),
    ))

    private fun contract(
        modelId: String,
        sendId: String,
        context: List<ContextMessage> = emptyList(),
        endpoint: String = ollamaDestination,
    ) = EgressContract(
        conversationId = "conversation-$sendId",
        currentMessageId = sendId,
        origin = DataOrigin.USER_AUTHORED,
        classification = DataClassification.PUBLIC,
        destination = endpoint,
        plannedBody = "synthetic prompt",
        context = context,
        preparation = PayloadPreparation.RAW,
        explicitSendIntentAtMillis = now,
        expiresAtMillis = now + 30_000,
        selectedModelId = null,
        plannedBodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
    )

    private fun request(modelId: String, sendId: String, context: List<ContextMessage> = emptyList()) =
        ModelChatRequest(
            conversationId = "conversation-$sendId",
            message = "synthetic prompt",
            sendId = sendId,
            classification = DataClassification.PUBLIC,
            context = context,
            egressContract = contract(modelId, sendId, context),
            selectedModelId = null,
            bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
        )

    private fun assistantContext(sendId: String) = listOf(ContextMessage(
        role = MessageRole.ASSISTANT,
        text = "previous assistant answer",
        classification = DataClassification.PUBLIC,
        sourceMessageId = "context-$sendId",
        origin = DataOrigin.USER_AUTHORED,
        conversationId = "conversation-$sendId",
        sourceLineage = setOf(DataOrigin.USER_AUTHORED),
    ))

    private fun registry(a: SyntheticProvider, b: SyntheticProvider) =
        CloudProviderRegistry(listOf(a, b), setOf("synthetic-a", "synthetic-b"))

    private fun composition(
        models: ModelCatalog,
        registry: CloudProviderRegistry,
        dao: MemoryUsageDao,
        modelId: String,
        sendId: String,
    ): ModelChatPort = DirectProviderChatComposition.create(
        credentials = object : CredentialReader { override fun readForRequest() = error("synthetic providers do not read credentials") },
        usage = UsageRepository(dao) { now },
        settings = CloudSettings(enabled = true),
        catalog = models,
        routingPolicy = {
            ModelRoutingPolicy(mode = ModelSelectionMode.MANUAL, manualModelId = modelId,
                requestedOutputTokens = 32, allowedProviderIds = setOf("synthetic-a", "synthetic-b", "synthetic-c"))
        },
        cloudProviderRegistry = registry,
    )

    @Test fun directCompositionRoutesTwoSyntheticProvidersByCatalogProviderIdOnceEach() = runBlocking {
        val a = SyntheticProvider("synthetic-a", setOf("user"), supportsStreaming = false, maxInputBytes = 512, maxOutputTokens = 64)
        val b = SyntheticProvider("synthetic-b", setOf("user", "assistant"), supportsStreaming = true, maxInputBytes = 4096, maxOutputTokens = 128)
        val models = catalog()
        val dao = MemoryUsageDao()
        val providers = registry(a, b)

        val replyA = composition(models, providers, dao, "synthetic-a/model-v1", "send-a").send(
            request("synthetic-a/model-v1", "send-a"),
        )
        val replyB = composition(models, providers, dao, "synthetic-b/model-v1", "send-b").send(
            request("synthetic-b/model-v1", "send-b", assistantContext("send-b")),
        )

        assertEquals("reply from synthetic-a", replyA.text)
        assertEquals("reply from synthetic-b", replyB.text)
        assertEquals(1, a.sendCount)
        assertEquals(1, b.sendCount)
        assertEquals("synthetic-a", dao.ledger("send-a")?.providerId)
        assertEquals("synthetic-b", dao.ledger("send-b")?.providerId)
        assertFalse(a.capabilities.supportsStreaming)
        assertTrue(b.capabilities.supportsStreaming)
    }

    @Test fun unknownAndUnapprovedModelsAreRejectedWithoutProviderDispatch() = runBlocking {
        val a = SyntheticProvider("synthetic-a", setOf("user"))
        val b = SyntheticProvider("synthetic-b", setOf("user"))
        val models = catalog()
        val dao = MemoryUsageDao()
        val providers = registry(a, b)

        val unknown = try {
            composition(models, providers, dao, "synthetic-missing/model", "send-unknown")
                .send(request("synthetic-missing/model", "send-unknown"))
            null
        } catch (failure: ModelChatFailure) { failure }
        assertTrue(unknown != null)
        assertEquals(0, a.sendCount + b.sendCount)
        assertEquals(0, dao.reservationCount)

        val unapproved = try {
            composition(models, providers, dao, "synthetic-c/model-v1", "send-unapproved")
                .send(request("synthetic-c/model-v1", "send-unapproved"))
            null
        } catch (failure: ModelChatFailure) { failure }
        assertTrue(unapproved is PreDispatchModelChatFailure)
        assertEquals(0, a.sendCount + b.sendCount)
        assertEquals(0, dao.reservationCount)
    }

    @Test fun providerRoleLimitAndGateDestinationFailBeforeTransport() = runBlocking {
        val a = SyntheticProvider("synthetic-a", setOf("user"))
        val b = SyntheticProvider("synthetic-b", setOf("user", "assistant"))
        val models = catalog()
        val dao = MemoryUsageDao()
        val providers = registry(a, b)

        val unsupportedRole = try {
            composition(models, providers, dao, "synthetic-a/model-v1", "send-role")
                .send(request("synthetic-a/model-v1", "send-role", assistantContext("send-role")))
            null
        } catch (failure: ModelChatFailure) { failure }
        assertTrue(unsupportedRole is PreDispatchModelChatFailure)
        assertEquals(0, a.sendCount + b.sendCount)
        assertEquals(0, dao.reservationCount)

        val wrongEndpointContract = contract("synthetic-b/model-v1", "send-wrong-endpoint", endpoint = "https://attacker.example/api/chat")
        val wrongEndpointRequest = request("synthetic-b/model-v1", "send-wrong-endpoint").copy(egressContract = wrongEndpointContract)
        val wrongEndpoint = try {
            composition(models, providers, dao, "synthetic-b/model-v1", "send-wrong-endpoint").send(wrongEndpointRequest)
            null
        } catch (failure: ModelChatFailure) { failure }
        assertTrue(wrongEndpoint != null)
        assertEquals(0, a.sendCount + b.sendCount)
        assertEquals(0, dao.reservationCount)
    }

    @Test fun registryPinsProviderDestinationAndInvokesTransportAtMostOnce() = runBlocking {
        val provider = SyntheticProvider("synthetic-a", setOf("user"), destination = "https://synthetic-a.example/v1/chat")
        val models = FakeModelCatalog(listOf(descriptor("synthetic-a/model-v1", "synthetic-a")))
        val providers = CloudProviderRegistry(listOf(provider), setOf("synthetic-a"))
        val validContract = contract("synthetic-a/model-v1", "direct-valid", endpoint = provider.destination)
            .copy(selectedModelId = "synthetic-a/model-v1")
        val validPayload = EgressApprovedPayload.fromApprovedContract(validContract, listOf("user" to "synthetic prompt"))
        assertEquals("reply from synthetic-a", providers.sendOnce(validPayload, "synthetic-a/model-v1", models).text)
        assertEquals(1, provider.sendCount)

        val wrongContract = contract("synthetic-a/model-v1", "direct-wrong", endpoint = "https://synthetic-b.example/v1/chat")
            .copy(selectedModelId = "synthetic-a/model-v1")
        val wrongPayload = EgressApprovedPayload.fromApprovedContract(wrongContract, listOf("user" to "synthetic prompt"))
        val failure = try {
            providers.sendOnce(wrongPayload, "synthetic-a/model-v1", models)
            null
        } catch (caught: ProviderFailure) { caught }
        assertTrue(failure is ProviderPreDispatchFailure)
        assertEquals(1, provider.sendCount)

        provider.failAfterDispatch = true
        try {
            providers.sendOnce(validPayload, "synthetic-a/model-v1", models)
            throw AssertionError("synthetic provider failure expected")
        } catch (_: ProviderFailure) { }
        assertEquals(2, provider.sendCount)
    }

    private class SyntheticProvider(
        override val providerId: String,
        roles: Set<String>,
        override val destination: String = SYNTHETIC_DEFAULT_DESTINATION,
        supportsStreaming: Boolean = false,
        maxInputBytes: Int = 1024,
        maxOutputTokens: Int = 64,
    ) : CloudChatProvider {
        override val capabilities = CloudProviderCapabilities(supportsStreaming, roles, maxInputBytes, maxOutputTokens)
        override val allowsTestFixtureModels = true
        var sendCount = 0
        var failAfterDispatch = false

        override suspend fun send(payload: EgressApprovedPayload, onDelta: (String) -> Unit, selectedModelId: String?): CloudReply {
            sendCount++
            check(selectedModelId == payload.selectedModelId)
            if (failAfterDispatch) throw ProviderFailure(ProviderFailureKind.NETWORK)
            return CloudReply("reply from $providerId", ProviderUsage(1, 1))
        }
    }

    private class MemoryUsageDao : UsageDao {
        private val windows = mutableMapOf<String, UsageWindowRow>()
        private val reservations = mutableMapOf<String, UsageReservationRow>()
        private val ledgers = mutableMapOf<String, ModelUsageLedgerRow>()
        private var savedSettings: UsageSettingsRow? = null
        val reservationCount: Int get() = reservations.size

        override suspend fun window(key: String) = windows[key]
        override suspend fun putWindow(row: UsageWindowRow) { windows[row.windowKey] = row }
        override suspend fun reservation(id: String) = reservations[id]
        override suspend fun insertReservation(row: UsageReservationRow) { reservations[row.requestId] = row }
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
        override suspend fun settings() = savedSettings
        override suspend fun putSettings(row: UsageSettingsRow) { savedSettings = row }
        override suspend fun ledger(id: String) = ledgers[id]
        override suspend fun insertLedger(row: ModelUsageLedgerRow) { ledgers[row.requestId] = row }
        override suspend fun putLedger(row: ModelUsageLedgerRow) { ledgers[row.requestId] = row }
        override suspend fun allLedgerRows() = ledgers.values.toList()
        override suspend fun markRequestUnknown(id: String) = markUnknown(id)
        override suspend fun totals(key: String) = UsageWindowTotals(0, 0, 0, 0)
        override suspend fun unresolvedCount() = reservations.values.count {
            it.state in setOf(UsageLedgerState.IN_FLIGHT.name, UsageLedgerState.OUTCOME_UNKNOWN.name, UsageLedgerState.USAGE_UNKNOWN.name)
        }.toLong()
    }
}
