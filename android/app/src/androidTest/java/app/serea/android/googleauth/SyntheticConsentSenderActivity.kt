package app.serea.android.googleauth

import android.app.Activity
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Bundle
import android.os.Looper
import android.os.Process
import android.util.Log
import android.widget.TextView
import java.util.UUID

/** External, synthetic consent UI hosted by the separate instrumentation APK process. */
class SyntheticConsentSenderActivity : Activity() {
    private val instanceNonce = UUID.randomUUID().toString()
    private val runToken by lazy { intent.getStringExtra(EXTRA_TOKEN).orEmpty() }
    private var completed = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        check(runToken.matches(Regex("[A-Za-z0-9-]{1,120}"))) { "invalid synthetic sender run token" }
        SyntheticConsentSenderRegistry.register(this, runToken, instanceNonce)
        setContentView(TextView(this).apply { text = "Synthetic Consent A is waiting" })
        Log.i(TAG, "SENDER_CREATED:$runToken:pid=${Process.myPid()}:instance=$instanceNonce")
        Log.i(TAG, "SENDER_IN_FLIGHT:$runToken:pid=${Process.myPid()}:instance=$instanceNonce")
    }

    override fun onResume() {
        super.onResume()
        Log.i(TAG, "SENDER_RESUMED:$runToken:pid=${Process.myPid()}:instance=$instanceNonce")
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        // Completion is deliberately not handled through ActivityManager reactivation. The host
        // must address the original in-memory Activity through the AndroidTest-only receiver.
        Log.w(TAG, "SENDER_UNEXPECTED_NEW_INTENT:$runToken:pid=${Process.myPid()}:instance=$instanceNonce")
    }

    fun completeFromTestHost(requestToken: String, targetPid: Int, expectedInstanceNonce: String): Boolean {
        check(Looper.myLooper() == Looper.getMainLooper()) { "synthetic Consent completion must run on the Sender main thread" }
        if (completed || requestToken != runToken || targetPid != Process.myPid() ||
            expectedInstanceNonce != instanceNonce || SyntheticConsentSenderRegistry.current !== this
        ) return false
        completed = true
        Log.i(TAG, "SENDER_COMPLETION_REQUESTED:$runToken:pid=${Process.myPid()}:instance=$instanceNonce")
        setResult(Activity.RESULT_OK, Intent().putExtra(EXTRA_PAYLOAD, "synthetic-consent-result-A"))
        Log.i(TAG, "SENDER_COMPLETED:$runToken:pid=${Process.myPid()}:instance=$instanceNonce:code=${Activity.RESULT_OK}:payload=synthetic-consent-result-A")
        finish()
        return true
    }

    override fun onDestroy() {
        SyntheticConsentSenderRegistry.unregister(this)
        super.onDestroy()
    }

    companion object {
        const val TAG = "SereaSyntheticConsent"
        const val EXTRA_TOKEN = "token"
        const val EXTRA_PAYLOAD = "synthetic-consent-payload"
    }
}

/** Volatile, test-process-only reference to the original Sender Activity. */
private object SyntheticConsentSenderRegistry {
    @Volatile var current: SyntheticConsentSenderActivity? = null
        private set
    @Volatile private var registeredToken: String? = null
    @Volatile private var registeredNonce: String? = null

    @Synchronized fun register(activity: SyntheticConsentSenderActivity, token: String, nonce: String) {
        current = activity
        registeredToken = token
        registeredNonce = nonce
    }

    @Synchronized fun complete(token: String, pid: Int, nonce: String): Boolean {
        val activity = current ?: return false
        if (registeredToken != token || registeredNonce != nonce || activity.isFinishing) return false
        return activity.completeFromTestHost(token, pid, nonce)
    }

    @Synchronized fun unregister(activity: SyntheticConsentSenderActivity) {
        if (current === activity) {
            current = null
            registeredToken = null
            registeredNonce = null
        }
    }
}

/** Explicit completion endpoint packaged only in the AndroidTest APK. */
class SyntheticConsentSenderCompletionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        check(Looper.myLooper() == Looper.getMainLooper()) { "synthetic Consent completion receiver must run on the app main thread" }
        val token = intent.getStringExtra(EXTRA_TOKEN).orEmpty()
        val nonce = intent.getStringExtra(EXTRA_INSTANCE_NONCE).orEmpty()
        val targetPid = intent.getIntExtra(EXTRA_TARGET_PID, -1)
        val completed = intent.action == ACTION_COMPLETE &&
            token.matches(Regex("[A-Za-z0-9-]{1,120}")) && nonce.isNotBlank() &&
            targetPid > 0 && SyntheticConsentSenderRegistry.complete(token, targetPid, nonce)
        val event = if (completed) "COMPLETION_BROADCAST_ACCEPTED" else "SENDER_COMPLETION_REJECTED"
        Log.i(SyntheticConsentSenderActivity.TAG, "$event:$token:pid=$targetPid:instance=$nonce")
        setResultCode(if (completed) android.app.Activity.RESULT_OK else android.app.Activity.RESULT_CANCELED)
    }

    companion object {
        const val ACTION_COMPLETE = "app.serea.android.test.action.COMPLETE_SYNTHETIC_CONSENT"
        const val EXTRA_TOKEN = "token"
        const val EXTRA_INSTANCE_NONCE = "instanceNonce"
        const val EXTRA_TARGET_PID = "targetPid"
    }
}
