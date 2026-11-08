package app.serea.android.googleauth

import android.accounts.Account
import android.app.Activity
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.googleauth.TokenSupplyResult.Unavailable
import app.serea.android.BuildConfig
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.resetMain
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
class PrivateGoogleAuthorizationViewModelTest {
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

    @Test
    fun publicDebugBuildDoesNotExposePrivateGoogleAuthorizationUi() {
        assertFalse(BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED)
        assertFalse(BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED)
    }

    @Test
    fun explicitSyntheticAccountAndCalendarConsentOpenOnlyFakeProvider() =
        runTest(mainDispatcher.dispatcher) {
            val viewModel = PrivateGoogleAuthorizationViewModel(
                context = null,
                enabledInThisVariant = true,
                realOAuthEnabled = false,
            )

            viewModel.requestPrimaryCalendarConsent()
            advanceUntilIdle()
            assertEquals(PrivateGoogleAuthStatus.DISCONNECTED, viewModel.state.value.status)
            assertNull(viewModel.state.value.pendingMockConsent)

            viewModel.selectSyntheticAccount(SyntheticGoogleAccount.A)
            viewModel.requestPrimaryCalendarConsent()
            advanceUntilIdle()
            val preview = checkNotNull(viewModel.state.value.pendingMockConsent)
            assertEquals(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY), preview.scopes)

            viewModel.approveMockConsent(preview)
            advanceUntilIdle()
            assertEquals(PrivateGoogleAuthStatus.AUTHORIZED, viewModel.state.value.status)
            assertTrue(viewModel.state.value.message.contains("架空データ"))

            val session = checkNotNull(viewModel.openFakePrimaryCalendarPreview())
            assertEquals(CalendarProviderKind.FAKE, session.kind)
            assertEquals(CalendarAuthorizationState.MOCK_ONLY, session.status().state)

            viewModel.disconnectCalendarInApp()
            assertEquals(PrivateGoogleAuthStatus.REVOKED, viewModel.state.value.status)
            assertNull(viewModel.calendarSession.value)
            session.close()
        }

    @Test
    fun duplicateConsentTapsAreSingleFlightAndAuthorizedPreviewIsReachable() =
        runTest(mainDispatcher.dispatcher) {
            val viewModel = PrivateGoogleAuthorizationViewModel(
                context = null,
                enabledInThisVariant = true,
                realOAuthEnabled = false,
            )
            viewModel.selectSyntheticAccount(SyntheticGoogleAccount.A)

            viewModel.requestPrimaryCalendarConsent()
            viewModel.requestPrimaryCalendarConsent()
            advanceUntilIdle()
            val preview = checkNotNull(viewModel.state.value.pendingMockConsent)
            assertEquals(PrivateGoogleAuthStatus.CONSENT_PENDING, viewModel.state.value.status)

            viewModel.approveMockConsent(preview)
            viewModel.approveMockConsent(preview)
            advanceUntilIdle()

            assertEquals(PrivateGoogleAuthStatus.AUTHORIZED, viewModel.state.value.status)
            val session = checkNotNull(viewModel.openFakePrimaryCalendarPreview())
            assertEquals(CalendarProviderKind.FAKE, session.kind)
            session.close()
        }

    @Test
    fun staleConsentFromPreviousSyntheticAccountCannotAuthorizeAfterSwitch() =
        runTest(mainDispatcher.dispatcher) {
            val viewModel = PrivateGoogleAuthorizationViewModel(
                context = null,
                enabledInThisVariant = true,
                realOAuthEnabled = false,
            )
            viewModel.selectSyntheticAccount(SyntheticGoogleAccount.A)
            viewModel.requestPrimaryCalendarConsent()
            advanceUntilIdle()
            val oldPreview = checkNotNull(viewModel.state.value.pendingMockConsent)

            viewModel.selectSyntheticAccount(SyntheticGoogleAccount.B)
            viewModel.approveMockConsent(oldPreview)
            advanceUntilIdle()

            assertEquals(PrivateGoogleAuthStatus.ACCOUNT_SELECTED, viewModel.state.value.status)
            assertNull(viewModel.state.value.pendingMockConsent)
            assertNull(viewModel.openFakePrimaryCalendarPreview())
        }

    @Test
    fun cancellationAndDenialDoNotCreateCalendarProviderSession() =
        runTest(mainDispatcher.dispatcher) {
            val viewModel = PrivateGoogleAuthorizationViewModel(
                context = null,
                enabledInThisVariant = true,
                realOAuthEnabled = false,
            )
            viewModel.selectSyntheticAccount(SyntheticGoogleAccount.A)
            viewModel.requestPrimaryCalendarConsent()
            advanceUntilIdle()
            val preview = checkNotNull(viewModel.state.value.pendingMockConsent)
            viewModel.cancelMockConsent(preview)
            assertEquals(PrivateGoogleAuthStatus.CANCELLED, viewModel.state.value.status)
            assertNull(viewModel.openFakePrimaryCalendarPreview())

            viewModel.selectSyntheticAccount(SyntheticGoogleAccount.B)
            viewModel.requestPrimaryCalendarConsent()
            advanceUntilIdle()
            viewModel.denyMockConsent(checkNotNull(viewModel.state.value.pendingMockConsent))
            advanceUntilIdle()
            assertEquals(PrivateGoogleAuthStatus.ERROR, viewModel.state.value.status)
            assertNull(viewModel.calendarSession.value)
        }

    @Test
    fun publicVariantCannotStartPrivateAccountOrConsentFlow() =
        runTest(mainDispatcher.dispatcher) {
            val viewModel = PrivateGoogleAuthorizationViewModel(
                context = null,
                enabledInThisVariant = false,
                realOAuthEnabled = false,
            )
            viewModel.selectSyntheticAccount(SyntheticGoogleAccount.A)
            viewModel.requestPrimaryCalendarConsent()
            advanceUntilIdle()

            assertFalse(viewModel.state.value.hasSelectedAccount)
            assertEquals(PrivateGoogleAuthStatus.DISCONNECTED, viewModel.state.value.status)
            assertNull(viewModel.state.value.pendingMockConsent)
            assertNull(viewModel.openFakePrimaryCalendarPreview())
        }

    @Test
    fun staleAndDuplicateRealAccountPickerResultsAreIgnored() {
        val viewModel = PrivateGoogleAuthorizationViewModel(
            context = null,
            enabledInThisVariant = true,
            realOAuthEnabled = true,
        )
        val request = checkNotNull(viewModel.beginRealAccountSelection())
        assertNull(viewModel.beginRealAccountSelection())

        viewModel.onRealAccountSelected(
            requestId = request.requestId + 1,
            requestGeneration = request.generation,
            account = Account("synthetic@example.invalid", PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE),
        )
        assertEquals(request.requestId, viewModel.state.value.pendingAccountSelectionId)
        assertFalse(viewModel.state.value.hasSelectedAccount)
    }

    @Test
    fun accountPickerActivityResultPreservesItsOriginalRequestIdentity() {
        val requestId = 17L
        val generation = 4L
        val result = parseGoogleAccountPickerResult(
            resultCode = Activity.RESULT_OK,
            requestId = requestId,
            generation = generation,
            accountName = "synthetic-account",
            accountType = "com.google",
        )

        assertEquals(requestId, result.requestId)
        assertEquals(generation, result.generation)
        assertEquals(PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE, result.accountType)
        assertEquals("synthetic-account", result.accountName)
    }

    @Test
    fun accountPickerResultFailsClosedOnCancellationOrMismatchedIdentity() {
        val cancelled = parseGoogleAccountPickerResult(
            resultCode = Activity.RESULT_CANCELED,
            requestId = 17L,
            generation = 4L,
            accountName = "synthetic-account",
            accountType = "com.google",
        )
        val missingIdentity = parseGoogleAccountPickerResult(
            resultCode = Activity.RESULT_OK,
            requestId = null,
            generation = 4L,
            accountName = "synthetic-account",
            accountType = "com.google",
        )
        val wrongType = parseGoogleAccountPickerResult(
            resultCode = Activity.RESULT_OK,
            requestId = 17L,
            generation = 4L,
            accountName = "synthetic-account",
            accountType = "other",
        )

        assertNull(cancelled.accountName)
        assertNull(missingIdentity.requestId)
        assertNull(wrongType.accountName)
    }

    @Test
    fun privateCalendarCompositionKeepsEvenRealModeGatewayOnFakeProvider() =
        runTest(mainDispatcher.dispatcher) {
            val gateway = object : GoogleAuthorizationGateway {
                override val mode = GoogleAuthorizationMode.PLAY_SERVICES
                override val authorizationSessionId = "synthetic-session"
                override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>) =
                    Unavailable(AuthorizationFailure.NOT_CONFIGURED)
                override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
                    service = GoogleServiceState.AVAILABLE,
                    permissions = scopes.associateWith { ScopePermission.GRANTED },
                )
            }

            val session = PrivateGoogleCalendarComposition().openAuthorizedPreview(gateway)

            assertEquals(CalendarProviderKind.FAKE, session.kind)
            assertEquals(CalendarAuthorizationState.MOCK_ONLY, session.status().state)
            session.close()
        }
}
