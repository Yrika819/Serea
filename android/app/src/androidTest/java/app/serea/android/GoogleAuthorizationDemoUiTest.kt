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
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.gmail.GmailProviderKind
import org.junit.Assert.assertEquals
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
        compose.onNodeWithText(
            "実Googleアカウント、OAuth画面、ネットワーク、メールや予定データは使用しません。",
            substring = true,
        )
            .performScrollTo().assertIsDisplayed()
    }

    @Test
    fun consentCanBeGrantedDeniedAndCancelledWithoutRealGoogleConnection() {
        openAuthorizationDemo()
        assertEquals(GmailProviderKind.FAKE, compose.activity.shellProviders.gmail.kind)
        assertEquals(CalendarProviderKind.FAKE, compose.activity.shellProviders.calendar.kind)

        compose.onNodeWithTag("google_auth_demo_request_gmail").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_cancel").performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("未接続（デモ）", substring = true)

        compose.onNodeWithTag("google_auth_demo_request_gmail").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_consent_deny").performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("拒否（デモ）", substring = true)

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
            .assertTextContains("アプリ内切断（デモ）", substring = true)
        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().assertIsNotEnabled()
        compose.onNodeWithTag("google_auth_demo_reconnect_calendar").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("承認済み（デモのみ）", substring = true)
    }
}
