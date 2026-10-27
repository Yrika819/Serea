package app.serea.g3poc

import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.suspendCancellableCoroutine
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.Executors
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/** Structural test contract matching Serea's suspend send(request): response seam.
 * This isolated app does not import or replace the production ChatViewModel contract.
 */
data class ModelChatRequest(
    val conversationId: String,
    val message: String,
    val context: List<ContextMessage> = emptyList(),
    val maxOutputTokens: Int = 32,
)

data class ContextMessage(val role: String, val text: String)
data class ModelChatResponse(val text: String)

fun interface ModelChatPort {
    suspend fun send(request: ModelChatRequest): ModelChatResponse
}

internal interface LocalInferenceEngine {
    suspend fun load(path: String)
    suspend fun generate(prompt: String, maxOutputTokens: Int): String
    fun close()
}

/** Test-only adapter. Model acquisition and hash verification stay outside this app. */
internal class GgufModelChatPort internal constructor(
    modelPath: String,
    expectedSha256: String,
    private val engine: LocalInferenceEngine = NativeLlamaEngine(),
) : ModelChatPort, AutoCloseable {
    private val model = File(modelPath)
    private val expectedHash = expectedSha256.lowercase()
    @Volatile private var closed = false

    suspend fun load() {
        check(!closed) { "Model port is closed" }
        require(model.isFile && model.canRead()) { "Model file is unavailable" }
        require(expectedHash.matches(Regex("[0-9a-f]{64}"))) { "Expected a SHA-256 hex digest" }
        require(sha256(model) == expectedHash) { "Model SHA-256 mismatch" }
        engine.load(model.absolutePath)
    }

    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        check(!closed) { "Model port is closed" }
        currentCoroutineContext().ensureActive()
        require(request.message.isNotBlank()) { "Message must not be blank" }
        require(request.maxOutputTokens in 1..256) { "Output token limit is outside the PoC bound" }
        val prompt = buildString {
            request.context.forEach { turn ->
                require(turn.role == "user" || turn.role == "assistant") { "Unsupported context role" }
                append(turn.role).append(": ").append(turn.text).append('\n')
            }
            append("user: ").append(request.message).append("\nassistant:")
        }
        return ModelChatResponse(engine.generate(prompt, request.maxOutputTokens))
    }

    override fun close() {
        if (!closed) {
            closed = true
            engine.close()
        }
    }

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().buffered().use { input ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                digest.update(buffer, 0, count)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }
}

/** JNI implementation isolated to this standalone test APK. */
internal class NativeLlamaEngine : LocalInferenceEngine, AutoCloseable {
    companion object {
        private val nextEngineId = java.util.concurrent.atomic.AtomicLong(0)
    }

    private val engineId = nextEngineId.incrementAndGet()
    private val executor = Executors.newSingleThreadExecutor { runnable ->
        Thread(runnable, "g3-llama-inference").apply { isDaemon = true }
    }
    private val lifecycleLock = Any()
    private val closeComplete = java.util.concurrent.CountDownLatch(1)
    private val activeRequests = mutableSetOf<GenerationRequest>()
    @Volatile private var loaded = false
    @Volatile private var closed = false
    private class GenerationRequest(
        val id: Long,
        var nativeMayRun: Boolean = false,
        var cancelled: Boolean = false,
        var completed: Boolean = false,
    )
    private var nextGenerationId = 1L
    private val nativeGenerationStarts = java.util.concurrent.atomic.AtomicInteger(0)
    @Volatile private var loadQueued: java.util.concurrent.CountDownLatch? = null
    @Volatile private var loadSkipped: java.util.concurrent.CountDownLatch? = null
    @Volatile private var closeStarted: java.util.concurrent.CountDownLatch? = null
    @Volatile private var nativeFreeResultForTest: Boolean? = null
    @Volatile private var generationStarted: java.util.concurrent.CountDownLatch? = null
    @Volatile private var generationGate: java.util.concurrent.CountDownLatch? = null
    @Volatile private var generationFinished: java.util.concurrent.CountDownLatch? = null
    @Volatile private var executorBlockStarted: java.util.concurrent.CountDownLatch? = null
    @Volatile private var executorBlockRelease: java.util.concurrent.CountDownLatch? = null
    @Volatile private var generationQueued: java.util.concurrent.CountDownLatch? = null
    @Volatile private var queuedGenerationSkipped: java.util.concurrent.CountDownLatch? = null

    init { System.loadLibrary("serea-g3-llama") }

    override suspend fun load(path: String) =
        suspendCancellableCoroutine { continuation ->
            val accepted = synchronized(lifecycleLock) {
                if (closed) {
                    false
                } else {
                    try {
                        executor.execute {
                            val shouldLoad = synchronized(lifecycleLock) {
                                !closed && continuation.isActive
                            }
                            if (!shouldLoad) {
                                loadSkipped?.countDown()
                                if (continuation.isActive) {
                                    continuation.resumeWithException(IllegalStateException("Engine closed before model load"))
                                }
                                return@execute
                            }
                            try {
                                check(nativeLoad(engineId, path)) {
                                    "llama.cpp rejected the model or could not create a context"
                                }
                                val mayPublish = synchronized(lifecycleLock) {
                                    if (!closed && continuation.isActive) loaded = true
                                    !closed && continuation.isActive
                                }
                                if (!mayPublish) {
                                    nativeFree(engineId)
                                    if (continuation.isActive) {
                                        continuation.resumeWithException(IllegalStateException("Engine closed during model load"))
                                    }
                                } else {
                                    continuation.resume(Unit)
                                }
                            } catch (failure: Throwable) {
                                if (continuation.isActive) continuation.resumeWithException(failure)
                            }
                        }
                        loadQueued?.countDown()
                        true
                    } catch (_: java.util.concurrent.RejectedExecutionException) {
                        false
                    }
                }
            }
            if (!accepted && continuation.isActive) {
                continuation.resumeWithException(IllegalStateException("Model engine is closed or unavailable"))
            }
        }

    override suspend fun generate(prompt: String, maxOutputTokens: Int): String =
        suspendCancellableCoroutine { continuation ->
            val request = synchronized(lifecycleLock) {
                if (closed || !loaded) null else GenerationRequest(nextGenerationId++).also { queuedRequest ->
                    activeRequests += queuedRequest
                    continuation.invokeOnCancellation {
                        synchronized(lifecycleLock) {
                            queuedRequest.cancelled = true
                            if (queuedRequest.nativeMayRun && !queuedRequest.completed) {
                                nativeCancel(engineId, queuedRequest.id)
                            }
                        }
                    }
                }
            }
            if (request == null) {
                continuation.resumeWithException(IllegalStateException("Model engine is not available"))
                return@suspendCancellableCoroutine
            }
            try {
                executor.execute {
                    val shouldStartNative = synchronized(lifecycleLock) {
                        if (request.cancelled || !continuation.isActive || closed) {
                            request.completed = true
                            activeRequests.remove(request)
                            false
                        } else {
                            // Once set, cancellation can mark this specific request in JNI even if
                            // it arrives just before nativeGenerate enters.
                            request.nativeMayRun = true
                            true
                        }
                    }
                    if (!shouldStartNative) {
                        queuedGenerationSkipped?.countDown()
                        return@execute
                    }
                    nativeGenerationStarts.incrementAndGet()
                    try {
                        val output = nativeGenerate(engineId, prompt, maxOutputTokens, request.id)
                        if (continuation.isActive) continuation.resume(output)
                    } catch (failure: Throwable) {
                        if (continuation.isActive) continuation.resumeWithException(failure)
                    } finally {
                        synchronized(lifecycleLock) {
                            request.completed = true
                            activeRequests.remove(request)
                        }
                        nativeForgetCancellation(engineId, request.id)
                        generationFinished?.countDown()
                    }
                }
                generationQueued?.countDown()
            } catch (failure: java.util.concurrent.RejectedExecutionException) {
                synchronized(lifecycleLock) {
                    request.completed = true
                    activeRequests.remove(request)
                }
                if (continuation.isActive) continuation.resumeWithException(failure)
            }
        }

    internal fun armStartGateForTest() {
        generationStarted = java.util.concurrent.CountDownLatch(1)
        generationGate = java.util.concurrent.CountDownLatch(1)
        generationFinished = java.util.concurrent.CountDownLatch(1)
    }

    internal fun awaitStartGateForTest(timeout: Long, unit: java.util.concurrent.TimeUnit): Boolean =
        generationStarted?.await(timeout, unit) ?: false

    internal fun releaseStartGateForTest() {
        generationGate?.countDown()
    }

    internal fun awaitFinishedForTest(timeout: Long, unit: java.util.concurrent.TimeUnit): Boolean =
        generationFinished?.await(timeout, unit) ?: false

    internal fun nativeGenerationStartsForTest(): Int = nativeGenerationStarts.get()

    internal fun activeGenerationIdForTest(): Long? = synchronized(lifecycleLock) {
        activeRequests.firstOrNull { it.nativeMayRun && !it.completed }?.id
    }

    internal fun hasNativeModelForTest(): Boolean = nativeHasModel(engineId)

    internal fun attemptForeignCancellationForTest(generationId: Long): Boolean =
        nativeCancel(engineId, generationId)

    internal fun attemptNativeFreeForTest(): Boolean = nativeFree(engineId)

    internal fun armLoadCloseRaceTest() {
        executorBlockStarted = java.util.concurrent.CountDownLatch(1)
        executorBlockRelease = java.util.concurrent.CountDownLatch(1)
        loadQueued = java.util.concurrent.CountDownLatch(1)
        loadSkipped = java.util.concurrent.CountDownLatch(1)
        closeStarted = java.util.concurrent.CountDownLatch(1)
    }

    internal fun awaitLoadQueuedForTest(timeout: Long, unit: java.util.concurrent.TimeUnit): Boolean =
        loadQueued?.await(timeout, unit) ?: false

    internal fun awaitLoadSkippedForTest(timeout: Long, unit: java.util.concurrent.TimeUnit): Boolean =
        loadSkipped?.await(timeout, unit) ?: false

    internal fun awaitCloseStartedForTest(timeout: Long, unit: java.util.concurrent.TimeUnit): Boolean =
        closeStarted?.await(timeout, unit) ?: false

    internal fun nativeFreeResultForTest(): Boolean? = nativeFreeResultForTest

    internal fun armQueuedCancellationTest() {
        executorBlockStarted = java.util.concurrent.CountDownLatch(1)
        executorBlockRelease = java.util.concurrent.CountDownLatch(1)
        generationQueued = java.util.concurrent.CountDownLatch(1)
        queuedGenerationSkipped = java.util.concurrent.CountDownLatch(1)
    }

    internal fun blockExecutorForTest() {
        val started = checkNotNull(executorBlockStarted)
        val release = checkNotNull(executorBlockRelease)
        executor.execute {
            started.countDown()
            release.await()
        }
    }

    internal fun awaitExecutorBlockedForTest(timeout: Long, unit: java.util.concurrent.TimeUnit): Boolean =
        executorBlockStarted?.await(timeout, unit) ?: false

    internal fun awaitGenerationQueuedForTest(timeout: Long, unit: java.util.concurrent.TimeUnit): Boolean =
        generationQueued?.await(timeout, unit) ?: false

    internal fun releaseExecutorBlockForTest() {
        executorBlockRelease?.countDown()
    }

    internal fun awaitQueuedGenerationSkippedForTest(timeout: Long, unit: java.util.concurrent.TimeUnit): Boolean =
        queuedGenerationSkipped?.await(timeout, unit) ?: false

    @JvmName("onNativeGenerationStart")
    fun onNativeGenerationStart() {
        val started = generationStarted ?: return
        started.countDown()
        generationGate?.await(30, java.util.concurrent.TimeUnit.SECONDS)
    }

    override fun close() {
        synchronized(lifecycleLock) {
            if (!closed) {
                closed = true
                closeStarted?.countDown()
                activeRequests.forEach { request ->
                    request.cancelled = true
                    if (request.nativeMayRun && !request.completed) nativeCancel(engineId, request.id)
                }
                try {
                    executor.execute {
                        try {
                            nativeFreeResultForTest = nativeFree(engineId)
                        } finally {
                            loaded = false
                            closeComplete.countDown()
                        }
                    }
                    executor.shutdown()
                } catch (_: java.util.concurrent.RejectedExecutionException) {
                    nativeFreeResultForTest = nativeFree(engineId)
                    loaded = false
                    closeComplete.countDown()
                }
            }
        }
        val completed = try {
            closeComplete.await(120, java.util.concurrent.TimeUnit.SECONDS)
        } catch (interrupted: InterruptedException) {
            Thread.currentThread().interrupt()
            throw IllegalStateException("Interrupted while releasing the native model lease", interrupted)
        }
        check(completed) { "Timed out while releasing the native model lease" }
    }

    private external fun nativeLoad(engineId: Long, path: String): Boolean
    private external fun nativeGenerate(engineId: Long, prompt: String, maxOutputTokens: Int, generationId: Long): String
    private external fun nativeCancel(engineId: Long, generationId: Long): Boolean
    private external fun nativeForgetCancellation(engineId: Long, generationId: Long)
    private external fun nativeFree(engineId: Long): Boolean
    private external fun nativeHasModel(engineId: Long): Boolean
}
