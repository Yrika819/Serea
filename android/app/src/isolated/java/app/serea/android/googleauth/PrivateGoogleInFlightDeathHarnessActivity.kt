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
import androidx.activity.result.contract.ActivityResultContracts

/** Host-controlled isolated harness for a real OS kill while a synthetic Consent sender is foreground. */
class PrivateGoogleInFlightDeathHarnessActivity : ComponentActivity() {
    private val relay = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { activityResult ->
        val result = PrivateGoogleConsentRelayProtocol.parseResult(activityResult.resultCode, activityResult.data)
        val identity = result.identity
        Log.i(TAG, "RESULT_DELIVERED:${token()}:request=${identity?.requestId}:generation=${identity?.generation}:session=${identity?.authorizationSessionId}:code=${result.resultCode}")
    }

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
            PHASE_VERIFY_B -> verifyFreshRequest(runToken)
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
        ).putExtra(EXTRA_TOKEN, runToken)
        val pendingIntent = PendingIntent.getActivity(
            this, 5101, sender, PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val relayIntent = PrivateGoogleConsentRelayContract()
            .createIntent(this, PendingGoogleConsentActivityResult(identityA, pendingIntent))
            .setClass(this, PrivateGoogleConsentRelayTestActivity::class.java)
        Log.i(TAG, "IN_FLIGHT:$runToken:request=${identityA.requestId}:generation=${identityA.generation}:session=${identityA.authorizationSessionId}:pid=${Process.myPid()}")
        relay.launch(relayIntent)
    }

    private fun verifyFreshRequest(runToken: String) {
        val identityB = PrivateGoogleConsentIdentity("request-B", 42L, "session-B")
        val gateB = PrivateGoogleConsentIdentityGate()
        check(gateB.begin(identityB))
        check(!gateB.consume(identityA())) { "old consent identity consumed new pending request" }
        check(gateB.isPending(identityB)) { "new request changed after stale consent result" }
        check(!gateB.begin(identityB)) { "second authorization was admitted" }

        // Exercise AccountPicker's OS result identity guard after restart. The IDs/generation
        // intentionally collide; the per-launch token must still protect request B.
        val viewModel = PrivateGoogleAuthorizationViewModel(
            null,
            enabledInThisVariant = true,
            realOAuthEnabled = true,
            realGatewayFactory = { _, _ -> error("real authorization gateway must never be constructed") },
        )
        check(viewModel.state.value.status == PrivateGoogleAuthStatus.DISCONNECTED)
        check(viewModel.state.value.pendingRealConsentRequestId == null)
        check(viewModel.calendarSession.value == null)
        val pickerB = checkNotNull(viewModel.beginRealAccountSelection())
        check(!viewModel.cancelRealAccountSelection(pickerB.requestId, pickerB.generation, "old-picker-token"))
        viewModel.completeRealConsentResult(
            PrivateGoogleConsentActivityResult(identityA(), Activity.RESULT_OK, Intent()),
        )
        check(viewModel.state.value.status == PrivateGoogleAuthStatus.ACCOUNT_SELECTION_PENDING)
        check(viewModel.state.value.pendingAccountSelectionToken == pickerB.requestToken)
        check(viewModel.calendarSession.value == null)
        Log.i(TAG, "STALE_REJECTED:$runToken:request=${identityB.requestId}:generation=${identityB.generation}:session=${identityB.authorizationSessionId}:pid=${Process.myPid()}:provider=Fake:http=0:tokens=0:gmail=0:calendarList=0")
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
}
