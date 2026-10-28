package app.serea.android

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test
import java.io.File
import java.security.MessageDigest

class LocalModelChatAdapterTest {
    private val now = 1_800_000_000_000L
    private val modelId = "serea-local/SmolLM2-135M-Q4_K_M"

    @Test fun adapterMatchesModelChatPortAndCarriesMeasuredUsageAcrossTurns() = runBlocking {
        val file = tempModel(byteArrayOf(1, 2, 3, 4))
        val backend = FakeBackend()
        val adapter = createAdapter(artifact(file), file, backend)
        val registry = registry { now }
        try {
            val first = request("First", sendId = "send-1")
            val response = gated(adapter, registry, first).send(first)
            assertEquals("local answer", response.text)
            assertEquals(ProviderUsage(4, 2), response.usage)
            assertTrue(backend.loaded)
            assertEquals(1, backend.generateCalls)

            val context = listOf(ContextMessage(
                role = MessageRole.USER, text = "First", classification = DataClassification.PUBLIC,
                sourceMessageId = "message-1", origin = DataOrigin.USER_AUTHORED,
                conversationId = first.conversationId, sourceLineage = setOf(DataOrigin.USER_AUTHORED),
            ))
            val second = request("Second", context = context, sendId = "send-2")
            val secondResponse = gated(adapter, registry, second).send(second)
            assertEquals("local answer", secondResponse.text)
            assertTrue(backend.lastPrompt!!.contains("user: First"))
            assertTrue(backend.lastPrompt!!.contains("user: Second"))
            assertEquals(2, backend.generateCalls)
        } finally {
            adapter.close()
            file.delete()
        }
        assertTrue(backend.closed)
    }

    @Test fun modelHashMismatchFailsBeforeNativeLoad() = runBlocking {
        val file = tempModel(byteArrayOf(1, 2, 3))
        val backend = FakeBackend()
        val adapter = createAdapter(artifact(file, sha256 = "0".repeat(64)), file, backend)
        try {
            val request = request("hello", sendId = "send-hash")
            val failure = runCatching { gated(adapter, registry { now }, request).send(request) }.exceptionOrNull()
            assertTrue(failure is PreDispatchModelChatFailure)
            assertFalse(backend.loaded)
            assertEquals(0, backend.generateCalls)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun cloudOrDemoContractCannotReachLocalBackend() = runBlocking {
        val file = tempModel(byteArrayOf(5))
        val backend = FakeBackend()
        val adapter = createAdapter(artifact(file), file, backend)
        try {
            val plain = request("hello", sendId = "send-target")
            val cloudRequest = plain.copy(
                selectedModelId = modelId,
                egressContract = localContract(plain, null).copy(
                    destination = "https://ollama.com/api/chat", target = EgressTarget.CLOUD,
                ),
            )
            val failure = runCatching { adapter.send(cloudRequest) }.exceptionOrNull()
            assertTrue(failure is PreDispatchModelChatFailure)
            assertEquals(0, backend.generateCalls)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun usageAttributionUsesRealLocalModelIdentityWithoutInventingPrice() {
        val file = tempModel(byteArrayOf(5, 5))
        try {
            val artifact = artifact(file)
            val resolver = LocalModelUsageAttributionResolver(artifact)
            val attributed = resolver.resolve(request("usage", "send-usage").copy(selectedModelId = modelId))
            assertEquals(modelId, attributed.modelId)
            assertEquals("local-device", attributed.providerId)
            assertNull(attributed.price)
            assertTrue(runCatching { resolver.resolve(request("usage", "send-usage")) }.exceptionOrNull() is PreDispatchModelChatFailure)
        } finally { file.delete() }
    }

    @Test fun providerHandoffRequiresCurrentTypedEvidenceAndCannotBeReused() = runBlocking {
        val file = tempModel(byteArrayOf(6, 7))
        val backend = FakeBackend()
        val adapter = createAdapter(artifact(file), file, backend)
        var current = true
        var currentGeneration = 7L
        val registry = LocalModelAuthorizationRegistry(
            { now },
            handoffValidator = LocalModelProviderHandoffValidator { _, _ -> current },
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding("conversation-1", currentGeneration)
            },
        )
        try {
            val request = request("Selected synthetic email body", sendId = "send-google", origin = DataOrigin.GMAIL)
            val evidence = FakeHandoff(
                handoffId = "handoff-1", origin = DataOrigin.GMAIL, serviceType = "GMAIL",
                syntheticAccountIdentifier = "synthetic-account-1", sessionIdentifier = "session-1",
                sessionGeneration = 3, scope = "gmail.readonly", sourceItemIdentifier = "message-1",
                sourceRevision = "rev-2", contentIdentitySha256 = "sha256:${sha256Utf8("raw source")}",
                selectedContent = "raw source", approvedChatBodySha256 = sha256Utf8(request.message),
                expiresAtMillis = now + 10_000, authorizationState = "MOCK_ONLY",
                selectionState = "SELECTED", consentState = "LOCAL_PROCESSING_ONLY",
                destination = "LOCAL_CHAT",
                destinationConversationId = request.conversationId, destinationConversationGeneration = 7,
            )
            val proof = registry.approve(request, modelId, evidence, conversationGeneration = 7)
            val contract = localContract(request, proof, DataOrigin.GMAIL)
            val gatedPort = EgressCheckedModelChatPort(adapter, { now }, EgressTarget.LOCAL_MODEL, registry)
            assertEquals("local answer", gatedPort.send(request.copy(selectedModelId = modelId, egressContract = contract)).text)
            assertEquals(1, backend.generateCalls)
            assertFalse(registry.consumeOnce(proof, contract))

            val revokedRequest = request("Revoked before send", sendId = "send-revoked", origin = DataOrigin.GMAIL)
            val revokedProof = registry.approve(revokedRequest, modelId, evidence.copy(
                approvedChatBodySha256 = sha256Utf8(revokedRequest.message),
            ), conversationGeneration = 7)
            current = false
            val denied = EgressCheckedModelChatPort(adapter, { now }, EgressTarget.LOCAL_MODEL, registry)
            assertTrue(runCatching {
                denied.send(revokedRequest.copy(selectedModelId = modelId,
                    egressContract = localContract(revokedRequest, revokedProof, DataOrigin.GMAIL)))
            }.exceptionOrNull() is PreDispatchModelChatFailure)
            assertEquals(1, backend.generateCalls)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun envelopeTamperExpiryAndReplayAreDeniedBeforeUsageReservation() = runBlocking {
        val file = tempModel(byteArrayOf(9, 1))
        val backend = FakeBackend()
        val adapter = createAdapter(artifact(file), file, backend)
        var clock = now
        var currentGeneration = 1L
        val registry = LocalModelAuthorizationRegistry(
            { clock },
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding("conversation-1", currentGeneration)
            },
        )
        val usageDao = CountingUsageDao()
        val usage = UsageControlledModelChatPort(
            adapter, UsageRepository(usageDao) { clock },
            attributionResolver = LocalModelUsageAttributionResolver(artifact(file)),
        )
        val egress = EgressCheckedModelChatPort(usage, { clock }, EgressTarget.LOCAL_MODEL, registry)
        try {
            val original = request("approved selected text", sendId = "send-envelope", origin = DataOrigin.GMAIL)
            val evidence = FakeHandoff(
                handoffId = "handoff-envelope", origin = DataOrigin.GMAIL, serviceType = "GMAIL",
                syntheticAccountIdentifier = "synthetic-account-1", sessionIdentifier = "session-1",
                sessionGeneration = 2, scope = "gmail.readonly", sourceItemIdentifier = "message-1",
                sourceRevision = "rev-1", contentIdentitySha256 = "sha256:${sha256Utf8("raw source")}",
                selectedContent = "raw source", approvedChatBodySha256 = sha256Utf8(original.message),
                expiresAtMillis = now + 20_000, authorizationState = "MOCK_ONLY",
                selectionState = "SELECTED", consentState = "LOCAL_PROCESSING_ONLY", destination = "LOCAL_CHAT",
                destinationConversationId = original.conversationId, destinationConversationGeneration = 1,
            )
            val proof = registry.approve(original, modelId, evidence, conversationGeneration = 1)
            val tampered = original.copy(message = "edited after preview", egressContract = null)
            val tamperedContract = localContract(tampered, proof, DataOrigin.GMAIL)
            assertTrue(runCatching {
                egress.send(tampered.copy(selectedModelId = modelId, egressContract = tamperedContract))
            }.isFailure)
            assertEquals(0, usageDao.reservationCount)
            assertEquals(0, backend.generateCalls)

            val expired = request("expired", sendId = "send-expired")
            val expiredProof = registry.approve(expired, modelId)
            val expiredContract = localContract(expired, expiredProof)
            clock += 60_001
            assertTrue(runCatching {
                egress.send(expired.copy(selectedModelId = modelId, egressContract = expiredContract))
            }.isFailure)
            assertEquals(0, usageDao.reservationCount)
            assertEquals(0, backend.generateCalls)

            val missingBindingRequest = request("missing live binding", sendId = "send-no-binding")
            val noBindingRegistry = LocalModelAuthorizationRegistry({ clock })
            val noBindingProof = noBindingRegistry.approve(missingBindingRequest, modelId)
            assertTrue(runCatching {
                egressWith(noBindingRegistry, usage, clock).send(
                    missingBindingRequest.copy(selectedModelId = modelId,
                        egressContract = localContract(missingBindingRequest, noBindingProof)),
                )
            }.exceptionOrNull() is PreDispatchModelChatFailure)
            assertEquals("Missing live conversation binding must fail before reservation", 0, usageDao.reservationCount)
            assertEquals(0, backend.generateCalls)

            currentGeneration = 0
            clock = now
            val approved = request("one-time approval", sendId = "send-once")
            val approvedProof = registry.approve(approved, modelId)
            val approvedContract = localContract(approved, approvedProof)
            val exactRequest = approved.copy(selectedModelId = modelId, egressContract = approvedContract)
            assertEquals("local answer", egress.send(exactRequest).text)
            assertEquals(1, usageDao.reservationCount)
            assertTrue(runCatching { egress.send(exactRequest) }.isFailure)
            assertEquals("Replay must be stopped before another usage reservation", 1, usageDao.reservationCount)
            assertEquals("Replay must be stopped before native dispatch", 1, backend.generateCalls)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun cancellationCallsNativeCancelAndCloseReleasesBackend() = runBlocking {
        val file = tempModel(byteArrayOf(8))
        val backend = FakeBackend(blockGeneration = true)
        val adapter = createAdapter(artifact(file), file, backend)
        try {
            val request = request("cancel", sendId = "send-cancel")
            val deferred = async { gated(adapter, registry { now }, request).send(request) }
            backend.generationStarted.await()
            deferred.cancelAndJoin()
            assertTrue(backend.cancelled)
            adapter.close()
            assertTrue(backend.closed)
            assertTrue(runCatching { gated(adapter, registry { now }, request).send(request) }.isFailure)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun closeDuringGenerationWaitsForCancellationBeforeNativeRelease() = runBlocking {
        val file = tempModel(byteArrayOf(8, 1))
        val backend = FakeBackend(blockGeneration = true)
        val adapter = createAdapter(artifact(file), file, backend)
        try {
            val request = request("close while running", sendId = "send-close")
            val deferred = async { gated(adapter, registry { now }, request).send(request) }
            backend.generationStarted.await()
            adapter.close()
            deferred.join()
            assertTrue(deferred.isCancelled)
            assertTrue(backend.cancelled)
            assertTrue("close must run after generation unwinds", backend.closed)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun nativeFailureDiscardsEngineAndNextRequestGetsFreshBackend() = runBlocking {
        val file = tempModel(byteArrayOf(8, 2))
        val broken = FakeBackend(failGeneration = true)
        val recovered = FakeBackend()
        val queue = ArrayDeque(listOf(broken, recovered))
        val adapter = LocalModelChatAdapter(
            artifact(file), file,
            backendFactory = LocalModelInferenceBackendFactory { queue.removeFirst() },
            deviceAbi = "x86_64",
        )
        try {
            val first = request("fail native generation", sendId = "send-native-fail")
            assertTrue(runCatching { gated(adapter, registry { now }, first).send(first) }.isFailure)
            assertTrue(broken.closed)
            assertEquals(1, broken.generateCalls)
            assertEquals(0, recovered.generateCalls)

            val next = request("recover with a new native engine", sendId = "send-native-retry")
            assertEquals("local answer", gated(adapter, registry { now }, next).send(next).text)
            assertTrue(recovered.loaded)
            assertEquals(1, recovered.generateCalls)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun nativeLoadFailureDiscardsEngineBeforeRetry() = runBlocking {
        val file = tempModel(byteArrayOf(8, 3))
        val broken = FakeBackend(failLoad = true)
        val recovered = FakeBackend()
        val queue = ArrayDeque(listOf(broken, recovered))
        val adapter = LocalModelChatAdapter(
            artifact(file), file,
            backendFactory = LocalModelInferenceBackendFactory { queue.removeFirst() },
            deviceAbi = "x86_64",
        )
        try {
            val first = request("fail native load", sendId = "send-load-fail")
            assertTrue(runCatching { gated(adapter, registry { now }, first).send(first) }.isFailure)
            assertTrue(broken.closed)
            assertEquals(0, broken.generateCalls)

            val next = request("retry with a new engine", sendId = "send-load-retry")
            assertEquals("local answer", gated(adapter, registry { now }, next).send(next).text)
            assertTrue(recovered.loaded)
            assertEquals(1, recovered.generateCalls)
        } finally { adapter.close(); file.delete() }
    }

    private fun gated(adapter: ModelChatPort, registry: LocalModelAuthorizationRegistry, request: ModelChatRequest) =
        egressWith(registry, adapter, { now }).let { egress ->
            val proof = registry.approve(request, modelId)
            val contract = localContract(request, proof)
            object : ModelChatPort {
                override suspend fun send(ignored: ModelChatRequest): ModelChatResponse =
                    egress.send(request.copy(selectedModelId = modelId, egressContract = contract))
            }
        }

    private fun egressWith(registry: LocalModelAuthorizationRegistry, delegate: ModelChatPort, clock: () -> Long) =
        EgressCheckedModelChatPort(delegate, clock, EgressTarget.LOCAL_MODEL, registry)

    private fun registry(clock: () -> Long) = LocalModelAuthorizationRegistry(
        clock,
        currentConversation = LocalModelCurrentConversationPort {
            LocalModelConversationBinding("conversation-1", 0)
        },
    )

    private fun localContract(
        request: ModelChatRequest,
        proof: LocalModelConsentProof?,
        origin: DataOrigin = DataOrigin.USER_AUTHORED,
    ) = EgressContract(
        conversationId = request.conversationId,
        currentMessageId = request.sendId,
        origin = origin,
        classification = request.classification,
        destination = ChatEgressGate.LOCAL_MODEL_DESTINATION,
        plannedBody = request.message,
        context = request.context,
        preparation = PayloadPreparation.RAW,
        explicitSendIntentAtMillis = null,
        expiresAtMillis = null,
        selectedModelId = modelId,
        target = EgressTarget.LOCAL_MODEL,
        plannedBodySourceLineage = request.bodySourceLineage,
        localModelConsent = proof,
    )

    private fun request(
        text: String,
        sendId: String,
        context: List<ContextMessage> = emptyList(),
        origin: DataOrigin = DataOrigin.USER_AUTHORED,
    ) = ModelChatRequest(
        conversationId = "conversation-1", message = text, sendId = sendId,
        classification = DataClassification.PUBLIC, context = context,
        bodySourceLineage = setOf(origin),
    )

    private fun artifact(file: File, sha256: String = sha256File(file)) = LocalModelArtifact(
        modelId = modelId, displayName = "SmolLM2 135M Q4_K_M (fixture)",
        modelRepository = "https://huggingface.co/ggml-org/SmolLM2-135M-GGUF",
        modelRevision = "1076b85d135017971845b19b8d07df1479155988", licenseId = "Apache-2.0",
        expectedSha256 = sha256, expectedSizeBytes = file.length(), runtimeName = "llama.cpp",
        runtimeRevision = "276378515ceebee5d14bcc8536603dd5c87491a2", supportedAbis = setOf("arm64-v8a", "x86_64"),
    )

    private fun createAdapter(artifact: LocalModelArtifact, file: File, backend: FakeBackend) =
        LocalModelChatAdapter(artifact, file, LocalModelInferenceBackendFactory { backend }, deviceAbi = "x86_64")

    private fun tempModel(bytes: ByteArray): File = File.createTempFile("serea-model-test", ".gguf").apply { writeBytes(bytes) }
    private fun sha256File(file: File) = MessageDigest.getInstance("SHA-256").digest(file.readBytes())
        .joinToString("") { "%02x".format(it) }

    private data class FakeHandoff(
        override val handoffId: String,
        override val origin: DataOrigin,
        override val serviceType: String,
        override val syntheticAccountIdentifier: String,
        override val sessionIdentifier: String,
        override val sessionGeneration: Long,
        override val scope: String,
        override val sourceItemIdentifier: String,
        override val sourceRevision: String,
        override val contentIdentitySha256: String,
        override val selectedContent: String,
        override val approvedChatBodySha256: String,
        override val expiresAtMillis: Long,
        override val authorizationState: String,
        override val selectionState: String,
        override val consentState: String,
        override val destination: String,
        override val destinationConversationId: String,
        override val destinationConversationGeneration: Long,
    ) : LocalModelProviderHandoffEvidence

    private class FakeBackend(
        private val blockGeneration: Boolean = false,
        private val failGeneration: Boolean = false,
        private val failLoad: Boolean = false,
    ) : LocalModelInferenceBackend {
        var loaded = false
        var closed = false
        var cancelled = false
        var generateCalls = 0
        var loadCalls = 0
        var lastPrompt: String? = null
        val generationStarted = CompletableDeferred<Unit>()
        private val releaseGeneration = CompletableDeferred<Unit>()

        override suspend fun load(modelPath: String, limits: LocalModelRuntimeLimits) {
            loadCalls++
            if (failLoad) error("synthetic native load failure")
            loaded = true
        }
        override suspend fun generate(prompt: String, maxOutputTokens: Int): LocalModelInferenceResult {
            generateCalls++
            lastPrompt = prompt
            generationStarted.complete(Unit)
            if (blockGeneration) releaseGeneration.await()
            if (failGeneration) error("synthetic native inference failure")
            return LocalModelInferenceResult("local answer", 4, 2)
        }
        override fun cancelActive() { cancelled = true; releaseGeneration.complete(Unit) }
        override fun close() { closed = true }
    }

    private class CountingUsageDao : UsageDao {
        private val windows = mutableMapOf<String, UsageWindowRow>()
        private val reservations = mutableMapOf<String, UsageReservationRow>()
        private var savedSettings: UsageSettingsRow? = null
        private val ledgers = mutableMapOf<String, ModelUsageLedgerRow>()
        val reservationCount get() = reservations.size

        override suspend fun window(key: String) = windows[key]
        override suspend fun putWindow(row: UsageWindowRow) { windows[row.windowKey] = row }
        override suspend fun reservation(id: String) = reservations[id]
        override suspend fun insertReservation(row: UsageReservationRow) { check(row.requestId !in reservations); reservations[row.requestId] = row }
        override suspend fun putReservation(row: UsageReservationRow) { reservations[row.requestId] = row }
        override suspend fun deleteInFlightReservation(id: String): Int {
            if (reservations[id]?.state != UsageLedgerState.IN_FLIGHT.name) return 0
            reservations.remove(id); ledgers.remove(id); return 1
        }
        override suspend fun inFlight() = reservations.values.filter { it.state == UsageLedgerState.IN_FLIGHT.name }
        override suspend fun markUnknown(id: String): Int {
            val row = reservations[id] ?: return 0
            if (row.state != UsageLedgerState.IN_FLIGHT.name) return 0
            reservations[id] = row.copy(state = UsageLedgerState.OUTCOME_UNKNOWN.name); return 1
        }
        override suspend fun settings() = savedSettings
        override suspend fun putSettings(row: UsageSettingsRow) { savedSettings = row }
        override suspend fun ledger(id: String) = ledgers[id]
        override suspend fun insertLedger(row: ModelUsageLedgerRow) { check(row.requestId !in ledgers); ledgers[row.requestId] = row }
        override suspend fun putLedger(row: ModelUsageLedgerRow) { ledgers[row.requestId] = row }
        override suspend fun allLedgerRows() = ledgers.values.toList()
        override suspend fun markRequestUnknown(id: String) = markUnknown(id)
        override suspend fun totals(key: String) = UsageWindowTotals(0, 0, 0, 0)
        override suspend fun unresolvedCount() = reservations.values.count {
            it.state in setOf(UsageLedgerState.IN_FLIGHT.name, UsageLedgerState.OUTCOME_UNKNOWN.name, UsageLedgerState.USAGE_UNKNOWN.name)
        }.toLong()
    }
}
