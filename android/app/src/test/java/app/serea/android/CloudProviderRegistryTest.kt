package app.serea.android

import java.time.Instant
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.launch
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
        outputTokens: Int = 32,
    ): ModelChatPort = DirectProviderChatComposition.create(
        credentials = object : CredentialReader { override fun readForRequest() = error("synthetic providers do not read credentials") },
        usage = UsageRepository(dao) { now },
        settings = CloudSettings(enabled = true),
        catalog = models,
        routingPolicy = {
            ModelRoutingPolicy(mode = ModelSelectionMode.MANUAL, manualModelId = modelId,
                requestedOutputTokens = outputTokens, allowedProviderIds = setOf("synthetic-a", "synthetic-b", "synthetic-c"))
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
        assertEquals(32, a.lastOutputTokenLimit)
        assertEquals(32, b.lastOutputTokenLimit)
        assertEquals(32L, dao.ledger("send-a")?.reservedOutputTokens)
        assertEquals(32L, dao.ledger("send-b")?.reservedOutputTokens)
        assertEquals(1L, dao.reservation("send-a")?.outputTokens)
        assertEquals(1L, dao.reservation("send-b")?.outputTokens)
        assertEquals("synthetic-a/model-v1", dao.ledger("send-a")?.modelId)
        assertEquals("synthetic-a", dao.ledger("send-a")?.providerId)
        assertEquals("synthetic-b/model-v1", dao.ledger("send-b")?.modelId)
        assertEquals("synthetic-b", dao.ledger("send-b")?.providerId)
        assertEquals(UsageLedgerState.SETTLED.name, dao.reservation("send-a")?.state)
        assertEquals(UsageLedgerState.SETTLED.name, dao.reservation("send-b")?.state)
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
        assertTrue(unapproved != null)
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

    @Test fun modelAndProviderOutputLimitsRejectBeforeReservationOrDispatch() = runBlocking {
        val a = SyntheticProvider("synthetic-a", setOf("user"), maxOutputTokens = 16)
        val b = SyntheticProvider("synthetic-b", setOf("user"), maxOutputTokens = 128)
        val models = catalog()
        val dao = MemoryUsageDao()
        val providers = registry(a, b)

        val modelLimit = try {
            composition(models, providers, dao, "synthetic-b/model-v1", "send-model-limit", outputTokens = 65)
                .send(request("synthetic-b/model-v1", "send-model-limit"))
            null
        } catch (failure: ModelChatFailure) { failure }
        assertTrue(modelLimit != null)
        assertEquals(0, dao.reservationCount)

        val providerLimit = try {
            composition(models, providers, dao, "synthetic-a/model-v1", "send-provider-limit", outputTokens = 32)
                .send(request("synthetic-a/model-v1", "send-provider-limit"))
            null
        } catch (failure: ModelChatFailure) { failure }
        assertTrue(providerLimit != null)
        assertEquals(0, a.sendCount + b.sendCount)
        assertEquals(0, dao.reservationCount)
    }

    @Test fun missingInvalidAndOutputOverLimitUsageStaysUnknownButInputMayExceedEstimate() = runBlocking {
        val provider = SyntheticProvider("synthetic-a", setOf("user"))
        val models = catalog()
        val dao = MemoryUsageDao()
        val port = { sendId: String -> composition(models, registry(provider, SyntheticProvider("synthetic-b", setOf("user"))), dao,
            "synthetic-a/model-v1", sendId) }

        listOf(null, ProviderUsage(-1, 1), ProviderUsage(1, 33)).forEachIndexed { index, usage ->
            provider.replyUsage = usage
            val sendId = "send-usage-$index"
            assertEquals("reply from synthetic-a", port(sendId).send(request("synthetic-a/model-v1", sendId)).text)
            val reservation = dao.reservation(sendId)
            val ledger = dao.ledger(sendId)
            assertEquals(UsageLedgerState.USAGE_UNKNOWN.name, reservation?.state)
            assertEquals(32L, reservation?.outputTokens)
            assertEquals(UsageLedgerState.USAGE_UNKNOWN.name, ledger?.usageStatus)
            assertEquals(null, ledger?.providerInputTokens)
            assertEquals(null, ledger?.providerOutputTokens)
            assertEquals(null, ledger?.estimatedCost)
        }
        val actualInputSend = "send-input-above-estimate"
        provider.replyUsage = ProviderUsage(100, 1)
        assertEquals("reply from synthetic-a", port(actualInputSend).send(request("synthetic-a/model-v1", actualInputSend)).text)
        assertEquals(UsageLedgerState.SETTLED.name, dao.reservation(actualInputSend)?.state)
        assertEquals(100L, dao.ledger(actualInputSend)?.providerInputTokens)
        assertEquals(1L, dao.ledger(actualInputSend)?.providerOutputTokens)
        assertEquals(4, provider.sendCount)
    }

    @Test fun repeatedSendIdCannotDispatchAgainAfterSettlement() = runBlocking {
        val a = SyntheticProvider("synthetic-a", setOf("user"))
        val b = SyntheticProvider("synthetic-b", setOf("user"))
        val models = catalog()
        val dao = MemoryUsageDao()
        val first = request("synthetic-a/model-v1", "send-duplicate")
        assertEquals("reply from synthetic-a", composition(models, registry(a, b), dao, "synthetic-a/model-v1", "send-duplicate").send(first).text)
        try {
            composition(models, registry(a, b), dao, "synthetic-a/model-v1", "send-duplicate").send(first)
            throw AssertionError("duplicate sendId must be refused")
        } catch (_: DuplicateUsageRequestException) { }
        assertEquals(1, a.sendCount)
        assertEquals(0, b.sendCount)
        assertEquals(UsageLedgerState.SETTLED.name, dao.reservation("send-duplicate")?.state)
    }

    @Test fun providerFailureAfterDispatchKeepsUsageOutcomeUnknownWithoutRetry() = runBlocking {
        val a = SyntheticProvider("synthetic-a", setOf("user")).apply { failAfterDispatch = true }
        val b = SyntheticProvider("synthetic-b", setOf("user"))
        val models = catalog()
        val dao = MemoryUsageDao()
        val sendId = "send-network-failure"
        try {
            composition(models, registry(a, b), dao, "synthetic-a/model-v1", sendId)
                .send(request("synthetic-a/model-v1", sendId))
            throw AssertionError("dispatched provider failure expected")
        } catch (_: ModelChatFailure) { }
        assertEquals(1, a.sendCount)
        assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, dao.reservation(sendId)?.state)
        assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, dao.ledger(sendId)?.usageStatus)
        try {
            composition(models, registry(a, b), dao, "synthetic-a/model-v1", sendId)
                .send(request("synthetic-a/model-v1", sendId))
            throw AssertionError("same sendId retry must be refused")
        } catch (_: DuplicateUsageRequestException) { }
        assertEquals(1, a.sendCount)
        assertEquals(0, b.sendCount)
    }

    @Test fun cancellationBeforeDispatchReleasesButAfterDispatchKeepsOutcomeUnknown() = runBlocking {
        val a = SyntheticProvider("synthetic-a", setOf("user"))
        val b = SyntheticProvider("synthetic-b", setOf("user"))
        val models = catalog()
        val dao = MemoryUsageDao()

        val preDispatchEntered = CompletableDeferred<Unit>()
        a.pause = CompletableDeferred()
        a.pauseEntered = preDispatchEntered
        a.markDispatchBeforePause = false
        val before = launch { composition(models, registry(a, b), dao, "synthetic-a/model-v1", "send-cancel-before")
            .send(request("synthetic-a/model-v1", "send-cancel-before")) }
        preDispatchEntered.await()
        before.cancel()
        before.join()
        assertEquals(null, dao.reservation("send-cancel-before"))
        assertEquals(null, dao.ledger("send-cancel-before"))

        val postDispatchEntered = CompletableDeferred<Unit>()
        a.pause = CompletableDeferred()
        a.pauseEntered = postDispatchEntered
        a.markDispatchBeforePause = true
        val after = launch { composition(models, registry(a, b), dao, "synthetic-a/model-v1", "send-cancel-after")
            .send(request("synthetic-a/model-v1", "send-cancel-after")) }
        postDispatchEntered.await()
        after.cancel()
        after.join()
        assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, dao.reservation("send-cancel-after")?.state)
        assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, dao.ledger("send-cancel-after")?.usageStatus)
        assertEquals(1L, dao.unresolvedCount())

        // No transport dispatch observation is not proof that no request was sent.
        val unobservedEntered = CompletableDeferred<Unit>()
        a.pause = CompletableDeferred()
        a.pauseEntered = unobservedEntered
        a.confirmNoDispatchBeforePause = false
        a.replyUsage = null
        a.failAfterDispatch = false
        val unobserved = launch { composition(models, registry(a, b), dao, "synthetic-a/model-v1", "send-cancel-unobserved")
            .send(request("synthetic-a/model-v1", "send-cancel-unobserved")) }
        unobservedEntered.await()
        unobserved.cancel()
        unobserved.join()
        assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, dao.reservation("send-cancel-unobserved")?.state)
        assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, dao.ledger("send-cancel-unobserved")?.usageStatus)
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
        var lastOutputTokenLimit: Int? = null
        var replyUsage: ProviderUsage? = ProviderUsage(1, 1)
        var pause: CompletableDeferred<Unit>? = null
        var pauseEntered: CompletableDeferred<Unit>? = null
        var markDispatchBeforePause = false
        var confirmNoDispatchBeforePause = true

        override suspend fun send(payload: EgressApprovedPayload, onDelta: (String) -> Unit, selectedModelId: String?, outputTokenLimit: Int): CloudReply {
            sendCount++
            lastOutputTokenLimit = outputTokenLimit
            check(selectedModelId == payload.selectedModelId)
            val pauseGate = pause
            if (pauseGate != null) {
                if (markDispatchBeforePause) currentCoroutineContext()[CloudProviderRequestContext]?.markDispatchPossible()
                else if (confirmNoDispatchBeforePause) currentCoroutineContext()[CloudProviderRequestContext]?.markDefinitelyUndispatched()
                pauseEntered?.complete(Unit)
                pauseGate.await()
            }
            if (failAfterDispatch) throw ProviderFailure(ProviderFailureKind.NETWORK)
            return CloudReply("reply from $providerId", replyUsage)
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
            ledgers[id]?.let { row -> ledgers[id] = row.copy(usageStatus = UsageLedgerState.OUTCOME_UNKNOWN.name,
                reservationState = UsageLedgerState.OUTCOME_UNKNOWN.name, costStatus = CostEstimationStatus.UNKNOWN.name,
                estimatedCost = null, uncertainOutcomeReason = "Synthetic dispatched result is unknown.") }
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
