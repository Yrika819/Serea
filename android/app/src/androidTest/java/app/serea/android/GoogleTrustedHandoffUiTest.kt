package app.serea.android

import androidx.compose.ui.test.assertDoesNotExist
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.calendar.CalendarProviderSessions
import app.serea.android.calendar.ui.CalendarAgendaScreen
import app.serea.android.gmail.GmailProviderSessions
import app.serea.android.gmail.ui.GmailScreen
import app.serea.android.gmail.ui.GmailViewModel
import app.serea.android.handoff.GoogleTrustedHandoffCoordinator
import app.serea.android.handoff.HandoffResult
import app.serea.android.handoff.TrustedGoogleHandoff
import app.serea.android.handoff.trustedHandoffSource
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CancellationException
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.time.ZoneId

@RunWith(AndroidJUnit4::class)
class GoogleTrustedHandoffUiTest {
    @get:Rule val compose = createComposeRule()

    @Test fun fakeGmailPreviewCanBeCancelledOrConfirmedForLocalChat() {
        val session = GmailProviderSessions().openFake()
        val viewModel = GmailViewModel(session, session)
        val coordinator = GoogleTrustedHandoffCoordinator(session.trustedHandoffSource())
        var delivered: TrustedGoogleHandoff? = null
        compose.setContent {
            SereaTheme {
                GmailScreen(
                    viewModel = viewModel,
                    onBack = {},
                    conversationId = "gmail-conversation",
                    conversationGeneration = 1,
                    onCreateHandoffPreview = coordinator::preparePreview,
                    onHandoffConfirmed = { preview ->
                        when (val result = coordinator.confirmLocalOnly(preview, "gmail-conversation", 1)) {
                            is HandoffResult.Accepted -> { delivered = result.value; true }
                            is HandoffResult.Rejected -> false
                        }
                    },
                    onCancelHandoffPreview = { coordinator.cancelPreview(it) },
                )
            }
        }

        compose.waitUntil(5_000) {
            compose.onNodeWithText("Project Finch schedule").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Project Finch schedule").performClick()
        compose.onNodeWithText("このメールをChatへ渡す").assertIsDisplayed().performClick()
        compose.onNodeWithText("Chatへ渡す内容を確認").assertIsDisplayed()
        compose.onNodeWithText("送信先: この端末内のローカルChat").assertIsDisplayed()
        compose.onNodeWithText("キャンセル").performClick()
        compose.onNodeWithText("Chatへ渡す内容を確認").assertDoesNotExist()
        assertNull(delivered)

        compose.onNodeWithText("このメールをChatへ渡す").performClick()
        compose.onNodeWithText("端末内で処理してChatへ渡す").performClick()
        compose.waitUntil(5_000) { delivered != null }
        assertEquals("GMAIL", delivered?.service?.name)
        assertEquals("LOCAL_PROCESSING_ONLY", delivered?.consentState?.name)
        assertEquals("gmail-conversation", delivered?.conversationId)
        session.close()
    }

    @Test fun fakeCalendarPreviewCanBeCancelledOrConfirmedForLocalChat() {
        val session = CalendarProviderSessions().openFake()
        val coordinator = GoogleTrustedHandoffCoordinator(session.trustedHandoffSource())
        var delivered: TrustedGoogleHandoff? = null
        compose.setContent {
            SereaTheme {
                CalendarAgendaScreen(
                    onBack = {},
                    provider = session,
                    authorizationPort = session,
                    clock = app.serea.android.calendar.FakeCalendarProvider().clock,
                    displayZone = ZoneId.of("UTC"),
                    conversationId = "calendar-conversation",
                    conversationGeneration = 2,
                    onCreateHandoffPreview = coordinator::preparePreview,
                    onHandoffConfirmed = { preview ->
                        when (val result = coordinator.confirmLocalOnly(preview, "calendar-conversation", 2)) {
                            is HandoffResult.Accepted -> { delivered = result.value; true }
                            is HandoffResult.Rejected -> false
                        }
                    },
                    onCancelHandoffPreview = { coordinator.cancelPreview(it) },
                )
            }
        }

        compose.waitUntil(5_000) {
            compose.onNodeWithText("Design review").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Design review").performClick()
        compose.onNodeWithText("この予定をChatへ渡す").assertIsDisplayed().performClick()
        compose.onNodeWithText("Chatへ渡す内容を確認").assertIsDisplayed()
        compose.onNodeWithText("送信先: この端末内のローカルChat").assertIsDisplayed()
        compose.onNodeWithText("キャンセル").performClick()
        compose.onNodeWithText("Chatへ渡す内容を確認").assertDoesNotExist()
        assertNull(delivered)

        compose.onNodeWithText("この予定をChatへ渡す").performClick()
        compose.onNodeWithText("端末内で処理してChatへ渡す").performClick()
        compose.waitUntil(5_000) { delivered != null }
        assertEquals("CALENDAR", delivered?.service?.name)
        assertEquals("LOCAL_PROCESSING_ONLY", delivered?.consentState?.name)
        assertEquals("calendar-conversation", delivered?.conversationId)
        assertNotNull(delivered?.selectedContent)
        session.close()
    }

    @Test fun gmailOutOfOrderPreviewCallbackCannotReplaceNewestSelection() {
        val session = GmailProviderSessions().openFake()
        val viewModel = GmailViewModel(session, session)
        val coordinator = GoogleTrustedHandoffCoordinator(session.trustedHandoffSource())
        val firstStarted = CompletableDeferred<Unit>()
        val firstRelease = CompletableDeferred<Unit>()
        val secondStarted = CompletableDeferred<Unit>()
        val secondRelease = CompletableDeferred<Unit>()
        compose.setContent {
            SereaTheme {
                GmailScreen(
                    viewModel = viewModel,
                    onBack = {},
                    conversationId = "gmail-conversation",
                    conversationGeneration = 1,
                    onCreateHandoffPreview = { request ->
                        if (request.candidate.sourceItemIdentifier == "fake-msg-001") {
                            firstStarted.complete(Unit)
                            firstRelease.await()
                        } else {
                            secondStarted.complete(Unit)
                            secondRelease.await()
                        }
                        coordinator.preparePreview(request)
                    },
                    onHandoffConfirmed = { false },
                    onCancelHandoffPreview = { coordinator.cancelPreview(it) },
                )
            }
        }

        compose.waitUntil(5_000) {
            compose.onNodeWithText("Project Finch schedule").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Project Finch schedule").performClick()
        compose.onNodeWithText("このメールをChatへ渡す").performClick()
        compose.waitUntil(5_000) { firstStarted.isCompleted }
        compose.onNodeWithText("← 戻る").performClick()
        compose.onNodeWithText("Re: Project Finch schedule").performClick()
        compose.onNodeWithText("このメールをChatへ渡す").performClick()
        compose.waitUntil(5_000) { secondStarted.isCompleted }
        secondRelease.complete(Unit)
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("対象: Re: Project Finch schedule").fetchSemanticsNodes().isNotEmpty()
        }
        firstRelease.complete(Unit)
        compose.waitUntil(5_000) { coordinator.pendingPreviewCount() == 1 }
        compose.onNodeWithText("対象: Re: Project Finch schedule").assertIsDisplayed()
        compose.onNodeWithText("対象: Project Finch schedule").assertDoesNotExist()
        compose.onNodeWithText("キャンセル").performClick()
        compose.waitUntil(5_000) { coordinator.pendingPreviewCount() == 0 }
        session.close()
    }

    @Test fun gmailProviderCancellationFailsClosedAndDialogRemainsCancellable() {
        val session = GmailProviderSessions().openFake()
        val viewModel = GmailViewModel(session, session)
        val coordinator = GoogleTrustedHandoffCoordinator(session.trustedHandoffSource())
        var previewAttempts = 0
        compose.setContent {
            SereaTheme {
                GmailScreen(
                    viewModel = viewModel,
                    onBack = {},
                    conversationId = "gmail-conversation",
                    conversationGeneration = 1,
                    onCreateHandoffPreview = { request ->
                        previewAttempts++
                        if (previewAttempts == 1) throw CancellationException("fake session revoked")
                        coordinator.preparePreview(request)
                    },
                    onHandoffConfirmed = { throw CancellationException("fake session revoked during confirm") },
                    onCancelHandoffPreview = { coordinator.cancelPreview(it) },
                )
            }
        }

        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Project Finch schedule").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Project Finch schedule").performClick()
        compose.onNodeWithText("このメールをChatへ渡す").performClick()
        compose.onNodeWithText("セッションが失効したか、選択内容を確認できません。Fake Gmailを開き直してください。")
            .assertIsDisplayed()

        compose.onNodeWithText("このメールをChatへ渡す").performClick()
        compose.onNodeWithText("Chatへ渡す内容を確認").assertIsDisplayed()
        compose.onNodeWithText("端末内で処理してChatへ渡す").performClick()
        compose.onNodeWithText("セッションが失効したか、Chatの送信先が変わりました。Fake Gmailを開き直してください。")
            .assertIsDisplayed()
        compose.onNodeWithText("キャンセル").performClick()
        compose.onNodeWithText("Chatへ渡す内容を確認").assertDoesNotExist()
        assertEquals(0, coordinator.pendingPreviewCount())
        session.close()
    }
}
