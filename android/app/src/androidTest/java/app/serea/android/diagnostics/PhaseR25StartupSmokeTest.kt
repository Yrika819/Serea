package app.serea.android.diagnostics

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Minimal runner-only probe for the API 35 startup case matrix; it launches no Activity. */
@RunWith(AndroidJUnit4::class)
class PhaseR25StartupSmokeTest {
    @Test fun runnerStarted() {
        assertTrue(true)
    }
}
