package app.serea.android.googleauth

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestDispatcher
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.calendar.CalendarReadFailure
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotSame
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.Description
import org.junit.runners.model.Statement

@OptIn(ExperimentalCoroutinesApi::class)
class GoogleAuthorizationLifecycleTest {
    class MainDispatcherRule(
        val dispatcher: TestDispatcher = StandardTestDispatcher(),
    ) : org.junit.rules.TestRule {
        override fun apply(base: Statement, description: Description): Statement = object : Statement() {
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
    fun requestedLifecycleStatesAreExplicitAndDistinct() {
        assertEquals(
            setOf(
                GoogleDemoFeatureState.DISCONNECTED,
                GoogleDemoFeatureState.AUTHORIZATION_REQUIRED,
                GoogleDemoFeatureState.AUTHORIZING,
                GoogleDemoFeatureState.AUTHORIZED,
                GoogleDemoFeatureState.EXPIRED,
                GoogleDemoFeatureState.REVOKED,
                GoogleDemoFeatureState.ERROR,
            ),
            GoogleDemoFeatureState.entries.toSet(),
        )
    }

    @Test
    fun authorizationOutcomeMapsToUserFacingLifecycleWithoutLeakingFailureDetails() {
        assertEquals(
            GoogleDemoFeatureState.AUTHORIZATION_REQUIRED,
            AuthorizationOutcome.ConsentRequired(setOf(GoogleScope.GMAIL_READ_ONLY)).toDemoFeatureState(),
        )
        assertEquals(
            GoogleDemoFeatureState.EXPIRED,
            AuthorizationOutcome.Rejected(AuthorizationFailure.EXPIRED).toDemoFeatureState(),
        )
        assertEquals(
            GoogleDemoFeatureState.REVOKED,
            AuthorizationOutcome.Rejected(AuthorizationFailure.ACCOUNT_CHANGED).toDemoFeatureState(),
        )
        assertEquals(
            GoogleDemoFeatureState.REVOKED,
            AuthorizationOutcome.Rejected(AuthorizationFailure.STALE_CONSENT_RESULT).toDemoFeatureState(),
        )
        assertEquals(
            GoogleDemoFeatureState.AUTHORIZATION_REQUIRED,
            AuthorizationOutcome.Rejected(AuthorizationFailure.CANCELLED).toDemoFeatureState(),
        )
        assertEquals(
            GoogleDemoFeatureState.ERROR,
            AuthorizationOutcome.Rejected(AuthorizationFailure.DENIED).toDemoFeatureState(),
        )
        assertEquals(
            GoogleDemoFeatureState.ERROR,
            AuthorizationOutcome.Rejected(AuthorizationFailure.NETWORK).toDemoFeatureState(),
        )
    }

    @Test
    fun fakeViewModelWalksThroughAuthorizationExpiryRevocationAndDisconnectStates() =
        runTest(mainDispatcher.dispatcher) {
            val viewModel = GoogleAuthorizationDemoViewModel(FakeGoogleAuthorizationGateway())
            assertEquals(GoogleDemoFeatureState.DISCONNECTED, viewModel.state.value.gmail)

            viewModel.requestConsent(GoogleFeature.GMAIL)
            assertEquals(GoogleDemoFeatureState.AUTHORIZING, viewModel.state.value.gmail)
            runCurrent()
            assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.gmail)
            assertEquals(GoogleFeature.GMAIL, viewModel.state.value.pendingFeature)

            viewModel.cancelPendingConsent()
            assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.gmail)
            assertEquals(null, viewModel.state.value.pendingFeature)

            viewModel.requestConsent(GoogleFeature.GMAIL)
            runCurrent()
            viewModel.denyPendingConsent()
            assertEquals(GoogleDemoFeatureState.AUTHORIZING, viewModel.state.value.gmail)
            runCurrent()
            assertEquals(GoogleDemoFeatureState.ERROR, viewModel.state.value.gmail)

            viewModel.requestConsent(GoogleFeature.GMAIL)
            runCurrent()
            viewModel.grantPendingConsent()
            assertEquals(GoogleDemoFeatureState.AUTHORIZING, viewModel.state.value.gmail)
            runCurrent()
            assertEquals(GoogleDemoFeatureState.AUTHORIZED, viewModel.state.value.gmail)

            viewModel.simulateTokenExpiry()
            assertEquals(GoogleDemoFeatureState.EXPIRED, viewModel.state.value.gmail)
            viewModel.switchDemoAccount()
            assertEquals(GoogleDemoFeatureState.REVOKED, viewModel.state.value.gmail)
            viewModel.disconnectInApp(GoogleFeature.GMAIL)
            assertEquals(GoogleDemoFeatureState.DISCONNECTED, viewModel.state.value.gmail)
            assertEquals(setOf(GoogleFeature.GMAIL), viewModel.state.value.disabledInAppFeatures)
            viewModel.reconnectInApp(GoogleFeature.GMAIL)
            assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.gmail)
            assertEquals(emptySet<GoogleFeature>(), viewModel.state.value.disabledInAppFeatures)
            viewModel.revokeDemoGrant()
            assertEquals(GoogleDemoFeatureState.REVOKED, viewModel.state.value.gmail)
            assertEquals(GoogleDemoFeatureState.REVOKED, viewModel.state.value.calendar)
        }

    @Test
    fun selectingAnotherSyntheticAccountClosesItsCalendarSessionBeforeReauthorization() =
        runTest(mainDispatcher.dispatcher) {
            val viewModel = GoogleAuthorizationDemoViewModel(FakeGoogleAuthorizationGateway())

            viewModel.requestConsent(GoogleFeature.CALENDAR)
            runCurrent()
            assertEquals(GoogleFeature.CALENDAR, viewModel.state.value.pendingFeature)
            assertEquals(null, viewModel.calendarDemoSession.value)

            viewModel.grantPendingConsent()
            viewModel.grantPendingConsent()
            runCurrent()
            val accountASession = checkNotNull(viewModel.calendarDemoSession.value)
            assertEquals(CalendarProviderKind.FAKE, accountASession.kind)
            assertEquals(CalendarAuthorizationState.MOCK_ONLY, accountASession.status().state)
            assertEquals(3, accountASession.listCalendars().size)

            viewModel.selectDemoAccount(GoogleDemoAccount.ACCOUNT_B)
            assertEquals(GoogleDemoAccount.ACCOUNT_B, viewModel.state.value.selectedAccount)
            assertEquals(GoogleDemoFeatureState.REVOKED, viewModel.state.value.calendar)
            assertEquals(null, viewModel.calendarDemoSession.value)
            assertTrue(runCatching { accountASession.listCalendars() }.exceptionOrNull() is CalendarReadFailure)

            viewModel.requestConsent(GoogleFeature.CALENDAR)
            runCurrent()
            viewModel.grantPendingConsent()
            runCurrent()
            val accountBSession = checkNotNull(viewModel.calendarDemoSession.value)
            assertNotSame(accountASession, accountBSession)
            assertEquals(CalendarProviderKind.FAKE, accountBSession.kind)
            assertEquals(3, accountBSession.listCalendars().size)
        }

    @Test
    fun reconnectAfterAppLocalDisconnectRequiresConfirmationAndReplacesTheOldCalendarSession() =
        runTest(mainDispatcher.dispatcher) {
            val gateway = FakeGoogleAuthorizationGateway()
            val viewModel = GoogleAuthorizationDemoViewModel(gateway)
            viewModel.requestConsent(GoogleFeature.CALENDAR)
            runCurrent()
            viewModel.grantPendingConsent()
            runCurrent()
            val disconnectedSession = checkNotNull(viewModel.calendarDemoSession.value)

            viewModel.disconnectInApp(GoogleFeature.CALENDAR)
            assertEquals(null, viewModel.calendarDemoSession.value)
            assertTrue(runCatching { disconnectedSession.listCalendars() }.exceptionOrNull() is CalendarReadFailure)
            assertEquals(
                ScopePermission.GRANTED,
                gateway.status(GoogleFeature.CALENDAR.scopes).permissions[GoogleScope.CALENDAR_EVENTS_READ_ONLY],
            )

            viewModel.reconnectInApp(GoogleFeature.CALENDAR)
            viewModel.requestConsent(GoogleFeature.CALENDAR)
            assertEquals(GoogleFeature.CALENDAR, viewModel.state.value.pendingFeature)
            assertEquals(GoogleFeature.CALENDAR.scopes, viewModel.state.value.pendingScopes)
            assertEquals(null, viewModel.calendarDemoSession.value)

            viewModel.grantPendingConsent()
            viewModel.grantPendingConsent()
            runCurrent()
            val reconnectedSession = checkNotNull(viewModel.calendarDemoSession.value)
            assertNotSame(disconnectedSession, reconnectedSession)
            assertEquals(CalendarProviderKind.FAKE, reconnectedSession.kind)
            assertEquals(GoogleDemoFeatureState.AUTHORIZED, viewModel.state.value.calendar)
            assertTrue(runCatching { disconnectedSession.listCalendars() }.exceptionOrNull() is CalendarReadFailure)
        }

    @Test
    fun rapidFeatureRequestsKeepTheFirstAuthorizationAndCancellationReleasesTheGuard() =
        runTest(mainDispatcher.dispatcher) {
            val viewModel = GoogleAuthorizationDemoViewModel(FakeGoogleAuthorizationGateway())
            viewModel.requestConsent(GoogleFeature.GMAIL)
            viewModel.requestConsent(GoogleFeature.CALENDAR)

            assertEquals(GoogleFeature.GMAIL, viewModel.state.value.inFlightFeature)
            assertEquals(GoogleDemoFeatureState.AUTHORIZING, viewModel.state.value.gmail)
            assertEquals(GoogleDemoFeatureState.DISCONNECTED, viewModel.state.value.calendar)

            runCurrent()
            assertEquals(null, viewModel.state.value.inFlightFeature)
            assertEquals(GoogleFeature.GMAIL, viewModel.state.value.pendingFeature)
            assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.gmail)
            assertEquals(GoogleDemoFeatureState.DISCONNECTED, viewModel.state.value.calendar)

            viewModel.cancelPendingConsent()
            assertEquals(null, viewModel.state.value.pendingFeature)
            assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.gmail)

            viewModel.requestConsent(GoogleFeature.CALENDAR)
            assertEquals(GoogleFeature.CALENDAR, viewModel.state.value.inFlightFeature)
            viewModel.cancelPendingConsent()
            assertEquals(null, viewModel.state.value.inFlightFeature)
            assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.calendar)

            viewModel.requestConsent(GoogleFeature.GMAIL)
            assertEquals(GoogleFeature.GMAIL, viewModel.state.value.inFlightFeature)
            runCurrent()
            assertEquals(null, viewModel.state.value.inFlightFeature)
            assertEquals(GoogleFeature.GMAIL, viewModel.state.value.pendingFeature)
            assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.gmail)
        }

    @Test
    fun queuedGrantCannotRestoreFakeScopesAfterSharedRevocation() = runTest(mainDispatcher.dispatcher) {
        val gateway = FakeGoogleAuthorizationGateway()
        val viewModel = GoogleAuthorizationDemoViewModel(gateway)
        viewModel.requestConsent(GoogleFeature.GMAIL)
        runCurrent()
        assertEquals(GoogleFeature.GMAIL, viewModel.state.value.pendingFeature)

        viewModel.grantPendingConsent()
        assertEquals(GoogleFeature.GMAIL, viewModel.state.value.inFlightFeature)
        viewModel.revokeDemoGrant()
        assertEquals(GoogleDemoFeatureState.REVOKED, viewModel.state.value.gmail)
        assertEquals(null, viewModel.state.value.inFlightFeature)

        runCurrent()
        assertEquals(1, gateway.revokeCount)
        assertEquals(
            ScopePermission.NEEDS_CONSENT,
            gateway.status(setOf(GoogleScope.GMAIL_READ_ONLY)).permissions[GoogleScope.GMAIL_READ_ONLY],
        )
        assertEquals(GoogleDemoFeatureState.REVOKED, viewModel.state.value.gmail)
    }

    @Test
    fun queuedGrantCannotRestoreFakeScopesAfterInAppDisconnect() = runTest(mainDispatcher.dispatcher) {
        val gateway = FakeGoogleAuthorizationGateway()
        val viewModel = GoogleAuthorizationDemoViewModel(gateway)
        viewModel.requestConsent(GoogleFeature.CALENDAR)
        runCurrent()
        assertEquals(GoogleFeature.CALENDAR, viewModel.state.value.pendingFeature)

        viewModel.grantPendingConsent()
        assertEquals(GoogleFeature.CALENDAR, viewModel.state.value.inFlightFeature)
        viewModel.disconnectInApp(GoogleFeature.CALENDAR)
        assertEquals(GoogleDemoFeatureState.DISCONNECTED, viewModel.state.value.calendar)
        assertEquals(null, viewModel.state.value.inFlightFeature)

        runCurrent()
        assertFalse(gateway.isEnabledInApp(GoogleFeature.CALENDAR))
        assertEquals(
            ScopePermission.NEEDS_CONSENT,
            gateway.status(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)).permissions[GoogleScope.CALENDAR_EVENTS_READ_ONLY],
        )
        assertEquals(GoogleDemoFeatureState.DISCONNECTED, viewModel.state.value.calendar)
    }

    @Test
    fun sharedRevocationPreservesAppDisconnectUntilExplicitReconnect() = runTest(mainDispatcher.dispatcher) {
        val gateway = FakeGoogleAuthorizationGateway()
        val viewModel = GoogleAuthorizationDemoViewModel(gateway)

        viewModel.disconnectInApp(GoogleFeature.GMAIL)
        assertFalse(gateway.isEnabledInApp(GoogleFeature.GMAIL))
        assertEquals(setOf(GoogleFeature.GMAIL), viewModel.state.value.disabledInAppFeatures)

        viewModel.revokeDemoGrant()

        assertFalse(gateway.isEnabledInApp(GoogleFeature.GMAIL))
        assertEquals(setOf(GoogleFeature.GMAIL), viewModel.state.value.disabledInAppFeatures)
        assertEquals(GoogleDemoFeatureState.REVOKED, viewModel.state.value.gmail)
        assertEquals("アプリ内切断（デモ）", viewModel.state.value.gmail.label(isEnabledInApp = false))
        assertEquals(null, viewModel.state.value.pendingFeature)
        assertEquals(
            ScopePermission.NEEDS_CONSENT,
            gateway.status(setOf(GoogleScope.GMAIL_READ_ONLY)).permissions[GoogleScope.GMAIL_READ_ONLY],
        )

        // The panel derives its request/reconnect controls from this same disabled state:
        // reconnect remains available and a direct consent request must not dispatch.
        viewModel.requestConsent(GoogleFeature.GMAIL)
        assertEquals(null, viewModel.state.value.inFlightFeature)
        assertEquals(null, viewModel.state.value.pendingFeature)
        assertEquals(GoogleDemoFeatureState.REVOKED, viewModel.state.value.gmail)
        assertTrue(viewModel.state.value.message.contains("再接続"))

        viewModel.reconnectInApp(GoogleFeature.GMAIL)
        assertTrue(gateway.isEnabledInApp(GoogleFeature.GMAIL))
        assertEquals(emptySet<GoogleFeature>(), viewModel.state.value.disabledInAppFeatures)
        assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.gmail)
        assertEquals("承認が必要（デモ）", viewModel.state.value.gmail.label(isEnabledInApp = true))
        assertEquals(
            ScopePermission.NEEDS_CONSENT,
            gateway.status(setOf(GoogleScope.GMAIL_READ_ONLY)).permissions[GoogleScope.GMAIL_READ_ONLY],
        )

        viewModel.requestConsent(GoogleFeature.GMAIL)
        assertEquals(null, viewModel.state.value.inFlightFeature)
        assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.gmail)
        assertEquals(GoogleFeature.GMAIL, viewModel.state.value.pendingFeature)
        assertEquals(setOf(GoogleScope.GMAIL_READ_ONLY), viewModel.state.value.pendingScopes)
        assertEquals(emptySet<GoogleFeature>(), viewModel.state.value.disabledInAppFeatures)

        // Reconnect requires an explicit preview before any synthetic reauthorization proceeds.
        viewModel.grantPendingConsent()
        viewModel.grantPendingConsent()
        runCurrent()
        assertEquals(GoogleDemoFeatureState.AUTHORIZED, viewModel.state.value.gmail)
    }
}
