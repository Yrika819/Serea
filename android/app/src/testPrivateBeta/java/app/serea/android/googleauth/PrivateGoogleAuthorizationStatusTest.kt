package app.serea.android.googleauth

import android.content.Intent
import android.content.Context
import app.serea.android.BuildConfig
import app.serea.android.googleauth.TokenSupplyResult.Unavailable
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.Description
import org.junit.runners.model.Statement

@OptIn(ExperimentalCoroutinesApi::class)
class PrivateGoogleAuthorizationStatusTest {
    class MainDispatcherRule(
        val dispatcher: TestDispatcher = StandardTestDispatcher(),
    ) : org.junit.rules.TestRule {
        override fun apply(base: Statement, description: Description) = object : Statement() {
            override fun evaluate() {
                Dispatchers.setMain(dispatcher)
                try {
                    base.evaluate()
                } finally {
                    Dispatchers.resetMain()
                }
            }
        }
    }

    @get:Rule val mainDispatcher = MainDispatcherRule()

    private class SyntheticGateway : PrivateGoogleRealAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.PLAY_SERVICES
        override val authorizationSessionId = "synthetic-status-session"
        var statusCalls = 0
        var authorizeCalls = 0
        var tokenCalls = 0

        override fun isEnabledInApp(feature: GoogleFeature) = true
        override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
            statusCalls++
            return AuthorizationStatus(
                GoogleServiceState.AVAILABLE,
                scopes.associateWith { ScopePermission.GRANTED },
            )
        }

        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            tokenCalls++
            return Unavailable(AuthorizationFailure.NOT_CONFIGURED)
        }

        override suspend fun authorize(scopes: Set<GoogleScope>): AuthorizationOutcome {
            authorizeCalls++
            return AuthorizationOutcome.Authorized(
                AccessTokenLease("synthetic-status-token", scopes, null, authorizationSessionId),
            )
        }

        override fun completeConsentResult(requestId: String, data: Intent?) =
            AuthorizationOutcome.Rejected(AuthorizationFailure.STALE_CONSENT_RESULT)
        override fun takePendingConsentIntent(requestId: String) = null
        override fun cancelConsentRequest(requestId: String) = false
        override fun disableInApp(feature: GoogleFeature) = Unit
        override suspend fun revokeGoogleGrant(): AuthorizationFailure? = null
    }

    @Test
    fun initialStatusUsesOnlyEffectiveLiveBuildGatesAndDoesNotClaimCalendarData() {
        val viewModel = PrivateGoogleAuthorizationViewModel(
            context = null,
            enabledInThisVariant = BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED,
            realOAuthEnabled = BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED,
        )
        val liveBuildConfiguration = BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED &&
            BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED

        assertEquals(liveBuildConfiguration, viewModel.opensGoogleProvider)
        assertEquals(
            !BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED || liveBuildConfiguration,
            viewModel.calendarProviderSelectionAllowed,
        )
        if (liveBuildConfiguration) {
            assertTrue(viewModel.state.value.message.contains("明示"))
            assertTrue(viewModel.state.value.message.contains("未取得"))
            assertTrue(viewModel.state.value.message.contains("認可だけではデータを取得せず"))
            assertFalse(viewModel.state.value.message.contains("Mock Provider"))
            assertFalse(viewModel.state.value.message.contains("HTTPは無効"))
        } else if (!BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED &&
            BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED
        ) {
            assertTrue(viewModel.state.value.message.contains("遮断"))
            assertFalse(viewModel.state.value.message.contains("Mock"))
            assertFalse(viewModel.state.value.message.contains("架空"))
        } else {
            assertTrue(viewModel.state.value.message.contains("Mock") || viewModel.state.value.message.contains("架空"))
        }
    }

    @Test
    fun providerSelectionEligibilityAndMockStatusRespectBuildGateCombination() =
        runTest(mainDispatcher.dispatcher) {
            if (BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED) {
                assertEquals(
                    BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED,
                    PrivateGoogleAuthorizationViewModel(
                        context = null,
                        enabledInThisVariant = BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED,
                        realOAuthEnabled = true,
                    ).opensGoogleProvider,
                )
                return@runTest
            }

            val viewModel = PrivateGoogleAuthorizationViewModel(
                context = null,
                enabledInThisVariant = BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED,
                realOAuthEnabled = false,
            )
            viewModel.selectSyntheticAccount(SyntheticGoogleAccount.A)
            viewModel.requestPrimaryCalendarConsent()
            advanceUntilIdle()
            viewModel.approveMockConsent(checkNotNull(viewModel.state.value.pendingMockConsent))
            advanceUntilIdle()

            assertEquals(PrivateGoogleAuthStatus.AUTHORIZED, viewModel.state.value.status)
            assertNull(viewModel.calendarSession.value)
            if (BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED) {
                assertFalse(viewModel.opensGoogleProvider)
                assertFalse(viewModel.calendarProviderSelectionAllowed)
                assertTrue(viewModel.state.value.message.contains("遮断"))
                assertFalse(viewModel.state.value.message.contains("架空データです"))
            } else {
                assertTrue(viewModel.calendarProviderSelectionAllowed)
                assertTrue(viewModel.state.value.message.contains("架空データです"))
            }
            viewModel.disconnectCalendarInApp()
            assertNull(viewModel.calendarSession.value)
            if (BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED) {
                assertTrue(viewModel.state.value.message.contains("遮断"))
                assertFalse(viewModel.state.value.message.contains("ProviderはFake"))
            } else {
                assertTrue(viewModel.state.value.message.contains("Demo") || viewModel.state.value.message.contains("Fake"))
            }
        }

    @Test
    fun confirmedOAuthDoesNotClaimCalendarDataWasReadOrOpenAProvider() =
        runTest(mainDispatcher.dispatcher) {
            val gateway = SyntheticGateway()
            if (!BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED) {
                val viewModel = PrivateGoogleAuthorizationViewModel(
                    context = null,
                    enabledInThisVariant = BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED,
                    realOAuthEnabled = BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED,
                    realGatewayFactory = { _: Context?, _: String -> gateway },
                )
                assertNull(viewModel.beginRealAccountSelection())
                assertEquals(PrivateGoogleAuthStatus.DISCONNECTED, viewModel.state.value.status)
                assertEquals(0, gateway.authorizeCalls)
                assertEquals(0, gateway.statusCalls)
                assertEquals(0, gateway.tokenCalls)
                return@runTest
            }

            val viewModel = PrivateGoogleAuthorizationViewModel(
                context = null,
                enabledInThisVariant = BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED,
                realOAuthEnabled = BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED,
                realGatewayFactory = { _: Context?, _: String -> gateway },
            )
            val accountSelection = checkNotNull(viewModel.beginRealAccountSelection())
            viewModel.onRealAccountSelected(
                accountSelection.requestId,
                accountSelection.generation,
                accountSelection.requestToken,
                "synthetic-owner@example.invalid",
                PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE,
            )
            viewModel.requestPrimaryCalendarConsent()
            advanceUntilIdle()

            val message = viewModel.state.value.message
            assertEquals(PrivateGoogleAuthStatus.AUTHORIZED, viewModel.state.value.status)
            assertTrue(message.contains("認可操作自体では予定データを取得しません"))
            assertTrue(message.contains("未取得"))
            assertFalse(message.contains("Google HTTPは無効"))
            if (BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED) {
                assertFalse(message.contains("Mock Provider"))
            } else {
                assertTrue(message.contains("Mock") || message.contains("架空"))
            }
            assertNull("authorization alone must not construct a Calendar provider", viewModel.calendarSession.value)
            assertEquals(1, gateway.authorizeCalls)
            assertEquals(1, gateway.statusCalls)
            assertEquals("authorization must not acquire a Calendar token", 0, gateway.tokenCalls)

            val session = checkNotNull(viewModel.openPrimaryCalendar())
            assertEquals(
                if (BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED) {
                    app.serea.android.calendar.CalendarProviderKind.GOOGLE_READ_ONLY
                } else {
                    app.serea.android.calendar.CalendarProviderKind.FAKE
                },
                session.kind,
            )
            assertTrue(viewModel.state.value.message.contains("Calendar画面"))
            assertFalse("post-open status must not keep the stale not-yet-read claim", viewModel.state.value.message.contains("未取得"))
            assertEquals("opening a provider does not acquire a Calendar token", 0, gateway.tokenCalls)

            viewModel.disconnectCalendarInApp()
            assertNull(viewModel.calendarSession.value)
            if (BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED) {
                assertTrue(viewModel.state.value.message.contains("Live"))
                assertFalse(viewModel.state.value.message.contains("ProviderはFake"))
            } else {
                assertTrue(viewModel.state.value.message.contains("Demo") || viewModel.state.value.message.contains("Fake"))
            }
        }
}
