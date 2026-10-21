package app.serea.android.assistant.taskproposal

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import app.serea.android.tasks.TaskPriority
import app.serea.android.tasks.TaskProposalOperation
import app.serea.android.tasks.TaskStatus
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch

/** Standalone review UI. A caller must prepare a fresh session for every review. */
@Composable
fun TaskProposalReviewScreen(
    session: TaskProposalReviewSession,
    selectedTaskId: String?,
    onResult: (TaskProposalReviewResult) -> Unit,
    modifier: Modifier = Modifier,
    onClose: (() -> Unit)? = null,
    onActionPendingChanged: (Boolean) -> Unit = {},
) = TaskProposalReviewContent(
    session = session,
    selectedTaskId = selectedTaskId,
    onResult = onResult,
    modifier = modifier,
    onClose = onClose,
    onActionPendingChanged = onActionPendingChanged,
    ownScroll = true,
)

@Composable
internal fun TaskProposalReviewEmbedded(
    session: TaskProposalReviewSession,
    selectedTaskId: String?,
    onResult: (TaskProposalReviewResult) -> Unit,
    modifier: Modifier = Modifier,
    onClose: (() -> Unit)? = null,
    onActionPendingChanged: (Boolean) -> Unit = {},
) = TaskProposalReviewContent(
    session = session,
    selectedTaskId = selectedTaskId,
    onResult = onResult,
    modifier = modifier,
    onClose = onClose,
    onActionPendingChanged = onActionPendingChanged,
    ownScroll = false,
)

@Composable
private fun TaskProposalReviewContent(
    session: TaskProposalReviewSession,
    selectedTaskId: String?,
    onResult: (TaskProposalReviewResult) -> Unit,
    modifier: Modifier,
    onClose: (() -> Unit)?,
    onActionPendingChanged: (Boolean) -> Unit,
    ownScroll: Boolean,
) {
    val snapshot = session.snapshot
    val proposal = snapshot.proposal
    // UI state is tied to this ephemeral session and is never restored from saved state.
    var deleteAcknowledged by remember(session) { mutableStateOf(false) }
    var actionStarted by remember(session) { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val status = session.statusState.collectAsState().value
    val busy = status != TaskProposalReviewStatus.READY || actionStarted
    val currentOnResult by rememberUpdatedState(onResult)
    val currentOnClose by rememberUpdatedState(onClose)
    val currentOnActionPendingChanged by rememberUpdatedState(onActionPendingChanged)

    LaunchedEffect(session) {
        while (session.status == TaskProposalReviewStatus.READY) {
            delay(1_000)
            session.expireIfNeeded()
        }
    }

    fun startAction(action: suspend () -> Unit) {
        if (session.status != TaskProposalReviewStatus.READY || actionStarted) return
        actionStarted = true
        currentOnActionPendingChanged(true)
        scope.launch {
            try {
                action()
            } finally {
                currentOnActionPendingChanged(false)
            }
        }
    }

    fun closeReview() = startAction {
        currentOnResult(session.decline())
        currentOnClose?.invoke()
    }

    BackHandler(enabled = !busy, onBack = ::closeReview)

    val contentModifier = if (ownScroll) modifier.verticalScroll(rememberScrollState()) else modifier
    Column(
        modifier = contentModifier.padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(
            "TaskProposal の確認",
            style = MaterialTheme.typography.headlineSmall,
            modifier = Modifier.semantics { heading() },
        )
        Text("操作: ${operationLabel(proposal.operation)}", modifier = Modifier.testTag("task_proposal_operation"))
        Text("提案 ID: ${proposal.proposalId}")

        when (proposal.operation) {
            TaskProposalOperation.CREATE -> {
                Text("対象タスク ID: 承認後に新しく割り当て")
                Text("作成時のリビジョン: 1")
                Detail("タイトル", proposal.title.orEmpty())
                Detail("説明", proposal.description.orEmpty().ifEmpty { "（空欄）" })
                Detail("優先度", priorityLabel(proposal.priority ?: TaskPriority.NORMAL))
                Detail("期限日", proposal.dueDate?.toString() ?: "なし")
                Detail("期限時刻", proposal.dueTime?.toString() ?: "なし")
                Detail("タイムゾーン", proposal.timeZoneId ?: "なし")
                Detail("作成後の状態", "未完了")
                Detail("作成日時・更新日時", "承認時に設定")
                Detail("完了日時", "なし")
            }
            TaskProposalOperation.UPDATE -> {
                val previous = snapshot.previousTask
                TargetDetails(proposal.targetTaskId, proposal.expectedRevision)
                if (previous != null) {
                    Change("タイトル", previous.title, proposal.title.orEmpty())
                    Change("説明", previous.description.ifEmpty { "（空欄）" }, proposal.description.orEmpty().ifEmpty { "（空欄）" })
                    Change("優先度", priorityLabel(previous.priority), priorityLabel(proposal.priority ?: TaskPriority.NORMAL))
                    Change("期限日", previous.dueDate?.toString() ?: "なし", proposal.dueDate?.toString() ?: "なし")
                    Change("期限時刻", previous.dueTime?.toString() ?: "なし", proposal.dueTime?.toString() ?: "なし")
                    Change("タイムゾーン", previous.timeZoneId ?: "なし", proposal.timeZoneId ?: "なし")
                }
            }
            TaskProposalOperation.COMPLETE -> {
                TargetDetails(proposal.targetTaskId, proposal.expectedRevision)
                val previous = snapshot.previousTask
                Detail("対象タスク", previous?.title ?: "確認できません")
                Change("状態", statusLabel(previous?.status), statusLabel(TaskStatus.COMPLETED))
            }
            TaskProposalOperation.DELETE -> {
                TargetDetails(proposal.targetTaskId, proposal.expectedRevision)
                Detail("削除するタスク", snapshot.previousTask?.title ?: "確認できません")
            }
        }

        if (snapshot.requiresDangerousConfirmation) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(
                    checked = deleteAcknowledged,
                    enabled = !busy,
                    onCheckedChange = { if (!busy) deleteAcknowledged = it },
                    modifier = Modifier
                        .testTag("task_proposal_delete_ack")
                        .semantics {
                            contentDescription = "このタスクを削除することを理解したと確認"
                        },
                )
                Text("このタスクを完全に削除することを理解しました。", color = MaterialTheme.colorScheme.error)
            }
        }

        Text(
            statusMessage(status),
            modifier = Modifier
                .testTag("task_proposal_status")
                .semantics { liveRegion = LiveRegionMode.Polite },
        )

        Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                enabled = !busy && (!snapshot.requiresDangerousConfirmation || deleteAcknowledged),
                modifier = Modifier.fillMaxWidth().testTag("task_proposal_approve"),
                onClick = {
                    startAction {
                        currentOnResult(
                            session.approve(
                                explicitlyApproved = true,
                                selectedTaskId = selectedTaskId,
                                dangerousActionAcknowledged = deleteAcknowledged,
                            ),
                        )
                    }
                },
            ) { Text("承認して実行") }
            OutlinedButton(
                enabled = !busy,
                modifier = Modifier.fillMaxWidth().testTag("task_proposal_decline"),
                onClick = { startAction { currentOnResult(session.decline()) } },
            ) { Text("拒否") }
            TextButton(
                enabled = !busy,
                modifier = Modifier.fillMaxWidth().testTag("task_proposal_close"),
                onClick = ::closeReview,
            ) { Text("閉じる") }
        }
    }
}

@Composable
private fun TargetDetails(taskId: String?, expectedRevision: Long?) {
    Detail("対象タスク ID", taskId ?: "なし")
    Detail("期待リビジョン", expectedRevision?.toString() ?: "なし")
}

@Composable
private fun Detail(label: String, value: String) {
    Text("$label: $value")
}

@Composable
private fun Change(label: String, oldValue: String, newValue: String) {
    Text("$label: $oldValue → $newValue")
}

private fun operationLabel(operation: TaskProposalOperation): String = when (operation) {
    TaskProposalOperation.CREATE -> "新規作成"
    TaskProposalOperation.UPDATE -> "更新"
    TaskProposalOperation.COMPLETE -> "完了"
    TaskProposalOperation.DELETE -> "削除"
}

private fun priorityLabel(priority: TaskPriority): String = when (priority) {
    TaskPriority.LOW -> "低"
    TaskPriority.NORMAL -> "標準"
    TaskPriority.HIGH -> "高"
}

private fun statusLabel(status: TaskStatus?): String = when (status) {
    TaskStatus.TODO -> "未完了"
    TaskStatus.COMPLETED -> "完了"
    null -> "確認できません"
}

private fun statusMessage(status: TaskProposalReviewStatus): String = when (status) {
    TaskProposalReviewStatus.READY -> "明示的に承認するまで、タスクは変更されません。"
    TaskProposalReviewStatus.EXECUTING -> "承認された変更を処理しています…"
    TaskProposalReviewStatus.APPROVED -> "承認済みです。変更を適用しました。"
    TaskProposalReviewStatus.DECLINED -> "拒否しました。タスクは変更されていません。"
    TaskProposalReviewStatus.EXPIRED -> "期限切れです。状態を再取得・再検証し、新しいレビューで改めて承認してください。同じレビューは再実行できません。"
    TaskProposalReviewStatus.CONFLICT -> "対象タスクが変更されています。最新状態を再取得・再検証して、新しいレビューを作成してください。同じ提案を再実行できません。"
    TaskProposalReviewStatus.REJECTED -> "このレビューは使用済みのため承認できません。状態を確認してから新しいレビューを作成してください。"
}
