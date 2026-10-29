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
import org.junit.Assert.assertSame
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

    private fun confirmCurrentPreview(viewModel: GoogleAuthorizationDemoViewModel) =
        viewModel.grantPendingConsent(checkNotNull(viewModel.state.value.pendingConsentPreview))

    private fun denyCurrentPreview(viewModel: GoogleAuthorizationDemoViewModel) =
        viewModel.denyPendingConsent(checkNotNull(viewModel.state.value.pendingConsentPreview))

    private fun cancelCurrentPreview(viewModel: GoogleAuthorizationDemoViewModel) =
        viewModel.cancelPendingConsent(checkNotNull(viewModel.state.value.pendingConsentPreview))

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

            cancelCurrentPreview(viewModel)
            assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.gmail)
            assertEquals(null, viewModel.state.value.pendingFeature)

            viewModel.requestConsent(GoogleFeature.GMAIL)
            runCurrent()
            denyCurrentPreview(viewModel)
            assertEquals(GoogleDemoFeatureState.AUTHORIZING, viewModel.state.value.gmail)
            runCurrent()
            assertEquals(GoogleDemoFeatureState.ERROR, viewModel.state.value.gmail)

            viewModel.requestConsent(GoogleFeature.GMAIL)
            runCurrent()
            confirmCurrentPreview(viewModel)
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

            val samePreview = checkNotNull(viewModel.state.value.pendingConsentPreview)
            viewModel.grantPendingConsent(samePreview)
            viewModel.grantPendingConsent(samePreview)
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
            confirmCurrentPreview(viewModel)
            runCurrent()
            val accountBSession = checkNotNull(viewModel.calendarDemoSession.value)
            assertNotSame(accountASession, accountBSession)
            assertEquals(CalendarProviderKind.FAKE, accountBSession.kind)
            assertEquals(3, accountBSession.listCalendars().size)
        }

    @Test
    fun staleCalendarConfirmFromPreviousAccountCannotGrantOrCreateASession() = runTest(mainDispatcher.dispatcher) {
        val gateway = FakeGoogleAuthorizationGateway()
        val openedBindings = mutableListOf<Pair<GoogleDemoAccount, Int>>()
        val sessionFactory = CalendarDemoSessionFactory { account, generation ->
            openedBindings += account to generation
            app.serea.android.calendar.CalendarProviderSessions().openFake()
        }
        val viewModel = GoogleAuthorizationDemoViewModel(gateway, sessionFactory)
        viewModel.requestConsent(GoogleFeature.CALENDAR)
        runCurrent()
        val accountAPreview = checkNotNull(viewModel.state.value.pendingConsentPreview)
        assertEquals(GoogleFeature.CALENDAR, accountAPreview.feature)
        assertEquals(GoogleFeature.CALENDAR.scopes, accountAPreview.requestedScopes)
        assertEquals(GoogleFeature.CALENDAR.scopes, accountAPreview.consentScopes)
        assertEquals(GoogleDemoAccount.ACCOUNT_A, accountAPreview.selectedAccount)
        assertEquals(1, accountAPreview.accountGeneration)

        viewModel.selectDemoAccount(GoogleDemoAccount.ACCOUNT_B)
        viewModel.requestConsent(GoogleFeature.CALENDAR)
        runCurrent()
        val accountBPreview = checkNotNull(viewModel.state.value.pendingConsentPreview)
        assertEquals(GoogleFeature.CALENDAR, accountBPreview.feature)
        assertEquals(GoogleFeature.CALENDAR.scopes, accountBPreview.requestedScopes)
        assertEquals(GoogleFeature.CALENDAR.scopes, accountBPreview.consentScopes)
        assertEquals(GoogleDemoAccount.ACCOUNT_B, accountBPreview.selectedAccount)
        assertEquals(2, accountBPreview.accountGeneration)
        assertTrue(accountBPreview.id > accountAPreview.id)

        // Simulate a delayed UI callback retained from the Account A preview.
        viewModel.grantPendingConsent(accountAPreview)
        runCurrent()
        assertSame(accountBPreview, viewModel.state.value.pendingConsentPreview)
        assertEquals(null, viewModel.state.value.inFlightFeature)
        assertTrue(openedBindings.isEmpty())
        assertEquals(null, viewModel.calendarDemoSession.value)
        assertEquals(
            ScopePermission.NEEDS_CONSENT,
            gateway.status(GoogleFeature.CALENDAR.scopes).permissions[GoogleScope.CALENDAR_EVENTS_READ_ONLY],
        )

        viewModel.grantPendingConsent(accountBPreview)
        runCurrent()
        assertEquals(listOf(GoogleDemoAccount.ACCOUNT_B to 2), openedBindings)
        assertEquals(CalendarProviderKind.FAKE, viewModel.calendarDemoSession.value?.kind)
    }

    @Test
    fun cancelledPreviewCallbackCannotConsumeOrGrantAReplacementPreview() = runTest(mainDispatcher.dispatcher) {
        val gateway = FakeGoogleAuthorizationGateway()
        var openedSessions = 0
        val sessionFactory = CalendarDemoSessionFactory { _, _ ->
            openedSessions++
            app.serea.android.calendar.CalendarProviderSessions().openFake()
        }
        val viewModel = GoogleAuthorizationDemoViewModel(gateway, sessionFactory)
        viewModel.requestConsent(GoogleFeature.CALENDAR)
        runCurrent()
        val cancelledPreview = checkNotNull(viewModel.state.value.pendingConsentPreview)
        viewModel.cancelPendingConsent(cancelledPreview)

        viewModel.requestConsent(GoogleFeature.CALENDAR)
        runCurrent()
        val replacementPreview = checkNotNull(viewModel.state.value.pendingConsentPreview)
        assertTrue(replacementPreview.id > cancelledPreview.id)
        assertEquals(cancelledPreview.requestedScopes, replacementPreview.requestedScopes)
        assertEquals(cancelledPreview.feature, replacementPreview.feature)
        assertEquals(cancelledPreview.consentScopes, replacementPreview.consentScopes)

        // A stale Confirm from the earlier rendering is ignored even though account and scopes match.
        viewModel.grantPendingConsent(cancelledPreview)
        runCurrent()
        assertSame(replacementPreview, viewModel.state.value.pendingConsentPreview)
        assertEquals(null, viewModel.state.value.inFlightFeature)
        assertEquals(0, openedSessions)
        assertEquals(null, viewModel.calendarDemoSession.value)
        assertEquals(
            ScopePermission.NEEDS_CONSENT,
            gateway.status(GoogleFeature.CALENDAR.scopes).permissions[GoogleScope.CALENDAR_EVENTS_READ_ONLY],
        )
    }

    @Test
    fun confirmingOnePreviewTwiceCreatesOnlyOneCalendarSession() = runTest(mainDispatcher.dispatcher) {
        val gateway = FakeGoogleAuthorizationGateway()
        var openedSessions = 0
        val sessionFactory = CalendarDemoSessionFactory { _, _ ->
            openedSessions++
            app.serea.android.calendar.CalendarProviderSessions().openFake()
        }
        val viewModel = GoogleAuthorizationDemoViewModel(gateway, sessionFactory)
        viewModel.requestConsent(GoogleFeature.CALENDAR)
        runCurrent()
        val preview = checkNotNull(viewModel.state.value.pendingConsentPreview)

        viewModel.grantPendingConsent(preview)
        viewModel.grantPendingConsent(preview)
        assertEquals(GoogleFeature.CALENDAR, viewModel.state.value.inFlightFeature)
        assertEquals(null, viewModel.state.value.pendingConsentPreview)
        assertEquals(0, openedSessions)
        assertEquals(
            ScopePermission.NEEDS_CONSENT,
            gateway.status(GoogleFeature.CALENDAR.scopes).permissions[GoogleScope.CALENDAR_EVENTS_READ_ONLY],
        )

        runCurrent()
        assertEquals(1, openedSessions)
        assertEquals(GoogleDemoAccount.ACCOUNT_A, preview.selectedAccount)
        assertEquals(1, preview.accountGeneration)
        assertEquals(GoogleDemoFeatureState.AUTHORIZED, viewModel.state.value.calendar)
        assertEquals(CalendarProviderKind.FAKE, viewModel.calendarDemoSession.value?.kind)
    }

    @Test
    fun reconnectAfterAppLocalDisconnectRequiresConfirmationAndReplacesTheOldCalendarSession() =
        runTest(mainDispatcher.dispatcher) {
            val gateway = FakeGoogleAuthorizationGateway()
            val viewModel = GoogleAuthorizationDemoViewModel(gateway)
            viewModel.requestConsent(GoogleFeature.CALENDAR)
            runCurrent()
            confirmCurrentPreview(viewModel)
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

            val samePreview = checkNotNull(viewModel.state.value.pendingConsentPreview)
            viewModel.grantPendingConsent(samePreview)
            viewModel.grantPendingConsent(samePreview)
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

            cancelCurrentPreview(viewModel)
            assertEquals(null, viewModel.state.value.pendingFeature)
            assertEquals(GoogleDemoFeatureState.AUTHORIZATION_REQUIRED, viewModel.state.value.gmail)

            viewModel.requestConsent(GoogleFeature.CALENDAR)
            assertEquals(GoogleFeature.CALENDAR, viewModel.state.value.inFlightFeature)
            viewModel.cancelAuthorization()
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

        confirmCurrentPreview(viewModel)
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

        confirmCurrentPreview(viewModel)
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
        val samePreview = checkNotNull(viewModel.state.value.pendingConsentPreview)
        viewModel.grantPendingConsent(samePreview)
        viewModel.grantPendingConsent(samePreview)
        runCurrent()
        assertEquals(GoogleDemoFeatureState.AUTHORIZED, viewModel.state.value.gmail)
    }
}
