package app.serea.android

/** Public app-shell destinations. Routes contain no credentials, message content, or demo sessions. */
enum class SereaDestination(val label: String) {
    CHAT("会話"),
    TASKS("タスク"),
    GMAIL("Gmail"),
    CALENDAR("カレンダー"),
    SETTINGS("設定"),
    TASK_PROPOSAL_DEMO("提案デモ"),
}

/** Stable contract used by the shell and its tests; external services remain unavailable here. */
data class DestinationContract(
    val destination: SereaDestination,
    val title: String,
    val mockOnly: Boolean = false,
) {
    companion object {
        val all: List<DestinationContract> = listOf(
            DestinationContract(SereaDestination.CHAT, "会話"),
            DestinationContract(SereaDestination.TASKS, "タスク"),
            DestinationContract(SereaDestination.GMAIL, "Gmail", mockOnly = true),
            DestinationContract(SereaDestination.CALENDAR, "カレンダー", mockOnly = true),
            DestinationContract(SereaDestination.SETTINGS, "設定"),
            DestinationContract(SereaDestination.TASK_PROPOSAL_DEMO, "提案デモ", mockOnly = true),
        )
    }
}

/** Activity-restorable navigation state. It stores route identifiers only; demo sessions are ephemeral. */
data class SereaNavigationState(
    val current: SereaDestination = SereaDestination.CHAT,
    val backStack: List<SereaDestination> = emptyList(),
) {
    val canGoBack: Boolean get() = backStack.isNotEmpty()

    fun navigate(destination: SereaDestination): SereaNavigationState {
        if (destination == current) return this
        return copy(current = destination, backStack = backStack + current)
    }

    fun back(): SereaNavigationState = if (backStack.isEmpty()) this else copy(
        current = backStack.last(),
        backStack = backStack.dropLast(1),
    )

    companion object {
        fun restore(current: String?, backStack: List<String>?): SereaNavigationState {
            val restoredCurrent = current?.let { raw ->
                SereaDestination.entries.firstOrNull { it.name == raw }
            } ?: SereaDestination.CHAT
            val restoredStack = backStack.orEmpty().mapNotNull { raw ->
                SereaDestination.entries.firstOrNull { it.name == raw }
            }.takeLast(MAX_BACK_STACK)
            return SereaNavigationState(restoredCurrent, restoredStack)
        }

        const val MAX_BACK_STACK = 32
    }
}
