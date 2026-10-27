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
    // One UTF-16 code unit per budget token is intentionally conservative; tokenizer output varies.
    private const val CHARS_PER_TOKEN = 1
    const val MAX_CONTEXT_CHARS = MAX_CONTEXT_TOKENS * CHARS_PER_TOKEN

    /**
     * Includes only completed turns. Newest turns win when the approximate token budget is
     * exhausted; an oldest partial turn may be clipped at a Unicode code point boundary.
     */
    fun build(history: List<ChatMessage>, currentInput: String = "", expectedConversationId: String? = null): List<ContextMessage> {
        var remaining = (MAX_CONTEXT_CHARS - currentInput.length).coerceAtLeast(0)
        val selected = ArrayDeque<ContextMessage>()
        for (message in history.asReversed()) {
            if (message.status != "COMPLETED") continue
            if (expectedConversationId != null && message.conversationId != expectedConversationId) continue
            if (remaining <= 0) break
            var clippedLength = minOf(message.text.length, remaining)
            if (clippedLength in 1 until message.text.length &&
                message.text[clippedLength - 1].isHighSurrogate() && message.text[clippedLength].isLowSurrogate()) {
                clippedLength--
            }
            val clipped = message.text.substring(0, clippedLength)
            if (clipped.isNotEmpty()) {
                selected.addFirst(ContextMessage(message.role, clipped, message.classification, message.id, message.origin,
                    message.status, message.conversationId, message.sourceLineage.toSet()))
                remaining -= clipped.length
            }
        }
        return selected.toList()
    }
}
