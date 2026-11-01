package app.serea.android.tasks

import app.serea.android.KeystoreAesGcm
import java.io.ByteArrayInputStream
import java.io.ByteArrayOutputStream
import java.io.DataInputStream
import java.io.DataOutputStream
import java.nio.charset.CodingErrorAction
import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import java.security.KeyStore

internal interface TaskCipher {
    fun encrypt(id: String, plaintext: ByteArray): String
    fun decrypt(id: String, ciphertext: String): ByteArray
}

internal class KeystoreTaskCipher : TaskCipher {
    private val alias = "serea-tasks-v1"
    private val aes = KeystoreAesGcm(alias)
    override fun encrypt(id: String, plaintext: ByteArray): String = aes.encrypt(plaintext, aad(id))
    override fun decrypt(id: String, ciphertext: String): ByteArray = try {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        if (!store.containsAlias(alias)) throw TaskStorageUnavailableException("Saved tasks are unavailable because their encryption key is missing.")
        aes.decrypt(ciphertext, aad(id))
    } catch (error: Exception) {
        if (error is TaskStorageUnavailableException) throw error
        throw TaskStorageUnavailableException("Saved tasks are unavailable because their key or encrypted data is invalid.", error)
    }
    private fun aad(id: String) = "serea:tasks:v1:record:$id".toByteArray(Charsets.UTF_8)
}

/** Versioned bounded binary payload. Every user and task metadata field stays encrypted in SQLite. */
internal object TaskPayloadCodec {
    private const val VERSION = 1
    private const val MAX_FIELD_BYTES = 100_000

    fun encode(task: Task): ByteArray {
        TaskValidation.validate(task)
        val bytes = ByteArrayOutputStream()
        DataOutputStream(bytes).use { out ->
            out.writeInt(VERSION)
            out.writeUtf8(task.title); out.writeUtf8(task.description)
            out.writeUtf8(task.status.name); out.writeUtf8(task.priority.name)
            out.writeLong(task.createdAt.toEpochMilli()); out.writeLong(task.updatedAt.toEpochMilli())
            out.writeNullable(task.dueDate?.toString()); out.writeNullable(task.dueTime?.toString())
            out.writeNullable(task.timeZoneId); out.writeNullable(task.completedAt?.toEpochMilli()?.toString())
        }
        return bytes.toByteArray()
    }

    fun decode(id: String, revision: Long, bytes: ByteArray): Task = try {
        DataInputStream(ByteArrayInputStream(bytes)).use { input ->
            require(input.readInt() == VERSION)
            val title = input.readUtf8(); val description = input.readUtf8()
            val status = TaskStatus.valueOf(input.readUtf8()); val priority = TaskPriority.valueOf(input.readUtf8())
            val created = Instant.ofEpochMilli(input.readLong()); val updated = Instant.ofEpochMilli(input.readLong())
            val date = input.readNullable()?.let(LocalDate::parse); val time = input.readNullable()?.let(LocalTime::parse)
            val zone = input.readNullable(); val completed = input.readNullable()?.toLong()?.let(Instant::ofEpochMilli)
            require(input.available() == 0)
            Task(id, title, description, status, priority, created, updated, date, time, zone, completed, revision)
                .also(TaskValidation::validate)
        }
    } catch (error: Exception) {
        if (error is TaskStorageUnavailableException) throw error
        throw TaskStorageUnavailableException("Saved tasks contain an invalid encrypted record.", error)
    }

    private fun DataOutputStream.writeUtf8(value: String) {
        val encoded = value.toByteArray(Charsets.UTF_8); require(encoded.size <= MAX_FIELD_BYTES)
        writeInt(encoded.size); write(encoded)
    }
    private fun DataOutputStream.writeNullable(value: String?) { writeBoolean(value != null); if (value != null) writeUtf8(value) }
    private fun DataInputStream.readUtf8(): String {
        val size = readInt(); require(size in 0..MAX_FIELD_BYTES && size <= available())
        val raw = ByteArray(size); readFully(raw)
        return Charsets.UTF_8.newDecoder().onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT).decode(java.nio.ByteBuffer.wrap(raw)).toString()
    }
    private fun DataInputStream.readNullable(): String? = if (readBoolean()) readUtf8() else null
}
