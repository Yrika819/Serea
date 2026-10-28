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
    /** Thread-safe while generate is active; cancels that request without freeing model state. */
    fun cancelActive()
    /** Called by the adapter only after active generation has unwound under its lifecycle lock. */
    override fun close()
}

internal fun interface LocalModelInferenceBackendFactory {
    fun create(): LocalModelInferenceBackend
}

/** Limits passed to the native backend; its implementation must reject unsupported values. */
data class LocalModelRuntimeLimits(
    val maxResidentMemoryBytes: Long = 768L * 1024 * 1024,
    val contextTokens: Int = 2_048,
    val maxOutputTokens: Int = 256,
    val workerThreads: Int = 2,
) {
    init {
        require(maxResidentMemoryBytes in (64L * 1024 * 1024)..(2L * 1024 * 1024 * 1024))
        require(contextTokens in 128..32_768 && maxOutputTokens in 1..4_096 && workerThreads in 1..8)
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
    private val closed = AtomicBoolean(false)
    @Volatile private var backend: LocalModelInferenceBackend? = null
    private var loaded = false

    init {
        require(deviceAbi in artifact.supportedAbis) { "Local model does not include this device ABI." }
        require(maxPromptChars in 1..65_536)
    }

    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        check(!closed.get()) { "Local model is closed." }
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
        return mutex.withLock {
            var activeBackend: LocalModelInferenceBackend? = null
            try {
                withTimeout(request.timeoutMillis) {
                    check(!closed.get()) { "Local model is closed." }
                    if (!loaded) {
                        verifyModelFile()
                        activeBackend = backend ?: backendFactory.create().also { backend = it }
                        activeBackend!!.load(modelFile.canonicalPath, runtimeLimits)
                        loaded = true
                    }
                    activeBackend = backend ?: throw ModelChatFailure("Local model backend is unavailable.")
                    if (closed.get()) throw CancellationException("Local model closed before generation.")
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
                throw cancelled
            } catch (failure: ModelChatFailure) {
                if (failure !is PreDispatchModelChatFailure) releaseBackendLocked()
                throw failure
            } catch (failure: Exception) {
                releaseBackendLocked()
                throw ModelChatFailure("Local model inference failed: ${failure::class.simpleName ?: "native error"}.")
            } catch (failure: LinkageError) {
                releaseBackendLocked()
                throw ModelChatFailure("Local model runtime is unavailable for this device ABI.")
            } finally {
                if (closed.get()) releaseBackendLocked()
            }
        }
    }

    override fun close() {
        if (closed.compareAndSet(false, true)) {
            val runningBackend = backend
            runCatching { runningBackend?.cancelActive() }
            if (mutex.tryLock()) {
                try { releaseBackendLocked() } finally { mutex.unlock() }
            }
        }
    }

    private fun releaseBackendLocked() {
        val toRelease = backend
        backend = null
        loaded = false
        runCatching { toRelease?.close() }
    }

    private fun verifyModelFile() {
        if (!modelFile.isFile || !modelFile.canRead() || modelFile.length() != artifact.expectedSizeBytes) {
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
