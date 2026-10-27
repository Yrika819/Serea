package app.serea.android

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.os.Build
import android.util.Base64
import androidx.room.Dao
import androidx.room.ColumnInfo
import androidx.room.Database
import androidx.room.Entity
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.Room
import androidx.room.RoomDatabase
import androidx.room.Transaction
import androidx.room.migration.Migration
import androidx.sqlite.db.SupportSQLiteDatabase
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.SecretKeyFactory
import javax.crypto.spec.GCMParameterSpec
import android.security.keystore.KeyInfo

/** AES-GCM backed by a non-exportable Android Keystore key. Ciphertext includes IV and tag. */
internal class KeystoreAesGcm(private val alias: String) {
    private val key: SecretKey
        get() {
            val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            val existing = store.getKey(alias, null) as? SecretKey
            return existing ?: KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").run {
                init(KeyGenParameterSpec.Builder(alias, KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                    .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                    .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                    .setKeySize(256)
                    .setRandomizedEncryptionRequired(true)
                    .build())
                generateKey()
            }
        }

    fun encrypt(clear: ByteArray, associatedData: ByteArray): String {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key)
        cipher.updateAAD(associatedData)
        val encrypted = cipher.doFinal(clear)
        return Base64.encodeToString(cipher.iv + encrypted, Base64.NO_WRAP)
    }

    fun decrypt(encoded: String, associatedData: ByteArray): ByteArray {
        try {
            val bytes = Base64.decode(encoded, Base64.NO_WRAP)
            require(bytes.size > GCM_IV_BYTES)
            val cipher = Cipher.getInstance("AES/GCM/NoPadding")
            cipher.init(Cipher.DECRYPT_MODE, key, GCMParameterSpec(GCM_TAG_BITS, bytes.copyOfRange(0, GCM_IV_BYTES)))
            cipher.updateAAD(associatedData)
            return cipher.doFinal(bytes.copyOfRange(GCM_IV_BYTES, bytes.size))
        } catch (error: Exception) {
            throw HistoryUnavailableException("Saved history cannot be decrypted on this device.", error)
        }
    }

    @Suppress("DEPRECATION") // Used only on API 26–30; API 31+ reports the exact security level above.
    fun securityProfile(): String {
        return try {
            val secretKey = key
            val info = SecretKeyFactory.getInstance(secretKey.algorithm, "AndroidKeyStore")
                .getKeySpec(secretKey, KeyInfo::class.java) as? KeyInfo
                ?: return "Android Keystore level unavailable"
            if (Build.VERSION.SDK_INT >= 31) when (info.getSecurityLevel()) {
                KeyProperties.SECURITY_LEVEL_STRONGBOX -> "StrongBox"
                KeyProperties.SECURITY_LEVEL_TRUSTED_ENVIRONMENT -> "Hardware-backed TEE"
                KeyProperties.SECURITY_LEVEL_SOFTWARE -> "Software-backed Android Keystore"
                else -> "Android Keystore level unavailable"
            } else if (info.isInsideSecureHardware()) {
                "Hardware-backed Android Keystore"
            } else {
                "Software-backed Android Keystore"
            }
        } catch (_: Exception) {
            // KeyInfo lookup failures are unknown protection, never evidence of hardware backing.
            "Android Keystore level unavailable"
        }
    }

    companion object { private const val GCM_IV_BYTES = 12; private const val GCM_TAG_BITS = 128 }
}

class HistoryUnavailableException(message: String, cause: Throwable? = null) : Exception(message, cause)
class HardwareBackedKeyRequiredException : Exception("Cloud credentials require hardware-backed Android Keystore protection.")

@Entity(tableName = "conversations")
data class ConversationRow(
    @PrimaryKey val id: String,
    val encryptedTitle: String,
    val createdAt: Long,
    val updatedAt: Long,
    val modelId: String,
    val deletedAt: Long? = null,
)

@Entity(tableName = "messages", primaryKeys = ["id"])
data class MessageRow(
    val id: String,
    val conversationId: String,
    val role: String,
    val encryptedBody: String,
    val classification: String,
    val createdAt: Long,
    val status: String,
    val inputTokens: Int? = null,
    val outputTokens: Int? = null,
    @ColumnInfo(defaultValue = "'UNKNOWN'") val origin: String = "UNKNOWN",
    /** Ciphertext containing the immutable message source lineage; UNKNOWN marks pre-lineage rows. */
    @ColumnInfo(defaultValue = "'UNKNOWN'") val encryptedSourceLineage: String = LEGACY_SOURCE_LINEAGE,
)

private const val LEGACY_SOURCE_LINEAGE = "UNKNOWN"

@Dao
interface HistoryDao {
    @Query("SELECT * FROM conversations WHERE deletedAt IS NULL ORDER BY updatedAt DESC") suspend fun conversations(): List<ConversationRow>
    @Query("SELECT * FROM conversations WHERE id = :id AND deletedAt IS NULL") suspend fun conversation(id: String): ConversationRow?
    @Query("SELECT * FROM messages WHERE conversationId = :id ORDER BY createdAt, id") suspend fun messages(id: String): List<MessageRow>
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun putConversation(row: ConversationRow)
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun putMessage(row: MessageRow)
    @Query("UPDATE messages SET status = :status WHERE id = :messageId AND conversationId = :conversationId AND status = 'PENDING'")
    suspend fun transitionPendingMessage(messageId: String, conversationId: String, status: String): Int
    @Query("SELECT status FROM messages WHERE id = :messageId AND conversationId = :conversationId")
    suspend fun messageStatus(messageId: String, conversationId: String): String?
    @Transaction
    suspend fun putMessageIfConversationIsActive(row: MessageRow) {
        val conversation = conversation(row.conversationId)
            ?: throw IllegalArgumentException("Conversation does not exist")
        putMessage(row)
        putConversation(conversation.copy(updatedAt = maxOf(conversation.updatedAt, row.createdAt)))
    }
    @Transaction
    suspend fun createConversationWithFirstMessage(conversation: ConversationRow, message: MessageRow) {
        require(conversation.id == message.conversationId)
        require(message.role == MessageRole.USER.name && message.status == "PENDING")
        putConversation(conversation)
        putMessage(message)
    }
    @Transaction
    suspend fun completePendingTurn(userMessageId: String, assistant: MessageRow) {
        val conversation = conversation(assistant.conversationId)
            ?: throw IllegalArgumentException("Conversation does not exist")
        val userMessage = messages(assistant.conversationId).firstOrNull { it.id == userMessageId }
            ?: throw IllegalArgumentException("Pending user message does not exist")
        require(userMessage.role == MessageRole.USER.name && userMessage.status == "PENDING") {
            "Only a pending user message can be completed"
        }
        require(assistant.role == MessageRole.ASSISTANT.name && assistant.status == "COMPLETED")
        require(assistant.id != userMessageId)
        putMessage(assistant)
        check(transitionPendingMessage(userMessageId, assistant.conversationId, "COMPLETED") == 1) {
            "Pending message state changed before completion"
        }
        putConversation(conversation.copy(updatedAt = maxOf(conversation.updatedAt, assistant.createdAt)))
    }
    @Query("UPDATE conversations SET deletedAt = :at, updatedAt = :at WHERE id = :id") suspend fun deleteConversation(id: String, at: Long)
    @Query("DELETE FROM conversations") suspend fun deleteAllConversations()
    @Query("DELETE FROM messages WHERE conversationId = :id") suspend fun deleteMessages(id: String)
    @Query("DELETE FROM messages") suspend fun deleteAllMessages()
    @Transaction suspend fun deleteConversationAndMessages(id: String, at: Long) {
        deleteConversation(id, at)
        deleteMessages(id)
    }
    @Transaction suspend fun deleteAllHistory() {
        deleteAllMessages()
        deleteAllConversations()
    }
}

@Database(entities = [ConversationRow::class, MessageRow::class, UsageWindowRow::class, UsageReservationRow::class, UsageSettingsRow::class, ModelUsageLedgerRow::class], version = 5, exportSchema = true)
abstract class SereaHistoryDatabase : RoomDatabase() {
    abstract fun historyDao(): HistoryDao
    abstract fun usageDao(): UsageDao

    companion object {
        @Volatile private var instance: SereaHistoryDatabase? = null
        fun open(context: Context): SereaHistoryDatabase = instance ?: synchronized(this) {
            instance ?: Room.databaseBuilder(context.applicationContext, SereaHistoryDatabase::class.java, "serea-history.db")
                .addMigrations(MIGRATION_1_2, MIGRATION_2_3, MIGRATION_3_4, MIGRATION_4_5)
                .build().also { instance = it }
        }

        internal val MIGRATION_1_2 = object : Migration(1, 2) {
            override fun migrate(database: SupportSQLiteDatabase) {
                database.execSQL("CREATE TABLE IF NOT EXISTS usage_windows (windowKey TEXT NOT NULL, requests INTEGER NOT NULL, inputTokens INTEGER NOT NULL, outputTokens INTEGER NOT NULL, PRIMARY KEY(windowKey))")
                database.execSQL("CREATE TABLE IF NOT EXISTS usage_reservations (requestId TEXT NOT NULL, dayKey TEXT NOT NULL, monthKey TEXT NOT NULL, inputTokens INTEGER NOT NULL, outputTokens INTEGER NOT NULL, state TEXT NOT NULL, PRIMARY KEY(requestId))")
                database.execSQL("CREATE TABLE IF NOT EXISTS usage_settings (id INTEGER NOT NULL, dailyRequests INTEGER NOT NULL, dailyTokens INTEGER NOT NULL, monthlyRequests INTEGER NOT NULL, monthlyTokens INTEGER NOT NULL, lastObservedAt INTEGER NOT NULL, PRIMARY KEY(id))")
            }
        }
        internal val MIGRATION_2_3 = object : Migration(2, 3) {
            override fun migrate(database: SupportSQLiteDatabase) {
                database.execSQL("ALTER TABLE messages ADD COLUMN origin TEXT NOT NULL DEFAULT 'UNKNOWN'")
            }
        }
        internal val MIGRATION_3_4 = object : Migration(3, 4) {
            override fun migrate(database: SupportSQLiteDatabase) {
                database.execSQL("CREATE TABLE IF NOT EXISTS model_usage_ledger (requestId TEXT NOT NULL, modelId TEXT NOT NULL, providerId TEXT NOT NULL, createdAt INTEGER NOT NULL, dayKey TEXT NOT NULL, monthKey TEXT NOT NULL, estimatedInputTokens INTEGER NOT NULL, reservedOutputTokens INTEGER NOT NULL, providerInputTokens INTEGER, providerOutputTokens INTEGER, usageStatus TEXT NOT NULL, costStatus TEXT NOT NULL, estimatedCost TEXT, currency TEXT, pricingSource TEXT, pricingVerifiedAt TEXT, inputPricePerMillion TEXT, outputPricePerMillion TEXT, pricingEffectiveAt TEXT, pricingRevision TEXT, reservationState TEXT NOT NULL, settledAt INTEGER, uncertainOutcomeReason TEXT, modelAttributionStatus TEXT NOT NULL, PRIMARY KEY(requestId), FOREIGN KEY(requestId) REFERENCES usage_reservations(requestId) ON UPDATE NO ACTION ON DELETE CASCADE)")
                database.execSQL("CREATE INDEX IF NOT EXISTS index_model_usage_ledger_dayKey_providerId_modelId ON model_usage_ledger(dayKey, providerId, modelId)")
                database.execSQL("CREATE INDEX IF NOT EXISTS index_model_usage_ledger_monthKey_providerId_modelId ON model_usage_ledger(monthKey, providerId, modelId)")
                database.execSQL("INSERT INTO model_usage_ledger (requestId, modelId, providerId, createdAt, dayKey, monthKey, estimatedInputTokens, reservedOutputTokens, providerInputTokens, providerOutputTokens, usageStatus, costStatus, estimatedCost, currency, pricingSource, pricingVerifiedAt, inputPricePerMillion, outputPricePerMillion, pricingEffectiveAt, pricingRevision, reservationState, settledAt, uncertainOutcomeReason, modelAttributionStatus) SELECT requestId, 'UNKNOWN', 'UNKNOWN', 0, dayKey, monthKey, CASE WHEN state = 'SETTLED' THEN 0 ELSE inputTokens END, CASE WHEN state = 'SETTLED' THEN 0 ELSE outputTokens END, CASE WHEN state = 'SETTLED' THEN inputTokens ELSE NULL END, CASE WHEN state = 'SETTLED' THEN outputTokens ELSE NULL END, state, 'UNKNOWN', NULL, NULL, NULL, NULL, NULL, NULL, NULL, NULL, state, NULL, CASE WHEN state IN ('OUTCOME_UNKNOWN', 'USAGE_UNKNOWN') THEN 'Migrated pre-ledger uncertain reservation.' ELSE NULL END, 'UNKNOWN' FROM usage_reservations")
            }
        }
        internal val MIGRATION_4_5 = object : Migration(4, 5) {
            override fun migrate(database: SupportSQLiteDatabase) {
                // Legacy rows have no authenticated lineage. The sentinel is interpreted through
                // defaultSourceLineage(origin), which treats old MODEL_OUTPUT as UNKNOWN ancestry.
                database.execSQL("ALTER TABLE messages ADD COLUMN encryptedSourceLineage TEXT NOT NULL DEFAULT '$LEGACY_SOURCE_LINEAGE'")
            }
        }
    }
}

data class StoredMessage(
    val id: String, val conversationId: String, val role: MessageRole, val text: String,
    val classification: DataClassification, val createdAt: Long, val status: String,
    val origin: DataOrigin = DataOrigin.UNKNOWN,
    val sourceLineage: Set<DataOrigin> = defaultSourceLineage(origin),
)
data class StoredConversation(val id: String, val title: String, val createdAt: Long, val updatedAt: Long, val modelId: String, val messages: List<StoredMessage>)

internal interface ConversationHistoryPort {
    suspend fun list(): List<StoredConversation>
    suspend fun get(id: String): StoredConversation?
    suspend fun addMessage(message: StoredMessage)
    suspend fun createWithFirstMessage(id: String, title: String, message: StoredMessage, modelId: String = "demo-local")
    suspend fun completeTurn(userMessageId: String, assistant: StoredMessage)
    suspend fun updateStatus(messageId: String, conversationId: String, status: String): Boolean
    suspend fun delete(id: String, now: Long = System.currentTimeMillis())
    suspend fun deleteAll()
}

internal class ConversationRepository(private val dao: HistoryDao, private val cipher: KeystoreAesGcm) : ConversationHistoryPort {
    suspend fun create(id: String, title: String, modelId: String = "demo-local", now: Long = System.currentTimeMillis()) {
        dao.putConversation(ConversationRow(id, encrypt(title, "conversation:$id:title"), now, now, modelId))
    }
    override suspend fun list(): List<StoredConversation> = dao.conversations().map { load(it) }
    override suspend fun get(id: String): StoredConversation? = dao.conversation(id)?.let { load(it) }
    override suspend fun addMessage(message: StoredMessage) {
        dao.putMessageIfConversationIsActive(MessageRow(message.id, message.conversationId, message.role.name,
            encrypt(message.text, "message:${message.id}:body"), message.classification.name,
            message.createdAt, message.status, origin = message.origin.name,
            encryptedSourceLineage = encrypt(encodeSourceLineage(message.sourceLineage), "message:${message.id}:lineage")))
    }
    override suspend fun createWithFirstMessage(id: String, title: String, message: StoredMessage, modelId: String) {
        require(message.conversationId == id && message.role == MessageRole.USER && message.status == "PENDING")
        val timestamp = message.createdAt
        dao.createConversationWithFirstMessage(
            ConversationRow(id, encrypt(title, "conversation:$id:title"), timestamp, timestamp, modelId),
            MessageRow(message.id, id, message.role.name, encrypt(message.text, "message:${message.id}:body"),
                message.classification.name, timestamp, message.status, origin = message.origin.name,
                encryptedSourceLineage = encrypt(encodeSourceLineage(message.sourceLineage), "message:${message.id}:lineage")),
        )
    }
    override suspend fun completeTurn(userMessageId: String, assistant: StoredMessage) {
        require(assistant.role == MessageRole.ASSISTANT && assistant.status == "COMPLETED")
        dao.completePendingTurn(userMessageId, MessageRow(
            assistant.id, assistant.conversationId, assistant.role.name,
            encrypt(assistant.text, "message:${assistant.id}:body"), assistant.classification.name,
            assistant.createdAt, assistant.status, origin = assistant.origin.name,
            encryptedSourceLineage = encrypt(encodeSourceLineage(assistant.sourceLineage), "message:${assistant.id}:lineage"),
        ))
    }
    override suspend fun updateStatus(messageId: String, conversationId: String, status: String): Boolean {
        require(status in setOf("COMPLETED", "FAILED", "OUTCOME_UNKNOWN"))
        val updated = dao.transitionPendingMessage(messageId, conversationId, status)
        return updated == 1 || dao.messageStatus(messageId, conversationId) == status
    }
    override suspend fun delete(id: String, now: Long) {
        dao.deleteConversationAndMessages(id, now)
    }
    override suspend fun deleteAll() { dao.deleteAllHistory() }
    private suspend fun load(row: ConversationRow): StoredConversation = StoredConversation(
        row.id, decrypt(row.encryptedTitle, "conversation:${row.id}:title"), row.createdAt, row.updatedAt, row.modelId,
        dao.messages(row.id).map { message ->
            try {
                val origin = DataOrigin.valueOf(message.origin)
                StoredMessage(message.id, message.conversationId, MessageRole.valueOf(message.role),
                    decrypt(message.encryptedBody, "message:${message.id}:body"),
                    DataClassification.valueOf(message.classification), message.createdAt, message.status,
                    origin, decryptSourceLineage(message))
            } catch (error: Exception) {
                if (error is HistoryUnavailableException) throw error
                throw HistoryUnavailableException("Saved history contains an invalid record.", error)
            }
        },
    )
    private fun decryptSourceLineage(row: MessageRow): Set<DataOrigin> {
        if (row.encryptedSourceLineage == LEGACY_SOURCE_LINEAGE) return setOf(DataOrigin.UNKNOWN)
        return try {
            decodeSourceLineage(decrypt(row.encryptedSourceLineage, "message:${row.id}:lineage"))
        } catch (_: Exception) {
            // Corrupt/unreadable trust metadata must not be replaced with an allow-capable default.
            setOf(DataOrigin.UNKNOWN)
        }
    }

    private fun encodeSourceLineage(lineage: Set<DataOrigin>): String =
        lineage.toSet().sortedBy { it.ordinal }.joinToString(",") { it.name }.ifBlank { DataOrigin.UNKNOWN.name }

    private fun decodeSourceLineage(encoded: String): Set<DataOrigin> {
        val names = encoded.split(',')
        if (names.isEmpty() || names.any(String::isBlank)) return setOf(DataOrigin.UNKNOWN)
        val values = names.map { name ->
            runCatching { DataOrigin.valueOf(name) }.getOrElse { return setOf(DataOrigin.UNKNOWN) }
        }.toSet()
        return values.takeIf { it.isNotEmpty() } ?: setOf(DataOrigin.UNKNOWN)
    }
    private fun encrypt(value: String, aad: String) = cipher.encrypt(value.toByteArray(Charsets.UTF_8), aad.toByteArray(Charsets.UTF_8))
    private fun decrypt(value: String, aad: String): String = cipher.decrypt(value, aad.toByteArray(Charsets.UTF_8)).toString(Charsets.UTF_8)
}

/** API credential ciphertext stays in app-private SharedPreferences; only opaque handles leave this class. */
internal class CredentialRepository(context: Context) : CredentialPort, CredentialReader {
    private val prefs = context.applicationContext.getSharedPreferences("cloud-credential", Context.MODE_PRIVATE)
    private val cipher = KeystoreAesGcm(CREDENTIAL_KEY_ALIAS)

    override suspend fun store(secret: CharArray): CredentialHandle {
        var encoded: java.nio.ByteBuffer? = null
        var bytes: ByteArray? = null
        try {
            require(secret.isNotEmpty() && secret.size <= 512)
            encoded = Charsets.UTF_8.newEncoder().encode(java.nio.CharBuffer.wrap(secret))
            val clearBytes = ByteArray(encoded.remaining()).also(encoded::get)
            bytes = clearBytes
            requireHardwareBackedKey()
            val encrypted = cipher.encrypt(clearBytes, CREDENTIAL_AAD)
            val handle = CredentialHandle(java.util.UUID.randomUUID().toString())
            check(prefs.edit().putString(KEY, encrypted).putString(HANDLE_KEY, handle.value).commit())
            return handle
        } finally {
            bytes?.fill(0)
            encoded?.takeIf { it.hasArray() }?.array()?.fill(0)
            secret.fill('\u0000')
        }
    }
    override suspend fun delete(handle: CredentialHandle) {
        require(prefs.getString(HANDLE_KEY, null) == handle.value) { "credential reference is stale" }
        check(prefs.edit().remove(KEY).remove(HANDLE_KEY).commit())
        KeyStore.getInstance("AndroidKeyStore").apply { load(null); deleteEntry(CREDENTIAL_KEY_ALIAS) }
    }
    fun isConfigured(): Boolean = prefs.contains(KEY) && prefs.contains(HANDLE_KEY)
    fun currentHandle(): CredentialHandle? = prefs.getString(HANDLE_KEY, null)?.let(::CredentialHandle)
    fun securityProfile(): String = cipher.securityProfile()
    fun isHardwareBacked(): Boolean = securityProfile() in setOf(
        "StrongBox", "Hardware-backed TEE", "Hardware-backed Android Keystore",
    )

    override fun readForRequest(): String {
        val encrypted = prefs.getString(KEY, null) ?: throw IllegalStateException("credential missing")
        try { requireHardwareBackedKey() } catch (error: HardwareBackedKeyRequiredException) {
            throw HistoryUnavailableException("Cloud credentials require hardware-backed Android Keystore protection.", error)
        }
        val bytes = cipher.decrypt(encrypted, CREDENTIAL_AAD)
        return try { bytes.toString(Charsets.UTF_8) } finally { bytes.fill(0) }
    }

    private fun requireHardwareBackedKey() {
        if (!isHardwareBacked()) throw HardwareBackedKeyRequiredException()
    }

    companion object {
        private const val KEY = "encrypted_api_key"
        private const val HANDLE_KEY = "credential_handle"
        private const val CREDENTIAL_KEY_ALIAS = "serea-cloud-credential-v1"
        private val CREDENTIAL_AAD = "serea:ollama-cloud:credential:v1".toByteArray()
    }
}
