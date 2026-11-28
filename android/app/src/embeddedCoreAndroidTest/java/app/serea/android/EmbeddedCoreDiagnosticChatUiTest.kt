package app.serea.android

import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.lifecycle.ViewModelProvider
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import androidx.test.ext.junit.runners.AndroidJUnit4

@RunWith(AndroidJUnit4::class)
class EmbeddedCoreDiagnosticChatUiTest {
    @get:Rule
    val compose = createAndroidComposeRule<MainActivity>()

    @Test
    fun optInActivityExposesOnlySyntheticMockChatControls() {
        compose.onNode(hasText("EMBEDDED CORE DIAGNOSTIC", substring = true)).assertIsDisplayed()
        compose.onNode(hasText("合成プロンプトのみ使用してください", substring = true)).assertIsDisplayed()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
        compose.onAllNodesWithTag("open_settings").assertCountEquals(0)
        compose.onAllNodesWithTag("open_tasks").assertCountEquals(0)
    }

    @Test
    fun diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp() {
        val ownerBefore = ViewModelProvider(compose.activity)
            .get(EmbeddedCoreDiagnosticChatSessionViewModel::class.java)
        val prompt = "S1-02 synthetic runtime check"
        compose.onNodeWithTag("chat-composer").performTextInput(prompt)
        compose.onNodeWithText("送信", substring = false).performClick()
        compose.waitUntil(20_000) {
            compose.activity.chatViewModel.state.value.conversation.messages.any {
                it.role == MessageRole.ASSISTANT && it.text.contains("Demo response: I received “$prompt”.")
            }
        }
        val completed = compose.activity.chatViewModel.state.value.conversation.messages
        assertEquals(listOf(MessageRole.USER, MessageRole.ASSISTANT), completed.map { it.role })
        assertEquals(prompt, completed.first().text)
        assertTrue(completed.all { it.status == "COMPLETED" })
        assertFalse(ownerBefore.cleanupResult.isCompleted)

        compose.activityRule.scenario.recreate()
        compose.waitUntil(20_000) {
            compose.onAllNodesWithText("Mock AI応答", substring = false).fetchSemanticsNodes().isNotEmpty()
        }
        val ownerAfter = ViewModelProvider(compose.activity)
            .get(EmbeddedCoreDiagnosticChatSessionViewModel::class.java)
        assertSame(ownerBefore, ownerAfter)
        assertFalse(ownerAfter.cleanupResult.isCompleted)
        assertTrue(compose.activity.chatViewModel.state.value.conversation.messages.any { it.text == prompt })

        compose.activityRule.scenario.close()
        compose.waitUntil(10_000) { ownerAfter.cleanupResult.isCompleted }
        val cleanup = runBlocking { withTimeout(10_000) { ownerAfter.cleanupResult.await() } }
        assertTrue("Embedded Core owner cleanup failed", cleanup.isSuccess)
    }
}
