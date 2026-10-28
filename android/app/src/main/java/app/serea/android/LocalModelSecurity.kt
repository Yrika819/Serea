package app.serea.android

import java.nio.ByteBuffer
import java.security.MessageDigest
import java.util.UUID
import java.util.concurrent.ConcurrentHashMap
import kotlinx.coroutines.CancellationException

/** Read-only bridge to Agent A's TrustedGoogleHandoff; the evidence must come from its coordinator. */
internal interface LocalModelProviderHandoffEvidence {
    val handoffId: String
    val origin: DataOrigin
    val serviceType: String
    val syntheticAccountIdentifier: String
    val sessionIdentifier: String
    val sessionGeneration: Long
    val scope: String
    val sourceItemIdentifier: String
    val sourceRevision: String
    val contentIdentitySha256: String
    val selectedContent: String
    /** SHA-256 of the exact reviewed `TrustedGoogleHandoff.localChatText()` rendering. */
    val approvedChatBodySha256: String
    /** Monotonic milliseconds, shared with `SystemClock.elapsedRealtime()`, never epoch time. */
    val expiresAtMillis: Long
    val authorizationState: String
    val selectionState: String
    val consentState: String
    val destination: String
    val destinationConversationId: String
    val destinationConversationGeneration: Long
}

/**
 * Short-lived consent to process one exact request using one exact local model. Its timestamps
 * use monotonic elapsed milliseconds. It has no constructor exposed to product callers; the
 * registry issues and consumes it once.
 */
class LocalModelConsentProof private constructor(
    internal val id: String,
    internal val conversationId: String,
    internal val sendId: String,
    internal val selectedModelId: String,
    internal val conversationGeneration: Long,
    internal val payloadSha256: String,
    internal val sourceLineage: Set<DataOrigin>,
    internal val approvedAtMillis: Long,
    internal val expiresAtMillis: Long,
    internal val providerHandoff: LocalModelProviderHandoffEvidence?,
) {
    internal companion object {
        fun issue(
            id: String,
            request: ModelChatRequest,
            selectedModelId: String,
            conversationGeneration: Long,
            approvedAtMillis: Long,
            expiresAtMillis: Long,
            providerHandoff: LocalModelProviderHandoffEvidence?,
        ) = LocalModelConsentProof(
            id = id,
            conversationId = request.conversationId,
            sendId = request.sendId,
            selectedModelId = selectedModelId,
            conversationGeneration = conversationGeneration,
            payloadSha256 = localModelPayloadSha256(request.message, request.context),
            sourceLineage = request.bodySourceLineage.toSet(),
            approvedAtMillis = approvedAtMillis,
            expiresAtMillis = expiresAtMillis,
            providerHandoff = providerHandoff,
        )
    }
}

/** Egress stage consumes consent before usage reservation or provider dispatch. */
internal interface LocalModelAuthorizationPort {
    suspend fun consumeOnce(proof: LocalModelConsentProof, contract: EgressContract): Boolean
}

internal fun interface LocalModelProviderHandoffValidator {
    suspend fun isCurrent(
        evidence: LocalModelProviderHandoffEvidence,
        currentConversation: LocalModelConversationBinding,
    ): Boolean
}

internal data class LocalModelConversationBinding(val conversationId: String, val generation: Long) {
    init { require(conversationId.isNotBlank() && generation >= 0) }
}

internal fun interface LocalModelCurrentConversationPort {
    suspend fun currentBinding(): LocalModelConversationBinding?
}

/**
 * Process-local one-use registry. Process death intentionally invalidates all outstanding
 * approvals. Composition must call [approve] only in response to the explicit local consent UI.
 */
internal class LocalModelAuthorizationRegistry(
    private val clock: () -> Long = { android.os.SystemClock.elapsedRealtime() },
    private val maxLifetimeMillis: Long = 60_000L,
    private val handoffValidator: LocalModelProviderHandoffValidator = LocalModelProviderHandoffValidator { _, _ -> false },
    private val currentConversation: LocalModelCurrentConversationPort? = null,
) : LocalModelAuthorizationPort {
    private val pending = ConcurrentHashMap<String, LocalModelConsentProof>()

    init { require(maxLifetimeMillis in 1..60_000L) }

    fun approve(
        request: ModelChatRequest,
        selectedModelId: String,
        providerHandoff: LocalModelProviderHandoffEvidence? = null,
        conversationGeneration: Long = 0,
    ): LocalModelConsentProof {
        require(selectedModelId.matches(Regex("[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}")) && !selectedModelId.contains("://"))
        require(conversationGeneration >= 0)
        val lineage = request.bodySourceLineage
        require(lineage.isNotEmpty() && DataOrigin.UNKNOWN !in lineage)
        val providerOrigins = lineage.intersect(setOf(DataOrigin.GMAIL, DataOrigin.CALENDAR))
        if (providerOrigins.isNotEmpty()) {
            require(providerOrigins.size == 1 && providerHandoff != null)
            require(providerHandoff.origin in providerOrigins)
            require(providerHandoff.contentIdentitySha256 == "sha256:${sha256Utf8(providerHandoff.selectedContent)}")
            require(providerHandoff.approvedChatBodySha256 == sha256Utf8(request.message))
            require(providerHandoff.expiresAtMillis > clock())
            require(providerHandoff.serviceType == providerHandoff.origin.name)
            require(providerHandoff.authorizationState == "MOCK_ONLY")
            require(providerHandoff.selectionState == "SELECTED" && providerHandoff.consentState == "LOCAL_PROCESSING_ONLY")
            require(providerHandoff.destination == "LOCAL_CHAT")
            require(providerHandoff.destinationConversationId == request.conversationId &&
                providerHandoff.destinationConversationGeneration == conversationGeneration)
            require(providerHandoff.syntheticAccountIdentifier.isNotBlank() && providerHandoff.sessionIdentifier.isNotBlank() &&
                providerHandoff.sessionGeneration > 0 && providerHandoff.scope.isNotBlank() &&
                providerHandoff.sourceItemIdentifier.isNotBlank() && providerHandoff.sourceRevision.isNotBlank())
        } else {
            require(providerHandoff == null)
        }
        val now = clock()
        pending.entries.removeAll { it.value.expiresAtMillis < now }
        check(pending.size < MAX_PENDING_CONSENTS) { "Too many pending local-model consent proofs." }
        val proof = LocalModelConsentProof.issue(
            id = UUID.randomUUID().toString(), request = request, selectedModelId = selectedModelId,
            conversationGeneration = conversationGeneration,
            approvedAtMillis = now, expiresAtMillis = Math.addExact(now, maxLifetimeMillis),
            providerHandoff = providerHandoff,
        )
        pending[proof.id] = proof
        return proof
    }

    /** Remove an approval when the user cancels before Chat dispatch. */
    fun revoke(proof: LocalModelConsentProof): Boolean = pending.remove(proof.id, proof)

    override suspend fun consumeOnce(proof: LocalModelConsentProof, contract: EgressContract): Boolean {
        val issued = pending.remove(proof.id) ?: return false
        val now = clock()
        val currentBinding = try {
            currentConversation?.currentBinding() ?: return false
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: Exception) {
            return false
        }
        if (currentBinding.conversationId != contract.conversationId ||
            currentBinding.conversationId != proof.conversationId ||
            currentBinding.generation != proof.conversationGeneration) return false
        val handoff = proof.providerHandoff
        if (handoff != null && (currentBinding.conversationId != handoff.destinationConversationId ||
                currentBinding.generation != handoff.destinationConversationGeneration)) return false
        val currentHandoff = try {
            proof.providerHandoff?.let { evidence ->
                handoffValidator.isCurrent(evidence, currentBinding)
            } ?: true
        } catch (cancelled: CancellationException) {
            throw cancelled // proof was already removed, so cancellation cannot leave it reusable
        } catch (_: Exception) {
            false
        }
        return issued === proof && currentHandoff && now in proof.approvedAtMillis..proof.expiresAtMillis &&
            proof.conversationId == contract.conversationId && proof.sendId == contract.currentMessageId &&
            proof.selectedModelId == contract.selectedModelId &&
            proof.payloadSha256 == localModelPayloadSha256(contract.plannedBody, contract.context) &&
            proof.sourceLineage == contract.plannedBodySourceLineage &&
            (proof.providerHandoff == null || proof.providerHandoff.expiresAtMillis >= now)
    }

    private companion object { const val MAX_PENDING_CONSENTS = 16 }
}

internal fun localModelPayloadSha256(body: String, context: List<ContextMessage>): String {
    val digest = MessageDigest.getInstance("SHA-256")
    fun field(value: String) {
        val bytes = value.toByteArray(Charsets.UTF_8)
        digest.update(ByteBuffer.allocate(Int.SIZE_BYTES).putInt(bytes.size).array())
        digest.update(bytes)
    }
    field("serea-local-model-payload-v1")
    field(body)
    field(context.size.toString())
    context.forEach { message ->
        field(message.role.name)
        field(message.text)
        field(message.classification.name)
        field(message.sourceMessageId)
        field(message.origin.name)
        field(message.status)
        field(message.conversationId.orEmpty())
        message.sourceLineage.map { it.name }.sorted().forEach(::field)
        field(";")
    }
    return digest.digest().joinToString("") { "%02x".format(it) }
}

internal fun sha256Utf8(value: String): String = MessageDigest.getInstance("SHA-256")
    .digest(value.toByteArray(Charsets.UTF_8)).joinToString("") { "%02x".format(it) }
