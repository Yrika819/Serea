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
/** Separates local-only processing from an external provider destination. */
enum class EgressTarget { CLOUD, LOCAL_DEMO, LOCAL_MODEL }

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
    /** Defaults to the existing external-cloud contract; local demo must be explicit. */
    val target: EgressTarget = EgressTarget.CLOUD,
    /** Inherited provenance for plannedBody. Empty or untrusted ancestry denies Cloud egress. */
    val plannedBodySourceLineage: Set<DataOrigin> = emptySet(),
    /** Local real-model use requires a fresh, content-bound local consent proof. */
    val localModelConsent: LocalModelConsentProof? = null,
)

enum class EgressContractDecision { APPROVED, MISSING_INTENT, EXPIRED_INTENT, DESTINATION_DENIED, ORIGIN_DENIED, SOURCE_LINEAGE_DENIED, CLASSIFICATION_DENIED, PREPARATION_DENIED, CONTENT_MISMATCH, LIMIT_EXCEEDED }

data class EgressEvaluation(val decision: EgressContractDecision, val preview: String, internal val payload: EgressApprovedPayload?)

/** Test-only legacy marker. It never grants permission to send PERSONAL data. */
class PreparedPersonalData internal constructor(val content: String)

object ChatEgressGate {
    const val LOCAL_DEMO_DESTINATION = "serea://local-demo"
    const val LOCAL_DEMO_MODEL_ID = "serea-demo/local-chat"
    const val LOCAL_MODEL_DESTINATION = "serea://local-model"

    fun decide(classification: DataClassification, preparedPersonal: PreparedPersonalData? = null): EgressDecision =
        when (classification) {
            DataClassification.PUBLIC -> EgressDecision.ALLOW
            DataClassification.PERSONAL -> EgressDecision.DENY_UNPREPARED
            DataClassification.PRIVATE, DataClassification.SECRET, DataClassification.CREDENTIAL -> EgressDecision.DENY_SENSITIVE
            DataClassification.UNKNOWN -> EgressDecision.DENY_UNCLASSIFIED
        }

    fun evaluate(contract: EgressContract, nowMillis: Long): EgressEvaluation {
        fun denied(reason: EgressContractDecision) = EgressEvaluation(reason, contract.plannedBody, null)
        if (contract.target == EgressTarget.LOCAL_DEMO) {
            if (contract.destination != LOCAL_DEMO_DESTINATION || contract.selectedModelId != LOCAL_DEMO_MODEL_ID) {
                return denied(EgressContractDecision.DESTINATION_DENIED)
            }
            if (contract.explicitSendIntentAtMillis != null || contract.expiresAtMillis != null) {
                return denied(EgressContractDecision.CONTENT_MISMATCH)
            }
            if (contract.origin != DataOrigin.USER_AUTHORED) return denied(EgressContractDecision.ORIGIN_DENIED)
            if (contract.preparation != PayloadPreparation.RAW) return denied(EgressContractDecision.PREPARATION_DENIED)
            val locallyAllowed = setOf(DataClassification.PUBLIC, DataClassification.PERSONAL, DataClassification.UNKNOWN)
            if (contract.classification !in locallyAllowed) return denied(EgressContractDecision.CLASSIFICATION_DENIED)
            if (contract.context.any { it.origin !in setOf(DataOrigin.USER_AUTHORED, DataOrigin.MODEL_OUTPUT) }) {
                return denied(EgressContractDecision.ORIGIN_DENIED)
            }
            if (contract.context.any { it.classification !in locallyAllowed }) {
                return denied(EgressContractDecision.CLASSIFICATION_DENIED)
            }
            if (contract.plannedBody.isBlank() || contract.context.any {
                    it.status != "COMPLETED" || it.sourceMessageId.isBlank() || it.conversationId != contract.conversationId
                }) {
                return denied(EgressContractDecision.CONTENT_MISMATCH)
            }
            val requestBytes = contract.plannedBody.toByteArray(Charsets.UTF_8).size.toLong() +
                contract.context.sumOf { it.text.toByteArray(Charsets.UTF_8).size.toLong() + it.role.name.length + 64L } + 64L
            if (contract.plannedBody.length > ChatContextBuilder.MAX_INPUT_CHARS ||
                contract.context.sumOf { it.text.length } > ChatContextBuilder.MAX_CONTEXT_CHARS || requestBytes > 16 * 1024L) {
                return denied(EgressContractDecision.LIMIT_EXCEEDED)
            }
            val messages = contract.context.map { it.role.name.lowercase() to it.text } + ("user" to contract.plannedBody)
            return EgressEvaluation(
                EgressContractDecision.APPROVED,
                contract.plannedBody,
                EgressApprovedPayload.fromApprovedContract(contract, messages),
            )
        }
        if (contract.target == EgressTarget.LOCAL_MODEL) {
            val consent = contract.localModelConsent
                ?: return denied(EgressContractDecision.MISSING_INTENT)
            if (contract.destination != LOCAL_MODEL_DESTINATION ||
                contract.selectedModelId.isNullOrBlank() || consent.selectedModelId != contract.selectedModelId ||
                consent.conversationId != contract.conversationId ||
                consent.sendId != contract.currentMessageId ||
                consent.payloadSha256 != localModelPayloadSha256(contract.plannedBody, contract.context) ||
                consent.sourceLineage != contract.plannedBodySourceLineage.toSet()) {
                return denied(EgressContractDecision.CONTENT_MISMATCH)
            }
            if (consent.approvedAtMillis > nowMillis || nowMillis > consent.expiresAtMillis ||
                consent.expiresAtMillis < consent.approvedAtMillis ||
                consent.expiresAtMillis - consent.approvedAtMillis > 60_000L) {
                return denied(EgressContractDecision.EXPIRED_INTENT)
            }
            val allowedOrigins = setOf(DataOrigin.USER_AUTHORED, DataOrigin.MODEL_OUTPUT, DataOrigin.GMAIL, DataOrigin.CALENDAR)
            if (contract.origin !in allowedOrigins || contract.plannedBodySourceLineage.isEmpty() ||
                contract.origin !in contract.plannedBodySourceLineage ||
                contract.plannedBodySourceLineage.any { it !in allowedOrigins }) {
                return denied(EgressContractDecision.SOURCE_LINEAGE_DENIED)
            }
            if (contract.origin in setOf(DataOrigin.GMAIL, DataOrigin.CALENDAR) &&
                (consent.providerHandoff == null || consent.providerHandoff.origin != contract.origin ||
                    consent.providerHandoff.contentIdentitySha256 !=
                        "sha256:${sha256Utf8(consent.providerHandoff.selectedContent)}" ||
                    consent.providerHandoff.approvedChatBodySha256 != sha256Utf8(contract.plannedBody))) {
                return denied(EgressContractDecision.SOURCE_LINEAGE_DENIED)
            }
            if (contract.preparation != PayloadPreparation.RAW || contract.plannedBody.isBlank() ||
                contract.context.any { it.status != "COMPLETED" || it.sourceMessageId.isBlank() ||
                    it.conversationId != contract.conversationId ||
                    it.sourceLineage.isEmpty() || it.origin !in it.sourceLineage ||
                    it.sourceLineage.any { origin -> origin !in setOf(DataOrigin.USER_AUTHORED, DataOrigin.MODEL_OUTPUT) } }) {
                return denied(EgressContractDecision.CONTENT_MISMATCH)
            }
            if (contract.classification in setOf(DataClassification.SECRET, DataClassification.CREDENTIAL) ||
                contract.context.any { it.classification in setOf(DataClassification.SECRET, DataClassification.CREDENTIAL) }) {
                return denied(EgressContractDecision.CLASSIFICATION_DENIED)
            }
            if (contract.plannedBody.length > ChatContextBuilder.MAX_INPUT_CHARS ||
                contract.context.sumOf { it.text.length } > ChatContextBuilder.MAX_CONTEXT_CHARS) {
                return denied(EgressContractDecision.LIMIT_EXCEEDED)
            }
            val messages = contract.context.map { it.role.name.lowercase() to it.text } + ("user" to contract.plannedBody)
            return EgressEvaluation(EgressContractDecision.APPROVED, contract.plannedBody,
                EgressApprovedPayload.fromApprovedContract(contract, messages))
        }
        if (contract.destination != "https://ollama.com/api/chat") return denied(EgressContractDecision.DESTINATION_DENIED)
        val intent = contract.explicitSendIntentAtMillis ?: return denied(EgressContractDecision.MISSING_INTENT)
        val expiry = contract.expiresAtMillis ?: return denied(EgressContractDecision.MISSING_INTENT)
        if (intent > nowMillis || nowMillis > expiry || expiry < intent || expiry - intent > 60_000) return denied(EgressContractDecision.EXPIRED_INTENT)
        if (contract.origin !in setOf(DataOrigin.USER_AUTHORED, DataOrigin.MODEL_OUTPUT)) return denied(EgressContractDecision.ORIGIN_DENIED)
        if (contract.preparation != PayloadPreparation.RAW) return denied(EgressContractDecision.PREPARATION_DENIED)
        val cloudLineageSources = setOf(DataOrigin.USER_AUTHORED, DataOrigin.MODEL_OUTPUT)
        val plannedBodyLineage = contract.plannedBodySourceLineage.toSet()
        if (plannedBodyLineage.isEmpty() ||
            contract.origin !in plannedBodyLineage ||
            DataOrigin.USER_AUTHORED !in plannedBodyLineage ||
            plannedBodyLineage.any { it !in cloudLineageSources }) {
            return denied(EgressContractDecision.SOURCE_LINEAGE_DENIED)
        }
        if (contract.context.any { item ->
                item.sourceLineage.isEmpty() ||
                    item.origin !in item.sourceLineage ||
                    DataOrigin.USER_AUTHORED !in item.sourceLineage ||
                    item.sourceLineage.any { it !in cloudLineageSources }
            }) {
            return denied(EgressContractDecision.SOURCE_LINEAGE_DENIED)
        }
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
internal class EgressCheckedModelChatPort(
    private val delegate: ModelChatPort,
    private val clock: () -> Long = System::currentTimeMillis,
    private val expectedTarget: EgressTarget? = null,
    private val localModelAuthorizations: LocalModelAuthorizationPort? = null,
) : ModelChatPort {
    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        if (request.message.length > ChatContextBuilder.MAX_INPUT_CHARS ||
            request.context.sumOf { it.text.length.toLong() } > ChatContextBuilder.MAX_CONTEXT_CHARS ||
            request.timeoutMillis !in 1..60_000) {
            throw ModelChatFailure("Request exceeds the approved local bounds.")
        }
        val contract = request.egressContract ?: throw ModelChatFailure("A typed Egress contract is required.")
        val localProviderOrigin = contract.target == EgressTarget.LOCAL_MODEL &&
            contract.origin in setOf(DataOrigin.GMAIL, DataOrigin.CALENDAR)
        if (contract.conversationId != request.conversationId || contract.currentMessageId != request.sendId ||
            contract.plannedBody != request.message || contract.context != request.context ||
            contract.plannedBodySourceLineage.toSet() != request.bodySourceLineage.toSet() ||
            contract.classification != request.classification ||
            (contract.origin != DataOrigin.USER_AUTHORED && !localProviderOrigin) ||
            (contract.selectedModelId != null && contract.selectedModelId != request.selectedModelId)) {
            throw ModelChatFailure("The approved content no longer matches the request.")
        }
        val boundContract = contract.copy(
            selectedModelId = request.selectedModelId,
            plannedBodySourceLineage = contract.plannedBodySourceLineage.toSet(),
        )
        if (expectedTarget != null && boundContract.target != expectedTarget) {
            throw ModelChatFailure("The request target does not match this provider composition.")
        }
        if (boundContract.target == EgressTarget.LOCAL_DEMO && request.preparedPersonal != null) {
            throw ModelChatFailure("The local demo accepts only the original typed message and context.")
        }
        val result = ChatEgressGate.evaluate(boundContract, clock())
        if (result.decision != EgressContractDecision.APPROVED) {
            throw ModelChatFailure("Egress was denied: ${result.decision.name.lowercase()}.")
        }
        if (boundContract.target == EgressTarget.CLOUD) {
            val decisions = sequenceOf(request.classification) + request.context.asSequence().map { it.classification }
            for (classification in decisions) {
                when (ChatEgressGate.decide(classification, request.preparedPersonal)) {
                    EgressDecision.ALLOW -> Unit
                    EgressDecision.DENY_UNCLASSIFIED -> throw ModelChatFailure("Unclassified data cannot be sent.")
                    EgressDecision.DENY_UNPREPARED -> throw ModelChatFailure("Personal data is not prepared for egress.")
                    EgressDecision.DENY_SENSITIVE -> throw ModelChatFailure("Sensitive data cannot be sent.")
                }
            }
        }
        if (boundContract.target == EgressTarget.LOCAL_MODEL) {
            val proof = boundContract.localModelConsent
                ?: throw PreDispatchModelChatFailure("Local model consent is missing.")
            val authorizer = localModelAuthorizations
                ?: throw PreDispatchModelChatFailure("Local model authorization is not composed.")
            if (!authorizer.consumeOnce(proof, boundContract)) {
                throw PreDispatchModelChatFailure("Local model consent is stale, mismatched, or already used.")
            }
        }
        return delegate.send(request)
    }
}
