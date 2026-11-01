package app.serea.android.tasks

import java.time.Instant
import java.time.LocalDate
import java.time.LocalTime
import org.junit.Assert.*
import org.junit.Test

class TaskDomainTest {
    @Test fun acceptsUnicodeAndNewlinesAndPreservesAllDaySemantics() {
        val draft = TaskDraft("提出する 📚 日本語", "1行目\nsecond line", TaskPriority.HIGH, LocalDate.parse("2026-10-25"))
        TaskValidation.validateDraft(draft)
        val timed = draft.copy(dueTime = LocalTime.of(18, 0), timeZoneId = "Asia/Tokyo")
        val task = Task("00000000-0000-0000-0000-000000000001", timed.title, timed.description, TaskStatus.TODO,
            timed.priority, Instant.parse("2026-10-20T00:00:00Z"), Instant.parse("2026-10-20T00:00:00Z"),
            timed.dueDate, timed.dueTime, timed.timeZoneId)
        assertEquals(Instant.parse("2026-10-25T09:00:00Z"), task.dueInstantOrNull())
        assertNull(Task("00000000-0000-0000-0000-000000000001", draft.title, draft.description, TaskStatus.TODO,
            draft.priority, task.createdAt, task.updatedAt, draft.dueDate).dueInstantOrNull())
    }

    @Test fun rejectsInvalidTitleLengthIdTimezoneTimeAndDstTransitions() {
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validateDraft(TaskDraft(" \n ")) }
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validateDraft(TaskDraft("x".repeat(201))) }
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validateDraft(TaskDraft("ok", "x".repeat(10_001))) }
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validateDraft(TaskDraft("x", dueTime = LocalTime.NOON)) }
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validateDraft(TaskDraft("x", timeZoneId = "UTC")) }
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validateDraft(TaskDraft("offset", dueDate = LocalDate.now(), timeZoneId = "+09:00")) }
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validateDraft(TaskDraft("x", dueDate = LocalDate.now(), dueTime = LocalTime.NOON, timeZoneId = "Not/AZone")) }
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validateId("task-1") }
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validateDraft(TaskDraft("gap", dueDate = LocalDate.parse("2026-03-08"), dueTime = LocalTime.of(2, 30), timeZoneId = "America/New_York")) }
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validateDraft(TaskDraft("overlap", dueDate = LocalDate.parse("2026-11-01"), dueTime = LocalTime.of(1, 30), timeZoneId = "America/New_York")) }
    }

    @Test fun overdueUsesInjectedInstantAndCompletedAtMustMatch() {
        val task = Task("00000000-0000-0000-0000-000000000001", "x", "", TaskStatus.TODO, TaskPriority.NORMAL,
            Instant.EPOCH, Instant.EPOCH, LocalDate.parse("2026-01-01"))
        assertTrue(task.isOverdue(Instant.parse("2026-01-02T00:00:00Z")))
        assertFalse(task.isOverdue(Instant.parse("2025-12-31T23:59:00Z")))
        val allDay = task.copy(dueTime = null, timeZoneId = "Asia/Tokyo")
        assertFalse(allDay.isOverdue(Instant.parse("2026-01-01T02:00:00Z")))
        assertTrue(allDay.isOverdue(Instant.parse("2026-01-01T16:00:00Z")))
        assertThrows(InvalidTaskException::class.java) { TaskValidation.validate(task.copy(status = TaskStatus.COMPLETED)) }
    }
}
