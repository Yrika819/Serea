package app.serea.android.googleauth

import android.app.Activity
import android.app.PendingIntent
import android.content.Intent
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PrivateGoogleConsentRelayInstrumentationTest {
    @Test
    fun accountPickerContractCarriesPerLaunchTokenAcrossSuccessAndCancellation() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val request = PendingGoogleAccountSelection(1L, 1L, "picker-token-unique")
        val contract = PrivateGoogleAccountPickerContract()
        val launch = contract.createIntent(context, request)
        assertEquals(request.requestId, launch.getLongExtra(PrivateGoogleAccountPickerActivity.EXTRA_REQUEST_ID, -1L))
        assertEquals(request.generation, launch.getLongExtra(PrivateGoogleAccountPickerActivity.EXTRA_GENERATION, -1L))
        assertEquals(request.requestToken, launch.getStringExtra(PrivateGoogleAccountPickerActivity.EXTRA_REQUEST_TOKEN))

        val response = Intent()
            .putExtra(PrivateGoogleAccountPickerActivity.EXTRA_REQUEST_ID, request.requestId)
            .putExtra(PrivateGoogleAccountPickerActivity.EXTRA_GENERATION, request.generation)
            .putExtra(PrivateGoogleAccountPickerActivity.EXTRA_REQUEST_TOKEN, request.requestToken)
            .putExtra(PrivateGoogleAccountPickerActivity.EXTRA_ACCOUNT_NAME, "synthetic-account")
            .putExtra(PrivateGoogleAccountPickerActivity.EXTRA_ACCOUNT_TYPE, "com.google")
        val success = contract.parseResult(Activity.RESULT_OK, response)
        assertEquals(request.requestToken, success.requestToken)
        assertEquals("synthetic-account", success.accountName)

        val cancelled = contract.parseResult(Activity.RESULT_CANCELED, response)
        assertEquals(request.requestToken, cancelled.requestToken)
        assertEquals(null, cancelled.accountName)
    }

    @Test
    fun nestedActivityResultReturnsLaunchIdentityAndOriginalPayload() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val contract = PrivateGoogleConsentRelayContract()
        val identity = PrivateGoogleConsentIdentity("consent-A", 41L, "authorization-session-A")
        val pendingIntent = senderPendingIntent(context, 1)
        val launch = contract.createIntent(context, PendingGoogleConsentActivityResult(identity, pendingIntent))
            .setClass(context, PrivateGoogleConsentRelayTestActivity::class.java)

        val scenario = ActivityScenario.launchActivityForResult<PrivateGoogleConsentRelayTestActivity>(launch)
        try {
            val outerResult = scenario.result
            val result = contract.parseResult(outerResult.resultCode, outerResult.resultData)
            assertEquals(identity, result.identity)
            assertEquals(417, result.resultCode)
            assertEquals("original-google-result", result.resultData?.getStringExtra(PrivateGoogleConsentSenderTestActivity.EXTRA_PAYLOAD))
        } finally {
            scenario.close()
        }
    }

    @Test
    fun relayRecreationDoesNotRelaunchOrLoseTheInFlightIdentity() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        val contract = PrivateGoogleConsentRelayContract()
        PrivateGoogleConsentSenderTestActivity.prepareDelayedResult()
        val identity = PrivateGoogleConsentIdentity("consent-recreated", 53L, "session-recreated")
        PrivateGoogleConsentRelayTestActivity.prepareRecreation()
        val relayLaunch = contract.createIntent(
            context,
            PendingGoogleConsentActivityResult(identity, senderPendingIntent(context, 2)),
        ).setClass(context, PrivateGoogleConsentRelayTestActivity::class.java)
        PrivateGoogleConsentRelayResultHarnessActivity.prepare(relayLaunch)
        val launch = Intent(context, PrivateGoogleConsentRelayResultHarnessActivity::class.java)

        // Keep ActivityScenario attached to an outer harness. The relay and its sender are
        // deliberately recreated/launched outside Scenario's managed Activity so the scenario
        // lifecycle tracker remains valid while we exercise the in-flight nested result.
        val scenario = ActivityScenario.launch<PrivateGoogleConsentRelayResultHarnessActivity>(launch)
        try {
            assertTrue(PrivateGoogleConsentSenderTestActivity.awaitLaunch())
            // The nested sender is foreground, so ActivityScenario.recreate() would wait forever
            // for the relay to resume. Request the same Activity recreation directly and await
            // the replacement's onCreate while the sender remains in flight.
            instrumentation.runOnMainSync {
                PrivateGoogleConsentRelayTestActivity.recreateCurrent()
            }
            assertTrue(PrivateGoogleConsentRelayTestActivity.awaitRecreation())
            assertEquals(1, PrivateGoogleConsentSenderTestActivity.launchCount())
            PrivateGoogleConsentSenderTestActivity.completeDelayedResult(instrumentation)
            assertTrue(PrivateGoogleConsentRelayResultHarnessActivity.awaitResult())
            val outerResult = PrivateGoogleConsentRelayResultHarnessActivity.result()
            val result = contract.parseResult(outerResult.resultCode, outerResult.resultData)
            assertEquals(identity, result.identity)
            assertEquals(418, result.resultCode)
            assertEquals(1, PrivateGoogleConsentSenderTestActivity.launchCount())
            assertEquals("recreated-google-result", result.resultData?.getStringExtra(PrivateGoogleConsentSenderTestActivity.EXTRA_PAYLOAD))
        } finally {
            PrivateGoogleConsentSenderTestActivity.reset()
            PrivateGoogleConsentRelayResultHarnessActivity.reset()
            scenario.close()
        }
    }

    @Test
    fun missingRelayIdentityFailsClosed() {
        val result = PrivateGoogleConsentRelayContract().parseResult(Activity.RESULT_OK, Intent())
        assertNull(result.identity)
    }

    private fun senderPendingIntent(context: android.content.Context, requestCode: Int): PendingIntent =
        PendingIntent.getActivity(
            context,
            requestCode,
            Intent(context, PrivateGoogleConsentSenderTestActivity::class.java),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
}
