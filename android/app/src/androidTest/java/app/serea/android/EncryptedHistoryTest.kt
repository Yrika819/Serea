package app.serea.android

import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import android.content.Context
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.Dispatchers
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.UUID
import java.security.KeyStore

@RunWith(AndroidJUnit4::class)
class EncryptedHistoryTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private val database = Room.inMemoryDatabaseBuilder(context, SereaHistoryDatabase::class.java).build()
    private val repository = ConversationRepository(database.historyDao(), KeystoreAesGcm("serea-test-${UUID.randomUUID()}"))

    @After fun close() { database.close() }

    @Test fun historyCrudSeparatesConversationsAndStoresOnlyCiphertext() = runBlocking {
        repository.create("c1", "Personal title", now = 1)
        repository.create("c2", "Second title", now = 2)
        repository.addMessage(StoredMessage("m1", "c1", MessageRole.USER, "my private note", DataClassification.UNKNOWN, 3, "COMPLETED"))
        repository.addMessage(StoredMessage("m2", "c1", MessageRole.ASSISTANT, "reply", DataClassification.PUBLIC, 4, "COMPLETED"))
        val raw = database.historyDao().messages("c1").first().encryptedBody
        assertFalse(raw.contains("my private note"))
        assertEquals(listOf("m1", "m2"), repository.get("c1")!!.messages.map { it.id })
        assertTrue(repository.get("c2")!!.messages.isEmpty())
        repository.delete("c1", now = 5)
        assertNull(repository.get("c1"))
        assertEquals(1, repository.list().size)
        repository.deleteAll()
        assertTrue(repository.list().isEmpty())
    }

    @Test fun databaseReopenRestoresMessagesAndConcurrentWritesStaySeparated() = runBlocking {
        val name = "history-reopen-${UUID.randomUUID()}.db"
        val alias = "history-reopen-${UUID.randomUUID()}"
        val firstDb = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, name).build()
        val first = ConversationRepository(firstDb.historyDao(), KeystoreAesGcm(alias))
        first.create("conversation-a", "A", now = 1)
        first.create("conversation-b", "B", now = 1)
        (1..12).map { index -> async(Dispatchers.IO) {
            val id = "message-${index.toString().padStart(2, '0')}"
            val conversationId = if (index % 2 == 0) "conversation-a" else "conversation-b"
            first.addMessage(StoredMessage(id, conversationId, MessageRole.USER, "body-$index", DataClassification.UNKNOWN, index.toLong() + 1, "COMPLETED"))
        } }.awaitAll()
        firstDb.close()

        val reopened = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, name).build()
        val restored = ConversationRepository(reopened.historyDao(), KeystoreAesGcm(alias))
        assertEquals("body-2", restored.get("conversation-a")!!.messages.first().text)
        assertEquals(6, restored.get("conversation-a")!!.messages.size)
        assertEquals(6, restored.get("conversation-b")!!.messages.size)
        restored.delete("conversation-a")
        assertThrows<IllegalArgumentException> {
            kotlinx.coroutines.runBlocking {
                restored.addMessage(StoredMessage("late", "conversation-a", MessageRole.ASSISTANT, "late", DataClassification.UNKNOWN, 50, "COMPLETED"))
            }
        }
        reopened.close()
        context.deleteDatabase(name)
        KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry(alias) }
    }

    @Test fun authenticationTagRejectsTamperingAndWrongAssociatedData() {
        val cipher = KeystoreAesGcm("serea-test-aad-${UUID.randomUUID()}")
        val sealed = cipher.encrypt("secret body".toByteArray(), "message:id".toByteArray())
        assertEquals("secret body", cipher.decrypt(sealed, "message:id".toByteArray()).toString(Charsets.UTF_8))
        val tampered = sealed.dropLast(2) + if (sealed.last() == 'A') "B=" else "A="
        assertThrows<HistoryUnavailableException> { cipher.decrypt(tampered, "message:id".toByteArray()) }
        assertThrows<HistoryUnavailableException> { cipher.decrypt(sealed, "message:other".toByteArray()) }
    }

    @Test fun credentialsAreEncryptedReplaceableAndDeleted() = runBlocking {
        val credentials = CredentialRepository(context)
        credentials.currentHandle()?.let { credentials.delete(it) }
        val original = "test-key-never-a-real-secret".toCharArray()
        val firstHandle = credentials.store(original)
        assertTrue(original.all { it == '\u0000' })
        assertTrue(credentials.isConfigured())
        val stored = context.getSharedPreferences("cloud-credential", Context.MODE_PRIVATE)
            .getString("encrypted_api_key", "")!!
        assertFalse(stored.contains("test-key-never-a-real-secret"))
        assertEquals("test-key-never-a-real-secret", credentials.readForRequest())
        credentials.store("replacement-key".toCharArray())
        assertEquals("replacement-key", credentials.readForRequest())
        assertThrows<IllegalArgumentException> { runBlocking { credentials.delete(firstHandle) } }
        credentials.currentHandle()?.let { credentials.delete(it) }
        assertFalse(credentials.isConfigured())
        assertTrue(credentials.securityProfile() in setOf("StrongBox", "Hardware-backed TEE", "Hardware-backed Android Keystore", "Software-backed Android Keystore", "Android Keystore level unavailable"))
    }

    @Test fun missingKeystoreKeyDoesNotSilentlyDiscardEncryptedCredential() = runBlocking {
        val credentials = CredentialRepository(context)
        credentials.currentHandle()?.let { credentials.delete(it) }
        credentials.store("temporary-test-key".toCharArray())
        KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry("serea-cloud-credential-v1") }
        assertThrows<HistoryUnavailableException> { credentials.readForRequest() }
        assertTrue(credentials.isConfigured())
        credentials.currentHandle()?.let { credentials.delete(it) }
    }

    @Test fun applicationDisablesBackupAndCleartextTraffic() {
        val flags = context.packageManager.getApplicationInfo(context.packageName, 0).flags
        assertEquals(0, flags and android.content.pm.ApplicationInfo.FLAG_ALLOW_BACKUP)
        assertEquals(0, flags and android.content.pm.ApplicationInfo.FLAG_USES_CLEARTEXT_TRAFFIC)
    }

    private inline fun <reified T : Throwable> assertThrows(block: () -> Unit) {
        try { block() } catch (error: Throwable) { if (error is T) return else throw error }
        throw AssertionError("Expected ${T::class.java.simpleName}")
    }
}
