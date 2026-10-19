package app.serea.android

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performScrollToIndex
import androidx.test.ext.junit.runners.AndroidJUnit4
import android.view.WindowManager
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ChatScreenTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    private fun composer() = compose.onNodeWithTag("chat-composer")

    private fun awaitComposerEnabled() {
        composer().assertExists()
        compose.waitUntil(30_000) {
            runCatching { composer().assertIsEnabled() }.isSuccess
        }
    }

    private fun stateSnapshot(): ChatUiState {
        var snapshot: ChatUiState? = null
        compose.activityRule.scenario.onActivity { snapshot = it.chatViewModel.state.value }
        return requireNotNull(snapshot)
    }

    private fun awaitResponse(text: String) {
        compose.waitUntil(30_000) {
            val state = stateSnapshot()
            !state.isLoading && (state.errorMessage != null || state.conversation.messages.lastOrNull()?.role == MessageRole.ASSISTANT)
        }
        val state = stateSnapshot()
        assertTrue(
            "Chat did not complete normally; error=${state.errorMessage}, " +
                "statuses=${state.conversation.messages.map { it.role.name + ":" + it.status }}",
            state.errorMessage == null && state.conversation.messages.lastOrNull()?.role == MessageRole.ASSISTANT,
        )
        compose.onNodeWithTag("conversation-list").performScrollToIndex(state.conversation.messages.lastIndex)
        val response = compose.onAllNodes(hasText(text, substring = true))
        compose.waitUntil(5_000) { response.fetchSemanticsNodes().isNotEmpty() }
        response.assertCountEquals(1)
    }

    @Test fun demoLabelAndConversationAreVisible() {
        awaitComposerEnabled()
        compose.onNodeWithText("新しい会話").performClick()
        awaitComposerEnabled()
        compose.onNodeWithText("DEMO · Mock chat · No cloud AI").assertIsDisplayed()
        composer().performTextInput("hello")
        compose.onNodeWithText("送信").performClick()
        compose.onNodeWithText("You", substring = true).assertIsDisplayed()
        awaitResponse("Demo response: I received “hello”.")
        compose.onAllNodes(hasText("Demo response: I received “hello”.", substring = true))
            .onFirst().assertTextContains("No cloud AI", substring = true)
        compose.onNodeWithText("履歴").performClick()
        compose.waitUntil(30_000) { stateSnapshot().showingHistory }
        compose.onNodeWithText("保存された会話").assertIsDisplayed()
        compose.onAllNodes(hasText("Conversation", substring = true)).onFirst().assertIsDisplayed()
    }

    @Test fun completedDemoConversationSurvivesActivityRecreation() {
        awaitComposerEnabled()
        composer().performTextInput("rotation-check")
        compose.onNodeWithText("送信").performClick()
        awaitResponse("Demo response: I received “rotation-check”.")
        compose.activityRule.scenario.recreate()
        compose.waitUntil(30_000) {
            val state = stateSnapshot()
            !state.isRestoring && state.conversation.messages.any { it.text == "rotation-check" }
        }
        val restored = stateSnapshot()
        compose.onNodeWithTag("conversation-list").performScrollToIndex(restored.conversation.messages.lastIndex)
        compose.onNodeWithText("rotation-check").assertIsDisplayed()
        compose.onAllNodes(hasText("Demo response: I received “rotation-check”.", substring = true)).assertCountEquals(1)
    }

    @Test fun settingsPreventScreenshotsWhileCredentialFieldIsAvailable() {
        awaitComposerEnabled()
        compose.onNodeWithText("設定").performClick()
        compose.waitUntil(5_000) {
            var secure = false
            compose.activityRule.scenario.onActivity {
                secure = it.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE != 0
            }
            secure
        }
        compose.runOnIdle {
            assertEquals(WindowManager.LayoutParams.FLAG_SECURE,
                compose.activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE)
        }
        compose.onNodeWithText("設定").performClick()
        compose.waitUntil(5_000) {
            var secure = true
            compose.activityRule.scenario.onActivity {
                secure = it.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE != 0
            }
            !secure
        }
        compose.runOnIdle {
            assertEquals(0, compose.activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE)
        }
    }

    @Test fun previewShowsUnclassifiedUserTextWhileCloudSendRemainsDisabled() {
        awaitComposerEnabled()
        composer().performTextInput("draft stays local")
        compose.onNodeWithText("設定").performClick()
        compose.onNodeWithText("Cloud接続テスト（未承認）").assertIsNotEnabled()
        compose.onNodeWithText("送信予定内容をプレビュー").performClick()
        compose.onNodeWithText("送信先候補", substring = true).assertIsDisplayed()
        compose.onNodeWithText("分類: UNKNOWN", substring = true).assertIsDisplayed()
        compose.onNodeWithText("予定本文: draft stays local").assertIsDisplayed()
    }
}
