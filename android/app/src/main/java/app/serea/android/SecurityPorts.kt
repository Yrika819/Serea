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

/** Internal marker produced only by a future trusted minimization/redaction pipeline. */
class PreparedPersonalData internal constructor(val content: String)

object ChatEgressGate {
    fun decide(classification: DataClassification, preparedPersonal: PreparedPersonalData? = null): EgressDecision =
        when (classification) {
            DataClassification.PUBLIC -> EgressDecision.ALLOW
            DataClassification.PERSONAL -> if (preparedPersonal == null) EgressDecision.DENY_UNPREPARED else EgressDecision.ALLOW
            DataClassification.PRIVATE, DataClassification.SECRET, DataClassification.CREDENTIAL -> EgressDecision.DENY_SENSITIVE
            DataClassification.UNKNOWN -> EgressDecision.DENY_UNCLASSIFIED
        }
}

/** Mandatory wrapper for any future cloud adapter. Rejected requests never reach its delegate. */
class EgressCheckedModelChatPort(private val delegate: ModelChatPort) : ModelChatPort {
    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        when (ChatEgressGate.decide(request.classification, request.preparedPersonal)) {
            EgressDecision.ALLOW -> {
                val safeRequest = if (request.classification == DataClassification.PERSONAL) {
                    request.copy(message = requireNotNull(request.preparedPersonal).content)
                } else request
                return delegate.send(safeRequest)
            }
            EgressDecision.DENY_UNCLASSIFIED -> throw ModelChatFailure("Unclassified data cannot be sent.")
            EgressDecision.DENY_UNPREPARED -> throw ModelChatFailure("Personal data is not prepared for egress.")
            EgressDecision.DENY_SENSITIVE -> throw ModelChatFailure("Sensitive data cannot be sent.")
        }
    }
}
