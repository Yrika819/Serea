package app.serea.android

import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import android.content.Context
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import kotlinx.coroutines.runBlocking
import java.util.UUID

@RunWith(AndroidJUnit4::class)
class ChatHistoryUxTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun openingSeveralNewConversationsLeavesHistoryEmptyUntilFirstMessage() {
        val repository = ConversationRepository(
            SereaHistoryDatabase.open(compose.activity).historyDao(), KeystoreAesGcm("serea-history-v1"),
        )
        runBlocking { repository.deleteAll() }
        compose.waitUntil(30_000) { !stateSnapshot().isRestoring }
        repeat(5) { compose.activityRule.scenario.onActivity { it.chatViewModel.newConversation() } }
        assertTrue(runBlocking { repository.list() }.isEmpty())

        compose.onNodeWithTag("chat-composer").performTextInput("日本語の相談 😀")
        compose.onNodeWithText("送信").performClick()
        compose.waitUntil(30_000) {
            val state = stateSnapshot()
            !state.isLoading && state.conversation.messages.any { it.role == MessageRole.ASSISTANT }
        }
        val saved = runBlocking { repository.list() }
        assertEquals(1, saved.size)
        assertEquals("日本語の相談 😀", saved.single().title)
        assertEquals(2, saved.single().messages.size)
        runBlocking { repository.deleteAll() }
    }

    @Test fun firstMessageAndTitleAreStoredAtomicallyAndEncrypted() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val database = Room.inMemoryDatabaseBuilder(context, SereaHistoryDatabase::class.java).build()
        val repository = ConversationRepository(database.historyDao(), KeystoreAesGcm("serea-title-${UUID.randomUUID()}"))
        try {
            assertTrue(repository.list().isEmpty())
            val title = "日本語の相談 😀"
            repository.createWithFirstMessage(
                "first-conversation", title,
                StoredMessage("first-message", "first-conversation", MessageRole.USER, title,
                    DataClassification.UNKNOWN, 10, "PENDING", DataOrigin.USER_AUTHORED),
            )
            val rawConversation = database.historyDao().conversation("first-conversation")!!
            val rawMessage = database.historyDao().messages("first-conversation").single()
            assertFalse(rawConversation.encryptedTitle.contains(title))
            assertFalse(rawMessage.encryptedBody.contains(title))
            val restored = repository.get("first-conversation")!!
            assertEquals(title, restored.title)
            assertEquals(title, restored.messages.single().text)
            assertEquals(1, repository.list().size)
        } finally {
            database.close()
        }
    }

    private fun stateSnapshot(): ChatUiState {
        var snapshot: ChatUiState? = null
        compose.activityRule.scenario.onActivity { snapshot = it.chatViewModel.state.value }
        return requireNotNull(snapshot)
    }
}
