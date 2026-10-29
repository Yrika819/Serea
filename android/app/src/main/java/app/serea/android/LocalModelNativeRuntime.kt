package app.serea.android

import java.io.File
import android.os.Debug
import java.util.concurrent.CountDownLatch
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.CancellableContinuation
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
internal class LlamaCppLocalModelInferenceBackend(
    /** Optional bounded diagnostic hook; values are PSS KiB and never contain model output. */
    private val diagnosticMetric: ((String, Long) -> Unit)? = null,
) : LocalModelInferenceBackend {
    private data class Generation(
        val id: Long,
        var mayRunInNative: Boolean = false,
        var cancelled: Boolean = false,
        var completed: Boolean = false,
    )

    companion object {
        private val nextEngineId = AtomicLong(0)

        init { System.loadLibrary("serea_local_llama") }
    }

    private val engineId = nextEngineId.incrementAndGet()
    private val lifecycleLock = Any()
    private val executor = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "serea-local-llama").apply { isDaemon = true }
    }
    private val activeGenerations = linkedSetOf<Generation>()
    private val releaseComplete = CountDownLatch(1)
    private var nextGenerationId = 1L
    private var diagnosticInferenceIndex = 0
    @Volatile private var closed = false
    @Volatile private var loaded = false

    override suspend fun load(modelPath: String, limits: LocalModelRuntimeLimits) {
        val model = File(modelPath)
        if (!model.isFile || !model.canRead() || model.length() > limits.maxModelFileBytes) {
            throw PreDispatchModelChatFailure("The model file exceeds the native runtime's file-size bound.")
        }
        // The reviewed PoC ABI supports 128–512 context tokens, two CPU threads, and at most
        // 256 output tokens. Reject unsupported configurations before native allocation.
        if (limits.contextTokens !in 128..512 || limits.workerThreads !in 1..2 || limits.maxOutputTokens !in 1..256) {
            throw PreDispatchModelChatFailure("The selected native runtime limits are unsupported.")
        }

        suspendCancellableCoroutine { continuation ->
            val queued = synchronized(lifecycleLock) {
                if (closed) false else try {
                    executor.execute {
                        if (!mayContinue(continuation)) {
                            continuation.failIfActive(LocalModelBackendStartDeniedException())
                            return@execute
                        }
                        try {
                            reportPss("load_before_kb")
                            val success = nativeLoad(
                                engineId,
                                model.canonicalPath,
                                limits.contextTokens,
                                limits.workerThreads,
                            )
                            reportPss("load_after_kb")
                            val publish = synchronized(lifecycleLock) {
                                if (success && !closed && continuation.isActive) loaded = true
                                success && !closed && continuation.isActive
                            }
                            if (!publish) {
                                nativeFree(engineId)
                                if (success) continuation.failIfActive(LocalModelBackendStartDeniedException())
                                else continuation.failIfActive(IllegalStateException("llama.cpp could not load the verified model"))
                            } else {
                                if (continuation.isActive) continuation.resume(Unit)
                            }
                        } catch (failure: Throwable) {
                            continuation.failIfActive(failure)
                        }
                    }
                    true
                } catch (_: java.util.concurrent.RejectedExecutionException) {
                    false
                }
            }
            if (!queued) continuation.failIfActive(IllegalStateException("The native model executor is closed"))
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
                            if (request.mayRunInNative && !request.completed) nativeCancel(engineId, request.id)
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
                        val output = nativeGenerate(engineId, prompt, maxOutputTokens, generation.id)
                        reportPss("inference_${measurementIndex}_after_kb")
                        val usage = nativeUsageCounts(engineId, generation.id)
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
                        nativeForgetCancellation(engineId, generation.id)
                        nativeForgetUsage(engineId, generation.id)
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
                if (generation.mayRunInNative && !generation.completed) nativeCancel(engineId, generation.id)
            }
        }
    }

    override fun close() {
        synchronized(lifecycleLock) {
            if (closed) return
            closed = true
            activeGenerations.forEach { generation ->
                generation.cancelled = true
                if (generation.mayRunInNative && !generation.completed) nativeCancel(engineId, generation.id)
            }
            try {
                executor.execute {
                    try {
                        nativeFree(engineId)
                    } finally {
                        loaded = false
                        executor.shutdown()
                        releaseComplete.countDown()
                    }
                }
                executor.shutdown()
            } catch (_: java.util.concurrent.RejectedExecutionException) {
                executor.shutdownNow()
                releaseComplete.countDown()
            }
        }
    }

    /** Wait for background JNI release in diagnostics without blocking product lifecycle callers. */
    internal fun awaitReleased(timeout: Long, unit: TimeUnit): Boolean = releaseComplete.await(timeout, unit)

    private fun mayContinue(continuation: CancellableContinuation<*>): Boolean =
        synchronized(lifecycleLock) { !closed && continuation.isActive }

    private fun <T> CancellableContinuation<T>.failIfActive(failure: Throwable) {
        if (isActive) resumeWithException(failure)
    }

    private fun reportPss(name: String) {
        val sink = diagnosticMetric ?: return
        runCatching { sink(name, Debug.getPss().toLong()) }
    }

    private external fun nativeLoad(engineId: Long, path: String, contextTokens: Int, workerThreads: Int): Boolean
    private external fun nativeGenerate(engineId: Long, prompt: String, maxOutputTokens: Int, generationId: Long): String
    private external fun nativeCancel(engineId: Long, generationId: Long): Boolean
    private external fun nativeForgetCancellation(engineId: Long, generationId: Long)
    private external fun nativeUsageCounts(engineId: Long, generationId: Long): IntArray?
    private external fun nativeForgetUsage(engineId: Long, generationId: Long)
    private external fun nativeFree(engineId: Long): Boolean
}
