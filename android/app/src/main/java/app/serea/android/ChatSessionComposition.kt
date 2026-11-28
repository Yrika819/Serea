package app.serea.android

import android.content.Context
import androidx.lifecycle.ViewModelStoreOwner

/** A Chat dependency bundle selected once by the Activity's build-gated composition root. */
internal data class ChatSessionComposition(
    val modelChatPort: ModelChatPort,
    val historyRepository: ConversationHistoryPort?,
    val usageRepository: UsageRepository?,
    val embeddedCoreDiagnostic: Boolean,
)

/** Implemented only in the explicitly opted-in isolated Embedded Core source set. */
internal interface EmbeddedCoreDiagnosticChatSessionFactory {
    fun create(owner: ViewModelStoreOwner, applicationContext: Context): ChatSessionComposition
}

internal object EmbeddedCoreDiagnosticChatSessionFactoryLoader {
    private const val PROVIDER_CLASS = "app.serea.android.EmbeddedCoreDiagnosticChatSessionProvider"

    fun load(): EmbeddedCoreDiagnosticChatSessionFactory =
        Class.forName(PROVIDER_CLASS).getDeclaredConstructor().newInstance() as EmbeddedCoreDiagnosticChatSessionFactory
}

internal object ChatSessionCompositionFactory {
    fun create(
        embeddedCoreDiagnosticEnabled: Boolean,
        demo: () -> ChatSessionComposition,
        embeddedCoreDiagnostic: () -> ChatSessionComposition,
    ): ChatSessionComposition {
        if (!embeddedCoreDiagnosticEnabled) return demo()
        return try {
            embeddedCoreDiagnostic().copy(embeddedCoreDiagnostic = true)
        } catch (_: Exception) {
            unavailableDiagnosticSession()
        } catch (_: LinkageError) {
            unavailableDiagnosticSession()
        }
    }

    private fun unavailableDiagnosticSession() = ChatSessionComposition(
        modelChatPort = object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse =
                throw ModelChatFailure("Embedded Core diagnostic is unavailable; the local Chat request was not sent.")
        },
        // Never fall back to the Room-backed product history or Usage path in a broken diagnostic build.
        historyRepository = null,
        usageRepository = null,
        embeddedCoreDiagnostic = true,
    )
}
