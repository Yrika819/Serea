package app.serea.android.googleauth

import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.Process
import android.util.Log
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContracts
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.flow.collect
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout

/** Host-controlled isolated harness for a real OS kill while a synthetic Consent sender is foreground. */
class PrivateGoogleInFlightDeathHarnessActivity : ComponentActivity() {
    private var authorizationViewModel: PrivateGoogleAuthorizationViewModel? = null
    private var runTokenForResult: String = ""
    private val relay = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { activityResult ->
        val result = PrivateGoogleConsentRelayProtocol.parseResult(activityResult.resultCode, activityResult.data)
        val identity = result.identity
        Log.i(TAG, "RESULT_DELIVERED:$runTokenForResult:request=${identity?.requestId}:generation=${identity?.generation}:session=${identity?.authorizationSessionId}:code=${result.resultCode}:pid=${Process.myPid()}")
        applyOldResultToB(result, runTokenForResult)
        authorizationViewModel?.completeRealConsentResult(result)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(TextView(this).apply { text = "Private Google in-flight death harness" })
        val phase = intent.getStringExtra(EXTRA_PHASE)
        val runToken = token()
        runTokenForResult = runToken
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
        val stats = ConsentHarnessGatewayStats()
        val viewModel = newViewModel(stats, "request-A", "session-A", runToken)
        authorizationViewModel = viewModel
        val picker = checkNotNull(viewModel.beginRealAccountSelection())
        viewModel.onRealAccountSelected(
            picker.requestId, picker.generation, picker.requestToken,
            "synthetic@example.invalid", PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE,
        )
        lifecycleScope.launch {
            viewModel.pendingConsentLaunches.collect { pending ->
                Log.i(TAG, "IN_FLIGHT:$runToken:request=${pending.identity.requestId}:generation=${pending.identity.generation}:session=${pending.identity.authorizationSessionId}:pid=${Process.myPid()}:status=${viewModel.state.value.status}")
                val relayIntent = PrivateGoogleConsentRelayContract().createIntent(this@PrivateGoogleInFlightDeathHarnessActivity, pending)
                    .setClass(this@PrivateGoogleInFlightDeathHarnessActivity, PrivateGoogleConsentRelayTestActivity::class.java)
                relay.launch(relayIntent)
            }
        }
        viewModel.requestPrimaryCalendarConsent()
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
        val viewModel = newViewModel(fakeGatewayStats, "request-B", "session-B", runToken)
        activeBViewModel = viewModel
        activeBStats = fakeGatewayStats
        check(viewModel.state.value.status == PrivateGoogleAuthStatus.DISCONNECTED)
        check(viewModel.state.value.pendingRealConsentRequestId == null)
        check(viewModel.calendarSession.value == null)
        Log.i(TAG, "RECOVERED_DISCONNECTED:$runToken:status=${viewModel.state.value.status}:pending=${viewModel.state.value.pendingRealConsentRequestId}:calendarSession=${viewModel.calendarSession.value}:pid=${Process.myPid()}")
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
        check(viewModel.calendarSession.value == null)
        check(fakeGatewayStats.authorizeCalls == 1)
        check(fakeGatewayStats.resultCalls == 0)
        check(fakeGatewayStats.cancelCalls == 0)
        check(fakeGatewayStats.tokenCalls == 0)
        check(fakeGatewayStats.gatewayMode == GoogleAuthorizationMode.MOCK_ONLY)
        Log.i(TAG, "B_READY:$runToken:request=${viewModel.state.value.pendingRealConsentRequestId}:generation=${viewModel.state.value.pendingRealConsentGeneration}:session=${viewModel.state.value.pendingRealConsentSessionId}:pid=${Process.myPid()}:provider=Fake:gateway=${fakeGatewayStats.gatewayMode}:realTokens=${fakeGatewayStats.tokenCalls}:googleTransport=not-constructed:gmailClient=absent:calendarListClient=absent:factory=${fakeGatewayStats.factoryCalls}:authorize=${fakeGatewayStats.authorizeCalls}:result=${fakeGatewayStats.resultCalls}:cancel=${fakeGatewayStats.cancelCalls}:token=${fakeGatewayStats.tokenCalls}")
    }

    private fun newViewModel(stats: ConsentHarnessGatewayStats, requestId: String, sessionId: String, runToken: String) =
        PrivateGoogleAuthorizationViewModel(
            this,
            enabledInThisVariant = true,
            realOAuthEnabled = true,
            realGatewayFactory = { _, _ ->
                stats.factoryCalls++
                ConsentHarnessGateway(this, stats, requestId, sessionId, runToken)
            },
        )

    private fun applyOldResultToB(result: PrivateGoogleConsentActivityResult, runToken: String) {
        val viewModel = activeBViewModel ?: return
        val before = viewModel.state.value
        viewModel.completeRealConsentResult(result)
        check(viewModel.state.value == before) { "old result changed new authorization state" }
        check(viewModel.calendarSession.value == null)
        check(activeBStats?.authorizeCalls == 1) { "old result caused a second authorization" }
        check(activeBStats?.resultCalls == 0) { "old result reached the fake provider" }
        check(activeBStats?.cancelCalls == 0) { "old result cancelled the new authorization" }
        check(activeBStats?.tokenCalls == 0)
        Log.i(TAG, "STALE_REJECTED:$runToken:request=${before.pendingRealConsentRequestId}:generation=${before.pendingRealConsentGeneration}:session=${before.pendingRealConsentSessionId}:pid=${Process.myPid()}:provider=Fake:resultCode=${result.resultCode}:gateway=${activeBStats?.gatewayMode}:googleTransport=not-constructed:gmailClient=absent:calendarListClient=absent:authorize=${activeBStats?.authorizeCalls}:result=${activeBStats?.resultCalls}:cancel=${activeBStats?.cancelCalls}:token=${activeBStats?.tokenCalls}")
    }

    private fun identityA() = PrivateGoogleConsentIdentity("request-A", 1L, "session-A")

    private fun token(): String = intent.getStringExtra(EXTRA_TOKEN).orEmpty()

    companion object {
        const val EXTRA_TOKEN = "token"
        const val EXTRA_PHASE = "phase"
        const val PHASE_IN_FLIGHT = "in-flight-A"
        const val PHASE_VERIFY_B = "verify-B"
        const val TAG = "SereaConsentDeath"
        @Volatile private var activeBViewModel: PrivateGoogleAuthorizationViewModel? = null
        @Volatile private var activeBStats: ConsentHarnessGatewayStats? = null

        fun replayOldResult(runToken: String) {
            val viewModel = checkNotNull(activeBViewModel) { "request B is not active" }
            val before = viewModel.state.value
            val result = PrivateGoogleConsentActivityResult(
                PrivateGoogleConsentIdentity("request-A", 1L, "session-A"),
                android.app.Activity.RESULT_OK,
                Intent(),
            )
            viewModel.completeRealConsentResult(result)
            check(viewModel.state.value == before) { "old replay changed new authorization state" }
            check(viewModel.calendarSession.value == null)
            check(activeBStats?.authorizeCalls == 1) { "old replay caused a second authorization" }
            check(activeBStats?.resultCalls == 0) { "old replay reached the fake provider" }
            check(activeBStats?.cancelCalls == 0) { "old replay cancelled the new authorization" }
            check(activeBStats?.tokenCalls == 0)
            Log.i(TAG, "STALE_REPLAY_REJECTED:$runToken:request=${before.pendingRealConsentRequestId}:generation=${before.pendingRealConsentGeneration}:session=${before.pendingRealConsentSessionId}:pid=${Process.myPid()}:provider=Fake:gateway=${activeBStats?.gatewayMode}:googleTransport=not-constructed:gmailClient=absent:calendarListClient=absent:authorize=${activeBStats?.authorizeCalls}:result=${activeBStats?.resultCalls}:cancel=${activeBStats?.cancelCalls}:token=${activeBStats?.tokenCalls}")
        }
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
        private val requestId: String,
        sessionId: String,
        runToken: String,
    ) : PrivateGoogleRealAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.MOCK_ONLY
        override val authorizationSessionId = sessionId
        init {
            stats.gatewayMode = mode
        }
        private val pendingIntent = PendingIntent.getActivity(
            context,
            991,
            Intent().setComponent(ComponentName("${context.packageName}.test", "app.serea.android.googleauth.SyntheticConsentSenderActivity"))
                .putExtra(EXTRA_TOKEN, runToken),
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
            return AuthorizationOutcome.ConsentRequired(scopes, requestId)
        }

        override fun completeConsentResult(requestId: String, data: Intent?): AuthorizationOutcome {
            stats.resultCalls++
            return AuthorizationOutcome.Rejected(AuthorizationFailure.STALE_CONSENT_RESULT)
        }

        override fun takePendingConsentIntent(requestId: String): PendingIntent? =
            if (requestId == this.requestId) pendingIntent else null

        override fun cancelConsentRequest(requestId: String): Boolean {
            stats.cancelCalls++
            return false
        }

        override fun disableInApp(feature: GoogleFeature) = Unit
    }
}

/** Host-only fallback that replays A after the synthetic sender has completed. */
class PrivateGoogleInFlightDeathReplayReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val token = intent.getStringExtra(PrivateGoogleInFlightDeathHarnessActivity.EXTRA_TOKEN).orEmpty()
        check(token.matches(Regex("[A-Za-z0-9-]{1,120}"))) { "invalid replay token" }
        PrivateGoogleInFlightDeathHarnessActivity.replayOldResult(token)
    }
}
