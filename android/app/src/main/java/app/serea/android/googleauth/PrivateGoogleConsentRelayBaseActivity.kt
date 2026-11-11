package app.serea.android.googleauth

import android.app.Activity
import android.app.PendingIntent
import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.result.ActivityResult
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts

/** Shared unregistered relay behavior; only Private Beta and isolated test manifests declare subclasses. */
internal open class PrivateGoogleConsentRelayBaseActivity : ComponentActivity() {
    private val consent = registerForActivityResult(ActivityResultContracts.StartIntentSenderForResult()) { result ->
        finishWithOriginalIdentity(result)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val identity = PrivateGoogleConsentRelayProtocol.identityFrom(intent)
        val pendingIntent = pendingIntent(intent)
        if (identity == null || pendingIntent == null) {
            setResult(Activity.RESULT_CANCELED, Intent())
            finish()
            return
        }
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() = finishWithOriginalIdentity(
                ActivityResult(Activity.RESULT_CANCELED, null),
            )
        })
        // ActivityResultRegistry restores the in-flight sender after recreation. Relaunching here
        // could duplicate consent; if restoration fails, the result remains uncorrelated/fail-closed.
        if (savedInstanceState != null) return
        try {
            consent.launch(IntentSenderRequest.Builder(pendingIntent.intentSender).build())
        } catch (_: Exception) {
            finishWithOriginalIdentity(ActivityResult(Activity.RESULT_CANCELED, null))
        }
    }

    private fun finishWithOriginalIdentity(result: ActivityResult) {
        val identity = PrivateGoogleConsentRelayProtocol.identityFrom(intent)
        if (identity == null) {
            setResult(Activity.RESULT_CANCELED, Intent())
            finish()
            return
        }
        setResult(result.resultCode, PrivateGoogleConsentRelayProtocol.resultIntent(identity, result.data))
        finish()
    }

    @Suppress("DEPRECATION")
    private fun pendingIntent(source: Intent): PendingIntent? =
        source.getParcelableExtra(PrivateGoogleConsentRelayProtocol.EXTRA_PENDING_INTENT)
}
