package app.serea.android

import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import android.content.Context
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.CompletableDeferred
import org.junit.After
import org.junit.Before
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.UUID
import java.security.KeyStore
import java.time.Instant

@RunWith(AndroidJUnit4::class)
class EncryptedHistoryTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private lateinit var database: SereaHistoryDatabase
    private lateinit var repository: ConversationRepository

    @Before fun setUp() {
        database = Room.inMemoryDatabaseBuilder(context, SereaHistoryDatabase::class.java).build()
        repository = ConversationRepository(database.historyDao(), KeystoreAesGcm("serea-test-${UUID.randomUUID()}"))
    }

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
        Unit
    }

    @Test fun oneHundredMessagesKeepOrderAfterDatabaseReopen() = runBlocking {
        val name = "history-hundred-${UUID.randomUUID()}.db"
        val alias = "history-hundred-${UUID.randomUUID()}"
        val firstDb = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, name).build()
        val first = ConversationRepository(firstDb.historyDao(), KeystoreAesGcm(alias))
        first.create("conversation", "Bulk history", now = 1)
        (1..100).forEach { index ->
            first.addMessage(StoredMessage(
                "message-${index.toString().padStart(3, '0')}", "conversation", MessageRole.USER,
                "body-$index", DataClassification.UNKNOWN, index.toLong() + 1, "COMPLETED",
            ))
        }
        firstDb.close()

        val reopened = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, name).build()
        val restored = ConversationRepository(reopened.historyDao(), KeystoreAesGcm(alias)).get("conversation")!!
        assertEquals(100, restored.messages.size)
        assertEquals((1..100).map { "message-${it.toString().padStart(3, '0')}" }, restored.messages.map { it.id })
        reopened.close()
        context.deleteDatabase(name)
        KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry(alias) }
        Unit
    }

    @Test fun oneHundredConversationCreateReadAndDeleteStress() = runBlocking {
        (0 until 100).forEach { index ->
            val id = "stress-conversation-${index.toString().padStart(3, '0')}"
            repository.create(id, "Stress conversation $index", now = index.toLong() + 1)
            repository.addMessage(StoredMessage(
                "stress-message-$index", id, MessageRole.USER,
                if (index == 0) "long-${"x".repeat(10_000)}" else "message-$index",
                DataClassification.UNKNOWN, index.toLong() + 2, "COMPLETED",
            ))
        }
        assertEquals(100, repository.list().size)
        assertEquals("long-${"x".repeat(10_000)}", repository.get("stress-conversation-000")!!.messages.single().text)
        (0 until 100 step 2).forEach { index -> repository.delete("stress-conversation-${index.toString().padStart(3, '0')}") }
        val remaining = repository.list()
        assertEquals(50, remaining.size)
        assertTrue(remaining.all { it.id.endsWith("1") || it.id.endsWith("3") || it.id.endsWith("5") || it.id.endsWith("7") || it.id.endsWith("9") })
        assertTrue((0 until 100 step 2).all { index -> repository.get("stress-conversation-${index.toString().padStart(3, '0')}") == null })
    }

    @Test fun assistantAndCompletedStatusCommitTogetherAndCannotCompleteTwice() = runBlocking {
        repository.create("atomic-turn", "Atomic turn", now = 1)
        repository.addMessage(StoredMessage(
            "atomic-user", "atomic-turn", MessageRole.USER, "question", DataClassification.UNKNOWN, 2, "PENDING",
        ))
        repository.completeTurn("atomic-user", StoredMessage(
            "atomic-assistant", "atomic-turn", MessageRole.ASSISTANT, "answer", DataClassification.UNKNOWN, 3, "COMPLETED",
        ))
        val completed = repository.get("atomic-turn")!!.messages
        assertEquals(listOf("COMPLETED", "COMPLETED"), completed.map { it.status })
        assertEquals(listOf(MessageRole.USER, MessageRole.ASSISTANT), completed.map { it.role })
        assertThrows<IllegalArgumentException> {
            kotlinx.coroutines.runBlocking {
                repository.completeTurn("atomic-user", StoredMessage(
                    "duplicate-assistant", "atomic-turn", MessageRole.ASSISTANT, "duplicate", DataClassification.UNKNOWN, 4, "COMPLETED",
                ))
            }
        }
        assertEquals(listOf("atomic-user", "atomic-assistant"), repository.get("atomic-turn")!!.messages.map { it.id })
    }

    @Test fun lateInsertRacingWithDeleteCannotReappearWhenConversationIdIsReused() = runBlocking {
        repository.create("racing-delete", "Race", now = 1)
        val start = CompletableDeferred<Unit>()
        val lateWrite = async(Dispatchers.IO) {
            start.await()
            runCatching {
                repository.addMessage(StoredMessage(
                    "late-response", "racing-delete", MessageRole.ASSISTANT, "late body",
                    DataClassification.UNKNOWN, 2, "COMPLETED",
                ))
            }
        }
        val deletion = async(Dispatchers.IO) { start.await(); repository.delete("racing-delete", now = 3) }
        start.complete(Unit)
        val insertResult = lateWrite.await()
        val insertFailure = insertResult.exceptionOrNull()
        if (insertFailure != null) assertTrue(insertFailure is IllegalArgumentException)
        deletion.await()
        assertNull(repository.get("racing-delete"))

        repository.create("racing-delete", "Reused id", now = 4)
        assertTrue(repository.get("racing-delete")!!.messages.isEmpty())
        assertTrue(database.historyDao().messages("racing-delete").isEmpty())
    }

    @Test fun authenticationTagRejectsTamperingAndWrongAssociatedData() {
        val cipher = KeystoreAesGcm("serea-test-aad-${UUID.randomUUID()}")
        val sealed = cipher.encrypt("secret body".toByteArray(), "message:id".toByteArray())
        assertEquals("secret body", cipher.decrypt(sealed, "message:id".toByteArray()).toString(Charsets.UTF_8))
        val tampered = sealed.dropLast(2) + if (sealed.last() == 'A') "B=" else "A="
        assertThrows<HistoryUnavailableException> { cipher.decrypt(tampered, "message:id".toByteArray()) }
        assertThrows<HistoryUnavailableException> { cipher.decrypt(sealed, "message:other".toByteArray()) }
    }

    @Test fun aesKeyInfoUsesAndroidKeystoreSecretKeyFactoryAndFailsClosed() {
        val cipher = KeystoreAesGcm("serea-key-info-${UUID.randomUUID()}")
        val profile = cipher.securityProfile()
        val supportedProfiles = if (android.os.Build.VERSION.SDK_INT >= 31) setOf(
            "StrongBox", "Hardware-backed TEE", "Software-backed Android Keystore",
        ) else setOf("Hardware-backed Android Keystore", "Software-backed Android Keystore")
        assertTrue("AES KeyInfo lookup returned no supported security level: $profile", profile in supportedProfiles)
        val hardwareBacked = profile in setOf("StrongBox", "Hardware-backed TEE", "Hardware-backed Android Keystore")
        assertEquals(hardwareBacked, CredentialRepository(context).isHardwareBacked())
        val sealed = cipher.encrypt("key-info-check".toByteArray(), "aad".toByteArray())
        assertEquals("key-info-check", cipher.decrypt(sealed, "aad".toByteArray()).toString(Charsets.UTF_8))
    }

    @Test fun credentialsAreEncryptedReplaceableAndDeleted() = runBlocking {
        val credentials = CredentialRepository(context)
        credentials.currentHandle()?.let { credentials.delete(it) }
        if (!credentials.isHardwareBacked()) {
            val rejected = "test-key-never-a-real-secret".toCharArray()
            assertThrows<HardwareBackedKeyRequiredException> { runBlocking { credentials.store(rejected) } }
            assertTrue(rejected.all { it == '\u0000' })
            assertFalse(credentials.isConfigured())
            KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry("serea-cloud-credential-v1") }
            return@runBlocking
        }
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
        if (credentials.isHardwareBacked()) {
            credentials.store("temporary-test-key".toCharArray())
            KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry("serea-cloud-credential-v1") }
            assertThrows<HistoryUnavailableException> { credentials.readForRequest() }
            assertTrue(credentials.isConfigured())
            credentials.currentHandle()?.let { credentials.delete(it) }
        } else {
            val candidate = "temporary-test-key".toCharArray()
            assertThrows<HardwareBackedKeyRequiredException> { runBlocking { credentials.store(candidate) } }
            assertTrue(candidate.all { it == '\u0000' })
            assertFalse(credentials.isConfigured())
            KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry("serea-cloud-credential-v1") }
        }
        Unit
    }

    @Test fun invalidCredentialInputIsClearedWithoutWritingPreferences() = runBlocking {
        val credentials = CredentialRepository(context)
        credentials.currentHandle()?.let { credentials.delete(it) }
        val oversized = CharArray(513) { 'x' }
        assertThrows<IllegalArgumentException> { runBlocking { credentials.store(oversized) } }
        assertTrue(oversized.all { it == '\u0000' })
        assertFalse(credentials.isConfigured())
    }

    @Test fun applicationDisablesBackupAndCleartextTraffic() {
        val flags = context.packageManager.getApplicationInfo(context.packageName, 0).flags
        assertEquals(0, flags and android.content.pm.ApplicationInfo.FLAG_ALLOW_BACKUP)
        assertEquals(0, flags and android.content.pm.ApplicationInfo.FLAG_USES_CLEARTEXT_TRAFFIC)
    }

    @Test fun softLimitReservationsAreAtomicAndSettingsSurviveRepositoryRecreation() = runBlocking {
        var now = Instant.parse("2026-10-19T23:59:00Z").toEpochMilli()
        val first = UsageRepository(database.usageDao()) { now }
        val limits = UsageLimits(dailyRequests = 1, dailyTokens = 600, monthlyRequests = 3, monthlyTokens = 1_800)
        first.updateLimits(limits)
        val outcomes = (0 until 8).map { index -> async(Dispatchers.IO) {
            runCatching { first.reserve("concurrent-$index", 12) }
        } }.awaitAll()
        assertEquals(1, outcomes.count { it.isSuccess })
        assertEquals(7, outcomes.count { it.exceptionOrNull() is UsageLimitExceededException })

        val successfulId = outcomes.indexOfFirst { it.isSuccess }.let { "concurrent-$it" }
        first.complete(successfulId, ProviderUsage(5, 2))
        val restored = UsageRepository(database.usageDao()) { now }
        val beforeBoundary = restored.snapshot()
        assertEquals(limits, beforeBoundary.limits)
        assertEquals(1L, beforeBoundary.dailyRequests)
        assertEquals(0L, beforeBoundary.dailyEstimatedInputTokens + beforeBoundary.dailyEstimatedOutputTokens)
        assertEquals(7L, beforeBoundary.dailyProviderInputTokens + beforeBoundary.dailyProviderOutputTokens)

        now = Instant.parse("2026-10-20T00:00:00Z").toEpochMilli()
        val nextDay = restored.snapshot()
        assertEquals(0L, nextDay.dailyRequests)
        assertEquals(1L, nextDay.monthlyRequests)
        now = Instant.parse("2026-11-01T00:00:00Z").toEpochMilli()
        val nextMonth = restored.snapshot()
        assertEquals(0L, nextMonth.monthlyRequests)
        now = Instant.parse("2026-10-18T00:00:00Z").toEpochMilli()
        assertEquals(0L, restored.snapshot().monthlyRequests)
    }

    @Test fun ambiguousAndMissingUsageKeepReservedBudgetAndAreMarkedUnknown() = runBlocking {
        val usage = UsageRepository(database.usageDao()) { Instant.parse("2026-10-19T12:00:00Z").toEpochMilli() }
        usage.reserve("ambiguous", promptChars = 300, maxOutputTokens = 64)
        usage.complete("ambiguous", providerUsage = null)
        usage.recoverAfterProcessRestart()
        val snapshot = usage.snapshot()
        assertEquals(1L, snapshot.dailyRequests)
        assertEquals(1_264L, snapshot.dailyEstimatedInputTokens + snapshot.dailyEstimatedOutputTokens)
        assertEquals(1L, snapshot.uncertainRequests)
    }

    @Test fun usageSettingsAndUncertainReservationsRemainInRoomAfterDatabaseReopen() = runBlocking {
        val name = "usage-reopen-${UUID.randomUUID()}.db"
        val limits = UsageLimits(dailyRequests = 2, dailyTokens = 900, monthlyRequests = 8, monthlyTokens = 3_600)
        val now = Instant.parse("2026-10-19T12:00:00Z").toEpochMilli()
        val originalDb = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, name).build()
        val original = UsageRepository(originalDb.usageDao()) { now }
        original.updateLimits(limits)
        original.reserve("pending-before-death", 30, 40)
        originalDb.close()

        val reopenedDb = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, name).build()
        val reopened = UsageRepository(reopenedDb.usageDao()) { now }
        reopened.recoverAfterProcessRestart()
        val snapshot = reopened.snapshot()
        assertEquals(limits, snapshot.limits)
        assertEquals(1L, snapshot.dailyRequests)
        assertEquals(160L, snapshot.dailyEstimatedInputTokens + snapshot.dailyEstimatedOutputTokens)
        assertEquals(1L, snapshot.uncertainRequests)
        reopenedDb.close()
        context.deleteDatabase(name)
    }

    @Test fun cancelledProviderAttemptRetainsReservationWithoutUsageRefund() = runBlocking {
        val usage = UsageRepository(database.usageDao()) { Instant.parse("2026-10-19T12:00:00Z").toEpochMilli() }
        val port = UsageControlledModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse =
                throw kotlinx.coroutines.CancellationException("test cancellation")
        }, usage)
        try {
            port.send(ModelChatRequest("conversation", "question", sendId = "cancelled-send"))
            throw AssertionError("expected cancellation")
        } catch (_: kotlinx.coroutines.CancellationException) { }
        val snapshot = usage.snapshot()
        assertEquals(1L, snapshot.dailyRequests)
        assertEquals(544L, snapshot.dailyEstimatedInputTokens + snapshot.dailyEstimatedOutputTokens)
        assertEquals(1L, snapshot.uncertainRequests)
    }

    private inline fun <reified T : Throwable> assertThrows(block: () -> Unit) {
        try { block() } catch (error: Throwable) { if (error is T) return else throw error }
        throw AssertionError("Expected ${T::class.java.simpleName}")
    }
}
