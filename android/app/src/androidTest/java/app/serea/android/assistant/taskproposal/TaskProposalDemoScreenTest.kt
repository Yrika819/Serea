package app.serea.android.assistant.taskproposal

import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.SereaTheme
import kotlinx.coroutines.CompletableDeferred
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class TaskProposalDemoScreenTest {
    @get:Rule val compose = createComposeRule()

    @Test fun demoControlsStayDisabledUntilPendingApprovalFinishes() {
        val repository = DemoTaskProposalRepository()
        val mutationStarted = CompletableDeferred<Unit>()
        val allowMutation = CompletableDeferred<Unit>()
        repository.beforeMutation = {
            mutationStarted.complete(Unit)
            allowMutation.await()
        }
        compose.setContent {
            SereaTheme { TaskProposalDemoScreenForTest(repository) }
        }

        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("task_proposal_approve").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("task_proposal_approve").performScrollTo().assertIsEnabled().performClick()
        compose.waitUntil(5_000) { mutationStarted.isCompleted }

        compose.onNodeWithTag("task_proposal_operation_create").assertIsNotEnabled()
        compose.onNodeWithText("新しいレビューを準備").assertIsNotEnabled()
        compose.onNodeWithText("デモをリセット").assertIsNotEnabled()
        compose.onNodeWithTag("task_proposal_status").assertTextContains("処理しています")
        compose.onNodeWithTag("task_proposal_close").assertIsNotEnabled()
        assertEquals(0, repository.writeCount())

        allowMutation.complete(Unit)
        compose.waitUntil(5_000) { repository.writeCount() == 1 }
        compose.onNodeWithTag("task_proposal_operation_create").assertExists()
        compose.onNodeWithTag("task_proposal_status").assertTextContains("承認済み")
        compose.onNodeWithTag("task_proposal_close").assertIsEnabled()
    }

    @Test fun cancelledApprovalShowsUnknownAndPreventsReapproval() {
        val repository = DemoTaskProposalRepository().apply {
            beforeMutation = { throw kotlinx.coroutines.CancellationException("fixture cancellation") }
        }
        compose.setContent { SereaTheme { TaskProposalDemoScreenForTest(repository) } }
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("task_proposal_approve").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("task_proposal_approve").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            runCatching {
                compose.onNodeWithTag("task_proposal_status")
                    .assertTextContains("処理結果を確認できません")
            }.isSuccess
        }

        compose.onNodeWithTag("task_proposal_approve").assertIsNotEnabled()
        compose.onNodeWithTag("task_proposal_close").assertIsEnabled()
        compose.onNodeWithText("Fake Repository 書き込み回数: 0").assertExists()
        compose.onNodeWithTag("task_proposal_status").assertTextContains("同じ承認は再実行できません")
    }

    @Test fun changingOperationImmediatelyHidesThePreviousReviewUntilFreshSessionIsReady() {
        val repository = DemoTaskProposalRepository()
        var prepareCount = 0
        val freshSessionStarted = CompletableDeferred<Unit>()
        val allowFreshSession = CompletableDeferred<Unit>()
        repository.beforePrepare = {
            prepareCount += 1
            if (prepareCount == 2) {
                freshSessionStarted.complete(Unit)
                allowFreshSession.await()
            }
        }
        compose.setContent {
            SereaTheme { TaskProposalDemoScreenForTest(repository) }
        }

        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("task_proposal_approve").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("task_proposal_operation_update").performClick()
        compose.onNodeWithTag("task_proposal_approve").assertIsEnabled()
        compose.onNodeWithText("操作: 更新").assertExists()
        compose.onNodeWithTag("task_proposal_operation_create").performScrollTo().performClick()
        compose.waitUntil(5_000) { freshSessionStarted.isCompleted }

        compose.onAllNodesWithTag("task_proposal_approve").assertCountEquals(0)
        assertEquals(0, repository.writeCount())

        allowFreshSession.complete(Unit)
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("操作: 新規作成").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("task_proposal_approve").assertIsEnabled()
        compose.onNodeWithText("操作: 新規作成").assertExists()
        assertEquals(0, repository.writeCount())
    }
}
