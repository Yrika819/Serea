package app.serea.android.googleauth

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.util.Log
import android.widget.TextView

/** External, synthetic consent UI hosted by the separate instrumentation APK process. */
class SyntheticConsentSenderActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(TextView(this).apply { text = "Synthetic Consent A is waiting" })
        Log.i(TAG, "SENDER_IN_FLIGHT:${token()}:pid=${android.os.Process.myPid()}")
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        if (intent.getStringExtra(EXTRA_ACTION) != ACTION_COMPLETE || token().isBlank()) return
        setResult(RESULT_OK, Intent().putExtra(EXTRA_PAYLOAD, "synthetic-consent-result-A"))
        Log.i(TAG, "SENDER_COMPLETED:${token()}:pid=${android.os.Process.myPid()}")
        finish()
    }

    private fun token(): String = intent.getStringExtra(EXTRA_TOKEN).orEmpty()

    companion object {
        const val TAG = "SereaSyntheticConsent"
        const val EXTRA_TOKEN = "token"
        const val EXTRA_ACTION = "action"
        const val ACTION_COMPLETE = "complete"
        const val EXTRA_PAYLOAD = "synthetic-consent-payload"
    }
}
