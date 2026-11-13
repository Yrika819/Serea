package app.serea.android

import android.content.ComponentCallbacks2
import android.os.Debug
import android.os.SystemClock
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.FileInputStream
import java.security.MessageDigest
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import org.junit.runners.JUnit4

/** Real llama.cpp lifecycle checks. The fixture is injected into app-private storage by CI. */
@RunWith(JUnit4::class)
class NativeGgufLifecycleTest {
    private val instrumentation = InstrumentationRegistry.getInstrumentation()
    private val targetContext = instrumentation.targetContext
    private val modelFile = File(targetContext.filesDir, MODEL_FILE_NAME)
    private val seam = NativeJniFailureSeam()
    private val pss = ConcurrentHashMap<String, Long>()
    private val measurements = linkedMapOf<String, String>()

    @Test fun realModelGeneratesCancelsRollsBackAndReloads() = runBlocking {
        validateFixture()
        measurements["api_level"] = android.os.Build.VERSION.SDK_INT.toString()
        measurements["abi"] = android.os.Build.SUPPORTED_ABIS.firstOrNull().orEmpty()
        assertEquals("R7 JNI integration requires API 35", 35, android.os.Build.VERSION.SDK_INT)
        assertEquals("R7 JNI integration requires x86_64", "x86_64", android.os.Build.SUPPORTED_ABIS.first())
        measurements["runner_profile"] = "Pixel_7_API_35_x86_64_CI"
        measurements["model_sha256"] = EXPECTED_SHA256
        measurements["model_size_bytes"] = EXPECTED_SIZE.toString()
        measurements["llama_cpp_revision"] = LLAMA_CPP_REVISION
        File(targetContext.filesDir, METRICS_FILE_NAME).delete()
        measurements["pss_before_load_kb"] = Debug.getPss().toString()

        val injected = LlamaCppLocalModelInferenceBackend(::recordPss) // Loads the explicitly opted-in diagnostic library.
        val allocationsBefore = seam.modelAllocationCount()
        val failuresBefore = seam.failureAfterModelCount()
        val rollbacksBefore = seam.partialRollbackCount()
        seam.failNextLoadAfterModel()
        val injectedFailure = runCatching { injected.load(modelFile.absolutePath, limits()) }
        assertTrue("injected post-model-allocation load must fail", injectedFailure.isFailure)
        assertTrue("JNI must have allocated the model before injection", seam.modelAllocationCount() > allocationsBefore)
        assertEquals(failuresBefore + 1, seam.failureAfterModelCount())
        assertEquals(rollbacksBefore + 1, seam.partialRollbackCount())
        assertEquals("rollback must clear all native ownership", 0, injected.nativeOwnershipForDiagnosticTest())
        assertEquals(NativeReleaseResult.Released, injected.awaitReleaseResult(30, TimeUnit.SECONDS))
        assertEquals(NativeReleaseCoordinator.State.RELEASED, processNativeReleaseCoordinator.currentState())
        measurements["owner_after_partial_rollback"] = "empty"
        measurements["coordinator_after_partial_rollback"] = "RELEASED"
        measurements["partial_allocation_failure_injected"] = "true"
        measurements["partial_allocation_rollback_confirmed"] = "true"
        injected.close() // Shut down its now-unowned executor after the rollback result was captured.

        val first = LlamaCppLocalModelInferenceBackend(::recordPss)
        val loadStarted = SystemClock.elapsedRealtimeNanos()
        first.load(modelFile.absolutePath, limits())
        measurements["load_ms"] = elapsedMs(loadStarted).toString()
        assertEquals(1, first.nativeOwnershipForDiagnosticTest())
        assertEquals(NativeReleaseCoordinator.State.ACTIVE, processNativeReleaseCoordinator.currentState())
        measurements["owner_after_first_load"] = "owned"
        measurements["coordinator_after_first_load"] = "ACTIVE"

        val firstGenerationStarted = SystemClock.elapsedRealtimeNanos()
        val firstTurn = first.generate("Say a short greeting.", 48)
        measurements["first_generation_ms"] = elapsedMs(firstGenerationStarted).toString()
        assertTrue("first native generation must return text", firstTurn.text.isNotBlank())
        assertTokenUsage(firstTurn.inputTokens, firstTurn.outputTokens, 48)
        measurements["first_input_tokens"] = firstTurn.inputTokens.toString()
        measurements["first_output_tokens"] = firstTurn.outputTokens.toString()

        val secondGenerationStarted = SystemClock.elapsedRealtimeNanos()
        val secondTurn = first.generate("Continue with one brief sentence.", 48)
        measurements["second_generation_ms"] = elapsedMs(secondGenerationStarted).toString()
        assertTrue("second native generation must return text", secondTurn.text.isNotBlank())
        assertTokenUsage(secondTurn.inputTokens, secondTurn.outputTokens, 48)
        measurements["second_input_tokens"] = secondTurn.inputTokens.toString()
        measurements["second_output_tokens"] = secondTurn.outputTokens.toString()

        val oldGenerationSequence = seam.generationSequence()
        val oldCancelCount = seam.nativeCancelCount()
        val cancellationStarted = SystemClock.elapsedRealtimeNanos()
        val inFlight = async(Dispatchers.IO) {
            runCatching {
                first.generate(
                    "Write a long, varied continuation with many details and keep going until stopped.",
                    256,
                )
            }
        }
        val generationId = awaitNativeGeneration(oldGenerationSequence)
        assertTrue("native generation ID must be positive", generationId > 0L)
        val cancelledGenerationEngineId = seam.generationEngineId()
        assertTrue("native generation engine ID must be positive", cancelledGenerationEngineId > 0L)
        first.cancelActive()
        val cancelledResult = withTimeout(30_000) { inFlight.await() }
        assertTrue("JNI generation must end through cancellation", cancelledResult.isFailure)
        assertEquals("cancel must target the active generation", generationId, seam.cancelObservedGenerationId())
        assertEquals(oldCancelCount + 1, seam.nativeCancelCount())
        assertEquals(3, seam.generationOutcome()) // TEST_GENERATION_CANCELLED
        val cancelledGenerationSequence = seam.generationSequence()
        measurements["cancel_completion_ms"] = elapsedMs(cancellationStarted).toString()
        measurements["cancelled_generation_id"] = generationId.toString()

        val firstFreeStarted = SystemClock.elapsedRealtimeNanos()
        first.close()
        assertEquals(NativeReleaseResult.Released, first.awaitReleaseResult(30, TimeUnit.SECONDS))
        measurements["first_free_ms"] = elapsedMs(firstFreeStarted).toString()
        assertEquals(0, first.nativeOwnershipForDiagnosticTest())
        assertEquals(NativeReleaseCoordinator.State.RELEASED, processNativeReleaseCoordinator.currentState())
        assertEquals(generationId, seam.cancelledReleaseGenerationId())
        assertEquals(cancelledGenerationSequence, seam.cancelledReleaseSequence())
        measurements["owner_after_cancel_and_free"] = "empty"
        measurements["coordinator_after_cancel_and_free"] = "RELEASED"
        measurements["cancelled_generation_engine_id"] = cancelledGenerationEngineId.toString()
        measurements["cancelled_generation_id"] = generationId.toString()

        val reload = LlamaCppLocalModelInferenceBackend(::recordPss)
        val reloadStarted = SystemClock.elapsedRealtimeNanos()
        reload.load(modelFile.absolutePath, limits())
        measurements["reload_ms"] = elapsedMs(reloadStarted).toString()
        assertEquals(1, reload.nativeOwnershipForDiagnosticTest())
        // A new backend must keep running even when the old engine/generation pair is cancelled.
        val reloadedGenerationStarted = SystemClock.elapsedRealtimeNanos()
        val reloadedSequence = seam.generationSequence()
        val reloadedRequest = async(Dispatchers.IO) {
            reload.generate("Confirm the reloaded engine responds.", 32)
        }
        val reloadedGenerationId = awaitNativeGeneration(reloadedSequence)
        val reloadedGenerationEngineId = seam.generationEngineId()
        assertTrue("reloaded generation ID must be positive", reloadedGenerationId > 0L)
        assertTrue("reloaded engine ID must be positive", reloadedGenerationEngineId > 0L)
        assertTrue("reloaded backend must use a new engine ID",
            reloadedGenerationEngineId != cancelledGenerationEngineId)
        assertTrue("reloaded request must use a distinct engine/generation identity pair",
            reloadedGenerationEngineId != cancelledGenerationEngineId || reloadedGenerationId != generationId)
        assertFalse("stale cancellation for the released engine must be rejected",
            seam.cancelGenerationForDiagnostic(cancelledGenerationEngineId, generationId))
        val reloadedResult = withTimeout(30_000) { reloadedRequest.await() }
        measurements["reloaded_generation_ms"] = elapsedMs(reloadedGenerationStarted).toString()
        assertTrue("reloaded native model must generate", reloadedResult.text.isNotBlank())
        assertTokenUsage(reloadedResult.inputTokens, reloadedResult.outputTokens, 32)
        measurements["reloaded_generation_engine_id"] = reloadedGenerationEngineId.toString()
        measurements["reloaded_generation_id"] = reloadedGenerationId.toString()
        measurements["reloaded_input_tokens"] = reloadedResult.inputTokens.toString()
        measurements["reloaded_output_tokens"] = reloadedResult.outputTokens.toString()
        assertEquals("prior cancellation must not target the new generation", 0, seam.cancelObservedGenerationId())
        assertEquals(oldCancelCount + 1, seam.nativeCancelCount())
        val finalFreeStarted = SystemClock.elapsedRealtimeNanos()
        reload.close()
        assertEquals(NativeReleaseResult.Released, reload.awaitReleaseResult(30, TimeUnit.SECONDS))
        measurements["final_free_ms"] = elapsedMs(finalFreeStarted).toString()
        assertEquals(0, reload.nativeOwnershipForDiagnosticTest())
        assertEquals(NativeReleaseCoordinator.State.RELEASED, processNativeReleaseCoordinator.currentState())
        measurements["owner_after_reload_free"] = "empty"
        measurements["coordinator_after_reload_free"] = "RELEASED"
        measurements["pss_after_release_kb"] = Debug.getPss().toString()

        runAdapterEvictionAndReload()
        pss.toSortedMap().forEach { (name, value) -> measurements[name] = value.toString() }
        appendMeasurements()
        assertTrue("test GGUF must be deleted from app-private storage", modelFile.delete())
    }

    private suspend fun runAdapterEvictionAndReload() {
        val modelId = "serea-local/native-eviction-test"
        val artifact = LocalModelArtifact(
            modelId = modelId, displayName = "Verified isolated GGUF fixture",
            modelRepository = "https://huggingface.co/ggml-org/SmolLM2-135M-GGUF",
            modelRevision = "1076b85d135017971845b19b8d07df1479155988",
            licenseId = "Apache-2.0", expectedSha256 = EXPECTED_SHA256,
            expectedSizeBytes = EXPECTED_SIZE, runtimeName = "llama.cpp",
            runtimeRevision = LLAMA_CPP_REVISION, supportedAbis = setOf("x86_64"),
        )
        val registry = LocalModelAuthorizationRegistry(
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding("native-eviction-conversation", 0)
            },
        )
        val callbackOwner = DiagnosticCallbacksOwner()
        val visibilityOwner = DiagnosticProcessVisibilityOwner()
        val lifecycleScope = CoroutineScope(SupervisorJob() + Dispatchers.IO)
        val lifecycleEvents = mutableListOf<LocalModelMemoryLifecycle.Event>()
        var backendCreations = 0
        var lastBackend: LlamaCppLocalModelInferenceBackend? = null
        val composition = LocalModelRuntimeApplicationComposition.create(
            modelDirectory = targetContext.noBackupFilesDir,
            callbacks = callbackOwner,
            processVisibility = visibilityOwner,
            scope = lifecycleScope,
            artifact = artifact,
            modelFile = modelFile,
            backendFactory = LocalModelInferenceBackendFactory {
                backendCreations++
                LlamaCppLocalModelInferenceBackend(::recordPss).also { lastBackend = it }
            },
            // Keep adapter lifecycle probes short on the CPU emulator. The direct JNI
            // generations above still exercise 48-token output and a 256-token cancelled
            // generation; these checks need only establish that eviction/reload can generate.
            deviceAbi = "x86_64",
            runtimeLimits = limits(maxOutputTokens = 16),
            memoryFitCheck = LocalModelMemoryFitCheck { true },
            authorizations = registry,
            eventObserver = { synchronized(lifecycleEvents) { lifecycleEvents += it } },
        )
        val adapter = composition.adapter
        assertTrue("registered lifecycle owns this same adapter", composition.lifecycle.adapter === adapter)
        val egress = EgressCheckedModelChatPort(
            adapter, expectedTarget = EgressTarget.LOCAL_MODEL,
            localModelAuthorizations = registry,
            localMonotonicClock = { SystemClock.elapsedRealtime() },
        )
        try {
            composition.register()
            assertEquals("one Application callback is registered", 1, callbackOwner.registered.size)
            val first = localRequest("adapter generation before eviction", "adapter-evict-first", modelId)
            val firstProof = registry.approve(first, modelId)
            val firstResult = egress.send(first.copy(
                selectedModelId = modelId,
                egressContract = localContract(first, modelId, firstProof),
            ))
            assertTrue(firstResult.text.isNotBlank())
            val firstEngine = seam.generationEngineId()
            assertTrue(firstEngine > 0L)

            val stale = localRequest("proof held across eviction", "adapter-evict-stale", modelId)
            val staleProof = registry.approve(stale, modelId)
            val evictionStarted = SystemClock.elapsedRealtimeNanos()
            val backgroundCallback = callbackOwner.registered.single() as LocalModelMemoryLifecycle
            // Synthetic-test dispatch uses the registered ComponentCallbacks2 object and preserves
            // SYNTHETIC_TEST attribution; it is not evidence that Android delivered an OS event.
            backgroundCallback.dispatchSyntheticTrimForTest(ComponentCallbacks2.TRIM_MEMORY_BACKGROUND)
            awaitLifecycleGeneration(registry, expected = 1L)
            assertEquals(NativeReleaseResult.Released, checkNotNull(lastBackend).awaitReleaseResult(30, TimeUnit.SECONDS))
            assertEquals(NativeReleaseCoordinator.State.RELEASED, processNativeReleaseCoordinator.currentState())
            assertEquals(1L, registry.lifecycle.currentGeneration())
            assertEquals(LocalModelMemoryLifecycle.Source.SYNTHETIC_TEST,
                synchronized(lifecycleEvents) { lifecycleEvents.last().source })
            measurements["adapter_eviction_callback_level"] = ComponentCallbacks2.TRIM_MEMORY_BACKGROUND.toString()
            measurements["adapter_eviction_ms"] = elapsedMs(evictionStarted).toString()
            measurements["adapter_eviction_released"] = "true"
            measurements["adapter_first_eviction_release_confirmed"] = "true"
            measurements["adapter_synthetic_callback_source"] = LocalModelMemoryLifecycle.Source.SYNTHETIC_TEST.name
            assertTrue("eviction invalidates prior local consent", runCatching {
                egress.send(stale.copy(
                    selectedModelId = modelId,
                    egressContract = localContract(stale, modelId, staleProof),
                ))
            }.exceptionOrNull() is PreDispatchModelChatFailure)
            measurements["adapter_stale_consent_rejected"] = "true"

            val fresh = localRequest("reload after adapter eviction", "adapter-evict-reload", modelId)
            val freshProof = registry.approve(fresh, modelId)
            val reloadStarted = SystemClock.elapsedRealtimeNanos()
            val freshResult = egress.send(fresh.copy(
                selectedModelId = modelId,
                egressContract = localContract(fresh, modelId, freshProof),
            ))
            measurements["adapter_reload_and_generation_ms"] = elapsedMs(reloadStarted).toString()
            assertTrue("new adapter backend must generate after eviction", freshResult.text.isNotBlank())
            val freshEngine = seam.generationEngineId()
            assertTrue("eviction reload uses a fresh engine identity", freshEngine > 0 && freshEngine != firstEngine)
            measurements["adapter_first_engine_id"] = firstEngine.toString()
            measurements["adapter_reloaded_engine_id"] = freshEngine.toString()

            // Inject the JNI failure through adapter eviction itself. It must retain Native
            // ownership and deny both adapter reload and a second backend acquisition.
            val allocationsWithRetainedModel = seam.modelAllocationCount()
            seam.failNextFree()
            val failedEvictionStarted = SystemClock.elapsedRealtimeNanos()
            backgroundCallback.dispatchSyntheticTrimForTest(ComponentCallbacks2.TRIM_MEMORY_BACKGROUND)
            awaitLifecycleGeneration(registry, expected = 2L)
            awaitCoordinatorState(NativeReleaseCoordinator.State.RELEASE_FAILED)
            assertTrue("all diagnostic callback stimuli are explicitly synthetic",
                synchronized(lifecycleEvents) { lifecycleEvents.all { it.source == LocalModelMemoryLifecycle.Source.SYNTHETIC_TEST } })
            measurements["adapter_failed_eviction_ms"] = elapsedMs(failedEvictionStarted).toString()
            assertTrue(checkNotNull(lastBackend).awaitReleaseResult(30, TimeUnit.SECONDS) is NativeReleaseResult.Failed)
            assertEquals(1, checkNotNull(lastBackend).nativeOwnershipForDiagnosticTest())
            assertEquals(NativeReleaseCoordinator.State.RELEASE_FAILED, processNativeReleaseCoordinator.currentState())
            measurements["adapter_eviction_free_failure_fail_closed"] = "true"
            measurements["adapter_native_owner_after_free_failure"] = "retained"

            val deniedRequest = localRequest("reload denied after JNI free refusal", "adapter-evict-free-fail", modelId)
            val deniedProof = registry.approve(deniedRequest, modelId)
            assertTrue(runCatching {
                egress.send(deniedRequest.copy(
                    selectedModelId = modelId,
                    egressContract = localContract(deniedRequest, modelId, deniedProof),
                ))
            }.exceptionOrNull() is PreDispatchModelChatFailure)
            assertEquals("failed release must not create another backend", 2, backendCreations)
            assertEquals(allocationsWithRetainedModel, seam.modelAllocationCount())
            measurements["jni_free_failure_preserved_native_owner"] = "true"
            measurements["reload_after_free_failure_denied"] = "true"

            val denied = LlamaCppLocalModelInferenceBackend()
            assertTrue("Native coordinator denies a backend after eviction free failure", runCatching {
                denied.load(modelFile.absolutePath, limits())
            }.isFailure)
            denied.close()
            assertEquals(NativeReleaseResult.NotAcquired, denied.awaitReleaseResult(30, TimeUnit.SECONDS))
            measurements["pss_after_free_failure_kb"] = Debug.getPss().toString()
        } finally {
            composition.close()
            lifecycleScope.cancel()
        }
    }

    private class DiagnosticCallbacksOwner : ComponentCallbacks2Owner {
        val registered = mutableListOf<android.content.ComponentCallbacks2>()
        override fun registerComponentCallbacks(callback: android.content.ComponentCallbacks2) {
            if (callback !in registered) registered += callback
        }
        override fun unregisterComponentCallbacks(callback: android.content.ComponentCallbacks2) {
            registered.remove(callback)
        }
    }

    private class DiagnosticProcessVisibilityOwner : ProcessVisibilityOwner {
        private val registered = mutableListOf<ProcessVisibilityObserver>()
        override fun register(observer: ProcessVisibilityObserver) {
            if (observer !in registered) registered += observer
        }
        override fun unregister(observer: ProcessVisibilityObserver) { registered.remove(observer) }
    }

    private suspend fun awaitLifecycleGeneration(registry: LocalModelAuthorizationRegistry, expected: Long) = withTimeout(30_000) {
        while (registry.lifecycle.currentGeneration() < expected) delay(5)
    }

    private suspend fun awaitCoordinatorState(expected: NativeReleaseCoordinator.State) = withTimeout(30_000) {
        while (processNativeReleaseCoordinator.currentState() != expected) delay(5)
    }

    private fun localRequest(message: String, sendId: String, modelId: String) = ModelChatRequest(
        conversationId = "native-eviction-conversation", message = message, sendId = sendId,
        timeoutMillis = 60_000, classification = DataClassification.PUBLIC,
        bodySourceLineage = setOf(DataOrigin.USER_AUTHORED), selectedModelId = modelId,
    )

    private fun localContract(request: ModelChatRequest, modelId: String, proof: LocalModelConsentProof) = EgressContract(
        conversationId = request.conversationId, currentMessageId = request.sendId,
        origin = DataOrigin.USER_AUTHORED, classification = request.classification,
        destination = ChatEgressGate.LOCAL_MODEL_DESTINATION, plannedBody = request.message,
        context = request.context, preparation = PayloadPreparation.RAW,
        explicitSendIntentAtMillis = null, expiresAtMillis = null,
        selectedModelId = modelId, target = EgressTarget.LOCAL_MODEL,
        plannedBodySourceLineage = request.bodySourceLineage, localModelConsent = proof,
    )

    private fun validateFixture() {
        assertTrue("verified GGUF fixture must be installed in app-private storage", modelFile.isFile)
        assertEquals(EXPECTED_SIZE, modelFile.length())
        val digest = MessageDigest.getInstance("SHA-256")
        FileInputStream(modelFile).use { input ->
            val buffer = ByteArray(1024 * 1024)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                digest.update(buffer, 0, read)
            }
        }
        val digestHex = digest.digest().joinToString("") { "%02x".format(it.toInt() and 0xff) }
        assertEquals(EXPECTED_SHA256, digestHex)
    }

    private suspend fun awaitNativeGeneration(previousSequence: Int): Long = withTimeout(30_000) {
        while (true) {
            val sequence = seam.generationSequence()
            val progress = seam.generationProgress()
            if (sequence > previousSequence) {
                check(progress != -2) { "Native generation completed before the cancellation checkpoint" }
                if (progress >= 1) return@withTimeout seam.generationId()
            }
            delay(5) // Poll the JNI progress event; cancellation is issued only after native entry.
        }
        error("unreachable")
    }

    private fun assertTokenUsage(input: Int?, output: Int?, requestBound: Int) {
        assertNotNull("JNI must return an input token count", input)
        assertNotNull("JNI must return an output token count", output)
        assertTrue("input token count must be positive", input!! > 0)
        assertTrue("output token count must be positive", output!! > 0)
        assertTrue("output tokens must not exceed the request bound", output <= requestBound)
    }

    private fun recordPss(name: String, value: Long) { pss[name] = value }

    private fun appendMeasurements() {
        val safeMeasurements = measurements.map { (key, value) -> "$key=$value" }
        targetContext.openFileOutput(METRICS_FILE_NAME, android.content.Context.MODE_APPEND).bufferedWriter().use { writer ->
            safeMeasurements.forEach { writer.append(it).append('\n') }
        }
        // Gradle may uninstall the isolated target after connected tests. Mirror only the
        // allowlisted numeric/boolean/provenance metrics to logcat so the host can collect them
        // before cleanup without relying on app-data persistence. Generated text is never logged.
        safeMeasurements.forEach { android.util.Log.i(METRICS_LOG_TAG, "R7_METRIC $it") }
    }

    private fun limits(maxOutputTokens: Int = 256) = LocalModelRuntimeLimits(
        maxResidentMemoryBytes = 768L * 1024 * 1024,
        contextTokens = 512,
        maxOutputTokens = maxOutputTokens,
        workerThreads = 2,
    )

    private fun elapsedMs(startedNanos: Long) = (SystemClock.elapsedRealtimeNanos() - startedNanos) / 1_000_000L

    companion object {
        const val MODEL_FILE_NAME = "serea-r7-smollm2-135m-q4_k_m.gguf"
        const val METRICS_FILE_NAME = "native-gguf-metrics.properties"
        const val METRICS_LOG_TAG = "SereaNativeGGUF"
        const val EXPECTED_SIZE = 101_016_128L
        const val EXPECTED_SHA256 = "e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5"
        const val LLAMA_CPP_REVISION = "276378515ceebee5d14bcc8536603dd5c87491a2"
    }
}

/** JNI entry points exist only in the explicitly opted-in isolated diagnostic library. */
internal class NativeJniFailureSeam {
    external fun failNextLoadAfterModel()
    external fun failNextFree()
    external fun cancelGenerationForDiagnostic(engineId: Long, generationId: Long): Boolean
    external fun generationProgress(): Int
    external fun generationSequence(): Int
    external fun nativeCancelCount(): Int
    external fun generationId(): Long
    external fun generationEngineId(): Long
    external fun generationOutcome(): Int
    external fun cancelObservedGenerationId(): Long
    external fun cancelledReleaseGenerationId(): Long
    external fun cancelledReleaseSequence(): Int
    external fun modelAllocationCount(): Int
    external fun failureAfterModelCount(): Int
    external fun partialRollbackCount(): Int
}
