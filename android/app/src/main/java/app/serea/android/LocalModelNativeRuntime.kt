package app.serea.android

import java.io.File
import android.os.Debug
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CancellableContinuation
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** Creates the JNI backend only in the explicit diagnostic/native build. */
internal class LlamaCppLocalModelInferenceBackendFactory(
    private val nativeRuntimeEnabled: Boolean = BuildConfig.LOCAL_NATIVE_RUNTIME_ENABLED,
) : LocalModelInferenceBackendFactory {
    override fun create(): LocalModelInferenceBackend {
        if (!nativeRuntimeEnabled) {
            throw PreDispatchModelChatFailure("The native local-model runtime is disabled in this build.")
        }
        return try {
            LlamaCppLocalModelInferenceBackend()
        } catch (_: LinkageError) {
            throw PreDispatchModelChatFailure("The native local-model library is unavailable for this build.")
        }
    }
}

/**
 * Product `LocalModelInferenceBackend` bridge, based on the separately reviewed G3 JNI PoC.
 * The native library owns one process-wide llama.cpp model lease and serializes every operation.
 * `close()` only signals cancellation and queues release; it never waits on the Android caller.
 */
internal class LlamaCppLocalModelInferenceBackend private constructor(
    /** Optional bounded diagnostic hook; values are PSS KiB and never contain model output. */
    private val diagnosticMetric: ((String, Long) -> Unit)? = null,
    /** Internal test seam; regular construction uses the JNI methods below. */
    private val nativeCallsOverride: NativeRuntimeCalls? = null,
    private val coordinator: NativeReleaseCoordinator = processNativeReleaseCoordinator,
    private val executorOverride: java.util.concurrent.ExecutorService? = null,
) : LocalModelInferenceBackend {
    internal constructor(
        diagnosticMetric: ((String, Long) -> Unit)? = null,
    ) : this(diagnosticMetric, null, processNativeReleaseCoordinator, null)

    private data class Generation(
        val id: Long,
        var mayRunInNative: Boolean = false,
        var cancelled: Boolean = false,
        var completed: Boolean = false,
    )

    companion object {
        private val nextEngineId = AtomicLong(0)
    }

    init { if (nativeCallsOverride == null) System.loadLibrary("serea_local_llama") }

    private val engineId = nextEngineId.incrementAndGet()
    private val lifecycleLock = Any()
    private val executor = executorOverride ?: Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "serea-local-llama").apply { isDaemon = true }
    }
    private val activeGenerations = linkedSetOf<Generation>()
    private val releaseResult = CompletableDeferred<NativeReleaseResult>()
    // Object identity is deliberately distinct from the JNI engine id (which is a boxed Long).
    private val ownershipToken = Any()
    private val nativeCalls: NativeRuntimeCalls by lazy {
        nativeCallsOverride ?: object : NativeRuntimeCalls {
            override fun load(engineId: Long, path: String, contextTokens: Int, workerThreads: Int) =
                nativeLoad(engineId, path, contextTokens, workerThreads)
            override fun ownership(engineId: Long) = nativeOwnsModel(engineId)
            override fun generate(engineId: Long, prompt: String, maxOutputTokens: Int, generationId: Long) =
                nativeGenerate(engineId, prompt, maxOutputTokens, generationId)
            override fun cancel(engineId: Long, generationId: Long) = nativeCancel(engineId, generationId)
            override fun forgetCancellation(engineId: Long, generationId: Long) = nativeForgetCancellation(engineId, generationId)
            override fun usageCounts(engineId: Long, generationId: Long) = nativeUsageCounts(engineId, generationId)
            override fun forgetUsage(engineId: Long, generationId: Long) = nativeForgetUsage(engineId, generationId)
            override fun free(engineId: Long) = nativeFree(engineId)
        }
    }
    private var nextGenerationId = 1L
    private var diagnosticInferenceIndex = 0
    private var nativeLoadEntered = false
    @Volatile private var closed = false
    @Volatile private var loaded = false
    private var loadAttempted = false

    override suspend fun load(modelPath: String, limits: LocalModelRuntimeLimits) {
        val model = File(modelPath)
        val capabilities = NativeRuntimeCapabilities()
        if (!model.isFile || !model.canRead()) {
            throw PreDispatchModelChatFailure("The model file exceeds the native runtime's file-size bound.")
        }
        // The reviewed PoC ABI supports 128–512 context tokens, two CPU threads, and at most
        // 256 output tokens. Reject unsupported configurations before native allocation.
        capabilities.validate(limits, model.length())
        suspendCancellableCoroutine { continuation ->
            val queued = synchronized(lifecycleLock) {
                if (closed || loaded || loadAttempted || !continuation.isActive) false else try {
                    // close() takes this same lock: closed check, process lease and executor
                    // submission are atomic with respect to close-vs-load races.
                    loadAttempted = true
                    coordinator.acquire(ownershipToken)
                    executor.execute {
                        val mayStartNativeLoad = synchronized(lifecycleLock) {
                            if (closed || !continuation.isActive) false
                            else { nativeLoadEntered = true; true }
                        }
                        if (!mayStartNativeLoad) {
                            if (coordinator.releaseWithoutNativeStart(ownershipToken)) {
                                completeRelease(NativeReleaseResult.Released) // JNI never acquired an engine.
                            }
                            continuation.failIfActive(LocalModelBackendStartDeniedException())
                            return@execute
                        }
                        try {
                            reportPss("load_before_kb")
                            val nativeLoadSucceeded = nativeCalls.load(
                                engineId,
                                model.canonicalPath,
                                limits.contextTokens,
                                limits.workerThreads,
                            )
                            val success = nativeLoadSucceeded && nativeCalls.ownership(engineId) == 1
                            reportPss("load_after_kb")
                            val publish = synchronized(lifecycleLock) {
                                if (success && !closed && continuation.isActive) loaded = true
                                success && !closed && continuation.isActive
                            }
                            if (!publish) {
                                val freed = releaseNativeOwnershipIfPresent()
                                finishRelease(freed, IllegalStateException("Native rollback failed"))
                                if (success) continuation.failIfActive(LocalModelBackendStartDeniedException())
                                else continuation.failIfActive(IllegalStateException("llama.cpp could not load the verified model"))
                            } else {
                                if (continuation.isActive) continuation.resume(Unit)
                            }
                        } catch (failure: Throwable) {
                            if (coordinator.owns(ownershipToken) && coordinator.currentState() == NativeReleaseCoordinator.State.ACTIVE) {
                                runCatching { coordinator.releasing(ownershipToken) }
                            }
                            val freed = runCatching { releaseNativeOwnershipIfPresent() }.getOrDefault(false)
                            finishRelease(freed, failure)
                            continuation.failIfActive(failure)
                        }
                    }
                    true
                } catch (_: java.util.concurrent.RejectedExecutionException) {
                    if (coordinator.releaseWithoutNativeStart(ownershipToken)) {
                        completeRelease(NativeReleaseResult.NotAcquired)
                    } else {
                        failRelease(IllegalStateException("The native executor rejected the load with Native ownership unresolved"))
                    }
                    false
                } catch (_: IllegalStateException) {
                    if (coordinator.owns(ownershipToken) && !nativeLoadEntered &&
                        coordinator.releaseWithoutNativeStart(ownershipToken)) {
                        completeRelease(NativeReleaseResult.NotAcquired)
                    }
                    false
                }
            }
            if (!queued) {
                continuation.failIfActive(IllegalStateException("The native model executor is closed"))
            }
        }
    }

    override suspend fun generate(prompt: String, maxOutputTokens: Int): LocalModelInferenceResult =
        suspendCancellableCoroutine { continuation ->
            val generation = synchronized(lifecycleLock) {
                if (closed || !loaded) null else Generation(nextGenerationId++).also { request ->
                    activeGenerations += request
                    continuation.invokeOnCancellation {
                        synchronized(lifecycleLock) {
                            request.cancelled = true
                            if (request.mayRunInNative && !request.completed) runCatching { nativeCalls.cancel(engineId, request.id) }
                        }
                    }
                }
            }
            if (generation == null) {
                continuation.failIfActive(LocalModelBackendStartDeniedException())
                return@suspendCancellableCoroutine
            }

            try {
                executor.execute {
                    val mayStart = synchronized(lifecycleLock) {
                        if (closed || generation.cancelled || !continuation.isActive) {
                            generation.completed = true
                            activeGenerations.remove(generation)
                            false
                        } else {
                            // This lock establishes whether close/cancellation or native dispatch won.
                            generation.mayRunInNative = true
                            true
                        }
                    }
                    if (!mayStart) {
                        continuation.failIfActive(LocalModelBackendStartDeniedException())
                        return@execute
                    }
                    try {
                        val measurementIndex = synchronized(lifecycleLock) { ++diagnosticInferenceIndex }
                        reportPss("inference_${measurementIndex}_before_kb")
                        val output = nativeCalls.generate(engineId, prompt, maxOutputTokens, generation.id)
                        reportPss("inference_${measurementIndex}_after_kb")
                        val usage = nativeCalls.usageCounts(engineId, generation.id)
                        val inputTokens = usage?.getOrNull(0)?.takeIf { it >= 0 }
                        val outputTokens = usage?.getOrNull(1)?.takeIf { it >= 0 }
                        if (continuation.isActive) {
                            continuation.resume(LocalModelInferenceResult(output, inputTokens, outputTokens))
                        }
                    } catch (failure: Throwable) {
                        continuation.failIfActive(failure)
                    } finally {
                        // Record a terminal sample even when generation throws or is cancelled.
                        // The callback is optional and its failures cannot affect inference.
                        val measurementIndex = synchronized(lifecycleLock) { diagnosticInferenceIndex }
                        reportPss("inference_${measurementIndex}_terminal_kb")
                        synchronized(lifecycleLock) {
                            generation.completed = true
                            activeGenerations.remove(generation)
                        }
                        nativeCalls.forgetCancellation(engineId, generation.id)
                        nativeCalls.forgetUsage(engineId, generation.id)
                    }
                }
            } catch (failure: java.util.concurrent.RejectedExecutionException) {
                synchronized(lifecycleLock) {
                    generation.completed = true
                    activeGenerations.remove(generation)
                }
                continuation.failIfActive(failure)
            }
        }

    override fun cancelActive() {
        synchronized(lifecycleLock) {
            activeGenerations.forEach { generation ->
                generation.cancelled = true
                if (generation.mayRunInNative && !generation.completed) runCatching { nativeCalls.cancel(engineId, generation.id) }
            }
        }
    }

    override fun close() {
        synchronized(lifecycleLock) {
            if (closed) return
            closed = true
            activeGenerations.forEach { generation ->
                generation.cancelled = true
                if (generation.mayRunInNative && !generation.completed) runCatching { nativeCalls.cancel(engineId, generation.id) }
            }
            try {
                if (coordinator.owns(ownershipToken) &&
                    coordinator.currentState() == NativeReleaseCoordinator.State.ACTIVE) {
                    if (!coordinator.releasing(ownershipToken)) {
                        failRelease(IllegalStateException("Coordinator rejected Native close transition"))
                        executor.shutdown()
                        return
                    }
                }
                executor.execute {
                    if (!coordinator.owns(ownershipToken)) {
                        completeRelease(NativeReleaseResult.NotAcquired) // This backend never acquired native ownership.
                        executor.shutdown()
                        return@execute
                    }
                    if (!nativeLoadEntered) {
                        finishNeverLoadedRelease()
                        executor.shutdown()
                        return@execute
                    }
                    val result = runCatching {
                        check(nativeCalls.free(engineId)) { "JNI did not confirm native model release" }
                        check(nativeCalls.ownership(engineId) == 0) { "JNI still reports Native ownership after release" }
                    }
                    finishRelease(result.isSuccess, result.exceptionOrNull())
                    loaded = false
                    executor.shutdown()
                }
                executor.shutdown()
            } catch (failure: Throwable) {
                executor.shutdownNow()
                failRelease(failure)
            }
        }
    }

    /** Nonblocking observation of release; failure and timeout never grant ownership. */
    internal suspend fun awaitReleaseResult(timeout: Long, unit: TimeUnit): NativeReleaseResult =
        kotlinx.coroutines.withTimeoutOrNull(unit.toMillis(timeout)) { releaseResult.await() }
            ?: NativeReleaseResult.TimedOut

    internal suspend fun awaitReleased(timeout: Long, unit: TimeUnit): Boolean = when (awaitReleaseResult(timeout, unit)) {
        NativeReleaseResult.Released, NativeReleaseResult.NotAcquired -> true
        is NativeReleaseResult.Failed, NativeReleaseResult.TimedOut -> false
    }

    /** JNI no-model probe usable only by the explicitly opted-in isolated diagnostic variant. */
    internal fun nativeOwnershipForDiagnosticTest(): Int {
        check(BuildConfig.LOCAL_NATIVE_RUNTIME_ENABLED) { "Native diagnostic probe is disabled in this build" }
        return nativeCalls.ownership(engineId)
    }

    internal fun nativeFreeUnownedForDiagnosticTest(): Boolean {
        check(BuildConfig.LOCAL_NATIVE_RUNTIME_ENABLED) { "Native diagnostic probe is disabled in this build" }
        return nativeCalls.free(engineId)
    }

    private fun <T> CancellableContinuation<T>.failIfActive(failure: Throwable) {
        if (isActive) resumeWithException(failure)
    }

    private fun reportPss(name: String) {
        val sink = diagnosticMetric ?: return
        runCatching { sink(name, Debug.getPss().toLong()) }
    }

    /** A failed nativeLoad rolls its partial allocation back before returning false. */
    private fun releaseNativeOwnershipIfPresent(): Boolean {
        return when (nativeCalls.ownership(engineId)) {
            0 -> true // C++ confirms no engine lease and no model/context/sampler pointers.
            1 -> nativeCalls.free(engineId) && nativeCalls.ownership(engineId) == 0
            else -> false // Foreign owner or inconsistent native state: fail closed.
        }
    }

    private fun finishNeverLoadedRelease() {
        val completion = runCatching {
            if (coordinator.currentState() == NativeReleaseCoordinator.State.ACTIVE) {
                check(coordinator.releasing(ownershipToken))
            }
            check(coordinator.released(ownershipToken)) { "Coordinator rejected release of an unloaded Native lease" }
        }
        if (completion.isSuccess) completeRelease(NativeReleaseResult.Released)
        else failRelease(completion.exceptionOrNull()!!)
    }

    private fun finishRelease(nativeReleaseSucceeded: Boolean, nativeFailure: Throwable?) {
        if (!nativeReleaseSucceeded) {
            failRelease(nativeFailure ?: IllegalStateException("Native release was not confirmed"))
            return
        }
        val completion = runCatching {
            when (coordinator.currentState()) {
                NativeReleaseCoordinator.State.ACTIVE -> check(coordinator.releasing(ownershipToken))
                NativeReleaseCoordinator.State.RELEASING -> Unit
                else -> error("Coordinator is not awaiting this Native release")
            }
            check(coordinator.released(ownershipToken)) { "Coordinator rejected the Native release completion" }
        }
        if (completion.isSuccess) completeRelease(NativeReleaseResult.Released)
        else failRelease(completion.exceptionOrNull()!!)
    }

    private fun failRelease(failure: Throwable) {
        runCatching {
            if (coordinator.owns(ownershipToken) && coordinator.currentState() != NativeReleaseCoordinator.State.RELEASE_FAILED) {
                if (coordinator.currentState() == NativeReleaseCoordinator.State.ACTIVE) coordinator.releasing(ownershipToken)
                if (coordinator.currentState() == NativeReleaseCoordinator.State.RELEASING) coordinator.failed(ownershipToken)
            }
        }
        completeRelease(NativeReleaseResult.Failed(failure))
    }

    private fun completeRelease(result: NativeReleaseResult) {
        releaseResult.complete(result)
    }

    private external fun nativeLoad(engineId: Long, path: String, contextTokens: Int, workerThreads: Int): Boolean
    private external fun nativeOwnsModel(engineId: Long): Int
    private external fun nativeGenerate(engineId: Long, prompt: String, maxOutputTokens: Int, generationId: Long): String
    private external fun nativeCancel(engineId: Long, generationId: Long): Boolean
    private external fun nativeForgetCancellation(engineId: Long, generationId: Long)
    private external fun nativeUsageCounts(engineId: Long, generationId: Long): IntArray?
    private external fun nativeForgetUsage(engineId: Long, generationId: Long)
    private external fun nativeFree(engineId: Long): Boolean
}

internal sealed interface NativeReleaseResult {
    data object Released : NativeReleaseResult
    data object NotAcquired : NativeReleaseResult
    data object TimedOut : NativeReleaseResult
    data class Failed(val cause: Throwable) : NativeReleaseResult
}

/** Internal seam for deterministic JVM lifecycle tests; no public or UI entry point is provided. */
internal interface NativeRuntimeCalls {
    fun load(engineId: Long, path: String, contextTokens: Int, workerThreads: Int): Boolean
    fun ownership(engineId: Long): Int
    fun generate(engineId: Long, prompt: String, maxOutputTokens: Int, generationId: Long): String
    fun cancel(engineId: Long, generationId: Long): Boolean
    fun forgetCancellation(engineId: Long, generationId: Long)
    fun usageCounts(engineId: Long, generationId: Long): IntArray?
    fun forgetUsage(engineId: Long, generationId: Long)
    fun free(engineId: Long): Boolean
}
