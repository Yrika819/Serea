package app.serea.android.googleauth

import android.content.Intent
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.result.ActivityResult
import androidx.activity.result.contract.ActivityResultContracts
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/** Outer result owner keeps ActivityScenario stable while the relay itself is recreated. */
internal class PrivateGoogleConsentRelayResultHarnessActivity : ComponentActivity() {
    private val relay = registerForActivityResult(ActivityResultContracts.StartActivityForResult()) { result ->
        synchronized(ResultState) {
            ResultState.result = result
            ResultState.complete.countDown()
        }
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (savedInstanceState == null) relay.launch(checkNotNull(ResultState.relayIntent))
    }

    companion object ResultState {
        @Volatile var relayIntent: Intent? = null
            private set
        @Volatile var result: ActivityResult? = null
            private set
        @Volatile var complete = CountDownLatch(1)
            private set

        fun prepare(intent: Intent) = synchronized(this) {
            relayIntent = intent
            result = null
            complete = CountDownLatch(1)
        }

        fun awaitResult(): Boolean = complete.await(10, TimeUnit.SECONDS)

        fun result(): ActivityResult = checkNotNull(result)

        fun reset() = synchronized(this) {
            relayIntent = null
            result = null
        }
    }
}
