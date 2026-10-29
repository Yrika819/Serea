package app.serea.android

import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performTextInput
import androidx.lifecycle.ViewModelProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.googleauth.GoogleAuthorizationDemoViewModel
import app.serea.android.gmail.GmailProviderKind
import java.time.Instant
import java.time.ZoneId
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Exercises the real app shell route while the Google demo provider remains Fake-only. */
@RunWith(AndroidJUnit4::class)
class GoogleAuthorizationAppIntegrationTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    private fun authorizationViewModel() = ViewModelProvider(compose.activity)
        .get(GoogleAuthorizationDemoViewModel::class.java)

    @After fun revokeDemoState() {
        runCatching { authorizationViewModel().revokeDemoGrant() }
    }

    @Test
    fun settingsConsentOpensCalendarSessionAndActivityRecreationReturnsToConsentPanel() {
        val viewModel = authorizationViewModel()
        assertEquals(GmailProviderKind.FAKE, compose.activity.shellProviders.gmail.kind)
        assertEquals(CalendarProviderKind.FAKE, compose.activity.shellProviders.calendar.kind)

        compose.onNodeWithTag("destination_settings").performClick()
        compose.onNodeWithTag("google_auth_demo_heading").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("google_auth_demo_account_account_b").performScrollTo().performClick()
        compose.onNodeWithTag("google_auth_demo_request_calendar").performScrollTo().performClick()
        compose.onNodeWithText("Calendarの認可プレビュー").assertIsDisplayed()
        compose.onNodeWithTag("google_auth_demo_consent_grant").performClick()
        compose.waitUntil(5_000) { viewModel.calendarDemoSession.value != null }

        val grantedSession = checkNotNull(viewModel.calendarDemoSession.value)
        assertEquals(CalendarProviderKind.FAKE, grantedSession.kind)
        assertEquals("demo-primary", runBlocking { grantedSession.listCalendars().first().id.value })
        compose.onNodeWithTag("google_auth_demo_open_calendar").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("google_auth_demo_calendar_destination").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("DEMO / 架空の予定").assertIsDisplayed()

        compose.onNodeWithText("指定日").performClick()
        compose.onNodeWithTag("calendar_date_input").performTextClearance()
        val reviewDate = Instant.parse("2026-10-20T00:00:00Z")
            .atZone(ZoneId.systemDefault()).toLocalDate().toString()
        compose.onNodeWithTag("calendar_date_input").performTextInput(reviewDate)
        compose.onNodeWithTag("calendar_date_input").assertTextContains(reviewDate, substring = true)
        compose.onNodeWithText("Demo Personal").assertIsSelected()
        try {
            compose.waitUntil(10_000) {
                compose.onAllNodesWithText("Design review").fetchSemanticsNodes().isNotEmpty()
            }
        } catch (failure: AssertionError) {
            throw AssertionError(
                "Design review missing for fixture instant 2026-10-20T00:00:00Z; " +
                    "deviceZone=${ZoneId.systemDefault().id}, queryDate=$reviewDate, selectedCalendar=demo-primary",
                failure,
            )
        }
        compose.onNodeWithText("Design review").assertIsDisplayed()

        // The nested route is transient across configuration recreation. The Activity ViewModel
        // keeps the Fake grant/session; the new Activity returns to Settings and can reopen it.
        compose.activityRule.scenario.recreate()
        compose.waitForIdle()
        val recreatedViewModel = authorizationViewModel()
        assertSame(viewModel, recreatedViewModel)
        assertSame(grantedSession, recreatedViewModel.calendarDemoSession.value)
        compose.onNodeWithTag("google_auth_demo_heading").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("承認済み（デモのみ）", substring = true)
        assertTrue(compose.onAllNodesWithTag("google_auth_demo_calendar_destination").fetchSemanticsNodes().isEmpty())

        // Selecting another synthetic account closes the session and removes the old agenda.
        compose.onNodeWithTag("google_auth_demo_account_account_a").performScrollTo().performClick()
        compose.waitUntil(5_000) { recreatedViewModel.calendarDemoSession.value == null }
        compose.onNodeWithTag("google_auth_demo_status").performScrollTo()
            .assertTextContains("失効（デモ）", substring = true)
        assertTrue(compose.onAllNodesWithText("Design review").fetchSemanticsNodes().isEmpty())
        assertFalse(runCatching { runBlocking { grantedSession.listCalendars() } }.isSuccess)
    }
}
