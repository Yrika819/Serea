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
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performScrollToIndex
import androidx.test.ext.junit.runners.AndroidJUnit4
import android.view.WindowManager
import android.os.ParcelFileDescriptor
import androidx.test.platform.app.InstrumentationRegistry
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
        compose.onNodeWithText("設定を閉じる").performScrollTo().performClick()
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
        compose.onNodeWithText("設定").performClick()
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
        compose.onNodeWithText("設定").performClick()
        compose.onNodeWithText("Manual").performScrollTo().performClick()
        compose.onNodeWithText("Cost Priority").performScrollTo().performClick()
        compose.onNodeWithText("任意の1要求費用上限").performScrollTo().performTextInput("0.05")
        compose.onNodeWithText("通貨コード").performScrollTo().performTextInput("USD")
        compose.onNodeWithText("Router設定を保存").performScrollTo().performClick()
        compose.onNodeWithText("Mode: MANUAL · Policy: COST_PRIORITY").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("設定").performClick()
        compose.onNodeWithText("設定").performClick()
        compose.onNodeWithText("Mode: MANUAL · Policy: COST_PRIORITY").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("選択Model ID: なし").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Manual Model Picker: 選択可能なモデルはありません。任意のModel IDやProvider Hostは入力できません。")
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("0.05").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("USD").performScrollTo().assertIsDisplayed()
    }

    private fun openOverflow() {
        compose.onNodeWithText("その他の操作").assertIsDisplayed().performClick()
    }

    private fun shell(command: String): String {
        val descriptor = InstrumentationRegistry.getInstrumentation().uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(descriptor).bufferedReader().use { it.readText() }.trim()
    }

    @Test fun portraitPrimaryActionsAndOverflowAreReachableAndTappable() {
        awaitComposerEnabled()
        val initialId = stateSnapshot().conversation.id
        compose.onNodeWithText("新しい会話").assertIsDisplayed().performClick()
        compose.waitUntil(10_000) { stateSnapshot().conversation.id != initialId && !stateSnapshot().isRestoring }
        compose.onNodeWithText("履歴").assertIsDisplayed().performClick()
        compose.waitUntil(10_000) { stateSnapshot().showingHistory }
        compose.onNodeWithText("その他の操作").assertIsDisplayed()
        openOverflow()
        compose.onNodeWithText("この会話を削除").assertIsDisplayed()
        compose.onNodeWithText("全履歴を削除").assertIsDisplayed()
        compose.onNodeWithText("キャンセル").performClick()
        compose.onNodeWithText("設定").assertIsDisplayed().performClick()
        compose.onNodeWithText("設定を閉じる").performScrollTo().performClick()
    }

    @Test fun currentConversationDeletionRequiresConfirmationAndCancelKeepsMessages() {
        awaitComposerEnabled()
        composer().performTextInput("keep on cancel")
        compose.onNodeWithText("送信").performClick()
        awaitResponse("Demo response: I received “keep on cancel”.")
        val before = stateSnapshot()
        openOverflow()
        compose.onNodeWithText("この会話を削除").performClick()
        compose.onNodeWithText("この会話を削除しますか？").assertIsDisplayed()
        compose.onNodeWithText("選択した会話のローカル履歴を削除します。この操作はアプリ内から取り消せません。").assertIsDisplayed()
        compose.onNodeWithText("キャンセル").performClick()
        assertEquals(before.conversation.id, stateSnapshot().conversation.id)
        assertEquals(before.conversation.messages, stateSnapshot().conversation.messages)
    }

    @Test fun confirmedCurrentConversationDeletionRemovesSelectedConversation() {
        awaitComposerEnabled()
        composer().performTextInput("delete this conversation")
        compose.onNodeWithText("送信").performClick()
        awaitResponse("Demo response: I received “delete this conversation”.")
        val deletedId = stateSnapshot().conversation.id
        openOverflow()
        compose.onNodeWithText("この会話を削除").performClick()
        compose.onNodeWithText("この会話を削除しますか？").assertIsDisplayed()
        compose.onNodeWithText("この会話を削除").performClick()
        compose.waitUntil(30_000) { !stateSnapshot().isRestoring && stateSnapshot().conversation.id != deletedId }
        assertTrue(stateSnapshot().conversation.messages.isEmpty())
        compose.onNodeWithText("この会話を削除しました。").assertIsDisplayed()
    }

    @Test fun deleteAllConfirmationCancelKeepsConversationAndConfirmClearsHistory() {
        awaitComposerEnabled()
        composer().performTextInput("history remains until confirmation")
        compose.onNodeWithText("送信").performClick()
        awaitResponse("Demo response: I received “history remains until confirmation”.")
        val originalId = stateSnapshot().conversation.id
        openOverflow()
        compose.onNodeWithText("全履歴を削除").performClick()
        compose.onNodeWithText("全会話履歴を削除しますか？").assertIsDisplayed()
        compose.onNodeWithText("キャンセル").performClick()
        assertEquals(originalId, stateSnapshot().conversation.id)
        assertTrue(stateSnapshot().conversation.messages.isNotEmpty())
        openOverflow()
        compose.onNodeWithText("全履歴を削除").performClick()
        compose.onNodeWithText("全会話履歴を削除しますか？").assertIsDisplayed()
        compose.onNodeWithText("全履歴を削除").performClick()
        compose.waitUntil(30_000) { stateSnapshot().history.isEmpty() && !stateSnapshot().isRestoring }
        assertTrue(stateSnapshot().conversation.messages.isEmpty())
        compose.onNodeWithText("全会話履歴を削除しました。").assertIsDisplayed()
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
        compose.waitForIdle()
        compose.onNodeWithText("新しい会話").assertIsDisplayed().performClick()
        awaitComposerEnabled()
        compose.onNodeWithText("設定").assertIsDisplayed()
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
        }
    }

    @Test fun fontScaleTwoKeepsDeletionActionsReachableAndTappable() {
        awaitComposerEnabled()
        val originalScale = compose.activity.resources.configuration.fontScale
        shell("settings put system font_scale 2.0")
        try {
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
            shell("settings put system font_scale $originalScale")
        }
    }

    @Test fun emptySoftLimitFieldShowsValidationAndDisablesSave() {
        awaitComposerEnabled()
        compose.onNodeWithText("設定").performClick()
        val dailyRequests = compose.onNodeWithText("日次requests")
        dailyRequests.performScrollTo().performTextClearance()
        val error = compose.onNodeWithText("日次requests はLong範囲内の整数で入力してください。")
        error.performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Soft Limitを保存").performScrollTo().assertIsNotEnabled()
    }

    @Test fun correctedSoftLimitInputCanBeSavedAndRestored() {
        awaitComposerEnabled()
        compose.onNodeWithText("設定").performClick()
        compose.onNodeWithText("日次requests").performScrollTo().performTextClearance().performTextInput("21")
        compose.onNodeWithText("Soft Limitを保存").performScrollTo().assertIsEnabled().performClick()
        compose.waitUntil(10_000) { compose.onAllNodes(hasText("Soft Limit設定を保存しました", substring = true)).fetchSemanticsNodes().isNotEmpty() }
        compose.onNodeWithText("設定を閉じる").performScrollTo().performClick()
        compose.onNodeWithText("設定").performClick()
        compose.onNodeWithText("日次requests").performScrollTo().assertTextContains("21")
    }

    @Test fun softLimitRejectsZeroNegativeNonNumericOverflowAndDailyMonthlyMismatch() {
        awaitComposerEnabled()
        compose.onNodeWithText("設定").performClick()
        fun verifyInvalid(value: String, expectedError: String) {
            compose.onNodeWithText("日次requests").performScrollTo().performTextClearance().performTextInput(value)
            compose.onNodeWithText(expectedError).performScrollTo().assertIsDisplayed()
            compose.onNodeWithText("Soft Limitを保存").performScrollTo().assertIsNotEnabled()
        }
        verifyInvalid("0", "日次requests は1以上で入力してください。")
        verifyInvalid("-1", "日次requests は1以上で入力してください。")
        verifyInvalid("abc", "日次requests はLong範囲内の整数で入力してください。")
        verifyInvalid("9223372036854775808", "日次requests はLong範囲内の整数で入力してください。")
        verifyInvalid("101", "日次requestsは月次requests以下にしてください。")
        compose.onNodeWithText("日次requests").performScrollTo().performTextClearance().performTextInput("20")
        compose.onNodeWithText("日次tokens").performScrollTo().performTextClearance().performTextInput("50001")
        compose.onNodeWithText("日次tokensは月次tokens以下にしてください。").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("Soft Limitを保存").performScrollTo().assertIsNotEnabled()
    }
}
