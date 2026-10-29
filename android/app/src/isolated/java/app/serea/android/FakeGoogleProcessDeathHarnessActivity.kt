package app.serea.android

import android.os.Bundle
import android.os.Process
import android.util.Log
import android.widget.TextView
import androidx.activity.ComponentActivity
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.lifecycleScope
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.googleauth.GoogleAuthorizationDemoViewModel
import app.serea.android.googleauth.GoogleDemoAccount
import app.serea.android.googleauth.GoogleDemoFeatureState
import app.serea.android.googleauth.GoogleFeature
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout

/** Isolated-only endpoint for a host-driven OS process-death check of the Fake Google flow. */
class FakeGoogleProcessDeathHarnessActivity : ComponentActivity() {
    private lateinit var status: TextView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        status = TextView(this).apply { text = "Fake Google process-death harness running" }
        setContentView(status)

        val token = intent.getStringExtra(EXTRA_TOKEN)
        val phase = intent.getStringExtra(EXTRA_PHASE)
        if (token.isNullOrBlank() || !token.matches(TOKEN_PATTERN) || phase !in PHASES) {
            fail("invalid", phase?.takeIf { it in PHASES } ?: "invalid", "invalid-request")
            return
        }

        lifecycleScope.launch {
            try {
                val viewModel = ViewModelProvider(this@FakeGoogleProcessDeathHarnessActivity)
                    .get(GoogleAuthorizationDemoViewModel::class.java)
                when (phase) {
                    PHASE_SEED -> seed(viewModel)
                    PHASE_VERIFY -> verifyFreshProcessState(viewModel)
                }
                status.text = "PASS $phase"
                Log.i(TAG, "PASS:$token:$phase:pid=${Process.myPid()}")
            } catch (error: Throwable) {
                status.text = "FAIL $phase"
                Log.e(TAG, "FAIL:$token:$phase:${error.javaClass.simpleName}")
            }
        }
    }

    private suspend fun seed(viewModel: GoogleAuthorizationDemoViewModel) {
        val initial = viewModel.state.value
        check(initial.gmail == GoogleDemoFeatureState.DISCONNECTED)
        check(initial.calendar == GoogleDemoFeatureState.DISCONNECTED)
        check(initial.pendingConsentPreview == null)
        check(viewModel.calendarDemoSession.value == null)

        viewModel.requestConsent(GoogleFeature.CALENDAR)
        val preview = withTimeout(PHASE_TIMEOUT_MS) {
            viewModel.state.first { it.pendingConsentPreview != null }.pendingConsentPreview
        } ?: error("Consent preview missing")
        check(preview.feature == GoogleFeature.CALENDAR)
        check(preview.requestedScopes == GoogleFeature.CALENDAR.scopes)
        check(preview.consentScopes == GoogleFeature.CALENDAR.scopes)
        check(preview.selectedAccount == GoogleDemoAccount.ACCOUNT_A)
        check(preview.accountGeneration == 1)

        viewModel.grantPendingConsent(preview)
        withTimeout(PHASE_TIMEOUT_MS) {
            viewModel.state.first { it.calendar == GoogleDemoFeatureState.AUTHORIZED }
        }
        val session = withTimeout(PHASE_TIMEOUT_MS) {
            viewModel.calendarDemoSession.first { it != null }
        } ?: error("Fake Calendar session missing")
        check(session.kind == CalendarProviderKind.FAKE)
        check(session.status().state == CalendarAuthorizationState.MOCK_ONLY)
        check(viewModel.state.value.gmail == GoogleDemoFeatureState.DISCONNECTED)
        check(viewModel.state.value.pendingConsentPreview == null)
        check(viewModel.state.value.selectedAccount == GoogleDemoAccount.ACCOUNT_A)
        check(viewModel.state.value.accountGeneration == 1)
    }

    private fun verifyFreshProcessState(viewModel: GoogleAuthorizationDemoViewModel) {
        val state = viewModel.state.value
        check(state.gmail == GoogleDemoFeatureState.DISCONNECTED)
        check(state.calendar == GoogleDemoFeatureState.DISCONNECTED)
        check(state.inFlightFeature == null)
        check(state.pendingConsentPreview == null)
        check(state.selectedAccount == GoogleDemoAccount.ACCOUNT_A)
        check(state.accountGeneration == 1)
        check(viewModel.calendarDemoSession.value == null)
    }

    private fun fail(token: String, phase: String, reason: String) {
        status = TextView(this).apply { text = "FAIL $phase" }
        setContentView(status)
        Log.e(TAG, "FAIL:$token:$phase:$reason")
    }

    private companion object {
        const val TAG = "SereaFakeGoogleDeath"
        const val EXTRA_TOKEN = "token"
        const val EXTRA_PHASE = "phase"
        const val PHASE_SEED = "seed"
        const val PHASE_VERIFY = "verify"
        const val PHASE_TIMEOUT_MS = 10_000L
        val PHASES = setOf(PHASE_SEED, PHASE_VERIFY)
        val TOKEN_PATTERN = Regex("[A-Za-z0-9-]{1,120}")
    }
}
