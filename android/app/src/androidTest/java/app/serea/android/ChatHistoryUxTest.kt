package app.serea.android

import android.content.Context
import androidx.lifecycle.ViewModelStore
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.UUID
import java.security.KeyStore

/**
 * These regressions use only in-memory Room databases and unique Keystore aliases.
 * They never open, clear, or modify the app's named serea-history.db. Running this
 * class alone is data-isolated; do not run the broader ChatScreenTest suite on a
 * personal device because other UI tests intentionally exercise app history.
 */
@RunWith(AndroidJUnit4::class)
class ChatHistoryUxTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()

    @Test fun openingSeveralNewConversationsLeavesHistoryEmptyUntilFirstMessage() = runBlocking {
        withIsolatedHistory { repository, store, _, _ ->
            val vm = ChatViewModel(MockModelChatPort(0), repository)
            store.put("chat-history-ux", vm)
            awaitCondition { !vm.state.value.isRestoring }
            repeat(5) { vm.newConversation() }
            assertTrue(repository.list().isEmpty())

            vm.updateInput("日本語の相談 😀")
            vm.sendMessage()
            awaitCondition { !vm.state.value.isLoading && vm.state.value.conversation.messages.any { it.role == MessageRole.ASSISTANT } }
            val saved = repository.list()
            assertEquals(1, saved.size)
            assertEquals("日本語の相談 😀", saved.single().title)
            assertEquals(2, saved.single().messages.size)
        }
    }

    @Test fun olderPendingConversationLoadCannotReplaceLaterSelection() = runBlocking {
        withIsolatedHistory { baseRepository, store, dao, cipher ->
            baseRepository.create("conversation-a", "Conversation A", now = 1)
            baseRepository.addMessage(StoredMessage(
                "pending-a", "conversation-a", MessageRole.USER, "A pending turn",
                DataClassification.UNKNOWN, 2, "PENDING", DataOrigin.USER_AUTHORED,
            ))
            baseRepository.create("conversation-b", "Conversation B", now = 3)
            baseRepository.addMessage(StoredMessage(
                "completed-b", "conversation-b", MessageRole.USER, "B completed turn",
                DataClassification.UNKNOWN, 4, "COMPLETED", DataOrigin.USER_AUTHORED,
            ))

            val delayedDao = DelayedStatusHistoryDao(dao.historyDao(), "pending-a")
            val repository = ConversationRepository(delayedDao, cipher)
            val vm = ChatViewModel(MockModelChatPort(0), repository, initialConversation = Conversation("initial"))
            store.put("chat-history-race", vm)

            vm.openConversation("conversation-a")
            delayedDao.statusUpdateEntered.await()
            vm.openConversation("conversation-b")
            awaitCondition { vm.state.value.conversation.id == "conversation-b" }

            delayedDao.releaseStatusUpdate.complete(Unit)
            delayedDao.statusUpdateFinished.await()
            assertEquals("conversation-b", vm.state.value.conversation.id)
            assertEquals("B completed turn", vm.state.value.conversation.messages.single().text)
        }
    }

    @Test fun titleAndFirstMessageTransactionRollsBackOnMessageInsertFailure() = runBlocking {
        withIsolatedHistory { repository, _, database, _ ->
            val dao = database.historyDao()
            database.openHelper.writableDatabase.execSQL(
                "CREATE TRIGGER reject_test_message BEFORE INSERT ON messages " +
                    "WHEN NEW.id = 'reject-first-message' BEGIN SELECT RAISE(ABORT, 'fixture failure'); END",
            )
            val failure = runCatching {
                repository.createWithFirstMessage(
                    "conversation-rollback", "Fixture title",
                    StoredMessage("reject-first-message", "conversation-rollback", MessageRole.USER,
                        "Fixture body", DataClassification.UNKNOWN, 2, "PENDING", DataOrigin.USER_AUTHORED),
                )
            }.exceptionOrNull()
            assertNotNull(failure)
            assertNull(dao.conversation("conversation-rollback"))
            assertTrue(dao.messages("conversation-rollback").isEmpty())
            assertTrue(repository.list().isEmpty())
        }
    }

    @Test fun firstMessageAndTitleAreEncryptedInIsolatedHistory() = runBlocking {
        withIsolatedHistory { repository, _, database, _ ->
            val dao = database.historyDao()
            assertTrue(repository.list().isEmpty())
            val title = "Local fixture title 😀"
            repository.createWithFirstMessage(
                "first-conversation", title,
                StoredMessage("first-message", "first-conversation", MessageRole.USER, title,
                    DataClassification.UNKNOWN, 10, "PENDING", DataOrigin.USER_AUTHORED),
            )
            val rawConversation = dao.conversation("first-conversation")!!
            val rawMessage = dao.messages("first-conversation").single()
            assertFalse(rawConversation.encryptedTitle.contains(title))
            assertFalse(rawMessage.encryptedBody.contains(title))
            val restored = repository.get("first-conversation")!!
            assertEquals(title, restored.title)
            assertEquals(title, restored.messages.single().text)
        }
    }

    private suspend fun withIsolatedHistory(
        test: suspend (ConversationRepository, ViewModelStore, SereaHistoryDatabase, KeystoreAesGcm) -> Unit,
    ) {
        val database = Room.inMemoryDatabaseBuilder(context, SereaHistoryDatabase::class.java).build()
        val keyAlias = "serea-history-ux-${UUID.randomUUID()}"
        val cipher = KeystoreAesGcm(keyAlias)
        val repository = ConversationRepository(database.historyDao(), cipher)
        val store = ViewModelStore()
        val appHistory = context.getDatabasePath("serea-history.db")
        val appHistoryBefore = FileSnapshot(appHistory.exists(), appHistory.takeIf { it.exists() }?.length(), appHistory.takeIf { it.exists() }?.lastModified())
        try {
            test(repository, store, database, cipher)
        } finally {
            store.clear()
            database.close()
            KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry(keyAlias) }
            val appHistoryAfter = FileSnapshot(appHistory.exists(), appHistory.takeIf { it.exists() }?.length(), appHistory.takeIf { it.exists() }?.lastModified())
            assertEquals("the app history database must remain untouched", appHistoryBefore, appHistoryAfter)
        }
    }

    private suspend fun awaitCondition(condition: () -> Boolean) {
        withTimeout(10_000) {
            while (!condition()) delay(10)
        }
    }

    private data class FileSnapshot(val exists: Boolean, val length: Long?, val modifiedAt: Long?)

    private class DelayedStatusHistoryDao(
        private val delegate: HistoryDao,
        private val delayedMessageId: String,
    ) : HistoryDao by delegate {
        val statusUpdateEntered = CompletableDeferred<Unit>()
        val releaseStatusUpdate = CompletableDeferred<Unit>()
        val statusUpdateFinished = CompletableDeferred<Unit>()

        override suspend fun transitionPendingMessage(messageId: String, conversationId: String, status: String): Int {
            if (messageId == delayedMessageId) {
                statusUpdateEntered.complete(Unit)
                releaseStatusUpdate.await()
                return try {
                    delegate.transitionPendingMessage(messageId, conversationId, status)
                } finally {
                    statusUpdateFinished.complete(Unit)
                }
            }
            return delegate.transitionPendingMessage(messageId, conversationId, status)
        }
    }
}
