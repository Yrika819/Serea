package app.serea.android.googleauth

import android.content.ComponentName
import android.content.Context
import android.content.Intent
import androidx.activity.result.contract.ActivityResultContract

/** Android Intent envelope shared by the Private Beta relay and isolated instrumentation. */
internal object PrivateGoogleConsentRelayProtocol {
    const val ACTIVITY_CLASS = "app.serea.android.googleauth.PrivateGoogleConsentRelayActivity"
    const val EXTRA_REQUEST_ID = "app.serea.android.private_google.consent_request_id"
    const val EXTRA_GENERATION = "app.serea.android.private_google.consent_generation"
    const val EXTRA_SESSION_ID = "app.serea.android.private_google.consent_session_id"
    const val EXTRA_PENDING_INTENT = "app.serea.android.private_google.consent_pending_intent"
    const val EXTRA_RESULT_DATA = "app.serea.android.private_google.consent_result_data"

    fun launchIntent(context: Context, request: PendingGoogleConsentActivityResult): Intent =
        Intent().setComponent(ComponentName(context.packageName, ACTIVITY_CLASS))
            .putExtra(EXTRA_REQUEST_ID, request.identity.requestId)
            .putExtra(EXTRA_GENERATION, request.identity.generation)
            .putExtra(EXTRA_SESSION_ID, request.identity.authorizationSessionId)
            .putExtra(EXTRA_PENDING_INTENT, request.pendingIntent)

    fun identityFrom(intent: Intent): PrivateGoogleConsentIdentity? {
        val requestId = intent.getStringExtra(EXTRA_REQUEST_ID)
        val generation = intent.getLongExtra(EXTRA_GENERATION, Long.MIN_VALUE)
        val sessionId = intent.getStringExtra(EXTRA_SESSION_ID)
        if (requestId.isNullOrBlank() || generation == Long.MIN_VALUE || sessionId.isNullOrBlank()) return null
        return PrivateGoogleConsentIdentity(requestId, generation, sessionId)
    }

    fun resultIntent(identity: PrivateGoogleConsentIdentity, resultData: Intent?): Intent =
        Intent()
            .putExtra(EXTRA_REQUEST_ID, identity.requestId)
            .putExtra(EXTRA_GENERATION, identity.generation)
            .putExtra(EXTRA_SESSION_ID, identity.authorizationSessionId)
            .also { response -> resultData?.let { data -> response.putExtra(EXTRA_RESULT_DATA, data) } }

    fun parseResult(resultCode: Int, intent: Intent?): PrivateGoogleConsentActivityResult {
        val identity = intent?.let { PrivateGoogleConsentRelayProtocol.identityFrom(it) }
        @Suppress("DEPRECATION")
        val data = intent?.getParcelableExtra<Intent>(EXTRA_RESULT_DATA)
        return PrivateGoogleConsentActivityResult(identity, resultCode, data)
    }
}

internal class PrivateGoogleConsentRelayContract :
    ActivityResultContract<PendingGoogleConsentActivityResult, PrivateGoogleConsentActivityResult>() {
    override fun createIntent(context: Context, input: PendingGoogleConsentActivityResult): Intent =
        PrivateGoogleConsentRelayProtocol.launchIntent(context, input)

    override fun parseResult(resultCode: Int, intent: Intent?): PrivateGoogleConsentActivityResult =
        PrivateGoogleConsentRelayProtocol.parseResult(resultCode, intent)
}
