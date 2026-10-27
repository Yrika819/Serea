package app.serea.android.googleauth

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestDispatcher
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.setMain
import org.junit.Assert.assertEquals
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
}
