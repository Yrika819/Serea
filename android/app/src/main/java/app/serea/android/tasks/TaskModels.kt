package app.serea.android.tasks

import java.time.DateTimeException
import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import java.time.ZoneId
import java.util.UUID

/** UI- and provider-independent task domain. All-day deadlines are represented by date with no time/zone. */
data class Task(
    val id: String,
    val title: String,
    val description: String,
    val status: TaskStatus,
    val priority: TaskPriority,
    val createdAt: Instant,
    val updatedAt: Instant,
    val dueDate: LocalDate? = null,
    val dueTime: LocalTime? = null,
    val timeZoneId: String? = null,
    val completedAt: Instant? = null,
    val revision: Long = 1,
) {
    val isOverdue: Boolean get() = isOverdue(Instant.now(), ZoneId.systemDefault())
    fun isOverdue(now: Instant, fallbackZone: ZoneId = ZoneId.systemDefault()): Boolean {
        if (status != TaskStatus.TODO) return false
        val date = dueDate ?: return false
        val time = dueTime
        if (time == null) return date.isBefore(now.atZone(timeZoneId?.let(ZoneId::of) ?: fallbackZone).toLocalDate())
        return dueInstantOrNull()?.let { it < now } ?: false
    }

    /** Resolves timed deadlines only when the local time maps to exactly one instant (DST gaps/overlaps are rejected). */
    fun dueInstantOrNull(): Instant? {
        val date = dueDate ?: return null
        val time = dueTime ?: return null
        val zone = timeZoneId?.let(ZoneId::of) ?: return null
        val local = date.atTime(time)
        val offsets = zone.rules.getValidOffsets(local)
        require(offsets.size == 1) { "Due time is ambiguous or does not exist in this timezone." }
        return local.toInstant(offsets.single())
    }
}

enum class TaskStatus { TODO, COMPLETED }
enum class TaskPriority { LOW, NORMAL, HIGH }

data class TaskDraft(
    val title: String,
    val description: String = "",
    val priority: TaskPriority = TaskPriority.NORMAL,
    val dueDate: LocalDate? = null,
    val dueTime: LocalTime? = null,
    val timeZoneId: String? = null,
)

class InvalidTaskException(message: String) : IllegalArgumentException(message)
class TaskNotFoundException : Exception("Task no longer exists.")
class TaskConflictException : Exception("Task changed elsewhere. Reload it before saving.")
class TaskStorageUnavailableException(message: String, cause: Throwable? = null) : Exception(message, cause)

internal object TaskValidation {
    const val MAX_TITLE_CODE_POINTS = 200
    const val MAX_DESCRIPTION_CODE_POINTS = 10_000

    fun validateId(id: String) {
        val parsed = try { UUID.fromString(id) } catch (_: Exception) { throw InvalidTaskException("Invalid task ID.") }
        if (parsed.toString() != id.lowercase()) throw InvalidTaskException("Invalid task ID.")
    }

    fun validateDraft(draft: TaskDraft) {
        if (!wellFormedUtf16(draft.title) || draft.title.isBlank()) throw InvalidTaskException("Title is required.")
        if (draft.title.codePointCount(0, draft.title.length) > MAX_TITLE_CODE_POINTS) throw InvalidTaskException("Title is too long.")
        if (!wellFormedUtf16(draft.description)) throw InvalidTaskException("Description contains invalid text.")
        if (draft.description.codePointCount(0, draft.description.length) > MAX_DESCRIPTION_CODE_POINTS) throw InvalidTaskException("Description is too long.")
        if (draft.dueTime != null && draft.dueDate == null) throw InvalidTaskException("A due time requires a due date.")
        val zone = draft.timeZoneId?.let(::parseIanaZone)
        if (draft.dueDate == null && zone != null) throw InvalidTaskException("A timezone requires a due date.")
        if (draft.dueTime != null) {
            val timedZone = zone ?: throw InvalidTaskException("A timezone is required for a due time.")
            val offsets = timedZone.rules.getValidOffsets(draft.dueDate!!.atTime(draft.dueTime))
            if (offsets.size != 1) throw InvalidTaskException("Due time is ambiguous or does not exist in this timezone.")
        }
    }

    fun validate(task: Task) {
        validateId(task.id)
        validateDraft(TaskDraft(task.title, task.description, task.priority, task.dueDate, task.dueTime, task.timeZoneId))
        if (task.revision < 1 || task.revision == Long.MAX_VALUE) throw InvalidTaskException("Invalid task revision.")
        if (runCatching { task.createdAt.toEpochMilli(); task.updatedAt.toEpochMilli(); task.completedAt?.toEpochMilli() }.isFailure)
            throw InvalidTaskException("Invalid task timestamp.")
        if ((task.status == TaskStatus.COMPLETED) != (task.completedAt != null)) throw InvalidTaskException("Completion timestamp does not match task status.")
        try { task.dueInstantOrNull() } catch (error: DateTimeException) { throw InvalidTaskException("Invalid due date or time.") }
    }

    private fun parseIanaZone(value: String): ZoneId = try {
        ZoneId.of(value).also { if (value !in ZoneId.getAvailableZoneIds()) throw DateTimeException("not an IANA zone") }
    } catch (_: Exception) { throw InvalidTaskException("Invalid IANA timezone.") }

    private fun wellFormedUtf16(value: String): Boolean {
        var index = 0
        while (index < value.length) {
            val ch = value[index]
            if (Character.isHighSurrogate(ch)) {
                if (index + 1 >= value.length || !Character.isLowSurrogate(value[index + 1])) return false
                index += 2
            } else {
                if (Character.isLowSurrogate(ch)) return false
                index++
            }
        }
        return true
    }
}
