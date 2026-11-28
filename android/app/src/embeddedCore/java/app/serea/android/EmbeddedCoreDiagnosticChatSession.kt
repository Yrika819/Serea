package app.serea.android

import android.content.Context
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStoreOwner
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

/** Shares one Core owner and volatile synthetic history across Activity/Chat ViewModel recreation. */
internal class EmbeddedCoreDiagnosticChatSessionViewModel(context: Context) : ViewModel() {
    val modelChatPort = EmbeddedCoreChatComposition.create(context.applicationContext)
    val historyRepository = InMemoryDiagnosticConversationHistory()
    internal val cleanupResult = CompletableDeferred<Result<Unit>>()

    override fun onCleared() {
        EmbeddedCoreDiagnosticCleanup.close(modelChatPort, cleanupResult)
        super.onCleared()
    }
}

private object EmbeddedCoreDiagnosticCleanup {
    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.IO)

    fun close(port: EmbeddedCoreModelChatPort, completion: CompletableDeferred<Result<Unit>>) {
        scope.launch { completion.complete(runCatching { port.close() }) }
    }
}

/** This fixture is intentionally memory-only: diagnostic Chat text never enters Room or disk. */
internal class InMemoryDiagnosticConversationHistory(
    private val clock: () -> Long = System::currentTimeMillis,
) : ConversationHistoryPort {
    private data class Row(
        val id: String,
        val title: String,
        val createdAt: Long,
        var updatedAt: Long,
        val modelId: String,
        val messages: MutableList<StoredMessage>,
    )

    private val mutex = Mutex()
    private val conversations = linkedMapOf<String, Row>()

    override suspend fun list(): List<StoredConversation> = mutex.withLock {
        conversations.values.sortedByDescending { it.updatedAt }.map { it.snapshot() }
    }

    override suspend fun get(id: String): StoredConversation? = mutex.withLock {
        conversations[id]?.snapshot()
    }

    override suspend fun addMessage(message: StoredMessage) {
        mutex.withLock {
            val row = checkNotNull(conversations[message.conversationId]) { "Synthetic conversation does not exist." }
            check(row.messages.none { it.id == message.id }) { "Synthetic message already exists." }
            row.messages += message
            row.updatedAt = maxOf(row.updatedAt, message.createdAt)
        }
    }

    override suspend fun createWithFirstMessage(
        id: String,
        title: String,
        message: StoredMessage,
        modelId: String,
    ) {
        require(id.isNotBlank() && message.conversationId == id && message.role == MessageRole.USER && message.status == "PENDING")
        mutex.withLock {
            check(id !in conversations) { "Synthetic conversation already exists." }
            conversations[id] = Row(id, title, message.createdAt, message.createdAt, modelId, mutableListOf(message))
        }
    }

    override suspend fun completeTurn(userMessageId: String, assistant: StoredMessage) {
        require(assistant.role == MessageRole.ASSISTANT && assistant.status == "COMPLETED")
        mutex.withLock {
            val row = checkNotNull(conversations[assistant.conversationId]) { "Synthetic conversation does not exist." }
            val index = row.messages.indexOfFirst { it.id == userMessageId && it.status == "PENDING" }
            check(index >= 0 && row.messages.none { it.id == assistant.id }) { "Synthetic turn is not pending." }
            row.messages[index] = row.messages[index].copy(status = "COMPLETED")
            row.messages += assistant
            row.updatedAt = maxOf(row.updatedAt, assistant.createdAt)
        }
    }

    override suspend fun updateStatus(messageId: String, conversationId: String, status: String): Boolean {
        require(status in setOf("COMPLETED", "FAILED", "OUTCOME_UNKNOWN"))
        return mutex.withLock {
            val row = conversations[conversationId] ?: return@withLock false
            val index = row.messages.indexOfFirst { it.id == messageId }
            if (index < 0) return@withLock false
            val current = row.messages[index]
            if (current.status == status) return@withLock true
            if (current.status != "PENDING") return@withLock false
            row.messages[index] = current.copy(status = status)
            row.updatedAt = maxOf(row.updatedAt, clock())
            true
        }
    }

    override suspend fun delete(id: String, now: Long) {
        mutex.withLock { conversations.remove(id) }
    }

    override suspend fun deleteAll() {
        mutex.withLock { conversations.clear() }
    }

    private fun Row.snapshot() = StoredConversation(
        id = id,
        title = title,
        createdAt = createdAt,
        updatedAt = updatedAt,
        modelId = modelId,
        messages = messages.toList(),
    )
}

internal class EmbeddedCoreDiagnosticChatSessionProvider :
    EmbeddedCoreDiagnosticChatSessionFactory,
    EmbeddedCoreCloudChatPortFactory {
    override fun create(
        owner: ViewModelStoreOwner,
        applicationContext: Context,
    ): ChatSessionComposition {
        val session = session(owner, applicationContext)
        return ChatSessionComposition(
            modelChatPort = session.modelChatPort,
            historyRepository = session.historyRepository,
            usageRepository = null,
            embeddedCoreDiagnostic = true,
        )
    }

    override fun wrap(
        owner: ViewModelStoreOwner,
        applicationContext: Context,
        cloudPort: GatedCloudModelChatPort,
    ): ModelChatPort = EmbeddedCoreCloudChatComposition.create(session(owner, applicationContext).modelChatPort, cloudPort)

    private fun session(owner: ViewModelStoreOwner, applicationContext: Context): EmbeddedCoreDiagnosticChatSessionViewModel {
        val factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T {
                check(modelClass.isAssignableFrom(EmbeddedCoreDiagnosticChatSessionViewModel::class.java))
                return EmbeddedCoreDiagnosticChatSessionViewModel(applicationContext) as T
            }
        }
        return ViewModelProvider(owner, factory).get(EmbeddedCoreDiagnosticChatSessionViewModel::class.java)
    }
}
