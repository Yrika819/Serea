package app.serea.android

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.hasText
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.printToLog
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToIndex
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.espresso.Espresso
import android.view.WindowManager
import android.os.ParcelFileDescriptor
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Before
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class ChatScreenTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Before fun startWithAnExplicitNewConversation() {
        awaitComposerEnabled()
        val restoredId = stateSnapshot().conversation.id
        compose.onNodeWithText("新しい会話").performClick()
        compose.waitUntil(10_000) {
            val state = stateSnapshot()
            state.conversation.id != restoredId && !state.isRestoring
        }
    }

    private fun composer() = compose.onNodeWithTag("chat-composer")

    private fun assertDisplayedWithEvidence(tag: String, assertion: () -> Unit) {
        try {
            assertion()
        } catch (failure: Throwable) {
            compose.onRoot(useUnmergedTree = true).printToLog(tag)
            throw failure
        }
    }

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

    private fun send(text: String) {
        composer().performTextInput(text)
        compose.waitUntil(30_000) { stateSnapshot().input == text }
        Espresso.closeSoftKeyboard()
        val sendButton = compose.onNodeWithText("送信", substring = false)
        try {
            sendButton.assertIsDisplayed().assertIsEnabled().performClick()
        } catch (failure: Throwable) {
            runCatching { compose.onRoot(useUnmergedTree = true).printToLog("chat-send-action") }
            throw failure
        }
        val submitted = runCatching { compose.waitUntil(10_000) {
            stateSnapshot().conversation.messages.any { it.role == MessageRole.USER && it.text == text }
        } }.isSuccess
        val state = stateSnapshot()
        assertTrue(
            "Send action did not append the message; input=${state.input}, loading=${state.isLoading}, " +
                "error=${state.errorMessage}, statuses=${state.conversation.messages.map { it.role.name + ":" + it.status }}",
            submitted,
        )
    }

    @Test fun demoLabelAndConversationTitleAreVisible() {
        awaitComposerEnabled()
        compose.onNodeWithText("新しい会話").performClick()
        awaitComposerEnabled()
        val compactChat = compose.activity.resources.configuration.fontScale >= 1.5f ||
            compose.activity.resources.configuration.screenWidthDp < 340
        assertDisplayedWithEvidence("chat-demo-label") {
            compose.onNodeWithText(
                if (compactChat) "DEMO · Cloud送信: 無効" else "DEMO · Mock会話 · クラウドAI未接続",
                substring = false,
            ).assertIsDisplayed()
        }
        send("hello")
        awaitResponse("Demo response: I received “hello”.")
        val sentIndex = stateSnapshot().conversation.messages.indexOfFirst {
            it.role == MessageRole.USER && it.text == "hello"
        }
        assertTrue("Sent user message is missing", sentIndex >= 0)
        compose.onNodeWithTag("conversation-list").performScrollToIndex(sentIndex)
        compose.onNodeWithText("あなた", substring = true).assertIsDisplayed()
        compose.onNodeWithTag("conversation-list").performScrollToIndex(stateSnapshot().conversation.messages.lastIndex)
        compose.onAllNodes(hasText("Demo response: I received “hello”.", substring = true))
            .onFirst().assertTextContains("No cloud AI", substring = true)
        compose.onNodeWithText("履歴").performClick()
        compose.waitUntil(30_000) { stateSnapshot().showingHistory }
        compose.onNodeWithText("保存された会話").assertIsDisplayed()
        compose.onNodeWithText("hello · 2件").assertIsDisplayed()
    }

    @Test fun completedDemoConversationSurvivesActivityRecreation() {
        awaitComposerEnabled()
        send("rotation-check")
        awaitResponse("Demo response: I received “rotation-check”.")
        compose.activityRule.scenario.recreate()
        compose.waitUntil(30_000) {
            val state = stateSnapshot()
            !state.isRestoring && state.conversation.messages.any { it.text == "rotation-check" }
        }
        val restored = stateSnapshot()
        val restoredUserIndex = restored.conversation.messages.indexOfFirst { it.text == "rotation-check" }
        assertTrue("Restored user message is missing", restoredUserIndex >= 0)
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("conversation-list").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("conversation-list").performScrollToIndex(restoredUserIndex)
        compose.onNodeWithText("rotation-check").assertIsDisplayed()
        compose.onNodeWithTag("conversation-list").performScrollToIndex(restored.conversation.messages.lastIndex)
        assertDisplayedWithEvidence("chat-restored-conversation") {
            val restoredResponse = compose.onAllNodes(hasText("Demo response: I received “rotation-check”.", substring = true))
            compose.waitUntil(5_000) { restoredResponse.fetchSemanticsNodes().isNotEmpty() }
            restoredResponse.assertCountEquals(1)
        }
    }

    @Test fun settingsPreventScreenshotsWhileCredentialFieldIsAvailable() {
        awaitComposerEnabled()
        compose.onNodeWithTag("open_settings").performClick()
        compose.waitUntil(30_000) {
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
        compose.onNodeWithTag("destination_back").performClick()
        compose.waitUntil(30_000) {
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
        compose.onNodeWithTag("open_settings").performClick()
        compose.onNodeWithText("Cloud接続テスト（未承認）").assertIsNotEnabled()
        compose.onNodeWithText("送信予定内容をプレビュー").performScrollTo().performClick()
        compose.onAllNodes(hasText("送信先候補", substring = true)).assertCountEquals(1)
        compose.onAllNodes(hasText("分類: UNKNOWN", substring = true)).assertCountEquals(1)
        compose.onAllNodes(hasText("予定本文: draft stays local")).assertCountEquals(1)
    }

    @Test fun modelModeAndPolicyPersistWhenSettingsAreReopened() {
        ModelSelectionSettings(compose.activity.getSharedPreferences("model_selection", android.content.Context.MODE_PRIVATE))
            .save(ModelRoutingPolicy())
        awaitComposerEnabled()
        compose.onNodeWithTag("open_settings").performClick()
        compose.onNodeWithText("手動").performScrollTo().performClick()
        compose.onNodeWithText("費用優先").performScrollTo().performClick()
        compose.onNodeWithText("任意の1要求費用上限").performScrollTo().performTextInput("0.05")
        compose.onNodeWithText("通貨コード").performScrollTo().performTextInput("USD")
        compose.onNodeWithText("ルーター設定を保存").performScrollTo().performClick()
        compose.onNodeWithText("選択方式: 手動 · 方針: 費用優先").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("destination_back").performClick()
        compose.onNodeWithTag("open_settings").performClick()
        compose.onNodeWithText("選択方式: 手動 · 方針: 費用優先").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("選択モデルID: なし").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("手動モデル選択: 選択可能なモデルはありません。任意のモデルIDやProvider Hostは入力できません。")
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("0.05").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("USD").performScrollTo().assertIsDisplayed()
    }

    private fun openOverflow() {
        compose.onNodeWithContentDescription("その他の操作").assertIsDisplayed().performClick()
    }

    private fun shell(command: String): String {
        val descriptor = InstrumentationRegistry.getInstrumentation().uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(descriptor).bufferedReader().use { it.readText() }.trim()
    }

    @Test fun portraitPrimaryActionsAndOverflowAreReachableAndTappable() {
        awaitComposerEnabled()
        compose.waitUntil(10_000) {
            var isPortrait = false
            compose.activityRule.scenario.onActivity {
                isPortrait = it.resources.configuration.orientation == android.content.res.Configuration.ORIENTATION_PORTRAIT
            }
            isPortrait
        }
        val initialId = stateSnapshot().conversation.id
        compose.onNodeWithText("新しい会話").assertIsDisplayed().performClick()
        compose.waitUntil(10_000) { stateSnapshot().conversation.id != initialId && !stateSnapshot().isRestoring }
        compose.onNodeWithText("履歴").assertIsDisplayed().performClick()
        compose.waitUntil(10_000) { stateSnapshot().showingHistory }
        compose.onNodeWithText("その他の操作").assertIsDisplayed()
        openOverflow()
        compose.onNodeWithText("この会話を削除").assertIsDisplayed()
        compose.onNodeWithText("全履歴を削除").assertIsDisplayed()
        openOverflow()
        compose.onNodeWithTag("open_settings").assertIsDisplayed().performClick()
        compose.onNodeWithTag("destination_back").performClick()
    }

    @Test fun currentConversationDeletionRequiresConfirmationAndCancelKeepsMessages() {
        awaitComposerEnabled()
        send("keep on cancel")
        awaitResponse("Demo response: I received “keep on cancel”.")
        val before = stateSnapshot()
        openOverflow()
        compose.onNodeWithText("この会話を削除").performClick()
        assertDisplayedWithEvidence("chat-current-delete-confirmation") {
            compose.onNodeWithText("この会話を削除しますか？").assertIsDisplayed()
        }
        compose.onNodeWithText("対象: keep on cancel", substring = true).performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("この操作はアプリ内から取り消せません。", substring = false)
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("キャンセル").performClick()
        assertEquals(before.conversation.id, stateSnapshot().conversation.id)
        assertEquals(before.conversation.messages, stateSnapshot().conversation.messages)
    }

    @Test fun confirmedCurrentConversationDeletionRemovesSelectedConversation() {
        awaitComposerEnabled()
        send("delete this conversation")
        awaitResponse("Demo response: I received “delete this conversation”.")
        val deletedId = stateSnapshot().conversation.id
        openOverflow()
        compose.onNodeWithText("この会話を削除").performClick()
        assertDisplayedWithEvidence("chat-current-delete-confirmation") {
            compose.onNodeWithText("この会話を削除しますか？").assertIsDisplayed()
        }
        compose.onNodeWithText("対象: delete this conversation", substring = false).performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("この会話を削除").performClick()
        compose.waitUntil(30_000) { !stateSnapshot().isRestoring && stateSnapshot().conversation.id != deletedId }
        assertTrue(stateSnapshot().conversation.messages.isEmpty())
        assertDisplayedWithEvidence("chat-current-delete-status") {
            compose.onNodeWithText("この会話を削除しました。").assertIsDisplayed()
        }
    }

    @Test fun deleteAllConfirmationCancelKeepsConversationAndConfirmClearsHistory() {
        awaitComposerEnabled()
        send("history remains until confirmation")
        awaitResponse("Demo response: I received “history remains until confirmation”.")
        val originalId = stateSnapshot().conversation.id
        openOverflow()
        compose.onNodeWithText("全履歴を削除").performClick()
        assertDisplayedWithEvidence("chat-delete-all-confirmation") {
            compose.onNodeWithText("全会話履歴を削除しますか？").assertIsDisplayed()
        }
        compose.onNodeWithText("すべてのローカル会話履歴が削除されます。", substring = false)
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("この操作はアプリ内から取り消せません。", substring = false)
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("キャンセル").performClick()
        assertEquals(originalId, stateSnapshot().conversation.id)
        assertTrue(stateSnapshot().conversation.messages.isNotEmpty())
        openOverflow()
        compose.onNodeWithText("全履歴を削除").performClick()
        assertDisplayedWithEvidence("chat-delete-all-confirmation") {
            compose.onNodeWithText("全会話履歴を削除しますか？").assertIsDisplayed()
        }
        compose.onNodeWithText("すべてのローカル会話履歴が削除されます。", substring = false)
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("この操作はアプリ内から取り消せません。", substring = false)
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("全履歴を削除").performClick()
        compose.waitUntil(30_000) { stateSnapshot().history.isEmpty() && !stateSnapshot().isRestoring }
        assertTrue(stateSnapshot().conversation.messages.isEmpty())
        assertDisplayedWithEvidence("chat-delete-all-status") {
            compose.onNodeWithText("全会話履歴を削除しました。").assertIsDisplayed()
        }
    }

    @Test fun openDeletionConfirmationSurvivesActivityRecreationWithoutDeleting() {
        awaitComposerEnabled()
        val currentId = stateSnapshot().conversation.id
        openOverflow()
        compose.onNodeWithText("この会話を削除").performClick()
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("この会話を削除しますか？").assertIsDisplayed()
        assertEquals(currentId, stateSnapshot().conversation.id)
        compose.onNodeWithText("キャンセル").performClick()
        assertEquals(currentId, stateSnapshot().conversation.id)
    }

    @Test fun landscapeKeepsPrimaryAndOverflowActionsTappable() {
        awaitComposerEnabled()
        compose.activity.requestedOrientation = android.content.pm.ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        compose.waitUntil(10_000) {
            var isLandscape = false
            compose.activityRule.scenario.onActivity {
                isLandscape = it.resources.configuration.orientation == android.content.res.Configuration.ORIENTATION_LANDSCAPE
            }
            isLandscape
        }
        compose.onNodeWithText("新しい会話").assertIsDisplayed().performClick()
        awaitComposerEnabled()
        compose.onNodeWithTag("open_settings").assertIsDisplayed()
        openOverflow()
        compose.onNodeWithText("全履歴を削除").assertIsDisplayed().performClick()
        compose.onNodeWithText("全会話履歴を削除しますか？").assertIsDisplayed()
        compose.onNodeWithText("キャンセル").performClick()
    }

    @Test fun smallScreenKeepsDeletionActionsReachableAndTappable() {
        awaitComposerEnabled()
        shell("wm size 320x640")
        shell("wm density 160")
        try {
            compose.waitUntil(15_000) {
                var widthDp = 0
                compose.activityRule.scenario.onActivity { widthDp = it.resources.configuration.screenWidthDp }
                widthDp in 300..340
            }
            compose.onNodeWithText("新しい会話").assertIsDisplayed().performClick()
            awaitComposerEnabled()
            openOverflow()
            compose.onNodeWithText("全履歴を削除").assertIsDisplayed().performClick()
            compose.onNodeWithText("全会話履歴を削除しますか？").assertIsDisplayed()
            compose.onNodeWithText("キャンセル").performClick()
            composer().performClick()
            compose.waitForIdle()
            openOverflow()
            compose.onNodeWithText("全履歴を削除").assertIsDisplayed().performClick()
            compose.onNodeWithText("全会話履歴を削除しますか？").assertIsDisplayed()
            compose.onNodeWithText("キャンセル").performClick()
        } finally {
            shell("wm size reset")
            shell("wm density reset")
            compose.waitUntil(15_000) {
                var width = 0
                compose.activityRule.scenario.onActivity { width = it.resources.configuration.screenWidthDp }
                width > 340
            }
        }
    }

    @Test fun fontScaleTwoKeepsDeletionActionsReachableAndTappable() {
        awaitComposerEnabled()
        val originalFontScaleSetting = shell("settings get system font_scale")
        val originalScale = originalFontScaleSetting.toFloatOrNull()
            ?: compose.activity.resources.configuration.fontScale
        check(originalFontScaleSetting == "null" || originalFontScaleSetting.toFloatOrNull() != null) {
            "Unable to read the original system font scale setting"
        }
        try {
            shell("settings put system font_scale 2.0")
            compose.waitUntil(15_000) {
                var fontScale = 1f
                compose.activityRule.scenario.onActivity { fontScale = it.resources.configuration.fontScale }
                fontScale >= 1.99f
            }
            compose.onNodeWithText("新しい会話").assertIsDisplayed().performClick()
            awaitComposerEnabled()
            openOverflow()
            compose.onNodeWithText("この会話を削除").assertIsDisplayed().performClick()
            compose.onNodeWithText("この会話を削除しますか？").assertIsDisplayed()
            compose.onNodeWithText("キャンセル").performClick()
        } finally {
            if (originalFontScaleSetting == "null") shell("settings delete system font_scale")
            else shell("settings put system font_scale $originalFontScaleSetting")
            compose.waitUntil(15_000) {
                var fontScale = 0f
                compose.activityRule.scenario.onActivity { fontScale = it.resources.configuration.fontScale }
                kotlin.math.abs(fontScale - originalScale) < 0.02f
            }
        }
    }

    @Test fun emptySoftLimitFieldShowsValidationAndDisablesSave() {
        awaitComposerEnabled()
        compose.onNodeWithTag("open_settings").performClick()
        val dailyRequests = compose.onNodeWithText("日次requests")
        dailyRequests.performScrollTo()
        dailyRequests.performTextClearance()
        val error = compose.onNodeWithText("日次requests はLong範囲内の整数で入力してください。")
        error.performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("ソフト上限を保存").performScrollTo().assertIsNotEnabled()
    }

    @Test fun correctedSoftLimitInputCanBeSavedAndRestored() {
        awaitComposerEnabled()
        compose.onNodeWithTag("open_settings").performClick()
        val dailyRequests = compose.onNodeWithText("日次requests")
        dailyRequests.performScrollTo()
        dailyRequests.performTextClearance()
        dailyRequests.performTextInput("21")
        compose.onNodeWithText("ソフト上限を保存").performScrollTo().assertIsEnabled().performClick()
        compose.waitUntil(10_000) { compose.onAllNodes(hasText("Soft Limit設定を保存しました", substring = true)).fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithTag("destination_back").performClick()
        compose.onNodeWithTag("open_settings").performClick()
        compose.onNodeWithText("日次requests").performScrollTo().assertTextContains("21")
    }

    @Test fun softLimitRejectsZeroNegativeNonNumericOverflowAndDailyMonthlyMismatch() {
        awaitComposerEnabled()
        compose.onNodeWithTag("open_settings").performClick()
        fun verifyInvalid(value: String, expectedError: String) {
            val dailyRequests = compose.onNodeWithText("日次requests")
            dailyRequests.performScrollTo()
            dailyRequests.performTextClearance()
            dailyRequests.performTextInput(value)
            compose.onNodeWithText(expectedError).performScrollTo().assertIsDisplayed()
            compose.onNodeWithText("ソフト上限を保存").performScrollTo().assertIsNotEnabled()
        }
        verifyInvalid("0", "日次requests は1以上で入力してください。")
        verifyInvalid("-1", "日次requests は1以上で入力してください。")
        verifyInvalid("abc", "日次requests はLong範囲内の整数で入力してください。")
        verifyInvalid("9223372036854775808", "日次requests はLong範囲内の整数で入力してください。")
        verifyInvalid("101", "日次requestsは月次requests以下にしてください。")
        val dailyRequests = compose.onNodeWithText("日次requests")
        dailyRequests.performScrollTo()
        dailyRequests.performTextClearance()
        dailyRequests.performTextInput("20")
        val dailyTokens = compose.onNodeWithText("日次tokens")
        dailyTokens.performScrollTo()
        dailyTokens.performTextClearance()
        dailyTokens.performTextInput("50001")
        compose.onNodeWithText("日次tokensは月次tokens以下にしてください。").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("ソフト上限を保存").performScrollTo().assertIsNotEnabled()
    }
}
