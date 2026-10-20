package app.serea.android.tasks

import android.content.Context
import androidx.room.Room
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.serea.android.KeystoreAesGcm
import java.io.File
import java.security.KeyStore
import java.time.Instant
import java.util.UUID
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.async
import kotlinx.coroutines.Dispatchers
import org.junit.After
import org.junit.Assert.*
import org.junit.Before
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TasksDatabaseTest {
    private lateinit var context: Context
    private lateinit var dbName: String
    private lateinit var alias: String
    private lateinit var db: TasksDatabase
    private lateinit var cipher: TestKeystoreCipher

    @Before fun setUp() {
        context = InstrumentationRegistry.getInstrumentation().targetContext
        dbName = "tasks-test-${UUID.randomUUID()}.db"
        alias = "serea-tasks-test-${UUID.randomUUID()}"
        cipher = TestKeystoreCipher(alias)
        db = Room.databaseBuilder(context, TasksDatabase::class.java, dbName).allowMainThreadQueries().build()
    }

    @After fun tearDown() {
        if (::db.isInitialized && db.isOpen) db.close()
        context.deleteDatabase(dbName)
        runCatching { KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry(alias) } }
    }

    @Test fun roomCreatesDatabaseAndEncryptedTaskSurvivesCloseReopen() = runBlocking {
        val repo = repo(db)
        val task = repo.create(TaskDraft("秘密 日本語 🎯", "line one\nprivate note", TaskPriority.HIGH))
        db.close()
        db = Room.databaseBuilder(context, TasksDatabase::class.java, dbName).allowMainThreadQueries().build()
        val restored = repo(db).get(task.id)
        assertEquals(task, restored)
        assertTrue(db.openHelper.readableDatabase.query("SELECT name FROM sqlite_master WHERE type='table' AND name='tasks'").count > 0)
        val raw = File(context.getDatabasePath(dbName).path).readBytes().toString(Charsets.ISO_8859_1)
        assertFalse(raw.contains("秘密 日本語")); assertFalse(raw.contains("private note"))
        val storedValue = checkNotNull(db.taskDao().row(task.id)).encryptedRecord
        assertFalse(storedValue.contains("秘密 日本語")); assertFalse(storedValue.contains("private note"))
        assertFalse(storedValue.contains("HIGH")); assertFalse(storedValue.contains("TODO"))
    }

    @Test fun ciphertextTamperingAndCiphertextSwapAreRejected() = runBlocking {
        val repository = repo(db)
        val one = repository.create(TaskDraft("alpha")); val two = repository.create(TaskDraft("beta"))
        val firstRow = db.taskDao().row(one.id)!!
        val corrupted = firstRow.encryptedRecord.toCharArray()
        corrupted[corrupted.lastIndex / 2] = if (corrupted[corrupted.lastIndex / 2] == 'A') 'B' else 'A'
        val tampered = firstRow.copy(encryptedRecord = String(corrupted))
        db.taskDao().compareAndSet(one.id, firstRow.revision, firstRow.revision + 1, tampered.encryptedRecord)
        assertThrows(TaskStorageUnavailableException::class.java) { runBlocking { repository.get(one.id) } }
        val secondRow = db.taskDao().row(two.id)!!
        db.taskDao().compareAndSet(two.id, secondRow.revision, secondRow.revision + 1, firstRow.encryptedRecord)
        assertThrows(TaskStorageUnavailableException::class.java) { runBlocking { repository.get(two.id) } }
    }

    @Test fun databaseAndKeyAliasesAreSeparateAndKeyLossPreservesCiphertext() = runBlocking {
        val task = repo(db).create(TaskDraft("retained"))
        val keyStore = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        assertTrue(keyStore.containsAlias(alias))
        assertFalse(keyStore.containsAlias("serea-tasks-v1"))
        assertTrue(context.getDatabasePath(dbName).exists())
        keyStore.deleteEntry(alias)
        assertThrows(TaskStorageUnavailableException::class.java) { runBlocking { repo(db).get(task.id) } }
        assertThrows(TaskStorageUnavailableException::class.java) { runBlocking { repo(db).create(TaskDraft("must not rotate key")) } }
        assertFalse(keyStore.containsAlias(alias))
        assertNotNull(db.taskDao().row(task.id))
    }

    @Test fun concurrentUpdatesUseRevisionCasAndDeleteDoesNotResurrect() = runBlocking {
        val repository = repo(db)
        val otherRepository = repo(db)
        val task = repository.create(TaskDraft("first"))
        val one = async(Dispatchers.IO) { runCatching { repository.update(task.id, task.revision, TaskDraft("second")) } }
        val two = async(Dispatchers.IO) { runCatching { otherRepository.update(task.id, task.revision, TaskDraft("parallel stale")) } }
        val outcomes = listOf(one.await(), two.await())
        assertEquals(1, outcomes.count { it.isSuccess })
        assertEquals(1, outcomes.count { it.exceptionOrNull() is TaskConflictException })
        val current = checkNotNull(repository.get(task.id))
        repository.delete(task.id, current.revision)
        assertNull(repository.get(task.id))
        assertThrows(TaskNotFoundException::class.java) { runBlocking { otherRepository.update(task.id, current.revision, TaskDraft("late")) } }
        assertNull(db.taskDao().row(task.id))
    }

    @Test fun proposalReceiptIsUniqueAndDuplicateConcurrentCreateRollsBack() = runBlocking {
        val dao = db.taskDao()
        val idCounter = java.util.concurrent.atomic.AtomicInteger(1)
        fun proposalRepo() = EncryptedTaskRepository(dao, cipher, { Instant.parse("2026-10-20T10:00:00Z") }) {
            "00000000-0000-0000-0000-${idCounter.getAndIncrement().toString().padStart(12, '0')}"
        }
        val proposalId = "10000000-0000-0000-0000-000000000001"
        val first = async(Dispatchers.IO) { runCatching { proposalRepo().createFromApprovedProposal(TaskDraft("approved once"), proposalId) } }
        val second = async(Dispatchers.IO) { runCatching { proposalRepo().createFromApprovedProposal(TaskDraft("approved once"), proposalId) } }
        val outcomes = listOf(first.await(), second.await())
        assertEquals(1, outcomes.count { it.isSuccess })
        assertEquals(1, outcomes.count { it.exceptionOrNull() is TaskConflictException })
        assertEquals(1, dao.allRows().size)
        assertEquals(1, dao.allRows().count { it.id == dao.receipt(proposalId)?.taskId })
    }

    @Test fun completedStateAndDeletePersistAcrossDatabaseReopen() = runBlocking {
        val repository = repo(db)
        val keep = repository.create(TaskDraft("keep"))
        val remove = repository.create(TaskDraft("remove"))
        val completed = repository.complete(keep.id, keep.revision)
        repository.delete(remove.id, remove.revision)
        db.close()
        db = Room.databaseBuilder(context, TasksDatabase::class.java, dbName).allowMainThreadQueries().build()
        val restoredRepo = repo(db)
        assertEquals(TaskStatus.COMPLETED, restoredRepo.get(keep.id)?.status)
        assertNotNull(restoredRepo.get(keep.id)?.completedAt)
        assertNull(restoredRepo.get(remove.id))
        assertEquals(completed.revision, restoredRepo.get(keep.id)?.revision)
    }

    private fun repo(database: TasksDatabase) = EncryptedTaskRepository(database.taskDao(), cipher, { Instant.parse("2026-10-20T10:00:00Z") })
    private class TestKeystoreCipher(private val alias: String) : TaskCipher {
        private val aes = KeystoreAesGcm(alias)
        private fun aad(id: String) = "test-tasks-v1:$id".toByteArray()
        override fun encrypt(id: String, plaintext: ByteArray) = aes.encrypt(plaintext, aad(id))
        override fun decrypt(id: String, ciphertext: String): ByteArray = try {
            val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            if (!store.containsAlias(alias)) throw TaskStorageUnavailableException("Test encryption key is missing.")
            aes.decrypt(ciphertext, aad(id))
        } catch (error: Exception) {
            if (error is TaskStorageUnavailableException) throw error
            throw TaskStorageUnavailableException("Test task decryption failed.", error)
        }
    }
}
