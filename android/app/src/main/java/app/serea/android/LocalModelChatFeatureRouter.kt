package app.serea.android

/**
 * Explicit model route boundary for the future local model product integration.
 * Place this below the Model Router and pass fully Egress/Usage-wrapped delegates. This class
 * never chooses a fallback: unavailable local or unknown models fail before another delegate runs.
 * The product default is generated as false for every Android variant. Unit tests may inject true
 * through the internal constructor; production composition must use the generated default.
 */
internal class LocalModelChatFeatureRouter(
    private val demoModelId: String = DemoChatComposition.DEMO_MODEL_ID,
    private val localModelId: String,
    private val demo: ModelChatPort,
    private val local: ModelChatPort,
    private val localModelChatEnabled: Boolean = BuildConfig.LOCAL_MODEL_CHAT_ENABLED,
) : ModelChatPort {
    init {
        require(demoModelId.isNotBlank() && demoModelId != localModelId)
        require(localModelId.isNotBlank())
    }

    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        val selected = request.selectedModelId
            ?: throw PreDispatchModelChatFailure("A model must be selected before Chat dispatch.")
        return when (selected) {
            demoModelId -> demo.send(request)
            localModelId -> {
                if (!localModelChatEnabled) {
                    throw PreDispatchModelChatFailure("Local model Chat is disabled in this build.")
                }
                val contract = request.egressContract
                if (contract == null || contract.target != EgressTarget.LOCAL_MODEL ||
                    contract.destination != ChatEgressGate.LOCAL_MODEL_DESTINATION ||
                    contract.selectedModelId != localModelId ||
                    contract.localModelConsent?.selectedModelId != localModelId) {
                    throw PreDispatchModelChatFailure("A fresh local-model consent contract is required.")
                }
                // A dispatch failure is surfaced to the caller. Do not retry or switch providers.
                local.send(request)
            }
            else -> throw PreDispatchModelChatFailure("The selected model is not available in this route.")
        }
    }
}
