package app.serea.android.googleauth

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import androidx.lifecycle.Lifecycle
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/** Isolated-only activity using the same relay implementation as the Private Beta component. */
internal class PrivateGoogleConsentRelayTestActivity : PrivateGoogleConsentRelayBaseActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        current = this
        created.countDown()
    }

    override fun onDestroy() {
        if (current === this) current = null
        super.onDestroy()
    }

    companion object {
        @Volatile private var created = CountDownLatch(2)
        @Volatile private var current: PrivateGoogleConsentRelayTestActivity? = null

        fun prepareRecreation() {
            created = CountDownLatch(2)
        }

        fun awaitRecreation(): Boolean = created.await(10, TimeUnit.SECONDS)

        fun currentLifecycleState(): Lifecycle.State? = current?.lifecycle?.currentState

        fun recreateCurrent() {
            val activity = checkNotNull(current)
            check(activity.lifecycle.currentState == Lifecycle.State.STARTED) {
                "Consent Relay recreation requires an attached, started window"
            }
            activity.recreate()
        }
    }
}

/** Isolated PendingIntent target lets instrumentation exercise nested Activity Results offline. */
class PrivateGoogleConsentSenderTestActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        instance = this
        launchCount++
        if (delayedResult) {
            launched.countDown()
            return
        }
        finishWith(417, "original-google-result")
    }

    override fun onResume() {
        super.onResume()
        if (delayedResult) launched.countDown()
    }

    private fun finishWith(resultCode: Int, payload: String) {
        setResult(resultCode, Intent().putExtra(EXTRA_PAYLOAD, payload))
        finish()
    }

    companion object {
        const val EXTRA_PAYLOAD = "relay-test-payload"
        @Volatile private var delayedResult = false
        private var launched = CountDownLatch(0)
        @Volatile private var instance: PrivateGoogleConsentSenderTestActivity? = null
        @Volatile private var launchCount = 0

        fun prepareDelayedResult() {
            delayedResult = true
            launched = CountDownLatch(1)
            instance = null
            launchCount = 0
        }

        fun awaitLaunch(): Boolean = launched.await(10, TimeUnit.SECONDS)
        fun launchCount(): Int = launchCount

        fun completeDelayedResult(instrumentation: android.app.Instrumentation) {
            val activity = checkNotNull(instance)
            instrumentation.runOnMainSync {
                activity.finishWith(418, "recreated-google-result")
            }
            delayedResult = false
        }

        fun reset() {
            delayedResult = false
            instance = null
        }
    }
}
