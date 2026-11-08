package app.serea.android

import android.os.Debug
import android.os.SystemClock
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import java.io.FileInputStream
import java.security.MessageDigest
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
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
        measurements["first_output_tokens"] = firstTurn.outputTokens.toString()

        val secondGenerationStarted = SystemClock.elapsedRealtimeNanos()
        val secondTurn = first.generate("Continue with one brief sentence.", 48)
        measurements["second_generation_ms"] = elapsedMs(secondGenerationStarted).toString()
        assertTrue("second native generation must return text", secondTurn.text.isNotBlank())
        assertTokenUsage(secondTurn.inputTokens, secondTurn.outputTokens, 48)
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
        measurements["reloaded_generation_ms"] = elapsedMs(reloadedGenerationStarted).toString()
        assertTrue("reloaded generation ID must be positive", reloadedGenerationId > 0L)
        assertTrue("reloaded engine ID must be positive", reloadedGenerationEngineId > 0L)
        assertTrue("reloaded backend must use a new engine ID",
            reloadedGenerationEngineId != cancelledGenerationEngineId)
        assertTrue("reloaded request must use a distinct engine/generation identity pair",
            reloadedGenerationEngineId != cancelledGenerationEngineId || reloadedGenerationId != generationId)
        assertFalse("stale cancellation for the released engine must be rejected",
            seam.cancelGenerationForDiagnostic(cancelledGenerationEngineId, generationId))
        val reloadedResult = withTimeout(30_000) { reloadedRequest.await() }
        assertTrue("reloaded native model must generate", reloadedResult.text.isNotBlank())
        assertTokenUsage(reloadedResult.inputTokens, reloadedResult.outputTokens, 32)
        measurements["reloaded_generation_engine_id"] = reloadedGenerationEngineId.toString()
        measurements["reloaded_generation_id"] = reloadedGenerationId.toString()
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
        pss.toSortedMap().forEach { (name, value) -> measurements[name] = value.toString() }

        // Inject a JNI free refusal last: the existing C++ owner must remain intact and the
        // process coordinator must permanently deny reload until this disposable process exits.
        val retained = LlamaCppLocalModelInferenceBackend(::recordPss)
        retained.load(modelFile.absolutePath, limits())
        val allocationsWithRetainedModel = seam.modelAllocationCount()
        seam.failNextFree()
        retained.close()
        val failedRelease = retained.awaitReleaseResult(30, TimeUnit.SECONDS)
        assertTrue("JNI free refusal must not be reported as success", failedRelease is NativeReleaseResult.Failed)
        assertEquals(1, retained.nativeOwnershipForDiagnosticTest())
        assertEquals(NativeReleaseCoordinator.State.RELEASE_FAILED, processNativeReleaseCoordinator.currentState())
        val denied = LlamaCppLocalModelInferenceBackend()
        assertTrue("reload must fail closed after unknown/unreleased Native ownership", runCatching {
            denied.load(modelFile.absolutePath, limits())
        }.isFailure)
        assertEquals(allocationsWithRetainedModel, seam.modelAllocationCount())
        denied.close()
        assertEquals(NativeReleaseResult.NotAcquired, denied.awaitReleaseResult(30, TimeUnit.SECONDS))
        measurements["jni_free_failure_preserved_native_owner"] = "true"
        measurements["reload_after_free_failure_denied"] = "true"
        measurements["pss_after_free_failure_kb"] = Debug.getPss().toString()
        pss.toSortedMap().forEach { (name, value) -> measurements[name] = value.toString() }
        appendMeasurements()
        assertTrue("test GGUF must be deleted from app-private storage", modelFile.delete())
    }

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
        targetContext.openFileOutput(METRICS_FILE_NAME, android.content.Context.MODE_APPEND).bufferedWriter().use { writer ->
            measurements.forEach { (key, value) -> writer.append(key).append('=').append(value).append('\n') }
        }
    }

    private fun limits() = LocalModelRuntimeLimits(
        maxResidentMemoryBytes = 768L * 1024 * 1024,
        contextTokens = 512,
        maxOutputTokens = 256,
        workerThreads = 2,
    )

    private fun elapsedMs(startedNanos: Long) = (SystemClock.elapsedRealtimeNanos() - startedNanos) / 1_000_000L

    companion object {
        const val MODEL_FILE_NAME = "serea-r7-smollm2-135m-q4_k_m.gguf"
        const val METRICS_FILE_NAME = "native-gguf-metrics.properties"
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
