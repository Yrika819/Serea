package app.serea.android

import android.content.Context
import android.content.ContextWrapper
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import androidx.lifecycle.ViewModelStoreOwner
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class EmbeddedCoreDiagnosticChatSessionTest {
    @Test
    fun activityProviderClassResolvesOnlyInTheEmbeddedCoreOptInSourceSet() {
        assertTrue(EmbeddedCoreDiagnosticChatSessionFactoryLoader.load() is EmbeddedCoreDiagnosticChatSessionProvider)
    }

    @Test
    fun diagnosticHistoryIsVolatileAndTransitionsOnlyPendingMessages() = runTest {
        var now = 100L
        val history = InMemoryDiagnosticConversationHistory { now }
        val user = StoredMessage(
            id = "user-1",
            conversationId = "conversation-1",
            role = MessageRole.USER,
            text = "synthetic question",
            classification = DataClassification.PUBLIC,
            createdAt = now,
            status = "PENDING",
            origin = DataOrigin.USER_AUTHORED,
        )
        history.createWithFirstMessage("conversation-1", "Synthetic conversation", user, "embedded-core-mock")

        now = 120L
        history.completeTurn(
            userMessageId = user.id,
            assistant = StoredMessage(
                id = "assistant-1",
                conversationId = user.conversationId,
                role = MessageRole.ASSISTANT,
                text = "synthetic response",
                classification = DataClassification.PUBLIC,
                createdAt = now,
                status = "COMPLETED",
                origin = DataOrigin.MODEL_OUTPUT,
            ),
        )

        val saved = checkNotNull(history.get(user.conversationId))
        assertEquals("embedded-core-mock", saved.modelId)
        assertEquals(listOf("COMPLETED", "COMPLETED"), saved.messages.map { it.status })
        assertFalse(history.updateStatus(user.id, user.conversationId, "FAILED"))
        assertEquals(listOf(saved), history.list())
        history.deleteAll()
        assertTrue(history.list().isEmpty())
    }

    @Test
    fun diagnosticProviderReusesOneSessionAcrossViewModelRecreation() {
        val owner = TestViewModelStoreOwner()
        val context = object : ContextWrapper(null) {
            override fun getApplicationContext(): Context = this
        }
        val provider = EmbeddedCoreDiagnosticChatSessionProvider()

        val first = provider.create(owner, context)
        val second = provider.create(owner, context)

        assertTrue(first.embeddedCoreDiagnostic)
        assertSame(first.modelChatPort, second.modelChatPort)
        assertSame(first.historyRepository, second.historyRepository)
        assertNull(first.usageRepository)
        val sessionOwner = ViewModelProvider(owner)
            .get(EmbeddedCoreDiagnosticChatSessionViewModel::class.java)
        owner.viewModelStore.clear()
        val cleanup = runBlocking { withTimeout(5_000) { sessionOwner.cleanupResult.await() } }
        assertTrue(cleanup.isSuccess)
    }

    @Test
    fun defaultCompositionNeverConstructsDiagnosticProvider() {
        val demoPort = object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest) = ModelChatResponse("demo")
        }
        var diagnosticProviderCalls = 0
        val composition = ChatSessionCompositionFactory.create(
            embeddedCoreDiagnosticEnabled = false,
            demo = { ChatSessionComposition(demoPort, null, null, embeddedCoreDiagnostic = false) },
            embeddedCoreDiagnostic = {
                diagnosticProviderCalls++
                error("the diagnostic provider must remain unreachable in normal variants")
            },
        )

        assertSame(demoPort, composition.modelChatPort)
        assertFalse(composition.embeddedCoreDiagnostic)
        assertEquals(0, diagnosticProviderCalls)
    }

    @Test
    fun brokenOptInCompositionFailsClosedWithoutRoomOrUsageFallback() = runTest {
        val composition = ChatSessionCompositionFactory.create(
            embeddedCoreDiagnosticEnabled = true,
            demo = { error("diagnostic build must never fall back to the product demo composition") },
            embeddedCoreDiagnostic = { error("provider linkage failure") },
        )

        assertTrue(composition.embeddedCoreDiagnostic)
        assertNull(composition.historyRepository)
        assertNull(composition.usageRepository)
        val failure = runCatching {
            composition.modelChatPort.send(ModelChatRequest("synthetic-conversation", "synthetic prompt"))
        }.exceptionOrNull()
        assertTrue(failure is ModelChatFailure)
        assertTrue(failure?.message?.contains("request was not sent") == true)
    }

    private class TestViewModelStoreOwner : ViewModelStoreOwner {
        override val viewModelStore = ViewModelStore()
    }
}
