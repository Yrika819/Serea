package app.serea.android

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ChatScreenTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun demoLabelAndConversationAreVisible() {
        compose.onNodeWithText("DEMO · Mock chat · No cloud AI").assertIsDisplayed()
        compose.onNodeWithText("Start a demo conversation. Messages stay in memory and are cleared when the app process ends.").assertIsDisplayed()
        compose.onNodeWithText("Message").performTextInput("hello")
        compose.onNodeWithText("Send").performClick()
        compose.onNodeWithText("You").assertIsDisplayed()
        compose.waitUntil(5_000) {
            compose.onAllNodes(androidx.compose.ui.test.hasText("Demo response: I received “hello”. No cloud AI was contacted.")).fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Demo response: I received “hello”. No cloud AI was contacted.").assertTextContains("No cloud AI")
    }
}
