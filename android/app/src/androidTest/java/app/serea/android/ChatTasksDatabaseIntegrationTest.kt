package app.serea.android

import android.content.Context
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.tasks.EncryptedTaskRepository
import app.serea.android.tasks.KeystoreTaskCipher
import app.serea.android.tasks.TaskDraft
import app.serea.android.tasks.TasksDatabase
import java.security.KeyStore
import java.util.UUID
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ChatTasksDatabaseIntegrationTest {
    @Test fun historyAndTasksRemainIndependentAcrossDeletesCloseAndReopen() = runBlocking {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val suffix = UUID.randomUUID().toString()
        val historyName = "serea-history-task-integration-$suffix.db"
        val tasksName = "serea-tasks-integration-$suffix.db"
        val historyAlias = "serea-history-task-integration-$suffix"
        var historyDb: SereaHistoryDatabase? = null
        var tasksDb: TasksDatabase? = null

        fun openHistory() = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, historyName).build()
        fun openTasks() = Room.databaseBuilder(context, TasksDatabase::class.java, tasksName).build()

        try {
            historyDb = openHistory()
            tasksDb = openTasks()
            val history = ConversationRepository(historyDb!!.historyDao(), KeystoreAesGcm(historyAlias))
            val tasks = EncryptedTaskRepository(tasksDb!!.taskDao(), KeystoreTaskCipher())
            val deletedConversationId = "conversation-delete-$suffix"
            val keptConversationId = "conversation-keep-$suffix"
            history.create(deletedConversationId, "Remove this chat")
            history.create(keptConversationId, "Keep this chat")
            history.addMessage(StoredMessage(
                UUID.randomUUID().toString(), keptConversationId, MessageRole.USER, "Encrypted history survives task deletion",
                DataClassification.PUBLIC, System.currentTimeMillis(), "COMPLETED", DataOrigin.USER_AUTHORED,
            ))
            val keptTask = tasks.create(TaskDraft("Keep this encrypted task", "Separate from chat history"))
            val deletedTask = tasks.create(TaskDraft("Delete only this task"))

            history.delete(deletedConversationId)
            assertNull(history.get(deletedConversationId))
            assertNotNull(tasks.get(keptTask.id))
            assertNotNull(tasks.get(deletedTask.id))

            tasks.delete(deletedTask.id, deletedTask.revision)
            assertNull(tasks.get(deletedTask.id))
            assertEquals("Encrypted history survives task deletion", history.get(keptConversationId)!!.messages.single().text)

            val rawHistory = historyDb!!.historyDao().messages(keptConversationId).single()
            assertFalse(rawHistory.encryptedBody.contains("Encrypted history survives task deletion"))
            val rawTask = tasksDb!!.taskDao().row(keptTask.id)!!
            assertFalse(rawTask.encryptedRecord.contains("Keep this encrypted task"))
            val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            assertTrue(keyStore.containsAlias(historyAlias))
            assertTrue(keyStore.containsAlias("serea-tasks-v1"))
            assertFalse(historyAlias == "serea-tasks-v1")

            historyDb!!.close(); historyDb = null
            tasksDb!!.close(); tasksDb = null

            historyDb = openHistory()
            tasksDb = openTasks()
            val restoredHistory = ConversationRepository(historyDb!!.historyDao(), KeystoreAesGcm(historyAlias))
            val restoredTasks = EncryptedTaskRepository(tasksDb!!.taskDao(), KeystoreTaskCipher())
            assertNull(restoredHistory.get(deletedConversationId))
            assertEquals("Encrypted history survives task deletion", restoredHistory.get(keptConversationId)!!.messages.single().text)
            assertNotNull(restoredTasks.get(keptTask.id))
            assertNull(restoredTasks.get(deletedTask.id))
        } finally {
            historyDb?.close()
            tasksDb?.close()
            context.deleteDatabase(historyName)
            context.deleteDatabase(tasksName)
            KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry(historyAlias) }
        }
    }
}
