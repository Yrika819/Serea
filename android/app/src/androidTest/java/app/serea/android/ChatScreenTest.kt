package app.serea.android

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
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

    private fun awaitResponse(text: String) {
        val response = compose.onAllNodes(hasText(text, substring = true))
        val failure = compose.onAllNodes(hasText("Demo response failed.", substring = true))
        val unknownOutcome = compose.onAllNodes(hasText("request timed out", substring = true))
        compose.waitUntil(30_000) {
            response.fetchSemanticsNodes().isNotEmpty() ||
                failure.fetchSemanticsNodes().isNotEmpty() ||
                unknownOutcome.fetchSemanticsNodes().isNotEmpty()
        }
        assertTrue(
            "Chat did not complete normally; failed=${failure.fetchSemanticsNodes().isNotEmpty()}, " +
                "outcomeUnknown=${unknownOutcome.fetchSemanticsNodes().isNotEmpty()}",
            response.fetchSemanticsNodes().isNotEmpty(),
        )
        response.assertCountEquals(1)
    }

    @Test fun demoLabelAndConversationAreVisible() {
        compose.onNodeWithText("DEMO · Mock chat · No cloud AI").assertIsDisplayed()
        compose.onNodeWithText("端末内の暗号化履歴に保存されるDemo会話です。Cloud送信は無効です。").assertIsDisplayed()
        awaitComposerEnabled()
        composer().performTextInput("hello")
        compose.onNodeWithText("送信").performClick()
        compose.onNodeWithText("You", substring = true).assertIsDisplayed()
        awaitResponse("Demo response: I received “hello”.")
        compose.onAllNodes(hasText("Demo response: I received “hello”.", substring = true))
            .onFirst().assertTextContains("No cloud AI", substring = true)
        compose.onNodeWithText("履歴").performClick()
        compose.onNodeWithText("保存された会話").assertIsDisplayed()
        compose.onNodeWithText("Conversation", substring = true).assertIsDisplayed()
    }

    @Test fun completedDemoConversationSurvivesActivityRecreation() {
        awaitComposerEnabled()
        composer().performTextInput("rotation-check")
        compose.onNodeWithText("送信").performClick()
        awaitResponse("Demo response: I received “rotation-check”.")
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("rotation-check").assertIsDisplayed()
        compose.onAllNodes(hasText("Demo response: I received “rotation-check”.", substring = true)).assertCountEquals(1)
    }

    @Test fun settingsPreventScreenshotsWhileCredentialFieldIsAvailable() {
        compose.onNodeWithText("設定").performClick()
        compose.runOnIdle {
            assertEquals(WindowManager.LayoutParams.FLAG_SECURE,
                compose.activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE)
        }
        compose.onNodeWithText("設定を閉じる").performClick()
        compose.runOnIdle {
            assertEquals(0, compose.activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE)
        }
    }
}
