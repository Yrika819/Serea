package app.serea.android

/** Opaque reference only. Implementations must never put secret bytes in this handle. */
@JvmInline
value class CredentialHandle internal constructor(val value: String)

/** A future platform adapter stores/removes credentials; no implementation ships in Phase 1. */
interface CredentialPort {
    suspend fun store(secret: CharArray): CredentialHandle
    suspend fun delete(handle: CredentialHandle)
}

enum class EgressDecision { ALLOW, DENY_UNCLASSIFIED, DENY_SENSITIVE, DENY_UNPREPARED }

enum class PayloadPreparation { RAW, TRUSTED_MINIMIZED, UNKNOWN }

/** Exact provider payload preview and its lineage. A user-authored string is not PUBLIC by default. */
data class EgressContract(
    val conversationId: String,
    val currentMessageId: String,
    val origin: DataOrigin,
    val classification: DataClassification,
    val destination: String,
    val plannedBody: String,
    val context: List<ContextMessage>,
    val preparation: PayloadPreparation,
    val explicitSendIntentAtMillis: Long?,
    val expiresAtMillis: Long?,
    /** Set only when the sender's approved intent names the routed catalog model. */
    val selectedModelId: String? = null,
)

enum class EgressContractDecision { APPROVED, MISSING_INTENT, EXPIRED_INTENT, DESTINATION_DENIED, ORIGIN_DENIED, CLASSIFICATION_DENIED, PREPARATION_DENIED, CONTENT_MISMATCH, LIMIT_EXCEEDED }

data class EgressEvaluation(val decision: EgressContractDecision, val preview: String, internal val payload: EgressApprovedPayload?)

/** Test-only legacy marker. It never grants permission to send PERSONAL data. */
class PreparedPersonalData internal constructor(val content: String)

object ChatEgressGate {
    fun decide(classification: DataClassification, preparedPersonal: PreparedPersonalData? = null): EgressDecision =
        when (classification) {
            DataClassification.PUBLIC -> EgressDecision.ALLOW
            DataClassification.PERSONAL -> EgressDecision.DENY_UNPREPARED
            DataClassification.PRIVATE, DataClassification.SECRET, DataClassification.CREDENTIAL -> EgressDecision.DENY_SENSITIVE
            DataClassification.UNKNOWN -> EgressDecision.DENY_UNCLASSIFIED
        }

    fun evaluate(contract: EgressContract, nowMillis: Long): EgressEvaluation {
        fun denied(reason: EgressContractDecision) = EgressEvaluation(reason, contract.plannedBody, null)
        if (contract.destination != "https://ollama.com/api/chat") return denied(EgressContractDecision.DESTINATION_DENIED)
        val intent = contract.explicitSendIntentAtMillis ?: return denied(EgressContractDecision.MISSING_INTENT)
        val expiry = contract.expiresAtMillis ?: return denied(EgressContractDecision.MISSING_INTENT)
        if (intent > nowMillis || nowMillis > expiry || expiry < intent || expiry - intent > 60_000) return denied(EgressContractDecision.EXPIRED_INTENT)
        if (contract.origin !in setOf(DataOrigin.USER_AUTHORED, DataOrigin.MODEL_OUTPUT)) return denied(EgressContractDecision.ORIGIN_DENIED)
        if (contract.preparation != PayloadPreparation.RAW) return denied(EgressContractDecision.PREPARATION_DENIED)
        if (decide(contract.classification) != EgressDecision.ALLOW ||
            contract.context.any { decide(it.classification) != EgressDecision.ALLOW || it.origin !in setOf(DataOrigin.USER_AUTHORED, DataOrigin.MODEL_OUTPUT) }) {
            return denied(EgressContractDecision.CLASSIFICATION_DENIED)
        }
        if (contract.plannedBody.isBlank() || contract.context.any {
                it.status != "COMPLETED" || it.sourceMessageId.isBlank() || it.conversationId != contract.conversationId
            }) {
            return denied(EgressContractDecision.CONTENT_MISMATCH)
        }
        val requestBytes = contract.plannedBody.toByteArray(Charsets.UTF_8).size.toLong() +
            contract.context.sumOf { it.text.toByteArray(Charsets.UTF_8).size.toLong() + it.role.name.length + 64L } +
            64L
        if (contract.plannedBody.length > ChatContextBuilder.MAX_INPUT_CHARS ||
            contract.context.sumOf { it.text.length } > ChatContextBuilder.MAX_CONTEXT_CHARS || requestBytes > 16 * 1024L) {
            return denied(EgressContractDecision.LIMIT_EXCEEDED)
        }
        val messages = contract.context.map { it.role.name.lowercase() to it.text } + ("user" to contract.plannedBody)
        return EgressEvaluation(EgressContractDecision.APPROVED, contract.plannedBody,
            EgressApprovedPayload.fromApprovedContract(contract, messages))
    }
}

/** Mandatory wrapper for any future cloud adapter. Rejected requests never reach its delegate. */
internal class EgressCheckedModelChatPort(private val delegate: ModelChatPort, private val clock: () -> Long = System::currentTimeMillis) : ModelChatPort {
    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        if (request.message.length > ChatContextBuilder.MAX_INPUT_CHARS ||
            request.context.sumOf { it.text.length.toLong() } > ChatContextBuilder.MAX_CONTEXT_CHARS ||
            request.timeoutMillis !in 1..60_000) {
            throw ModelChatFailure("Request exceeds the approved local bounds.")
        }
        val contract = request.egressContract ?: throw ModelChatFailure("Cloud send intent has not been approved.")
        if (contract.conversationId != request.conversationId || contract.currentMessageId != request.sendId ||
            contract.plannedBody != request.message || contract.context != request.context ||
            (contract.selectedModelId != null && contract.selectedModelId != request.selectedModelId)) {
            throw ModelChatFailure("The approved content no longer matches the request.")
        }
        val boundContract = contract.copy(selectedModelId = request.selectedModelId)
        val result = ChatEgressGate.evaluate(boundContract, clock())
        if (result.decision != EgressContractDecision.APPROVED) {
            throw ModelChatFailure("Cloud egress was denied: ${result.decision.name.lowercase()}.")
        }
        val decisions = sequenceOf(request.classification) + request.context.asSequence().map { it.classification }
        for (classification in decisions) {
            when (ChatEgressGate.decide(classification, request.preparedPersonal)) {
                EgressDecision.ALLOW -> Unit
                EgressDecision.DENY_UNCLASSIFIED -> throw ModelChatFailure("Unclassified data cannot be sent.")
                EgressDecision.DENY_UNPREPARED -> throw ModelChatFailure("Personal data is not prepared for egress.")
                EgressDecision.DENY_SENSITIVE -> throw ModelChatFailure("Sensitive data cannot be sent.")
            }
        }
        return delegate.send(request)
    }
}
