package app.serea.android

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith

/** Minimal diagnostic identity used to separate runner startup failures from native-test setup. */
@RunWith(AndroidJUnit4::class)
class SereaNativeRunnerNoOpTest {
    @Test
    fun androidJUnit4RunnerInstantiatesAndTargetsIsolatedApp() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        assertEquals("app.serea.android.isolated", instrumentation.targetContext.packageName)
        assertEquals("app.serea.android.isolated.test", instrumentation.context.packageName)
    }
}
