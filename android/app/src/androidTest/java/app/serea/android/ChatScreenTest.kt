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

    @Test fun demoLabelAndConversationAreVisible() {
        compose.onNodeWithText("DEMO · Mock chat · No cloud AI").assertIsDisplayed()
        compose.onNodeWithText("端末内の暗号化履歴に保存されるDemo会話です。Cloud送信は無効です。").assertIsDisplayed()
        awaitComposerEnabled()
        composer().performTextInput("hello")
        compose.onNodeWithText("送信").performClick()
        compose.onNodeWithText("You", substring = true).assertIsDisplayed()
        val response = compose.onAllNodes(hasText("Demo response: I received “hello”.", substring = true))
        compose.waitUntil(5_000) { response.fetchSemanticsNodes().isNotEmpty() }
        response.assertCountEquals(1)
        response.onFirst().assertTextContains("No cloud AI", substring = true)
        compose.onNodeWithText("履歴").performClick()
        compose.onNodeWithText("保存された会話").assertIsDisplayed()
        compose.onNodeWithText("Conversation", substring = true).assertIsDisplayed()
    }

    @Test fun completedDemoConversationSurvivesActivityRecreation() {
        awaitComposerEnabled()
        composer().performTextInput("rotation-check")
        compose.onNodeWithText("送信").performClick()
        val response = compose.onAllNodes(hasText("Demo response: I received “rotation-check”.", substring = true))
        compose.waitUntil(5_000) { response.fetchSemanticsNodes().isNotEmpty() }
        response.assertCountEquals(1)
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
