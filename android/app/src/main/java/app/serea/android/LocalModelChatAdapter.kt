package app.serea.android

import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock
import kotlinx.coroutines.withTimeout
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean

/** Immutable model/runtime provenance required before a native backend can load weights. */
data class LocalModelArtifact(
    val modelId: String,
    val displayName: String,
    val modelRepository: String,
    val modelRevision: String,
    val licenseId: String,
    val expectedSha256: String,
    val expectedSizeBytes: Long,
    val runtimeName: String,
    val runtimeRevision: String,
    val supportedAbis: Set<String>,
) {
    init {
        require(modelId.matches(Regex("[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}")) && !modelId.contains("://"))
        require(displayName.isNotBlank() && displayName.length <= 128)
        require(modelRepository.startsWith("https://") && modelRevision.matches(Regex("[a-f0-9]{40,64}")))
        require(licenseId.matches(Regex("[A-Za-z0-9.+-]{2,40}")))
        require(expectedSha256.matches(Regex("[a-f0-9]{64}")) && expectedSizeBytes in 1..(2L * 1024 * 1024 * 1024))
        require(runtimeName.isNotBlank() && runtimeRevision.matches(Regex("[a-f0-9]{7,64}")))
        require(supportedAbis.isNotEmpty() && supportedAbis.all { it in setOf("arm64-v8a", "x86_64") })
    }
}

/** The native implementation must be network-free and serialize its own JNI lifecycle. */
internal interface LocalModelInferenceBackend : AutoCloseable {
    suspend fun load(modelPath: String, limits: LocalModelRuntimeLimits)
    suspend fun generate(prompt: String, maxOutputTokens: Int): LocalModelInferenceResult
    /**
     * Thread-safe and sticky across the adapter's closed/start race. If this runs before the
     * backend has entered native load/generate, the backend must refuse that start and throw
     * [LocalModelBackendStartDeniedException] without entering JNI. If work already started, it
     * cancels that operation without freeing model state.
     */
    fun cancelActive()
    /** Called after active load/generation has unwound under the adapter mutex. */
    override fun close()
}

/** Backend confirmation that cancellation won before a native operation began. */
internal class LocalModelBackendStartDeniedException : Exception("Local model operation cancelled before native start.")

internal fun interface LocalModelInferenceBackendFactory {
    fun create(): LocalModelInferenceBackend
}

/**
 * Limits supported by the currently wired llama.cpp JNI backend. Keep these bounds aligned
 * with that backend's pre-allocation validation; a future backend with different capabilities
 * must add an explicit capability contract before widening them. `maxModelFileBytes` bounds
 * the immutable model file only. Runtime RSS/PSS is measured separately in diagnostic runs
 * and is not a hard memory-allocation cap.
 */
data class LocalModelRuntimeLimits(
    val maxModelFileBytes: Long = 768L * 1024 * 1024,
    // Keep the default compatible with the currently implemented llama.cpp JNI profile.
    // Backends with a larger context must declare and validate their own capability first.
    val contextTokens: Int = 512,
    val maxOutputTokens: Int = 256,
    val workerThreads: Int = 2,
) {
    init {
        require(maxModelFileBytes in (64L * 1024 * 1024)..(2L * 1024 * 1024 * 1024))
        require(contextTokens in 128..512 && maxOutputTokens in 1..256 && workerThreads in 1..2)
    }
}

internal data class LocalModelInferenceResult(
    val text: String,
    val inputTokens: Int?,
    val outputTokens: Int?,
)

/** Records local-token usage under a real, explicit model ID; monetary cost stays unknown. */
internal class LocalModelUsageAttributionResolver(private val artifact: LocalModelArtifact) : UsageAttributionResolver {
    override fun resolve(request: ModelChatRequest): ModelUsageAttribution {
        if (request.selectedModelId != artifact.modelId) {
            throw PreDispatchModelChatFailure("The selected model does not match the local model artifact.")
        }
        return ModelUsageAttribution(modelId = artifact.modelId, providerId = "local-device", price = null)
    }
}

/**
 * Product `ModelChatPort` adapter for a verified on-device model. It has no credential, HTTP,
 * endpoint, or fallback dependency. Composition must place it behind Egress and Usage wrappers.
 */
internal class LocalModelChatAdapter(
    private val artifact: LocalModelArtifact,
    private val modelFile: File,
    private val backendFactory: LocalModelInferenceBackendFactory,
    private val deviceAbi: String,
    private val runtimeLimits: LocalModelRuntimeLimits = LocalModelRuntimeLimits(),
    private val maxPromptChars: Int = 16_384,
) : ModelChatPort, AutoCloseable {
    private val mutex = Mutex()
    private val lifecycleLock = Any()
    private val closed = AtomicBoolean(false)
    @Volatile private var backend: LocalModelInferenceBackend? = null
    private var loaded = false

    init {
        require(deviceAbi in artifact.supportedAbis) { "Local model does not include this device ABI." }
        require(maxPromptChars in 1..65_536)
    }

    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        if (closed.get()) throw PreDispatchModelChatFailure("Local model is closed.")
        val contract = request.egressContract
            ?: throw PreDispatchModelChatFailure("A typed local-model consent contract is required.")
        if (contract.target != EgressTarget.LOCAL_MODEL ||
            contract.destination != ChatEgressGate.LOCAL_MODEL_DESTINATION ||
            contract.selectedModelId != artifact.modelId || request.selectedModelId != artifact.modelId ||
            contract.conversationId != request.conversationId || contract.currentMessageId != request.sendId ||
            contract.plannedBody != request.message || contract.context != request.context ||
            contract.localModelConsent?.selectedModelId != artifact.modelId) {
            throw PreDispatchModelChatFailure("The request does not match this local model and consent.")
        }
        val prompt = buildPrompt(request)
        val promptCharLimit = minOf(maxPromptChars.toLong(), runtimeLimits.contextTokens.toLong() * 4L)
        if (prompt.length.toLong() > promptCharLimit || request.timeoutMillis !in 1..60_000) {
            throw PreDispatchModelChatFailure("The local model request exceeds its bounded limits.")
        }
        var generationStarted = false
        try {
            return mutex.withLock {
                var activeBackend: LocalModelInferenceBackend? = null
                try {
                    withTimeout(request.timeoutMillis) {
                        if (closed.get()) throw PreDispatchModelChatFailure("Local model is closed.")
                        if (!loaded) {
                            verifyModelFile()
                            activeBackend = backend
                            if (activeBackend == null) {
                                val created = backendFactory.create()
                                val published = synchronized(lifecycleLock) {
                                    if (closed.get()) false else {
                                        backend = created
                                        true
                                    }
                                }
                                if (!published) {
                                    runCatching { created.close() }
                                    throw PreDispatchModelChatFailure("Local model was closed before runtime load.")
                                }
                                activeBackend = created
                            }
                            val loadMayStart = synchronized(lifecycleLock) { !closed.get() }
                            if (!loadMayStart) throw PreDispatchModelChatFailure("Local model was closed before runtime load.")
                            activeBackend!!.load(modelFile.canonicalPath, runtimeLimits)
                            if (closed.get()) throw PreDispatchModelChatFailure("Local model was closed during runtime load.")
                            loaded = true
                        }
                        activeBackend = backend
                            ?: throw PreDispatchModelChatFailure("Local model backend is unavailable.")
                        val generationMayStart = synchronized(lifecycleLock) {
                            if (closed.get()) false else {
                                // This is the lifecycle linearization point: close() takes the same
                                // lock, so whichever wins determines whether native dispatch began.
                                generationStarted = true
                                true
                            }
                        }
                        if (!generationMayStart) throw PreDispatchModelChatFailure("Local model was closed before generation.")
                        val generated = activeBackend!!.generate(prompt, runtimeLimits.maxOutputTokens)
                        if (closed.get()) throw CancellationException("Local model closed during inference.")
                        if (generated.text.isBlank() || generated.text.length > runtimeLimits.maxOutputTokens * 64) {
                            throw ModelChatFailure("The local model returned an invalid response.")
                        }
                        if (listOfNotNull(generated.inputTokens, generated.outputTokens).any { it < 0 || it > 1_000_000 }) {
                            throw ModelChatFailure("The local model returned invalid usage data.")
                        }
                        ModelChatResponse(
                            text = generated.text,
                            usage = ProviderUsage(generated.inputTokens, generated.outputTokens),
                        )
                    }
                } catch (cancelled: CancellationException) {
                    runCatching { activeBackend?.cancelActive() }
                    releaseBackendLocked()
                    if (!generationStarted) {
                        throw PreDispatchModelChatCancellation("Local model work was cancelled before inference started.")
                    }
                    throw cancelled
                } catch (_: LocalModelBackendStartDeniedException) {
                    // The backend start gate confirms that close/cancellation won before JNI.
                    // Although the adapter granted the dispatch slot, no inference was sent.
                    releaseBackendLocked()
                    throw PreDispatchModelChatFailure("Local model operation was cancelled before native start.")
                } catch (failure: ModelChatFailure) {
                    releaseBackendLocked()
                    if (!generationStarted && failure !is PreDispatchModelChatFailure) {
                        throw PreDispatchModelChatFailure("Local model setup failed before inference started.")
                    }
                    throw failure
                } catch (failure: Exception) {
                    releaseBackendLocked()
                    if (!generationStarted) {
                        throw PreDispatchModelChatFailure("Local model setup failed before inference started.")
                    }
                    throw ModelChatFailure("Local model inference failed: ${failure::class.simpleName ?: "native error"}.")
                } catch (failure: LinkageError) {
                    releaseBackendLocked()
                    if (!generationStarted) {
                        throw PreDispatchModelChatFailure("Local model runtime is unavailable before inference.")
                    }
                    throw ModelChatFailure("Local model runtime is unavailable for this device ABI.")
                } finally {
                    if (closed.get()) releaseBackendLocked()
                }
            }
        } catch (cancelled: CancellationException) {
            // Cancellation while waiting for the adapter mutex happens before this request can
            // enter native inference, so UsageControlledModelChatPort may safely release it.
            if (!generationStarted) {
                throw PreDispatchModelChatCancellation("Local model work was cancelled before inference started.")
            }
            throw cancelled
        }
    }

    override fun close() {
        val runningBackend = synchronized(lifecycleLock) {
            if (!closed.compareAndSet(false, true)) return
            backend
        }
        runCatching { runningBackend?.cancelActive() }
        if (mutex.tryLock()) {
            try { releaseBackendLocked() } finally { mutex.unlock() }
        }
    }

    private fun releaseBackendLocked() {
        val toRelease = backend
        backend = null
        loaded = false
        runCatching { toRelease?.close() }
    }

    private fun verifyModelFile() {
        if (!modelFile.isFile || !modelFile.canRead() ||
            modelFile.length() > runtimeLimits.maxModelFileBytes ||
            modelFile.length() != artifact.expectedSizeBytes
        ) {
            throw PreDispatchModelChatFailure("The verified local model file is unavailable.")
        }
        val digest = MessageDigest.getInstance("SHA-256")
        modelFile.inputStream().buffered().use { input ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val read = input.read(buffer)
                if (read < 0) break
                digest.update(buffer, 0, read)
            }
        }
        val actual = digest.digest().joinToString("") { "%02x".format(it) }
        if (actual != artifact.expectedSha256) {
            throw PreDispatchModelChatFailure("The local model file hash does not match its recorded identity.")
        }
    }

    private fun buildPrompt(request: ModelChatRequest): String = buildString {
        request.context.forEach { turn ->
            if (turn.role !in setOf(MessageRole.USER, MessageRole.ASSISTANT) ||
                turn.conversationId != request.conversationId || turn.status != "COMPLETED") {
                throw PreDispatchModelChatFailure("The local model context is not bound to this conversation.")
            }
            append(if (turn.role == MessageRole.USER) "user" else "assistant")
                .append(": ").append(turn.text).append('\n')
        }
        append("user: ").append(request.message).append("\nassistant:")
    }
}
