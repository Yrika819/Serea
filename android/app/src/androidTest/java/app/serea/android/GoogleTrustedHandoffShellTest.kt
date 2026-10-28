package app.serea.android

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollToNode
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class GoogleTrustedHandoffShellTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun gmailHandoffReachesChatComposerAndUsesLocalDemoOnlyAfterSend() {
        assertChatHandoff(
            destinationTag = "destination_gmail",
            selectedTitle = "Project Finch schedule",
            handoffAction = "このメールをChatへ渡す",
            expectedOrigin = DataOrigin.GMAIL,
        )
    }

    @Test fun calendarHandoffReachesChatComposerAndUsesLocalDemoOnlyAfterSend() {
        assertChatHandoff(
            destinationTag = "destination_calendar",
            selectedTitle = "Date-line test",
            handoffAction = "この予定をChatへ渡す",
            expectedOrigin = DataOrigin.CALENDAR,
            selectedCalendarName = "Demo Global",
        )
    }

    @Test fun activityRecreationDropsUnconfirmedPreview() {
        waitForChatReady()
        compose.onNodeWithTag("destination_gmail").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Project Finch schedule").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Project Finch schedule").performClick()
        compose.onNodeWithText("このメールをChatへ渡す").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Chatへ渡す内容を確認").fetchSemanticsNodes().isNotEmpty()
        }

        compose.activityRule.scenario.recreate()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Chatへ渡す内容を確認").fetchSemanticsNodes().isEmpty()
        }
        compose.onAllNodesWithText("端末内で処理してChatへ渡す").assertCountEquals(0)
        assertTrue(compose.activity.chatViewModel.state.value.input.isEmpty())
    }

    private fun assertChatHandoff(
        destinationTag: String,
        selectedTitle: String,
        handoffAction: String,
        expectedOrigin: DataOrigin,
        selectedCalendarName: String? = null,
    ) {
        waitForChatReady()
        compose.onNodeWithTag(destinationTag).performClick()
        if (selectedCalendarName != null) {
            compose.waitUntil(10_000) {
                compose.onAllNodesWithText("表示するカレンダー").fetchSemanticsNodes().isNotEmpty()
            }
            compose.onNodeWithText(selectedCalendarName).performClick()
            compose.waitUntil(10_000) {
                runCatching {
                    compose.onNodeWithTag("calendar_agenda_list")
                        .performScrollToNode(hasText(selectedTitle))
                }.isSuccess
            }
        } else {
            compose.waitUntil(5_000) {
                compose.onAllNodesWithText(selectedTitle).fetchSemanticsNodes().isNotEmpty()
            }
        }
        compose.onNodeWithText(selectedTitle).performClick()
        compose.onNodeWithText(handoffAction).performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Chatへ渡す内容を確認").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("送信先: この端末内のローカルChat")
            .assertTextContains("ローカルChat", substring = true)
        compose.onNodeWithText("端末内で処理してChatへ渡す").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("chat-composer").fetchSemanticsNodes().isNotEmpty()
        }
        val composer = compose.onNodeWithTag("chat-composer")
        composer.assertTextContains(selectedTitle, substring = true)
        compose.onAllNodesWithText("Demo response:", substring = true).assertCountEquals(0)
        val imported = compose.activity.chatViewModel.state.value.editorSourceLineage
        assertTrue(imported.contains(expectedOrigin))

        compose.onNodeWithText("送信").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Demo response:", substring = true).fetchSemanticsNodes().isNotEmpty()
        }
        val userMessage = compose.activity.chatViewModel.state.value.conversation.messages
            .first { it.role == MessageRole.USER }
        assertTrue(userMessage.sourceLineage.contains(expectedOrigin))
    }

    private fun waitForChatReady() {
        compose.waitUntil(10_000) {
            !compose.activity.chatViewModel.state.value.isRestoring
        }
        compose.activity.chatViewModel.newConversation()
    }
}
