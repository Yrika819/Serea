package app.serea.android.googleauth

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.os.Process
import android.util.Log

/** Isolated-flavor-only one-shot successful result for request B after stale A checks finish. */
class SyntheticConsentBResultActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val token = intent.getStringExtra("token").orEmpty()
        check(token.matches(Regex("[A-Za-z0-9-]{1,120}-B"))) { "invalid B synthetic result token" }
        Log.i(TAG, "B_SYNTHETIC_RESULT_CREATED:$token:pid=${Process.myPid()}")
        setResult(Activity.RESULT_OK, Intent().putExtra("synthetic-consent-payload", "synthetic-consent-result-B"))
        finish()
    }

    companion object {
        private const val TAG = "SereaSyntheticConsent"
    }
}
