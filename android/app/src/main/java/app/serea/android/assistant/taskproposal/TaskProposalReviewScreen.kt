package app.serea.android.assistant.taskproposal

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Checkbox
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import app.serea.android.tasks.TaskProposalOperation
import kotlinx.coroutines.launch

/** Standalone review UI. A caller must prepare a fresh session for every review. */
@Composable
fun TaskProposalReviewScreen(
    session: TaskProposalReviewSession,
    selectedTaskId: String? = session.snapshot.proposal.targetTaskId,
    onResult: (TaskProposalReviewResult) -> Unit,
    modifier: Modifier = Modifier,
) {
    val snapshot = session.snapshot
    val proposal = snapshot.proposal
    var deleteAcknowledged by remember(proposal.proposalId) { mutableStateOf(false) }
    val scope = rememberCoroutineScope()
    val status = session.statusState.collectAsState().value
    val busy = status != TaskProposalReviewStatus.READY

    Column(
        modifier = modifier.verticalScroll(rememberScrollState()).padding(20.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text("Review suggested task change", style = MaterialTheme.typography.headlineSmall)
        Text("Operation: ${proposal.operation}")
        if (snapshot.previousTask == null) {
            Text("Task: new task")
            Text("Title: ${proposal.title.orEmpty()}")
            Text("Description: ${proposal.description.orEmpty()}")
            Text("Priority: ${proposal.priority ?: app.serea.android.tasks.TaskPriority.NORMAL}")
            Text("Due date: ${proposal.dueDate ?: "None"}")
            Text("Due time: ${proposal.dueTime ?: "None"}")
            Text("Timezone: ${proposal.timeZoneId ?: "None"}")
        } else {
            val previous = snapshot.previousTask
            Text("Task ID: ${previous.id}")
            Text("Expected revision: ${proposal.expectedRevision}")
            when (proposal.operation) {
                TaskProposalOperation.UPDATE -> {
                    Change("Title", previous.title, proposal.title.orEmpty())
                    Change("Description", previous.description, proposal.description.orEmpty())
                    Change("Priority", previous.priority, proposal.priority ?: app.serea.android.tasks.TaskPriority.NORMAL)
                    Change("Due date", previous.dueDate ?: "None", proposal.dueDate ?: "None")
                    Change("Due time", previous.dueTime ?: "None", proposal.dueTime ?: "None")
                    Change("Timezone", previous.timeZoneId ?: "None", proposal.timeZoneId ?: "None")
                }
                TaskProposalOperation.COMPLETE -> Change("Status", previous.status, "COMPLETED")
                TaskProposalOperation.DELETE -> Text("Delete task: ${previous.title}")
                TaskProposalOperation.CREATE -> Unit
            }
        }

        if (snapshot.requiresDangerousConfirmation) {
            Row(verticalAlignment = Alignment.CenterVertically) {
                Checkbox(checked = deleteAcknowledged, onCheckedChange = { deleteAcknowledged = it })
                Text("I understand this permanently deletes the task.")
            }
        }

        when (status) {
            TaskProposalReviewStatus.EXPIRED -> Text("This review expired. Prepare the proposal again.")
            TaskProposalReviewStatus.CONFLICT -> Text("The task changed. Refresh and review a new proposal.")
            TaskProposalReviewStatus.DECLINED -> Text("Proposal declined.")
            TaskProposalReviewStatus.APPROVED -> Text("Approved operation finished.")
            TaskProposalReviewStatus.REJECTED -> Text("This proposal could not be approved.")
            TaskProposalReviewStatus.EXECUTING -> Text("Applying approved change…")
            TaskProposalReviewStatus.READY -> Text("Nothing changes until you select Approve.")
        }

        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                enabled = !busy && (!snapshot.requiresDangerousConfirmation || deleteAcknowledged),
                onClick = {
                    scope.launch {
                        onResult(session.approve(
                            explicitlyApproved = true,
                            selectedTaskId = selectedTaskId,
                            dangerousActionAcknowledged = deleteAcknowledged,
                        ))
                    }
                },
            ) { Text("Approve") }
            OutlinedButton(enabled = !busy, onClick = {
                scope.launch { onResult(session.decline()) }
            }) { Text("Decline") }
        }
    }
}

@Composable
private fun Change(label: String, oldValue: Any, newValue: Any) {
    Text("$label: $oldValue → $newValue")
}
