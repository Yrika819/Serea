package app.serea.android

import android.util.Log
import androidx.test.internal.runner.listener.InstrumentationRunListener
import org.junit.runner.Description
import org.junit.runner.Result
import org.junit.runner.notification.Failure

/** Emits test names and wall-clock boundaries without logging assertion text or fixture contents. */
class SereaInstrumentationLifecycleListener : InstrumentationRunListener() {
    override fun testStarted(description: Description) {
        log("START", description, null)
    }

    override fun testFinished(description: Description) {
        log("FINISH", description, null)
    }

    override fun testFailure(failure: Failure) {
        log("FAIL", failure.description, failure.exception.javaClass.simpleName)
    }

    override fun testRunFinished(result: Result) {
        Log.i(TAG, "RUN_FINISH|${System.currentTimeMillis()}|${result.runCount}|${result.failureCount}|${result.ignoreCount}")
    }

    private fun log(event: String, description: Description, failureType: String?) {
        // Do not use displayName: parameterized descriptions can embed test data.
        val className = description.className.orEmpty().replace(UNSAFE_NAME, "_")
        val methodName = description.methodName.orEmpty().replace(UNSAFE_NAME, "_")
        val suffix = failureType?.replace(UNSAFE_NAME, "_")?.let { "|$it" }.orEmpty()
        Log.i(TAG, "$event|${System.currentTimeMillis()}|$className#$methodName$suffix")
    }

    private companion object {
        const val TAG = "SereaTestLifecycle"
        val UNSAFE_NAME = Regex("[^A-Za-z0-9_.$#-]")
    }
}
