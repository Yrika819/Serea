package app.serea.android

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.isEditable
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.test.ext.junit.runners.AndroidJUnit4
import android.view.WindowManager
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ChatScreenTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    private fun awaitComposerEnabled() {
        compose.waitUntil(5_000) {
            compose.onAllNodes(isEditable()).fetchSemanticsNodes().any {
                !it.config.contains(SemanticsProperties.Disabled)
            }
        }
    }

    @Test fun demoLabelAndConversationAreVisible() {
        compose.onNodeWithText("DEMO · Mock chat · No cloud AI").assertIsDisplayed()
        compose.onNodeWithText("端末内の暗号化履歴に保存されるDemo会話です。Cloud送信は無効です。").assertIsDisplayed()
        awaitComposerEnabled()
        compose.onNodeWithText("Message").performTextInput("hello")
        compose.onNodeWithText("送信").performClick()
        compose.onNodeWithText("You", substring = true).assertIsDisplayed()
        compose.waitUntil(5_000) {
            compose.onAllNodes(androidx.compose.ui.test.hasText("Demo response: I received “hello”. No cloud AI was contacted.")).fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Demo response: I received “hello”. No cloud AI was contacted.")
            .assertTextContains("No cloud AI", substring = true)
        compose.onNodeWithText("履歴").performClick()
        compose.onNodeWithText("保存された会話").assertIsDisplayed()
        compose.onNodeWithText("Conversation", substring = true).assertIsDisplayed()
    }

    @Test fun completedDemoConversationSurvivesActivityRecreation() {
        awaitComposerEnabled()
        compose.onNodeWithText("Message").performTextInput("rotation-check")
        compose.onNodeWithText("送信").performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodes(androidx.compose.ui.test.hasText("Demo response: I received “rotation-check”. No cloud AI was contacted."))
                .fetchSemanticsNodes().isNotEmpty()
        }
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("rotation-check").assertIsDisplayed()
        compose.onNodeWithText("Demo response: I received “rotation-check”. No cloud AI was contacted.").assertIsDisplayed()
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
