package app.serea.android.tasks

import android.database.sqlite.SQLiteConstraintException
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.util.UUID
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.sync.Mutex
import kotlinx.coroutines.sync.withLock

interface TaskRepository {
    suspend fun create(draft: TaskDraft): Task
    suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task
    suspend fun get(id: String): Task?
    suspend fun list(): List<Task>
    suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task
    suspend fun complete(id: String, expectedRevision: Long): Task
    suspend fun reopen(id: String, expectedRevision: Long): Task
    suspend fun delete(id: String, expectedRevision: Long)
    suspend fun incomplete(): List<Task>
    suspend fun completed(): List<Task>
    suspend fun dueToday(today: LocalDate): List<Task>
    suspend fun overdue(now: Instant, fallbackZone: ZoneId = ZoneId.systemDefault()): List<Task>
}

/** Confirmer-created value passed only to durable repositories after explicit approval. */
internal class ApprovedTaskProposal internal constructor(val proposal: TaskProposal)

/** Optional stronger boundary; fake/demo repositories keep the existing TaskRepository contract. */
internal interface DurableTaskProposalApplyPort {
    suspend fun applyApprovedProposal(approval: ApprovedTaskProposal): Task?
}

/** The DAO only stores opaque ciphertext. CAS revisions and SQLite writes prevent stale updates from resurrecting deletes. */
internal class EncryptedTaskRepository(
    private val dao: TaskDao,
    private val cipher: TaskCipher,
    private val clock: () -> Instant = Instant::now,
    private val idFactory: () -> String = { UUID.randomUUID().toString() },
) : TaskRepository, DurableTaskProposalApplyPort {
    private val mutex = Mutex()

    override suspend fun create(draft: TaskDraft): Task = mutex.withLock { createLocked(draft, null) }

    override suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task = mutex.withLock {
        TaskValidation.validateId(proposalId)
        if (dao.receipt(proposalId) != null) throw TaskConflictException()
        createLocked(draft, proposalId)
    }

    override suspend fun applyApprovedProposal(approval: ApprovedTaskProposal): Task? = mutex.withLock {
        val proposal = approval.proposal
        when (val validation = proposal.validate()) {
            ProposalValidation.Valid -> Unit
            is ProposalValidation.Invalid -> throw InvalidTaskException(validation.reason)
        }
        if (dao.receipt(proposal.proposalId) != null) throw TaskConflictException()
        when (proposal.operation) {
            TaskProposalOperation.CREATE -> createLocked(proposal.toDraft(), proposal.proposalId)
            TaskProposalOperation.UPDATE -> {
                val taskId = proposal.targetTaskId!!
                val old = readForMutation(taskId, proposal.expectedRevision!!)
                val updated = old.copy(
                    title = proposal.title!!,
                    description = proposal.description.orEmpty(),
                    priority = proposal.priority ?: TaskPriority.NORMAL,
                    dueDate = proposal.dueDate,
                    dueTime = proposal.dueTime,
                    timeZoneId = proposal.timeZoneId,
                    updatedAt = monotonicNow(old.updatedAt),
                    revision = old.revision + 1,
                )
                saveForProposal(updated, proposal.proposalId)
                updated
            }
            TaskProposalOperation.COMPLETE -> {
                val taskId = proposal.targetTaskId!!
                val old = readForMutation(taskId, proposal.expectedRevision!!)
                if (old.status != TaskStatus.TODO) throw TaskConflictException()
                val completedAt = monotonicNow(old.updatedAt)
                val completed = old.copy(
                    status = TaskStatus.COMPLETED,
                    completedAt = completedAt,
                    updatedAt = completedAt,
                    revision = old.revision + 1,
                )
                saveForProposal(completed, proposal.proposalId)
                completed
            }
            TaskProposalOperation.DELETE -> {
                val taskId = proposal.targetTaskId!!
                readForMutation(taskId, proposal.expectedRevision!!)
                try {
                    dao.deleteTaskForProposal(
                        taskId,
                        proposal.expectedRevision,
                        ProposalReceiptRow(proposal.proposalId, taskId),
                    )
                } catch (error: TaskConflictException) { throw error }
                catch (error: SQLiteConstraintException) { throw TaskConflictException() }
                catch (error: CancellationException) { throw error }
                catch (error: Exception) { throw TaskStorageUnavailableException("Task could not be deleted.", error) }
                null
            }
        }
    }

    private suspend fun createLocked(draft: TaskDraft, proposalId: String?): Task {
        TaskValidation.validateDraft(draft)
        try { dao.allRows().firstOrNull()?.decode() }
        catch (error: TaskStorageUnavailableException) { throw error }
        catch (error: CancellationException) { throw error }
        catch (error: Exception) { throw TaskStorageUnavailableException("Task storage is unavailable.", error) }
        val id = idFactory().lowercase()
        try { TaskValidation.validateId(id) } catch (error: Exception) { throw InvalidTaskException("Task ID generation failed.") }
        val now = clock()
        val task = Task(id, draft.title, draft.description, TaskStatus.TODO, draft.priority, now, now,
            draft.dueDate, draft.dueTime, draft.timeZoneId, revision = 1)
        TaskValidation.validate(task)
        try {
            val row = task.toRow()
            if (proposalId == null) dao.insert(row) else dao.insertTaskForProposal(row, ProposalReceiptRow(proposalId, id))
        } catch (error: TaskConflictException) { throw error }
        catch (error: SQLiteConstraintException) { throw TaskConflictException() }
        catch (error: CancellationException) { throw error }
        catch (error: Exception) { throw TaskStorageUnavailableException("Task could not be saved.", error) }
        return task
    }

    override suspend fun get(id: String): Task? = mutex.withLock {
        TaskValidation.validateId(id)
        try { dao.row(id)?.decode() } catch (error: TaskStorageUnavailableException) { throw error }
        catch (error: CancellationException) { throw error }
        catch (error: Exception) { throw TaskStorageUnavailableException("Task storage is unavailable.", error) }
    }
    override suspend fun list(): List<Task> = mutex.withLock {
        try { dao.allRows().map { it.decode() }.sortedWith(TASK_ORDER) }
        catch (error: TaskStorageUnavailableException) { throw error }
        catch (error: CancellationException) { throw error }
        catch (error: Exception) { throw TaskStorageUnavailableException("Task storage is unavailable.", error) }
    }

    override suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task = mutex.withLock {
        TaskValidation.validateId(id); TaskValidation.validateDraft(draft)
        val old = readForMutation(id, expectedRevision)
        save(old.copy(title = draft.title, description = draft.description, priority = draft.priority,
            dueDate = draft.dueDate, dueTime = draft.dueTime, timeZoneId = draft.timeZoneId,
            updatedAt = monotonicNow(old.updatedAt), revision = old.revision + 1))
    }

    override suspend fun complete(id: String, expectedRevision: Long): Task = mutex.withLock {
        TaskValidation.validateId(id)
        val old = readForMutation(id, expectedRevision)
        if (old.status == TaskStatus.COMPLETED) return@withLock old
        save(old.copy(status = TaskStatus.COMPLETED, completedAt = monotonicNow(old.updatedAt),
            updatedAt = monotonicNow(old.updatedAt), revision = old.revision + 1))
    }

    override suspend fun reopen(id: String, expectedRevision: Long): Task = mutex.withLock {
        TaskValidation.validateId(id)
        val old = readForMutation(id, expectedRevision)
        if (old.status == TaskStatus.TODO) return@withLock old
        val now = monotonicNow(old.updatedAt)
        save(old.copy(status = TaskStatus.TODO, completedAt = null, updatedAt = now, revision = old.revision + 1))
    }

    override suspend fun delete(id: String, expectedRevision: Long) = mutex.withLock {
        TaskValidation.validateId(id)
        if (expectedRevision < 1) throw InvalidTaskException("Invalid task revision.")
        val row = try { dao.row(id) } catch (error: CancellationException) { throw error }
            catch (error: Exception) { throw TaskStorageUnavailableException("Task storage is unavailable.", error) }
            ?: throw TaskNotFoundException()
        if (row.revision != expectedRevision) throw TaskConflictException()
        val deleted = try { dao.deleteIfRevisionMatches(id, expectedRevision) }
            catch (error: CancellationException) { throw error }
            catch (error: Exception) { throw TaskStorageUnavailableException("Task could not be deleted.", error) }
        if (deleted != 1) throw TaskConflictException()
    }

    override suspend fun incomplete() = list().filter { it.status == TaskStatus.TODO }
    override suspend fun completed() = list().filter { it.status == TaskStatus.COMPLETED }
    override suspend fun dueToday(today: LocalDate) = incomplete().filter { it.dueDate == today }
    override suspend fun overdue(now: Instant, fallbackZone: ZoneId) = incomplete().filter { it.isOverdue(now, fallbackZone) }

    private suspend fun readForMutation(id: String, expectedRevision: Long): Task {
        if (expectedRevision < 1 || expectedRevision == Long.MAX_VALUE) throw InvalidTaskException("Invalid task revision.")
        val row = try { dao.row(id) } catch (error: CancellationException) { throw error }
            catch (error: Exception) { throw TaskStorageUnavailableException("Task storage is unavailable.", error) }
            ?: throw TaskNotFoundException()
        if (row.revision != expectedRevision) throw TaskConflictException()
        return row.decode()
    }

    private suspend fun save(task: Task): Task {
        TaskValidation.validate(task)
        val encoded = try { cipher.encrypt(task.id, TaskPayloadCodec.encode(task)) }
            catch (error: CancellationException) { throw error }
            catch (error: Exception) { throw TaskStorageUnavailableException("Task could not be encrypted and saved.", error) }
        val changed = try { dao.compareAndSet(task.id, task.revision - 1, task.revision, encoded) }
            catch (error: CancellationException) { throw error }
            catch (error: Exception) { throw TaskStorageUnavailableException("Task could not be saved.", error) }
        if (changed != 1) throw TaskConflictException()
        return task
    }

    private suspend fun saveForProposal(task: Task, proposalId: String) {
        TaskValidation.validate(task)
        val encoded = try { cipher.encrypt(task.id, TaskPayloadCodec.encode(task)) }
            catch (error: CancellationException) { throw error }
            catch (error: Exception) { throw TaskStorageUnavailableException("Task could not be encrypted and saved.", error) }
        try {
            dao.compareAndSetTaskForProposal(
                task.id,
                task.revision - 1,
                task.revision,
                encoded,
                ProposalReceiptRow(proposalId, task.id),
            )
        } catch (error: TaskConflictException) { throw error }
        catch (error: SQLiteConstraintException) { throw TaskConflictException() }
        catch (error: CancellationException) { throw error }
        catch (error: Exception) { throw TaskStorageUnavailableException("Task could not be saved.", error) }
    }

    private fun TaskProposal.toDraft() = TaskDraft(
        title!!,
        description.orEmpty(),
        priority ?: TaskPriority.NORMAL,
        dueDate,
        dueTime,
        timeZoneId,
    )

    private fun monotonicNow(previous: Instant): Instant {
        val now = clock()
        if (previous == Instant.MAX) throw InvalidTaskException("Timestamp is out of range.")
        return if (now.isAfter(previous)) now else previous.plusMillis(1)
    }

    private fun Task.toRow(): TaskRow = TaskRow(id, revision, cipher.encrypt(id, TaskPayloadCodec.encode(this)))
    private fun TaskRow.decode(): Task = TaskPayloadCodec.decode(id, revision, cipher.decrypt(id, encryptedRecord))

    companion object {
        private val TASK_ORDER = compareBy<Task> { it.status.ordinal }
            .thenBy { it.dueDate ?: LocalDate.MAX }
            .thenByDescending { it.priority.ordinal }
            .thenByDescending { it.updatedAt }
    }
}
