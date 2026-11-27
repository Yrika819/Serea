package app.serea.android

/** A history item retains its original trust classification all the way to an adapter. */
data class ContextMessage(
    val role: MessageRole,
    val text: String,
    val classification: DataClassification,
    val sourceMessageId: String,
    val origin: DataOrigin = DataOrigin.UNKNOWN,
    val status: String = "COMPLETED",
    val conversationId: String? = null,
    /** Immediate and inherited sources; model output without recorded ancestry fails closed. */
    val sourceLineage: Set<DataOrigin> = defaultSourceLineage(origin),
)

internal fun defaultSourceLineage(origin: DataOrigin): Set<DataOrigin> = when (origin) {
    DataOrigin.MODEL_OUTPUT -> setOf(DataOrigin.MODEL_OUTPUT, DataOrigin.UNKNOWN)
    else -> setOf(origin)
}

/** Bounded, chronological context construction for the Android-only conversation engine. */
object ChatContextBuilder {
    const val MAX_INPUT_CHARS = 12_000
    const val MAX_CONTEXT_TOKENS = 4_096
    const val MAX_CONTEXT_CHARS = MAX_CONTEXT_TOKENS

    /**
     * Includes only completed turns. Newest turns win when the approximate token budget is
     * exhausted; an oldest partial turn may be clipped at a Unicode code point boundary.
     */
    fun build(history: List<ChatMessage>, currentInput: String = "", expectedConversationId: String? = null): List<ContextMessage> {
        val remaining = (MAX_CONTEXT_CHARS - currentInput.length).coerceAtLeast(0)
        return selectRecent(history.map { message ->
            ContextMessage(message.role, message.text, message.classification, message.id, message.origin,
                message.status, message.conversationId, message.sourceLineage.toSet())
        }, remaining, expectedConversationId)
    }

    /**
     * Builds context for a selected model. Input is budgeted with the router's conservative
     * four-characters-per-estimated-token rule; the result is only a character bound, never an
     * exact tokenizer count. Unknown model limits keep the legacy fixed context cap.
     */
    fun buildForModel(
        history: List<ChatMessage>,
        currentInput: String,
        expectedConversationId: String?,
        model: ModelDescriptor,
        requestedOutputTokens: Int,
        maxInputTokens: Long? = null,
    ): List<ContextMessage> {
        val budget = historyCharacterBudget(model.contextWindow.knownInt(), currentInput, requestedOutputTokens, maxInputTokens)
        return build(history, currentInput, expectedConversationId, budget)
    }

    private fun build(
        history: List<ChatMessage>,
        currentInput: String,
        expectedConversationId: String?,
        historyCharacterBudget: Int,
    ): List<ContextMessage> = selectRecent(history.map { message ->
        ContextMessage(message.role, message.text, message.classification, message.id, message.origin,
            message.status, message.conversationId, message.sourceLineage.toSet())
    }, historyCharacterBudget, expectedConversationId)

    /** Trims an already-built history list after routing while preserving all retained metadata. */
    internal fun fitForModel(
        context: List<ContextMessage>,
        currentInput: String,
        expectedConversationId: String?,
        modelContextWindowTokens: Int?,
        requestedOutputTokens: Int,
        maxInputTokens: Long? = null,
    ): List<ContextMessage> = selectRecent(
        context,
        historyCharacterBudget(modelContextWindowTokens, currentInput, requestedOutputTokens, maxInputTokens),
        expectedConversationId,
    )

    /** Computes a conservative history character allowance without claiming tokenization. */
    internal fun historyCharacterBudget(
        modelContextWindowTokens: Int?,
        currentInput: String,
        requestedOutputTokens: Int,
        maxInputTokens: Long? = null,
    ): Int {
        require(requestedOutputTokens > 0)
        require(maxInputTokens == null || maxInputTokens > 0)
        if (modelContextWindowTokens == null) return (MAX_CONTEXT_CHARS - currentInput.length).coerceAtLeast(0)
        val contextLimit = modelContextWindowTokens
        if (contextLimit <= 0) return 0
        val bodyEstimatedTokens = Math.multiplyExact(currentInput.length.toLong(), ESTIMATED_TOKENS_PER_CHAR)
        val contextInputLimit = (contextLimit.toLong() - requestedOutputTokens).coerceAtLeast(0)
        val inputLimit = minOf(contextInputLimit, maxInputTokens ?: Long.MAX_VALUE)
        val remainingEstimatedTokens = (inputLimit - bodyEstimatedTokens).coerceAtLeast(0)
        val historyChars = (remainingEstimatedTokens / ESTIMATED_TOKENS_PER_CHAR).coerceAtMost(Int.MAX_VALUE.toLong()).toInt()
        return minOf(MAX_CONTEXT_CHARS, historyChars)
    }

    private fun selectRecent(context: List<ContextMessage>, maxCharacters: Int, expectedConversationId: String?): List<ContextMessage> {
        var remaining = maxCharacters.coerceAtLeast(0)
        val selected = ArrayDeque<ContextMessage>()
        for (message in context.asReversed()) {
            if (message.status != "COMPLETED") continue
            if (expectedConversationId != null && message.conversationId != null && message.conversationId != expectedConversationId) continue
            if (remaining <= 0) break
            val clipped = truncateAtCodePointBoundary(message.text, remaining)
            if (clipped.isNotEmpty()) {
                selected.addFirst(message.copy(text = clipped, sourceLineage = message.sourceLineage.toSet()))
                remaining -= clipped.length
            }
        }
        return selected.toList()
    }

    private fun truncateAtCodePointBoundary(value: String, maxCharacters: Int): String {
        var length = minOf(value.length, maxCharacters)
        if (length in 1 until value.length && value[length - 1].isHighSurrogate() && value[length].isLowSurrogate()) length--
        return value.substring(0, length)
    }

    private const val ESTIMATED_TOKENS_PER_CHAR = 4L
}
