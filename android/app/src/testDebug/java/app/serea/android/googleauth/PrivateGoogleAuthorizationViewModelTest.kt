package app.serea.android.googleauth

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
            requestToken = request.requestToken,
            accountName = "synthetic@example.invalid",
            accountType = PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE,
        )
        assertEquals(request.requestId, viewModel.state.value.pendingAccountSelectionId)
        assertFalse(viewModel.state.value.hasSelectedAccount)
    }

    @Test
    fun staleAccountPickerCancellationCannotCancelANewerSelection() {
        val oldProcess = PrivateGoogleAuthorizationViewModel(
            context = null,
            enabledInThisVariant = true,
            realOAuthEnabled = true,
        )
        val requestA = checkNotNull(oldProcess.beginRealAccountSelection())
        val newProcess = PrivateGoogleAuthorizationViewModel(
            context = null,
            enabledInThisVariant = true,
            realOAuthEnabled = true,
        )
        val requestB = checkNotNull(newProcess.beginRealAccountSelection())

        // Both ViewModels start their local counters at one; only the random token differs.
        assertEquals(requestA.requestId, requestB.requestId)
        assertEquals(requestA.generation, requestB.generation)
        assertFalse(newProcess.cancelRealAccountSelection(requestB.requestId, requestB.generation, requestA.requestToken))
        assertFalse(newProcess.cancelRealAccountSelection(null, null, null))
        newProcess.onRealAccountSelected(
            requestId = requestB.requestId,
            requestGeneration = requestB.generation,
            requestToken = requestA.requestToken,
            accountName = "stale@example.invalid",
            accountType = PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE,
        )
        assertEquals(requestB.requestId, newProcess.state.value.pendingAccountSelectionId)
        assertEquals(requestB.generation, newProcess.state.value.pendingAccountSelectionGeneration)
        assertEquals(requestB.requestToken, newProcess.state.value.pendingAccountSelectionToken)
        assertEquals(PrivateGoogleAuthStatus.ACCOUNT_SELECTION_PENDING, newProcess.state.value.status)
    }

    @Test
    fun cancelledConsentIdentityCannotConsumeOrCancelANewerRequest() {
        val gate = PrivateGoogleConsentIdentityGate()
        val requestA = PrivateGoogleConsentIdentity("request-A", 7L, "session-A")
        val requestB = PrivateGoogleConsentIdentity("request-B", 8L, "session-B")

        assertTrue(gate.begin(requestA))
        assertTrue(gate.cancel(requestA))
        assertTrue(gate.begin(requestB))

        assertFalse(gate.consume(requestA))
        assertTrue(gate.isPending(requestB))
        assertFalse(gate.begin(requestB))
        assertTrue(gate.consume(requestB))
        assertFalse(gate.consume(requestB))
    }

    @Test
    fun consentIdentityRequiresRequestGenerationAndAuthorizationSessionMatch() {
        val gate = PrivateGoogleConsentIdentityGate()
        val pending = PrivateGoogleConsentIdentity("request-B", 8L, "session-B")
        assertTrue(gate.begin(pending))

        assertFalse(gate.consume(pending.copy(requestId = "request-A")))
        assertFalse(gate.consume(pending.copy(generation = 7L)))
        assertFalse(gate.consume(pending.copy(authorizationSessionId = "session-A")))
        assertTrue(gate.isPending(pending))
        assertTrue(gate.consume(pending))
    }

    @Test
    fun processRestartCannotReuseAnOldConsentSessionIdentity() {
        val oldIdentity = PrivateGoogleConsentIdentity("request-1", 1L, "session-before-process-death")
        val oldProcessGate = PrivateGoogleConsentIdentityGate()
        assertTrue(oldProcessGate.begin(oldIdentity))

        // A recreated ViewModel starts with a fresh gate and a new authorization session.
        val newProcessGate = PrivateGoogleConsentIdentityGate()
        val newIdentity = PrivateGoogleConsentIdentity("request-1", 1L, "session-after-process-death")
        assertFalse(newProcessGate.consume(oldIdentity))
        assertTrue(newProcessGate.begin(newIdentity))
        assertFalse(newProcessGate.consume(oldIdentity))
        assertTrue(newProcessGate.isPending(newIdentity))
    }

    @Test
    fun recreatedViewModelCannotApplyAnOldConsentResult() {
        val oldIdentity = PrivateGoogleConsentIdentity("request-before-death", 3L, "old-session")
        val recreated = PrivateGoogleAuthorizationViewModel(
            context = null,
            enabledInThisVariant = true,
            realOAuthEnabled = true,
        )

        recreated.completeRealConsentResult(
            PrivateGoogleConsentActivityResult(oldIdentity, Activity.RESULT_OK, resultData = null),
        )

        assertEquals(PrivateGoogleAuthStatus.DISCONNECTED, recreated.state.value.status)
        assertNull(recreated.state.value.pendingRealConsentRequestId)
        assertNull(recreated.calendarSession.value)
    }

    @Test
    fun accountPickerActivityResultPreservesItsOriginalRequestIdentity() {
        val requestId = 17L
        val generation = 4L
        val result = parseGoogleAccountPickerResult(
            resultCode = Activity.RESULT_OK,
            requestId = requestId,
            generation = generation,
            requestToken = "picker-token-17",
            accountName = "synthetic-account",
            accountType = "com.google",
        )

        assertEquals(requestId, result.requestId)
        assertEquals(generation, result.generation)
        assertEquals("picker-token-17", result.requestToken)
        assertEquals(PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE, result.accountType)
        assertEquals("synthetic-account", result.accountName)
    }

    @Test
    fun accountPickerResultFailsClosedOnCancellationOrMismatchedIdentity() {
        val cancelled = parseGoogleAccountPickerResult(
            resultCode = Activity.RESULT_CANCELED,
            requestId = 17L,
            generation = 4L,
            requestToken = "picker-token-17",
            accountName = "synthetic-account",
            accountType = "com.google",
        )
        val missingIdentity = parseGoogleAccountPickerResult(
            resultCode = Activity.RESULT_OK,
            requestId = null,
            generation = 4L,
            requestToken = null,
            accountName = "synthetic-account",
            accountType = "com.google",
        )
        val wrongType = parseGoogleAccountPickerResult(
            resultCode = Activity.RESULT_OK,
            requestId = 17L,
            generation = 4L,
            requestToken = "picker-token-17",
            accountName = "synthetic-account",
            accountType = "other",
        )

        assertNull(cancelled.accountName)
        assertNull(missingIdentity.requestId)
        assertNull(missingIdentity.requestToken)
        assertNull(wrongType.accountName)
    }

    @Test
    fun privateCalendarCompositionKeepsEvenRealModeGatewayOnFakeProvider() =
        runTest(mainDispatcher.dispatcher) {
            var tokenRequests = 0
            val statusScopes = mutableListOf<Set<GoogleScope>>()
            val gateway = object : GoogleAuthorizationGateway {
                override val mode = GoogleAuthorizationMode.PLAY_SERVICES
                override val authorizationSessionId = "synthetic-session"
                override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
                    tokenRequests++
                    return Unavailable(AuthorizationFailure.NOT_CONFIGURED)
                }
                override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
                    statusScopes += scopes
                    return AuthorizationStatus(
                        service = GoogleServiceState.AVAILABLE,
                        permissions = scopes.associateWith { ScopePermission.GRANTED },
                    )
                }
            }

            val session = PrivateGoogleCalendarComposition().openAuthorizedPreview(gateway)

            assertEquals(CalendarProviderKind.FAKE, session.kind)
            assertEquals(CalendarAuthorizationState.MOCK_ONLY, session.status().state)
            session.listCalendars()
            assertEquals(listOf(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)), statusScopes)
            assertEquals(0, tokenRequests)
            session.close()
        }
}
