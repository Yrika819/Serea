package app.serea.android

import android.os.SystemClock
import android.util.Log
import androidx.test.internal.runner.listener.InstrumentationRunListener
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.runner.Description
import org.junit.runner.Result
import org.junit.runner.notification.Failure

/** Emits test names and wall-clock boundaries without logging assertion text or fixture contents. */
class SereaInstrumentationLifecycleListener : InstrumentationRunListener() {
    override fun testStarted(description: Description) {
        log("START", description, null)
        if (description.className == "app.serea.android.SereaAppShellTest" &&
            description.methodName == "chatDraftSurvivesGmailAndCalendarDestinationChanges") {
            requestHostSnapshot("before-chat-draft-target")
        }
    }

    override fun testFinished(description: Description) {
        log("FINISH", description, null)
        if (description.className == "app.serea.android.SereaAppShellTest" &&
            description.methodName == "screenHeadingsAreExposedToScreenReaders") {
            requestHostSnapshot("after-screen-headings-neighbor")
        }
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

    private fun requestHostSnapshot(stage: String) {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val directory = instrumentation.targetContext.getExternalFilesDir("root-cause") ?: return
        directory.mkdirs()
        val requestId = System.nanoTime().toString()
        val response = java.io.File(directory, "dumpsys-result-$requestId.txt")
        Log.i("SereaRootCauseFocusOrder", "HOST_DUMPSYS_REQUEST|id=$requestId|stage=$stage")
        val deadline = SystemClock.uptimeMillis() + 15_000
        while (!response.isFile && SystemClock.uptimeMillis() < deadline) {
            SystemClock.sleep(25)
        }
        Log.i(
            "SereaRootCauseFocusOrder",
            "HOST_DUMPSYS_RESULT|id=$requestId|stage=$stage|result=${if (response.isFile) response.readText().trim() else "HOST_RESULT_TIMEOUT"}",
        )
    }

    private companion object {
        const val TAG = "SereaTestLifecycle"
        val UNSAFE_NAME = Regex("[^A-Za-z0-9_.$#-]")
    }
}
