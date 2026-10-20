package app.serea.android.assistant.taskproposal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.unit.dp
import app.serea.android.tasks.Task
import app.serea.android.tasks.TaskConflictException
import app.serea.android.tasks.TaskDraft
import app.serea.android.tasks.TaskPriority
import app.serea.android.tasks.TaskProposal
import app.serea.android.tasks.TaskProposalOperation
import app.serea.android.tasks.TaskRepository
import app.serea.android.tasks.TaskStatus
import java.time.Instant
import java.time.LocalDate
import java.util.UUID

/** Navigation-independent demonstration using only an isolated in-memory fake repository. */
@Composable
fun TaskProposalDemoScreen(
    modifier: Modifier = Modifier,
    onClose: (() -> Unit)? = null,
) {
    val repository = remember { DemoTaskProposalRepository() }
    var operation by remember { mutableStateOf(TaskProposalOperation.UPDATE) }
    var reviewGeneration by remember { mutableStateOf(0) }
    var session by remember { mutableStateOf<TaskProposalReviewSession?>(null) }
    var preparationError by remember { mutableStateOf<String?>(null) }
    var tasks by remember { mutableStateOf(repository.demoTasks()) }
    var writeCount by remember { mutableStateOf(0) }

    LaunchedEffect(operation, reviewGeneration) {
        try {
            session = repository.prepareDemoReview(operation)
            preparationError = null
        } catch (_: TaskConflictException) {
            session = null
            preparationError = "架空の対象タスクがありません。デモをリセットしてから、新しいレビューを作成してください。"
        }
    }

    Column(
        modifier = modifier.fillMaxSize().padding(16.dp),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text("DEMO / 架空のタスク", style = MaterialTheme.typography.headlineSmall)
        Text("この画面は隔離されたメモリ内Fake Repositoryだけを使います。実データには触れません。")
        Text("確認する操作を選択")
        Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(2.dp)) {
            TaskProposalOperation.entries.forEach { candidate ->
                FilterChip(
                    selected = operation == candidate,
                    onClick = { operation = candidate },
                    label = { Text(demoOperationLabel(candidate)) },
                    modifier = Modifier.weight(1f),
                )
            }
        }

        val activeSession = session
        if (activeSession != null) {
            TaskProposalReviewScreen(
                session = activeSession,
                selectedTaskId = activeSession.snapshot.proposal.targetTaskId,
                onResult = {
                    writeCount = repository.writeCount()
                    tasks = repository.demoTasks()
                },
                modifier = Modifier.weight(1f),
                onClose = onClose,
            )
        } else {
            Text(
                preparationError ?: "新しいレビューを準備しています…",
                modifier = Modifier.weight(1f).semantics { liveRegion = LiveRegionMode.Polite },
            )
        }

        Text("Fake Repository 書き込み回数: $writeCount", modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
        Text("デモ内タスク: ${tasks.size}")
        tasks.take(3).forEach { task ->
            Text("${task.title} · ID ${task.id} · rev ${task.revision}")
        }
        if (tasks.size > 3) Text("ほか ${tasks.size - 3} 件の架空タスク")
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(onClick = { reviewGeneration++ }) { Text("新しいレビューを準備") }
            OutlinedButton(onClick = {
                repository.resetDemoData()
                tasks = repository.demoTasks()
                writeCount = 0
                reviewGeneration++
            }) { Text("デモをリセット") }
        }
    }
}

private class DemoTaskProposalRepository : TaskRepository {
    private val lock = Any()
    private val rows = linkedMapOf<String, Task>()
    private val proposalReceipts = mutableMapOf<String, String>()
    private var writes = 0

    init { resetDemoData() }

    fun resetDemoData() = synchronized(lock) {
        rows.clear()
        proposalReceipts.clear()
        writes = 0
        val now = Instant.parse("2026-10-20T12:00:00Z")
        rows[DEMO_TASK_ID] = Task(
            id = DEMO_TASK_ID,
            title = "DEMO: 週次計画を確認",
            description = "これはデモ専用の架空タスクです。",
            status = TaskStatus.TODO,
            priority = TaskPriority.NORMAL,
            createdAt = now,
            updatedAt = now,
            dueDate = LocalDate.parse("2026-10-24"),
            revision = 1,
        )
    }

    fun demoTasks(): List<Task> = synchronized(lock) { rows.values.toList() }
    fun writeCount(): Int = synchronized(lock) { writes }

    suspend fun prepareDemoReview(operation: TaskProposalOperation): TaskProposalReviewSession {
        val target = if (operation == TaskProposalOperation.CREATE) null else get(DEMO_TASK_ID) ?: throw TaskConflictException()
        val proposal = when (operation) {
            TaskProposalOperation.CREATE -> TaskProposal(
                operation = operation,
                title = "DEMO: 議事録を作成",
                description = "架空データを使った承認デモです。",
                dueDate = LocalDate.parse("2026-10-25"),
                priority = TaskPriority.HIGH,
            )
            TaskProposalOperation.UPDATE -> TaskProposal(
                operation = operation,
                title = "DEMO: 週次計画を更新",
                description = "更新後の架空タスクの説明です。",
                priority = TaskPriority.HIGH,
                dueDate = target!!.dueDate,
                targetTaskId = target.id,
                expectedRevision = target.revision,
            )
            TaskProposalOperation.COMPLETE -> TaskProposal(
                operation = operation,
                targetTaskId = target!!.id,
                expectedRevision = target.revision,
            )
            TaskProposalOperation.DELETE -> TaskProposal(
                operation = operation,
                targetTaskId = target!!.id,
                expectedRevision = target.revision,
            )
        }
        return TaskProposalReviewSession.prepare(proposal, this)
    }

    override suspend fun create(draft: TaskDraft): Task = error("Demo creation must use an approved proposal.")

    override suspend fun createFromApprovedProposal(draft: TaskDraft, proposalId: String): Task = synchronized(lock) {
        if (proposalReceipts.containsKey(proposalId)) throw TaskConflictException()
        createRow(draft).also { proposalReceipts[proposalId] = it.id }
    }

    private fun createRow(draft: TaskDraft): Task {
        val now = Instant.parse("2026-10-20T12:00:00Z").plusMillis(writes.toLong() + 1)
        val task = Task(
            id = UUID.randomUUID().toString(),
            title = draft.title,
            description = draft.description,
            status = TaskStatus.TODO,
            priority = draft.priority,
            createdAt = now,
            updatedAt = now,
            dueDate = draft.dueDate,
            dueTime = draft.dueTime,
            timeZoneId = draft.timeZoneId,
        )
        rows[task.id] = task
        writes++
        return task
    }

    override suspend fun get(id: String): Task? = synchronized(lock) { rows[id] }
    override suspend fun list(): List<Task> = demoTasks()

    override suspend fun update(id: String, expectedRevision: Long, draft: TaskDraft): Task = synchronized(lock) {
        val old = checked(id, expectedRevision)
        old.copy(
            title = draft.title,
            description = draft.description,
            priority = draft.priority,
            dueDate = draft.dueDate,
            dueTime = draft.dueTime,
            timeZoneId = draft.timeZoneId,
            revision = old.revision + 1,
        ).also { rows[id] = it; writes++ }
    }

    override suspend fun complete(id: String, expectedRevision: Long): Task = synchronized(lock) {
        val old = checked(id, expectedRevision)
        old.copy(status = TaskStatus.COMPLETED, completedAt = old.updatedAt.plusMillis(1), revision = old.revision + 1)
            .also { rows[id] = it; writes++ }
    }

    override suspend fun reopen(id: String, expectedRevision: Long): Task = synchronized(lock) {
        val old = checked(id, expectedRevision)
        old.copy(status = TaskStatus.TODO, completedAt = null, revision = old.revision + 1)
            .also { rows[id] = it; writes++ }
    }

    override suspend fun delete(id: String, expectedRevision: Long) = synchronized(lock) {
        checked(id, expectedRevision)
        rows.remove(id)
        writes++
        Unit
    }

    override suspend fun incomplete(): List<Task> = demoTasks().filter { it.status == TaskStatus.TODO }
    override suspend fun completed(): List<Task> = demoTasks().filter { it.status == TaskStatus.COMPLETED }
    override suspend fun dueToday(today: LocalDate): List<Task> = demoTasks().filter { it.dueDate == today }
    override suspend fun overdue(now: Instant, fallbackZone: java.time.ZoneId): List<Task> = emptyList()

    private fun checked(id: String, expectedRevision: Long): Task {
        val old = rows[id] ?: throw TaskConflictException()
        if (old.revision != expectedRevision) throw TaskConflictException()
        return old
    }

    private companion object {
        const val DEMO_TASK_ID = "00000000-0000-0000-0000-000000000066"
    }
}

private fun demoOperationLabel(operation: TaskProposalOperation): String = when (operation) {
    TaskProposalOperation.CREATE -> "新規作成"
    TaskProposalOperation.UPDATE -> "更新"
    TaskProposalOperation.COMPLETE -> "完了"
    TaskProposalOperation.DELETE -> "削除"
}
