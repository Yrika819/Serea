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
    private var phaseForLifecycle: String = "unknown"
    private val relay = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { activityResult ->
        val result = PrivateGoogleConsentRelayProtocol.parseResult(activityResult.resultCode, activityResult.data)
        val identity = result.identity
        val syntheticPayload = result.resultData?.getStringExtra("synthetic-consent-payload")
        Log.i(TAG, "RESULT_DELIVERED:$runTokenForResult:request=${identity?.requestId}:generation=${identity?.generation}:session=${identity?.authorizationSessionId}:code=${result.resultCode}:payload=${syntheticPayload.orEmpty()}:pid=${Process.myPid()}")
        if (identity != null && identity == identityB()) {
            val bViewModel = checkNotNull(activeBViewModel) { "request B ViewModel is unavailable" }
            bViewModel.completeRealConsentResult(result)
            val state = bViewModel.state.value
            check(state.status == PrivateGoogleAuthStatus.AUTHORIZED) { "B's own synthetic Consent result did not authorize B" }
            check(activeBStats?.resultCalls == 1) { "B's own result did not reach the fake provider exactly once" }
            check(bViewModel.calendarSession.value == null) { "B's synthetic result opened a Calendar session" }
            Log.i(TAG, "B_VALID_RESULT_DELIVERED:$runTokenForResult:request=${identity.requestId}:generation=${identity.generation}:session=${identity.authorizationSessionId}:status=${state.status}:pid=${Process.myPid()}:resultCalls=${activeBStats?.resultCalls}:cancel=${activeBStats?.cancelCalls}:token=${activeBStats?.tokenCalls}:googleTransport=not-constructed:calendarSession=absent")
        } else {
            applyOldResultToB(result, runTokenForResult)
            authorizationViewModel?.completeRealConsentResult(result)
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(TextView(this).apply { text = "Private Google in-flight death harness" })
        val phase = intent.getStringExtra(EXTRA_PHASE)
        phaseForLifecycle = phase.orEmpty()
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
            PHASE_COMPLETE_B -> {
                activeBActivity = this
                lifecycleScope.launch { launchBValidSyntheticResult(runToken) }
            }
            else -> error("invalid phase")
        }
    }

    override fun onResume() {
        super.onResume()
        Log.i(TAG, "HARNESS_RESUMED:$runTokenForResult:phase=$phaseForLifecycle:pid=${Process.myPid()}")
    }

    private fun startInFlightSender(runToken: String) {
        val launchState = getSharedPreferences("serea-inflight-consent-harness", MODE_PRIVATE)
        if (launchState.getString("activeRunToken", null) == runToken) {
            Log.i(TAG, "A_RECREATED_NO_RELAUNCH:$runToken:pid=${Process.myPid()}:reason=persisted-launch-marker")
            return
        }
        check(launchState.edit().putString("activeRunToken", runToken).commit()) {
            "could not persist the in-flight IntentSender one-shot marker"
        }
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
        activeBPendingConsent = withTimeout(5_000) { viewModel.pendingConsentLaunches.first() }
        activeBActivity = this
        val stateBeforeStaleConsent = viewModel.state.value
        check(stateBeforeStaleConsent.pendingRealConsentRequestId == "request-B")
        check(viewModel.calendarSession.value == null)
        check(fakeGatewayStats.authorizeCalls == 1)
        check(fakeGatewayStats.resultCalls == 0)
        check(fakeGatewayStats.cancelCalls == 0)
        check(fakeGatewayStats.tokenCalls == 0)
        check(fakeGatewayStats.gatewayMode == GoogleAuthorizationMode.MOCK_ONLY)
        Log.i(TAG, "B_READY:$runToken:request=${viewModel.state.value.pendingRealConsentRequestId}:generation=${viewModel.state.value.pendingRealConsentGeneration}:session=${viewModel.state.value.pendingRealConsentSessionId}:status=${viewModel.state.value.status}:pid=${Process.myPid()}:provider=Fake:gateway=${fakeGatewayStats.gatewayMode}:calendarSession=absent:realTokens=${fakeGatewayStats.tokenCalls}:googleTransport=not-constructed:gmailClient=absent:calendarListClient=absent:factory=${fakeGatewayStats.factoryCalls}:authorize=${fakeGatewayStats.authorizeCalls}:result=${fakeGatewayStats.resultCalls}:cancel=${fakeGatewayStats.cancelCalls}:token=${fakeGatewayStats.tokenCalls}")
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
        Log.i(TAG, "STALE_REJECTED:$runToken:request=${before.pendingRealConsentRequestId}:generation=${before.pendingRealConsentGeneration}:session=${before.pendingRealConsentSessionId}:status=${before.status}:pid=${Process.myPid()}:provider=Fake:calendarSession=absent:resultCode=${result.resultCode}:gateway=${activeBStats?.gatewayMode}:googleTransport=not-constructed:gmailClient=absent:calendarListClient=absent:authorize=${activeBStats?.authorizeCalls}:result=${activeBStats?.resultCalls}:cancel=${activeBStats?.cancelCalls}:token=${activeBStats?.tokenCalls}")
    }

    private fun identityA() = PrivateGoogleConsentIdentity("request-A", 1L, "session-A")

    private fun token(): String = intent.getStringExtra(EXTRA_TOKEN).orEmpty()

    companion object {
        const val EXTRA_TOKEN = "token"
        const val EXTRA_PHASE = "phase"
        const val PHASE_IN_FLIGHT = "in-flight-A"
        const val PHASE_VERIFY_B = "verify-B"
        const val PHASE_COMPLETE_B = "complete-B"
        const val TAG = "SereaConsentDeath"
        @Volatile private var activeBViewModel: PrivateGoogleAuthorizationViewModel? = null
        @Volatile private var activeBStats: ConsentHarnessGatewayStats? = null
        @Volatile private var activeBActivity: PrivateGoogleInFlightDeathHarnessActivity? = null
        @Volatile private var activeBPendingConsent: PendingGoogleConsentActivityResult? = null

        private fun identityB() = PrivateGoogleConsentIdentity("request-B", 1L, "session-B")

        fun launchBValidSyntheticResult(runToken: String) {
            val activity = checkNotNull(activeBActivity) { "request B Activity is not active" }
            check(activity.token() == runToken) { "request B run token mismatch" }
            val pending = checkNotNull(activeBPendingConsent) { "request B Consent is not pending" }
            check(pending.identity == identityB()) { "request B pending Consent identity mismatch" }
            val relayIntent = PrivateGoogleConsentRelayContract().createIntent(activity, pending)
                .setClass(activity, PrivateGoogleConsentRelayTestActivity::class.java)
            activity.relay.launch(relayIntent)
            Log.i(TAG, "B_VALID_RESULT_REQUESTED:$runToken:request=${pending.identity.requestId}:generation=${pending.identity.generation}:session=${pending.identity.authorizationSessionId}:pid=${Process.myPid()}")
        }

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
            Log.i(TAG, "STALE_REPLAY_REJECTED:$runToken:request=${before.pendingRealConsentRequestId}:generation=${before.pendingRealConsentGeneration}:session=${before.pendingRealConsentSessionId}:status=${before.status}:pid=${Process.myPid()}:provider=Fake:calendarSession=absent:gateway=${activeBStats?.gatewayMode}:googleTransport=not-constructed:gmailClient=absent:calendarListClient=absent:authorize=${activeBStats?.authorizeCalls}:result=${activeBStats?.resultCalls}:cancel=${activeBStats?.cancelCalls}:token=${activeBStats?.tokenCalls}")
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
            if (requestId == "request-B") 992 else 991,
            Intent().setComponent(ComponentName(
                if (requestId == "request-B") context.packageName else "${context.packageName}.test",
                if (requestId == "request-B") "app.serea.android.googleauth.SyntheticConsentBResultActivity"
                else "app.serea.android.googleauth.SyntheticConsentSenderActivity",
            )).putExtra(EXTRA_TOKEN, if (requestId == "request-B") "$runToken-B" else runToken),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
            stats.tokenCalls++
            return TokenSupplyResult.Unavailable(AuthorizationFailure.NOT_CONFIGURED)
        }

        override fun status(scopes: Set<GoogleScope>) = AuthorizationStatus(
            GoogleServiceState.AVAILABLE,
            scopes.associateWith { if (requestId == "request-B" && stats.resultCalls == 1) ScopePermission.GRANTED else ScopePermission.NEEDS_CONSENT },
        )

        override suspend fun authorize(scopes: Set<GoogleScope>): AuthorizationOutcome {
            stats.authorizeCalls++
            return AuthorizationOutcome.ConsentRequired(scopes, requestId)
        }

        override fun completeConsentResult(requestId: String, data: Intent?): AuthorizationOutcome {
            stats.resultCalls++
            if (this.requestId == "request-B" && requestId == this.requestId &&
                data?.getStringExtra("synthetic-consent-payload") == "synthetic-consent-result-B"
            ) {
                return AuthorizationOutcome.Authorized(AccessTokenLease(
                    "fake-test-only-credential",
                    setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY),
                    null,
                    authorizationSessionId,
                ))
            }
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
        check(intent.action == ACTION_REPLAY_OLD) { "invalid replay token action" }
        PrivateGoogleInFlightDeathHarnessActivity.replayOldResult(token)
    }

    companion object {
        const val ACTION_REPLAY_OLD = "app.serea.android.action.REPLAY_OLD_CONSENT_RESULT"
    }
}

/** Isolated-flavor-only one-shot successful result for request B after stale A checks finish. */
class SyntheticConsentBResultActivity : android.app.Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val token = intent.getStringExtra("token").orEmpty()
        check(token.matches(Regex("[A-Za-z0-9-]{1,120}-B"))) { "invalid B synthetic result token" }
        Log.i("SereaSyntheticConsent", "B_SYNTHETIC_RESULT_CREATED:$token:pid=${Process.myPid()}")
        setResult(android.app.Activity.RESULT_OK, Intent().putExtra("synthetic-consent-payload", "synthetic-consent-result-B"))
        finish()
    }
}
