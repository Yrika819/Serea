package app.serea.android

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.lifecycle.ViewModelProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.gmail.GmailProviderKind
import app.serea.android.googleauth.GoogleAuthorizationDemoViewModel
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** End-to-end coverage for the local-only authorization walkthrough in the app Settings UI. */
@RunWith(AndroidJUnit4::class)
class GoogleAuthorizationDemoUiTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    private fun openAuthorizationDemo() {
        compose.onNodeWithTag("destination_settings").performClick()
        compose.onNodeWithTag("google_auth_demo_heading").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("google_auth_demo_calendar_scope").performScrollTo()
            .assertTextContains("calendar.events.readonly", substring = true)
        compose.onNodeWithText("Calendarの利用目的: 架空の予定一覧と詳細を表示するだけです。", substring = true)
            .performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("google_auth_demo_account_account_a").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText(
            "実Googleアカウント、OAuth画面、ネットワーク、メールや予定データは使用しません。",
            substring = true,
        )
            .performScrollTo().assertIsDisplayed()
    }

    @Test
    fun selectedSyntheticAccountIsBoundToTheConsentPreviewAndRevokedOnSwitch() {
        openAuthorizationDemo()

        compose.onNodeWithTag("google_auth_demo_account_account_b").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_message").performScrollTo()
            .assertTextContains("架空アカウント Bを選択しました", substring = true)
        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().performClick()

        compose.onNodeWithText("Calendarの認可プレビュー").assertIsDisplayed()
        compose.onNodeWithText("選択中: 架空アカウント B").assertIsDisplayed()
        compose.onNodeWithText("目的: Calendarの予定一覧・詳細を表示する").assertIsDisplayed()
        compose.onNodeWithText("許可する操作: 読み取り専用。予定の作成、変更、削除はしません。").assertIsDisplayed()
        compose.onNodeWithText("Chatへ渡す場合は別途明示確認が必要です。Cloud送信はこの許可に含まれません。").assertIsDisplayed()
        compose.onNodeWithText("Calendar 予定読み取り専用: https://www.googleapis.com/auth/calendar.events.readonly")
            .assertIsDisplayed()
        compose.onNodeWithTag("google_auth_demo_consent_grant").performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("承認済み（デモのみ）", substring = true)

        compose.onNodeWithTag("google_auth_demo_account_account_a").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("失効（デモ）", substring = true)
    }

    @Test
    fun consentCanBeGrantedDeniedAndCancelledWithoutRealGoogleConnection() {
        openAuthorizationDemo()
        assertEquals(GmailProviderKind.FAKE, compose.activity.shellProviders.gmail.kind)
        assertEquals(CalendarProviderKind.FAKE, compose.activity.shellProviders.calendar.kind)

        compose.onNodeWithTag("google_auth_demo_request_gmail").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_cancel").performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("承認が必要（デモ）", substring = true)

        compose.onNodeWithTag("google_auth_demo_request_gmail").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_deny").performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("利用不可（デモ）", substring = true)
        compose.onNodeWithTag("google_auth_demo_message").performScrollTo()
            .assertTextContains("DENIED", substring = true)

        compose.onNodeWithTag("google_auth_demo_request_gmail").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_grant").performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("承認済み（デモのみ）", substring = true)
        compose.onNodeWithTag("google_auth_demo_message").performScrollTo()
            .assertTextContains("実データは取得していません", substring = true)
        assertEquals(GmailProviderKind.FAKE, compose.activity.shellProviders.gmail.kind)
        assertEquals(CalendarProviderKind.FAKE, compose.activity.shellProviders.calendar.kind)
    }

    @Test
    fun accountSwitchExpiryAndSharedRevocationInvalidateDemoAuthorization() {
        openAuthorizationDemo()
        compose.onNodeWithTag("google_auth_demo_request_gmail").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_grant").performClick()
        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_grant").performClick()

        compose.onNodeWithTag("google_auth_demo_switch_account").performScrollTo().performClick()
        compose.onNodeWithText("架空アカウント B", substring = true).performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("失効（デモ）", substring = true)

        compose.onNodeWithTag("google_auth_demo_request_gmail").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_grant").performClick()
        compose.onNodeWithTag("google_auth_demo_expire").performScrollTo().assertIsEnabled().performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("期限切れ（デモ）", substring = true)

        compose.onNodeWithTag("google_auth_demo_revoke").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("失効（デモ）", substring = true)
    }

    @Test
    fun inAppDisconnectDoesNotRevokeTheSharedDemoGrant() {
        openAuthorizationDemo()
        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_grant").performClick()
        compose.onNodeWithTag("google_auth_demo_disconnect_calendar").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("切断（デモ）", substring = true)
        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().assertIsNotEnabled()
        compose.onNodeWithTag("google_auth_demo_reconnect_calendar").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_grant").assertIsDisplayed().performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("承認済み（デモのみ）", substring = true)
    }

    @Test
    fun expiredCalendarReconnectShowsFreshPreviewAndWaitsForConfirm() {
        openAuthorizationDemo()
        val viewModel = ViewModelProvider(compose.activity)[GoogleAuthorizationDemoViewModel::class.java]

        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_grant").performClick()
        val expiredSession = checkNotNull(viewModel.calendarDemoSession.value)

        compose.onNodeWithTag("google_auth_demo_expire").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("期限切れ（デモ）", substring = true)
        assertNull(viewModel.calendarDemoSession.value)
        assertTrue(runCatching { runBlocking { expiredSession.listCalendars() } }.isFailure)

        // The same account can recover through its normal consent action. Opening the preview
        // does not restore the old session; only an explicit confirm creates a fresh Fake session.
        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().performClick()
        compose.onNodeWithText("Calendarの認可プレビュー").assertIsDisplayed()
        compose.onNodeWithText("選択中: 架空アカウント A").assertIsDisplayed()
        compose.onNodeWithText("Calendar 予定読み取り専用: https://www.googleapis.com/auth/calendar.events.readonly")
            .assertIsDisplayed()
        assertNull(viewModel.calendarDemoSession.value)
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("承認が必要（デモ）", substring = true)

        compose.onNodeWithTag("google_auth_demo_consent_grant").performClick()
        compose.waitForIdle()
        val reauthorizedSession = checkNotNull(viewModel.calendarDemoSession.value)
        assertNotSame(expiredSession, reauthorizedSession)
        assertEquals(CalendarProviderKind.FAKE, reauthorizedSession.kind)
        assertEquals("架空アカウント A", viewModel.state.value.selectedAccount.displayName)
        assertEquals(1, viewModel.state.value.accountGeneration)
    }
}
