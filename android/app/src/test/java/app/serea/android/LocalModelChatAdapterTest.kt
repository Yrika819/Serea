package app.serea.android

import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.supervisorScope
import kotlinx.coroutines.withTimeout
import org.junit.Assert.*
import org.junit.Test
import java.io.File
import java.io.RandomAccessFile
import java.security.MessageDigest
import java.time.Instant
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

class LocalModelChatAdapterTest {
    private val now = 1_800_000_000_000L
    private val modelId = "serea-local/SmolLM2-135M-Q4_K_M"

    @Test fun defaultRuntimeLimitsMatchCurrentNativeBackendAndNameFileBoundAccurately() {
        val defaults = LocalModelRuntimeLimits()
        assertEquals(512, defaults.contextTokens)
        assertEquals(768L * 1024 * 1024, defaults.maxModelFileBytes)
        assertEquals(256, defaults.maxOutputTokens)
        assertEquals(2, defaults.workerThreads)
    }

    @Test fun runtimeLimitsRejectValuesOutsideCurrentNativeBackendCapabilities() {
        assertTrue(runCatching { LocalModelRuntimeLimits(contextTokens = 513) }
            .exceptionOrNull() is IllegalArgumentException)
        assertTrue(runCatching { LocalModelRuntimeLimits(maxOutputTokens = 257) }
            .exceptionOrNull() is IllegalArgumentException)
        assertTrue(runCatching { LocalModelRuntimeLimits(workerThreads = 3) }
            .exceptionOrNull() is IllegalArgumentException)
    }

    @Test fun modelFileLimitRejectsOversizeBeforeHashingOrNativeLoad() = runBlocking {
        val file = File.createTempFile("serea-model-oversize", ".gguf")
        RandomAccessFile(file, "rw").use { it.setLength(64L * 1024 * 1024 + 1) }
        val backend = FakeBackend()
        val artifact = artifact(file, sha256 = "0".repeat(64))
        val adapter = LocalModelChatAdapter(
            artifact = artifact,
            modelFile = file,
            backendFactory = LocalModelInferenceBackendFactory { backend },
            deviceAbi = "x86_64",
            runtimeLimits = LocalModelRuntimeLimits(maxModelFileBytes = 64L * 1024 * 1024),
        )
        try {
            val request = request("too large", sendId = "send-file-limit")
            assertTrue(runCatching {
                gated(adapter, registry { now }, request).send(request)
            }.exceptionOrNull() is PreDispatchModelChatFailure)
            assertEquals("Oversized model must not enter backend loading", 0, backend.loadCalls)
        } finally {
            adapter.close()
            file.delete()
        }
    }

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
            assertEquals("default context must fit the current JNI backend", 512, backend.lastLimits?.contextTokens)
            assertEquals(768L * 1024 * 1024, backend.lastLimits?.maxModelFileBytes)
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

    @Test fun cancellingLocalConsentRevokesPendingOneUseProof() = runBlocking {
        val registry = registry { now }
        val request = request("consent cancelled", sendId = "send-cancel-consent")
        val proof = registry.approve(request, modelId)
        assertTrue(registry.revoke(proof))
        assertFalse(registry.consumeOnce(proof, localContract(request, proof)))
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
            val gatedPort = EgressCheckedModelChatPort(adapter, { now }, EgressTarget.LOCAL_MODEL, registry, { now })
            assertEquals("local answer", gatedPort.send(request.copy(selectedModelId = modelId, egressContract = contract)).text)
            assertEquals(1, backend.generateCalls)
            assertFalse(registry.consumeOnce(proof, contract))

            val revokedRequest = request("Revoked before send", sendId = "send-revoked", origin = DataOrigin.GMAIL)
            val revokedProof = registry.approve(revokedRequest, modelId, evidence.copy(
                approvedChatBodySha256 = sha256Utf8(revokedRequest.message),
            ), conversationGeneration = 7)
            current = false
            val denied = EgressCheckedModelChatPort(adapter, { now }, EgressTarget.LOCAL_MODEL, registry, { now })
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
        val egress = EgressCheckedModelChatPort(usage, { clock }, EgressTarget.LOCAL_MODEL, registry, { clock })
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
                egressWith(noBindingRegistry, usage, { clock }).send(
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

    @Test fun knownPreInferenceFailuresReleaseUsageAndDoNotReachNativeGeneration() = runBlocking {
        val file = tempModel(byteArrayOf(2, 3, 5, 7))
        try {
            val factoryDao = CountingUsageDao()
            val factoryRegistry = registry { now }
            val factoryAdapter = LocalModelChatAdapter(
                artifact(file), file,
                LocalModelInferenceBackendFactory { error("synthetic backend factory failure") },
                deviceAbi = "x86_64",
            )
            val factoryRequest = request("factory fails", sendId = "send-factory-failure")
            assertTrue(runCatching {
                sendWithConsent(composed(factoryAdapter, factoryRegistry, factoryDao) { now }, factoryRegistry, factoryRequest)
            }.exceptionOrNull() is PreDispatchModelChatFailure)
            assertEquals(1, factoryDao.insertedReservationCount)
            assertEquals(0, factoryDao.reservationCount)

            val loadDao = CountingUsageDao()
            val loadRegistry = registry { now }
            val brokenBackend = FakeBackend(failLoad = true)
            val loadAdapter = createAdapter(artifact(file), file, brokenBackend)
            val loadRequest = request("load fails", sendId = "send-load-failure")
            assertTrue(runCatching {
                sendWithConsent(composed(loadAdapter, loadRegistry, loadDao) { now }, loadRegistry, loadRequest)
            }.exceptionOrNull() is PreDispatchModelChatFailure)
            assertEquals(1, loadDao.insertedReservationCount)
            assertEquals(0, loadDao.reservationCount)
            assertEquals(0, brokenBackend.generateCalls)
            assertTrue(brokenBackend.closed)

            val closedDao = CountingUsageDao()
            val closedRegistry = registry { now }
            val closedBackend = FakeBackend()
            val closedAdapter = createAdapter(artifact(file), file, closedBackend)
            closedAdapter.close()
            val closedRequest = request("already closed", sendId = "send-closed")
            assertTrue(runCatching {
                sendWithConsent(composed(closedAdapter, closedRegistry, closedDao) { now }, closedRegistry, closedRequest)
            }.exceptionOrNull() is PreDispatchModelChatFailure)
            assertEquals(1, closedDao.insertedReservationCount)
            assertEquals(0, closedDao.reservationCount)
            assertEquals(0, closedBackend.generateCalls)
        } finally { file.delete() }
    }

    @Test fun preGenerationCancellationReleasesUsageReservation() = runBlocking {
        val file = tempModel(byteArrayOf(3, 1, 4, 1))
        val backend = FakeBackend(blockLoad = true)
        val adapter = createAdapter(artifact(file), file, backend)
        val registry = registry { now }
        val dao = CountingUsageDao()
        try {
            val request = request("cancel during model load", sendId = "send-cancel-load")
            val send = async {
                sendWithConsent(composed(adapter, registry, dao) { now }, registry, request)
            }
            backend.loadStarted.await()
            send.cancelAndJoin()
            assertTrue(send.isCancelled)
            assertEquals(1, dao.insertedReservationCount)
            assertTrue(backend.cancelled)
            assertTrue(backend.closed)
            assertEquals(0, backend.generateCalls)
            assertEquals("A pre-inference cancellation must release its reservation", 0, dao.reservationCount)
            assertEquals(0, dao.unknownCount)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun postGenerationCancellationRemainsOutcomeUnknown() = runBlocking {
        val file = tempModel(byteArrayOf(2, 7, 1, 8))
        val backend = FakeBackend(blockGeneration = true)
        val adapter = createAdapter(artifact(file), file, backend)
        val registry = registry { now }
        val dao = CountingUsageDao()
        try {
            val request = request("cancel after native generation starts", sendId = "send-cancel-generation")
            val send = async {
                sendWithConsent(composed(adapter, registry, dao) { now }, registry, request)
            }
            backend.generationStarted.await()
            send.cancelAndJoin()
            assertTrue(send.isCancelled)
            assertTrue(backend.cancelled)
            assertTrue(backend.closed)
            assertEquals(1, backend.generateCalls)
            assertEquals(1, dao.insertedReservationCount)
            assertEquals(1, dao.reservationCount)
            assertEquals(1, dao.unknownCount)
            assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, dao.reservationState(request.sendId))
        } finally { adapter.close(); file.delete() }
    }

    @Test fun closeDuringLoadCancelsLoadAndEventuallyReleasesBackend() = runBlocking {
        val file = tempModel(byteArrayOf(1, 6, 1, 8))
        val backend = FakeBackend(blockLoad = true)
        val adapter = createAdapter(artifact(file), file, backend)
        val registry = registry { now }
        val dao = CountingUsageDao()
        try {
            val request = request("close during model load", sendId = "send-close-load")
            supervisorScope {
                val send = async {
                    sendWithConsent(composed(adapter, registry, dao) { now }, registry, request)
                }
                backend.loadStarted.await()
                adapter.close()
                assertTrue(runCatching { send.await() }.exceptionOrNull() is PreDispatchModelChatFailure)
            }
            assertEquals(1, dao.insertedReservationCount)
            assertTrue(backend.cancelled)
            assertTrue(backend.closed)
            assertEquals("Close won before the fake backend's native-load gate", 0, backend.loadCalls)
            assertEquals(0, backend.generateCalls)
            assertEquals(0, dao.reservationCount)
            assertEquals(0, dao.unknownCount)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun closeBeforeBackendPublicationPreventsLoadAndClosesLateFactoryResult() = runBlocking {
        val file = tempModel(byteArrayOf(5, 3, 5, 8))
        val backend = FakeBackend()
        val factoryEntered = CompletableDeferred<Unit>()
        val releaseFactory = CountDownLatch(1)
        val adapter = LocalModelChatAdapter(
            artifact(file), file,
            LocalModelInferenceBackendFactory {
                factoryEntered.complete(Unit)
                check(releaseFactory.await(10, TimeUnit.SECONDS))
                backend
            },
            deviceAbi = "x86_64",
        )
        val registry = registry { now }
        val dao = CountingUsageDao()
        try {
            val request = request("close while factory creates backend", sendId = "send-close-factory")
            supervisorScope {
                val send = async(Dispatchers.Default) {
                    sendWithConsent(composed(adapter, registry, dao) { now }, registry, request)
                }
                withTimeout(10_000) { factoryEntered.await() }
                adapter.close()
                releaseFactory.countDown()
                assertTrue(runCatching { send.await() }.isFailure)
            }
            assertEquals(0, backend.loadCalls)
            assertEquals(0, backend.generateCalls)
            assertTrue(backend.closed)
            assertEquals(1, dao.insertedReservationCount)
            assertEquals(0, dao.reservationCount)
            assertEquals(0, dao.unknownCount)
        } finally {
            releaseFactory.countDown()
            adapter.close()
            file.delete()
        }
    }

    @Test fun closeBetweenGenerateEntryAndNativeStartReleasesUsageReservation() = runBlocking {
        val file = tempModel(byteArrayOf(8, 9, 7, 9))
        val backend = FakeBackend(blockGenerationStart = true)
        val adapter = createAdapter(artifact(file), file, backend)
        val registry = registry { now }
        val dao = CountingUsageDao()
        try {
            val request = request("close before native generation", sendId = "send-close-generate-start")
            supervisorScope {
                val send = async {
                    sendWithConsent(composed(adapter, registry, dao) { now }, registry, request)
                }
                backend.generateEntry.await()
                adapter.close()
                assertTrue(runCatching { send.await() }.exceptionOrNull() is PreDispatchModelChatFailure)
            }
            assertTrue(backend.cancelled)
            assertTrue(backend.closed)
            assertEquals("Close won before the fake backend's native-generation gate", 0, backend.generateCalls)
            assertEquals(1, dao.insertedReservationCount)
            assertEquals(0, dao.reservationCount)
            assertEquals(0, dao.unknownCount)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun generationFailureAfterNativeDispatchRemainsOutcomeUnknown() = runBlocking {
        val file = tempModel(byteArrayOf(2, 7, 1, 8))
        val backend = FakeBackend(failGeneration = true)
        val adapter = createAdapter(artifact(file), file, backend)
        val registry = registry { now }
        val dao = CountingUsageDao()
        try {
            val request = request("native generation fails", sendId = "send-generation-unknown")
            assertTrue(runCatching {
                sendWithConsent(composed(adapter, registry, dao) { now }, registry, request)
            }.isFailure)
            assertEquals(1, dao.insertedReservationCount)
            assertEquals(1, backend.generateCalls)
            assertEquals(1, dao.unknownCount)
            assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, dao.reservationState(request.sendId))
        } finally { adapter.close(); file.delete() }
    }

    @Test fun handoffValidationRechecksBindingAndClockAfterSuspension() = runBlocking {
        val file = tempModel(byteArrayOf(4, 2, 4, 2))
        val backend = FakeBackend()
        val adapter = createAdapter(artifact(file), file, backend)
        var clock = now
        var generation = 7L
        val validationStarted = CompletableDeferred<Unit>()
        val finishValidation = CompletableDeferred<Unit>()
        val registry = LocalModelAuthorizationRegistry(
            { clock },
            handoffValidator = LocalModelProviderHandoffValidator { _, _ ->
                validationStarted.complete(Unit)
                finishValidation.await()
                true
            },
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding("conversation-1", generation)
            },
        )
        val dao = CountingUsageDao()
        try {
            val request = request("approved provider text", sendId = "send-stale-handoff", origin = DataOrigin.GMAIL)
            val evidence = fakeHandoff(request, expiresAt = now + 30_000, conversationGeneration = 7)
            val proof = registry.approve(request, modelId, evidence, conversationGeneration = 7)
            val egress = composed(adapter, registry, dao) { clock }
            supervisorScope {
                val sending = async {
                    egress.send(request.copy(selectedModelId = modelId, egressContract = localContract(request, proof, DataOrigin.GMAIL)))
                }
                validationStarted.await()
                generation = 8
                clock = proof.expiresAtMillis + 1
                finishValidation.complete(Unit)
                assertTrue(runCatching { sending.await() }.exceptionOrNull() is PreDispatchModelChatFailure)
            }
            assertEquals(0, dao.reservationCount)
            assertEquals(0, dao.insertedReservationCount)
            assertEquals(0, backend.generateCalls)
        } finally { finishValidation.complete(Unit); adapter.close(); file.delete() }
    }

    @Test fun localModelConsentExpiresAtExactDeadline() = runBlocking {
        val file = tempModel(byteArrayOf(6, 2, 6, 4))
        val backend = FakeBackend()
        val adapter = createAdapter(artifact(file), file, backend)
        var clock = now
        val registry = LocalModelAuthorizationRegistry(
            { clock }, maxLifetimeMillis = 10,
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding("conversation-1", 0)
            },
        )
        val dao = CountingUsageDao()
        try {
            val request = request("deadline", sendId = "send-exact-deadline")
            val proof = registry.approve(request, modelId)
            clock = proof.expiresAtMillis
            val egress = composed(adapter, registry, dao) { clock }
            assertTrue(runCatching {
                egress.send(request.copy(selectedModelId = modelId, egressContract = localContract(request, proof)))
            }.isFailure)
            assertEquals(0, dao.reservationCount)
            assertEquals(0, dao.insertedReservationCount)
            assertEquals(0, backend.generateCalls)
        } finally { adapter.close(); file.delete() }
    }

    @Test fun routedEgressUsageAndLocalAdapterComposeWithTestOnlyDescriptor() = runBlocking {
        val file = tempModel(byteArrayOf(1, 6, 1, 8, 0))
        val backend = FakeBackend()
        val adapter = createAdapter(artifact(file), file, backend)
        val clockInstant = Instant.parse("2026-10-28T00:00:00Z")
        val catalog = FakeModelCatalog(
            entries = listOf(ModelDescriptor(
                id = modelId, providerId = "local-device", displayName = "Fake local model test fixture",
                contextWindow = Knowledge.Known(2_048), maxOutput = Knowledge.Known(512),
                capabilities = ModelCapabilities(input = setOf("text/plain")),
                availability = ModelAvailability.AVAILABLE, authorized = true,
                lastVerifiedAt = clockInstant.toString(), source = "test fixture only",
                verificationStatus = ModelVerificationStatus.TEST_FIXTURE,
            )),
            clock = { clockInstant },
        )
        val registry = registry { now }
        val dao = CountingUsageDao()
        val usage = UsageControlledModelChatPort(
            adapter, UsageRepository(dao) { now },
            attributionResolver = LocalModelUsageAttributionResolver(artifact(file)),
        )
        val egress = EgressCheckedModelChatPort(usage, { now }, EgressTarget.LOCAL_MODEL, registry, { now })
        val routed = RoutedModelChatPort(
            delegate = egress,
            router = AdaptiveModelRouter(catalog),
            contractFactory = { original, selected ->
                val proof = registry.approve(original, selected.id)
                localContract(original, proof)
            },
            policy = {
                ModelRoutingPolicy(
                    mode = ModelSelectionMode.MANUAL, manualModelId = modelId,
                    allowedProviderIds = setOf("local-device"),
                )
            },
        )
        try {
            val request = request("router test", sendId = "send-router")
            val response = routed.send(request)
            assertEquals("local answer", response.text)
            assertEquals(1, backend.generateCalls)
            assertEquals(1, dao.insertedReservationCount)
            assertEquals(1, dao.reservationCount)
            assertEquals(UsageLedgerState.SETTLED.name, dao.reservationState(request.sendId))
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
        EgressCheckedModelChatPort(delegate, clock, EgressTarget.LOCAL_MODEL, registry, clock)

    private fun composed(
        adapter: ModelChatPort,
        registry: LocalModelAuthorizationRegistry,
        dao: CountingUsageDao,
        clock: () -> Long,
    ): EgressCheckedModelChatPort {
        val usage = UsageControlledModelChatPort(
            adapter, UsageRepository(dao, clock),
            attributionResolver = LocalModelUsageAttributionResolver(artifactForResolver),
        )
        return EgressCheckedModelChatPort(usage, clock, EgressTarget.LOCAL_MODEL, registry, clock)
    }

    private val artifactForResolver: LocalModelArtifact by lazy {
        // The resolver uses identity only; the hash and size are verified by the adapter itself.
        LocalModelArtifact(
            modelId = modelId, displayName = "Test local model", modelRepository = "https://huggingface.co/test/model",
            modelRevision = "1076b85d135017971845b19b8d07df1479155988", licenseId = "Apache-2.0",
            expectedSha256 = "0".repeat(64), expectedSizeBytes = 1, runtimeName = "test-runtime",
            runtimeRevision = "2763785", supportedAbis = setOf("x86_64"),
        )
    }

    private suspend fun sendWithConsent(
        egress: ModelChatPort,
        registry: LocalModelAuthorizationRegistry,
        request: ModelChatRequest,
        origin: DataOrigin = request.bodySourceLineage.single(),
    ): ModelChatResponse {
        val proof = registry.approve(request, modelId)
        return egress.send(request.copy(
            selectedModelId = modelId,
            egressContract = localContract(request, proof, origin),
        ))
    }

    private fun fakeHandoff(request: ModelChatRequest, expiresAt: Long, conversationGeneration: Long) = FakeHandoff(
        handoffId = "handoff-${request.sendId}", origin = request.bodySourceLineage.single(),
        serviceType = request.bodySourceLineage.single().name,
        syntheticAccountIdentifier = "synthetic-account-1", sessionIdentifier = "session-1",
        sessionGeneration = 3, scope = "gmail.readonly", sourceItemIdentifier = "message-1",
        sourceRevision = "rev-1", contentIdentitySha256 = "sha256:${sha256Utf8("raw source")}",
        selectedContent = "raw source", approvedChatBodySha256 = sha256Utf8(request.message),
        expiresAtMillis = expiresAt, authorizationState = "MOCK_ONLY", selectionState = "SELECTED",
        consentState = "LOCAL_PROCESSING_ONLY", destination = "LOCAL_CHAT",
        destinationConversationId = request.conversationId,
        destinationConversationGeneration = conversationGeneration,
    )

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
        private val blockLoad: Boolean = false,
        private val blockGenerationStart: Boolean = false,
    ) : LocalModelInferenceBackend {
        var loaded = false
        var closed = false
        var cancelled = false
        var generateCalls = 0
        var loadCalls = 0
        var lastLimits: LocalModelRuntimeLimits? = null
        var lastPrompt: String? = null
        val generateEntry = CompletableDeferred<Unit>()
        val generationStarted = CompletableDeferred<Unit>()
        private val releaseGeneration = CompletableDeferred<Unit>()
        private val releaseGenerationStart = CompletableDeferred<Unit>()
        val loadStarted = CompletableDeferred<Unit>()
        private val releaseLoad = CompletableDeferred<Unit>()

        override suspend fun load(modelPath: String, limits: LocalModelRuntimeLimits) {
            loadStarted.complete(Unit)
            if (blockLoad) releaseLoad.await()
            if (cancelled) throw LocalModelBackendStartDeniedException()
            lastLimits = limits
            loadCalls++
            if (failLoad) error("synthetic native load failure")
            loaded = true
        }
        override suspend fun generate(prompt: String, maxOutputTokens: Int): LocalModelInferenceResult {
            generateEntry.complete(Unit)
            if (blockGenerationStart) releaseGenerationStart.await()
            if (cancelled) throw LocalModelBackendStartDeniedException()
            generateCalls++
            lastPrompt = prompt
            generationStarted.complete(Unit)
            if (blockGeneration) releaseGeneration.await()
            if (failGeneration) error("synthetic native inference failure")
            return LocalModelInferenceResult("local answer", 4, 2)
        }
        override fun cancelActive() {
            cancelled = true
            releaseGeneration.complete(Unit)
            releaseLoad.complete(Unit)
            releaseGenerationStart.complete(Unit)
        }
        override fun close() { closed = true }
    }

    private class CountingUsageDao : UsageDao {
        private val windows = mutableMapOf<String, UsageWindowRow>()
        private val reservations = mutableMapOf<String, UsageReservationRow>()
        private var savedSettings: UsageSettingsRow? = null
        private val ledgers = mutableMapOf<String, ModelUsageLedgerRow>()
        var insertedReservationCount = 0
            private set
        val reservationCount get() = reservations.size
        val unknownCount get() = reservations.values.count { it.state == UsageLedgerState.OUTCOME_UNKNOWN.name }
        fun reservationState(id: String) = reservations[id]?.state

        override suspend fun window(key: String) = windows[key]
        override suspend fun putWindow(row: UsageWindowRow) { windows[row.windowKey] = row }
        override suspend fun reservation(id: String) = reservations[id]
        override suspend fun insertReservation(row: UsageReservationRow) {
            check(row.requestId !in reservations)
            insertedReservationCount++
            reservations[row.requestId] = row
        }
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
