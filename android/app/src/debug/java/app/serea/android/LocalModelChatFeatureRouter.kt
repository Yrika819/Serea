package app.serea.android

/**
 * Explicit selector for a precomposed local-model port. It does not construct a backend and is
 * not used by MainActivity. The test-only composition must opt in before LOCAL_MODEL can resolve.
 */
internal class LocalModelChatFeatureRouter(
    private val demoPort: ModelChatPort,
    private val localModelPort: ModelChatPort?,
    private val expectedLocalModelId: String,
    private val enabledForExplicitTestSelection: Boolean = false,
) {
    sealed interface Selection {
        data object Demo : Selection
        data class LocalModel(val modelId: String) : Selection
    }

    fun resolve(selection: Selection?): ModelChatPort = when (selection) {
        Selection.Demo -> demoPort
        is Selection.LocalModel -> {
            if (!enabledForExplicitTestSelection || localModelPort == null ||
                selection.modelId != expectedLocalModelId ||
                !selection.modelId.matches(MODEL_ID) || selection.modelId.contains("://")) {
                throw PreDispatchModelChatFailure("The local model test route is disabled or mismatched.")
            }
            localModelPort
        }
        null -> throw PreDispatchModelChatFailure("A test route selection is required.")
    }

    private companion object {
        val MODEL_ID = Regex("[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}")
    }
}
