package app.serea.android.googleauth

import android.app.Activity
import android.app.PendingIntent
import android.content.ComponentName
import android.content.Intent
import android.os.Bundle
import android.os.Process
import android.util.Log
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout

/** Host-controlled isolated harness for a real OS kill while a synthetic Consent sender is foreground. */
class PrivateGoogleInFlightDeathHarnessActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(TextView(this).apply { text = "Private Google in-flight death harness" })
        val phase = intent.getStringExtra(EXTRA_PHASE)
        val runToken = token()
        if (!runToken.matches(Regex("[A-Za-z0-9-]{1,120}"))) error("invalid run token")
        when (phase) {
            PHASE_IN_FLIGHT -> if (savedInstanceState == null) {
                startInFlightSender(runToken)
            } else {
                Log.i(TAG, "A_RECREATED_NO_RELAUNCH:$runToken:pid=${Process.myPid()}")
            }
            PHASE_VERIFY_B -> lifecycleScope.launch { verifyFreshRequest(runToken) }
            else -> error("invalid phase")
        }
    }

    private fun startInFlightSender(runToken: String) {
        val identityA = identityA()
        val gate = PrivateGoogleConsentIdentityGate()
        check(gate.begin(identityA))
        // The sender is installed in the separate AndroidTest APK. force-stop of Serea must
        // leave this external synthetic consent UI alive so the host can return A afterward.
        val sender = Intent().setComponent(
            ComponentName("${packageName}.test", "app.serea.android.googleauth.SyntheticConsentSenderActivity"),
        ).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_MULTIPLE_TASK)
            .putExtra(EXTRA_TOKEN, runToken)
        Log.i(TAG, "IN_FLIGHT:$runToken:request=${identityA.requestId}:generation=${identityA.generation}:session=${identityA.authorizationSessionId}:pid=${Process.myPid()}")
        // Keep the external synthetic UI in its own task. The original Serea process owns the
        // pending identity, while the host later replays A directly into B's guarded ViewModel.
        startActivity(sender)
    }

    private suspend fun verifyFreshRequest(runToken: String) {
        val identityB = PrivateGoogleConsentIdentity("request-B", 42L, "session-B")
        val gateB = PrivateGoogleConsentIdentityGate()
        check(gateB.begin(identityB))
        check(!gateB.consume(identityB.copy(authorizationSessionId = "old-session"))) {
            "session mismatch consumed new pending request"
        }
        check(gateB.isPending(identityB)) { "session mismatch changed new request" }
        check(!gateB.consume(identityB.copy(generation = 41L))) {
            "generation mismatch consumed new pending request"
        }
        check(gateB.isPending(identityB)) { "generation mismatch changed new request" }
        check(!gateB.consume(identityA())) { "old consent identity consumed new pending request" }
        check(!gateB.cancel(identityA())) { "old consent identity cancelled new pending request" }
        check(gateB.isPending(identityB)) { "new request changed after stale consent result" }
        check(!gateB.begin(identityB)) { "second authorization was admitted" }

        // Exercise AccountPicker's OS result identity guard after restart. The IDs/generation
        // intentionally collide; the per-launch token must still protect request B.
        val fakeGatewayStats = ConsentHarnessGatewayStats()
        val viewModel = PrivateGoogleAuthorizationViewModel(
            null,
            enabledInThisVariant = true,
            realOAuthEnabled = true,
            realGatewayFactory = { _, _ ->
                fakeGatewayStats.factoryCalls++
                ConsentHarnessGateway(this, fakeGatewayStats)
            },
        )
        check(viewModel.state.value.status == PrivateGoogleAuthStatus.DISCONNECTED)
        check(viewModel.state.value.pendingRealConsentRequestId == null)
        check(viewModel.calendarSession.value == null)
        val pickerB = checkNotNull(viewModel.beginRealAccountSelection())
        viewModel.onRealAccountSelected(
            pickerB.requestId,
            pickerB.generation,
            "old-picker-token",
            "synthetic@example.invalid",
            PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE,
        )
        check(viewModel.state.value.status == PrivateGoogleAuthStatus.ACCOUNT_SELECTION_PENDING)
        check(viewModel.state.value.pendingAccountSelectionToken == pickerB.requestToken)
        check(fakeGatewayStats.factoryCalls == 0) { "stale AccountPicker result constructed an authorization gateway" }
        check(!viewModel.cancelRealAccountSelection(pickerB.requestId, pickerB.generation, "old-picker-token"))
        viewModel.onRealAccountSelected(
            pickerB.requestId,
            pickerB.generation,
            pickerB.requestToken,
            "synthetic@example.invalid",
            PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE,
        )
        check(fakeGatewayStats.factoryCalls == 1)
        viewModel.requestPrimaryCalendarConsent()
        withTimeout(5_000) {
            viewModel.state.first { it.status == PrivateGoogleAuthStatus.CONSENT_PENDING }
        }
        val stateBeforeStaleConsent = viewModel.state.value
        check(stateBeforeStaleConsent.pendingRealConsentRequestId == "request-B")
        viewModel.completeRealConsentResult(
            PrivateGoogleConsentActivityResult(identityA(), Activity.RESULT_OK, Intent()),
        )
        check(viewModel.state.value == stateBeforeStaleConsent) { "stale consent result changed the active B authorization state" }
        check(viewModel.calendarSession.value == null)
        check(fakeGatewayStats.authorizeCalls == 1)
        check(fakeGatewayStats.resultCalls == 0)
        check(fakeGatewayStats.cancelCalls == 0)
        check(fakeGatewayStats.tokenCalls == 0)
        check(fakeGatewayStats.gatewayMode == GoogleAuthorizationMode.MOCK_ONLY)
        Log.i(TAG, "STALE_REJECTED:$runToken:request=${identityB.requestId}:generation=${identityB.generation}:session=${identityB.authorizationSessionId}:pid=${Process.myPid()}:provider=Fake:gateway=${fakeGatewayStats.gatewayMode}:googleHttpClient=absent:gmailClient=absent:calendarListClient=absent:factory=${fakeGatewayStats.factoryCalls}:authorize=${fakeGatewayStats.authorizeCalls}:result=${fakeGatewayStats.resultCalls}:cancel=${fakeGatewayStats.cancelCalls}:token=${fakeGatewayStats.tokenCalls}")
    }

    private fun identityA() = PrivateGoogleConsentIdentity("request-A", 41L, "session-A")

    private fun token(): String = intent.getStringExtra(EXTRA_TOKEN).orEmpty()

    companion object {
        const val EXTRA_TOKEN = "token"
        const val EXTRA_PHASE = "phase"
        const val PHASE_IN_FLIGHT = "in-flight-A"
        const val PHASE_VERIFY_B = "verify-B"
        const val TAG = "SereaConsentDeath"
    }

    private class ConsentHarnessGatewayStats {
        var factoryCalls = 0
        var authorizeCalls = 0
        var resultCalls = 0
        var cancelCalls = 0
        var tokenCalls = 0
        var gatewayMode: GoogleAuthorizationMode? = null
    }

    /** In-memory gateway used only by this isolated test Activity; it has no Google client or HTTP path. */
    private class ConsentHarnessGateway(
        context: android.content.Context,
        private val stats: ConsentHarnessGatewayStats,
    ) : PrivateGoogleRealAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.MOCK_ONLY
        override val authorizationSessionId = "session-B"
        init {
            stats.gatewayMode = mode
        }
        private val pendingIntent = PendingIntent.getActivity(
            context,
            991,
            Intent(context, PrivateGoogleInFlightDeathHarnessActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            stats.tokenCalls++
            return TokenSupplyResult.Unavailable(AuthorizationFailure.NOT_CONFIGURED)
        }

        override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
            GoogleServiceState.AVAILABLE,
            scopes.associateWith { ScopePermission.NEEDS_CONSENT },
        )

        override suspend fun authorize(scopes: Set<GoogleScope>): AuthorizationOutcome {
            stats.authorizeCalls++
            return AuthorizationOutcome.ConsentRequired(scopes, "request-B")
        }

        override fun completeConsentResult(requestId: String, data: Intent?): AuthorizationOutcome {
            stats.resultCalls++
            return AuthorizationOutcome.Rejected(AuthorizationFailure.STALE_CONSENT_RESULT)
        }

        override fun takePendingConsentIntent(requestId: String): PendingIntent? =
            if (requestId == "request-B") pendingIntent else null

        override fun cancelConsentRequest(requestId: String): Boolean {
            stats.cancelCalls++
            return false
        }

        override fun disableInApp(feature: GoogleFeature) = Unit
    }
}
