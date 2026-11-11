package app.serea.android.googleauth

import android.accounts.AccountManager
import android.app.Activity
import android.content.Context
import android.content.Intent
import androidx.activity.ComponentActivity
import androidx.activity.result.contract.ActivityResultContract

/** Request and result carry the original selection generation across the external chooser. */
internal data class GoogleAccountPickerResult(
    val requestId: Long?,
    val generation: Long?,
    val requestToken: String?,
    val accountName: String?,
    val accountType: String?,
)

internal fun parseGoogleAccountPickerResult(
    resultCode: Int,
    requestId: Long?,
    generation: Long?,
    requestToken: String?,
    accountName: String?,
    accountType: String?,
): GoogleAccountPickerResult {
    val matchedRequestId = requestId?.takeUnless { it == INVALID_ID }
    val matchedGeneration = generation?.takeUnless { it == INVALID_ID }
    val matchedRequestToken = requestToken?.takeUnless { it.isBlank() }
    val selectedAccountName = if (resultCode == Activity.RESULT_OK && !accountName.isNullOrBlank() &&
        accountType == PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE
    ) accountName else null
    val selectedAccountType = selectedAccountName?.let { accountType }
    return GoogleAccountPickerResult(
        matchedRequestId,
        matchedGeneration,
        matchedRequestToken,
        selectedAccountName,
        selectedAccountType,
    )
}

private const val INVALID_ID = Long.MIN_VALUE

internal class PrivateGoogleAccountPickerContract :
    ActivityResultContract<PendingGoogleAccountSelection, GoogleAccountPickerResult>() {
    override fun createIntent(context: Context, input: PendingGoogleAccountSelection): Intent =
        Intent(context, PrivateGoogleAccountPickerActivity::class.java)
            .putExtra(PrivateGoogleAccountPickerActivity.EXTRA_REQUEST_ID, input.requestId)
            .putExtra(PrivateGoogleAccountPickerActivity.EXTRA_GENERATION, input.generation)
            .putExtra(PrivateGoogleAccountPickerActivity.EXTRA_REQUEST_TOKEN, input.requestToken)

    override fun parseResult(resultCode: Int, intent: Intent?): GoogleAccountPickerResult {
        val requestId = intent?.getLongExtra(PrivateGoogleAccountPickerActivity.EXTRA_REQUEST_ID, INVALID_ID)
        val generation = intent?.getLongExtra(PrivateGoogleAccountPickerActivity.EXTRA_GENERATION, INVALID_ID)
        val requestToken = intent?.getStringExtra(PrivateGoogleAccountPickerActivity.EXTRA_REQUEST_TOKEN)
        val name = intent?.getStringExtra(PrivateGoogleAccountPickerActivity.EXTRA_ACCOUNT_NAME)
        val type = intent?.getStringExtra(PrivateGoogleAccountPickerActivity.EXTRA_ACCOUNT_TYPE)
        return parseGoogleAccountPickerResult(resultCode, requestId, generation, requestToken, name, type)
    }
}

/** Private-variant-only relay that tags the external AccountPicker result with its request ID. */
internal class PrivateGoogleAccountPickerActivity : ComponentActivity() {
    private val accountPicker = registerForActivityResult(
        androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult(),
    ) { result ->
        val response = Intent()
            .putExtra(EXTRA_REQUEST_ID, intent.getLongExtra(EXTRA_REQUEST_ID, INVALID_ID))
            .putExtra(EXTRA_GENERATION, intent.getLongExtra(EXTRA_GENERATION, INVALID_ID))
            .putExtra(EXTRA_REQUEST_TOKEN, intent.getStringExtra(EXTRA_REQUEST_TOKEN))
        val name = result.data?.getStringExtra(AccountManager.KEY_ACCOUNT_NAME)
        val type = result.data?.getStringExtra(AccountManager.KEY_ACCOUNT_TYPE)
        if (result.resultCode == Activity.RESULT_OK && !name.isNullOrBlank() &&
            type == PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE
        ) {
            response.putExtra(EXTRA_ACCOUNT_NAME, name)
                .putExtra(EXTRA_ACCOUNT_TYPE, PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE)
            setResult(Activity.RESULT_OK, response)
        } else {
            setResult(Activity.RESULT_CANCELED, response)
        }
        finish()
    }

    override fun onCreate(savedInstanceState: android.os.Bundle?) {
        super.onCreate(savedInstanceState)
        val requestId = intent.getLongExtra(EXTRA_REQUEST_ID, INVALID_ID)
        val generation = intent.getLongExtra(EXTRA_GENERATION, INVALID_ID)
        if (requestId == INVALID_ID || generation == INVALID_ID ||
            intent.getStringExtra(EXTRA_REQUEST_TOKEN).isNullOrBlank()
        ) {
            setResult(Activity.RESULT_CANCELED, Intent())
            finish()
            return
        }
        if (savedInstanceState != null) return
        accountPicker.launch(
            AccountManager.newChooseAccountIntent(
                null,
                null,
                arrayOf(PrivateGoogleAuthorizationViewModel.GOOGLE_ACCOUNT_TYPE),
                false,
                null,
                null,
                null,
                null,
            ),
        )
    }

    internal companion object {
        const val EXTRA_REQUEST_ID = "app.serea.android.private_google.account_request_id"
        const val EXTRA_GENERATION = "app.serea.android.private_google.account_generation"
        const val EXTRA_REQUEST_TOKEN = "app.serea.android.private_google.account_request_token"
        const val EXTRA_ACCOUNT_NAME = "app.serea.android.private_google.account_name"
        const val EXTRA_ACCOUNT_TYPE = "app.serea.android.private_google.account_type"
        private const val INVALID_ID = Long.MIN_VALUE
    }
}
