package app.serea.android.diagnostics

import android.os.SystemClock
import android.os.Bundle
import android.util.Log
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.serea.android.MainActivity
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/** Focused R2 tests. Each method is a separate instrumentation case and gets a new Activity. */
@RunWith(AndroidJUnit4::class)
class IndependentImeCompositionTest {
    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    @Test fun A_directInputConnectionBaseline() {
        focusComposer()
        var connection: InputConnection? = null
        compose.activityRule.scenario.onActivity { activity ->
            connection = requireNotNull(activity.window.decorView.findFocus()?.onCreateInputConnection(EditorInfo()))
            log(activity, "A.initial", RealImeService.identity(connection), null)
        }
        val directConnectionId = RealImeService.identity(connection)
        logSemantics("A.initial", directConnectionId)
        compose.activityRule.scenario.onActivity { activity -> log(activity, "A.set-draft", directConnectionId, connection!!.setComposingText(FIXTURE, 1)) }
        logSemantics("A.set-draft", directConnectionId)
        compose.activityRule.scenario.onActivity { activity -> log(activity, "A.replace", directConnectionId, connection!!.setComposingText(REPLACEMENT, 1)) }
        logSemantics("A.replace", directConnectionId)
        compose.activityRule.scenario.onActivity { activity -> log(activity, "A.finish", directConnectionId, connection!!.finishComposingText()) }
        logSemantics("A.finish", directConnectionId)
        awaitValue("A.settled", directConnectionId) { it.text == REPLACEMENT && it.composition == null && it.selection == TextRange(REPLACEMENT.length) }
    }

    @Test fun B_realImeCompositionWithoutSynchronizationBoundaries() {
        focusComposer()
        val callbackCountBefore = imeEvents().count { it.contains("onStartInput") }
        activateRealIme()
        val session = awaitImeSession()
        val connection = session.connectionIdentity
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "B.initial", connection, null)
        }
        logSemantics("B.initial", connection)
        compose.activityRule.scenario.onActivity { activity -> log(activity, "B.set-draft", connection, imeSetComposingText(FIXTURE)) }
        logSemantics("B.set-draft", connection)
        compose.activityRule.scenario.onActivity { activity -> log(activity, "B.replace", connection, imeSetComposingText(REPLACEMENT)) }
        logSemantics("B.replace", connection)
        compose.activityRule.scenario.onActivity { activity -> log(activity, "B.finish", connection, imeFinishComposingText()) }
        logSemantics("B.finish", connection)
        awaitValue("B.settled", connection) { it.text == REPLACEMENT && it.composition == null && it.selection == TextRange(REPLACEMENT.length) }
        assertTrue("Android did not start a new input session: ${imeEvents()}", imeEvents().count { it.contains("onStartInput") } > callbackCountBefore)
    }

    @Test fun C_realImeCompositionWithSynchronizationBoundaries() {
        focusComposer()
        val callbackCountBefore = imeEvents().count { it.contains("onStartInput") }
        activateRealIme()
        awaitImeSession()
        val connection = currentImeSession().connectionIdentity
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "C.initial", connection, null)
            log(activity, "C.set-draft", connection, imeSetComposingText(FIXTURE))
        }
        awaitValue("C.draft-composing", connection) { it.text == FIXTURE && it.composition != null && it.selection == TextRange(FIXTURE.length) }
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "C.replace", connection, imeSetComposingText(REPLACEMENT))
        }
        awaitValue("C.replacement-composing", connection) { it.text == REPLACEMENT && it.composition != null && it.selection == TextRange(REPLACEMENT.length) }
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "C.finish", connection, imeFinishComposingText())
        }
        awaitValue("C.committed", connection) { it.text == REPLACEMENT && it.composition == null && it.selection == TextRange(REPLACEMENT.length) }
        assertTrue("Android did not start a new input session: ${imeEvents()}", imeEvents().count { it.contains("onStartInput") } > callbackCountBefore)
    }

    private fun focusComposer() {
        compose.waitUntil(30_000) { runCatching { compose.onNodeWithTag("chat-composer").assertExists() }.isSuccess }
        compose.onNodeWithTag("chat-composer").performClick()
        compose.waitForIdle()
        compose.activityRule.scenario.onActivity { activity ->
            Log.i(TAG, "INITIAL api=${android.os.Build.VERSION.SDK_INT} activity=${RealImeService.identity(activity)} " +
                "text=${activity.chatViewModel.state.value.editorValue} imeConfigured=${configuredIme()}")
            assertTrue("Each independent case must start from an empty composer.", activity.chatViewModel.state.value.editorValue.text.isEmpty())
        }
    }

    private fun activateRealIme() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        instrumentation.uiAutomation.executeShellCommand("ime enable $IME_ID").use { it.close() }
        instrumentation.uiAutomation.executeShellCommand("ime set $IME_ID").use { it.close() }
        compose.activityRule.scenario.onActivity { activity ->
            val decor = activity.window.decorView
            val imm = activity.getSystemService(InputMethodManager::class.java)
            imm.showSoftInput(decor.findFocus(), InputMethodManager.SHOW_IMPLICIT)
        }
    }

    private data class ImeSession(val serviceIdentity: String, val connectionIdentity: String, val session: Long)

    private fun awaitImeSession(): ImeSession {
        var result: ImeSession? = null
        compose.waitUntil(10_000) {
            result = currentImeSessionOrNull()
            result != null
        }
        assertNotNull("System did not bind test IME; events=${imeEvents()}", result)
        return result!!
    }

    private fun currentImeSession(): ImeSession = requireNotNull(currentImeSessionOrNull()) { "Bound test IME has no active InputConnection" }

    private fun currentImeSessionOrNull(): ImeSession? {
        val bundle = imeCall("snapshot")
        if (!bundle.getBoolean("serviceBound") || !bundle.getBoolean("connectionBound")) return null
        return ImeSession(
            requireNotNull(bundle.getString("serviceIdentity")),
            requireNotNull(bundle.getString("connectionIdentity")),
            bundle.getLong("session"),
        )
    }

    private fun imeEvents(): List<String> = imeCall("snapshot").getStringArrayList("events")?.toList().orEmpty()

    private fun imeSetComposingText(text: String): Boolean = imeCall(
        "setComposingText",
        Bundle().apply { putString("text", text); putInt("cursor", 1) },
    ).getBoolean("accepted")

    private fun imeFinishComposingText(): Boolean = imeCall("finishComposingText").getBoolean("accepted")

    private fun imeCall(method: String, extras: Bundle? = null): Bundle = requireNotNull(
        InstrumentationRegistry.getInstrumentation().context.contentResolver.call(IME_AUTHORITY, method, null, extras),
    ) { "Test IME diagnostics provider returned no result for $method" }

    private data class State(val text: String, val composition: TextRange?, val selection: TextRange)

    private fun snapshot(): State {
        var state: State? = null
        compose.activityRule.scenario.onActivity { activity ->
            val value = activity.chatViewModel.state.value.editorValue
            state = State(value.text, value.composition, value.selection)
        }
        return requireNotNull(state)
    }

    private fun logSemantics(stage: String, connection: String?) {
        val selection = runCatching {
            compose.onNodeWithTag("chat-composer").fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange]
        }.getOrNull()
        compose.activityRule.scenario.onActivity { activity ->
            Log.i(TAG, "uptimeNanos=${SystemClock.elapsedRealtimeNanos()} api=${android.os.Build.VERSION.SDK_INT} " +
                "stage=$stage.semantics text=${activity.chatViewModel.state.value.editorValue} " +
                "semanticsSelection=$selection editorInputGeneration=${activity.chatViewModel.state.value.editorInputGeneration} " +
                "editorSelectionCorrectionGeneration=${activity.chatViewModel.state.value.editorSelectionCorrectionGeneration} " +
                "connection=$connection")
        }
    }

    private fun awaitValue(stage: String, connection: String?, condition: (State) -> Boolean) {
        var reached = false
        compose.waitUntil(5_000) {
            reached = condition(snapshot())
            reached
        }
        val semanticSelection = runCatching {
            compose.onNodeWithTag("chat-composer").fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange]
        }.getOrNull()
        log(compose.activityRule.scenario, stage, connection, semanticSelection)
        assertTrue("State did not settle at $stage; last=${snapshot()}", reached)
    }

    private fun log(activity: MainActivity, stage: String, connection: String?, accepted: Boolean?) {
        val state = activity.chatViewModel.state.value
        val decor = activity.window.decorView
        val focused = decor.findFocus()
        val imm = activity.getSystemService(InputMethodManager::class.java)
        Log.i(TAG, "uptimeNanos=${SystemClock.elapsedRealtimeNanos()} api=${android.os.Build.VERSION.SDK_INT} " +
            "stage=$stage result=$accepted activity=${RealImeService.identity(activity)} " +
            "focused=${RealImeService.identity(focused)} imeSession=${currentImeSessionOrNull()?.session} " +
            "ime=${currentImeSessionOrNull()?.serviceIdentity ?: "none"} connection=$connection " +
            "textFieldValue=${state.editorValue} semanticsSelection=sampled-after-settle " +
            "editorInputGeneration=${state.editorInputGeneration} " +
            "editorSelectionCorrectionGeneration=${state.editorSelectionCorrectionGeneration} " +
            "windowFocus=${decor.hasWindowFocus()} acceptingText=${imm.isAcceptingText}")
    }

    private fun log(
        scenario: androidx.test.core.app.ActivityScenario<MainActivity>,
        stage: String,
        connection: String?,
        semanticSelection: TextRange?,
    ) {
        scenario.onActivity { activity ->
            val state = activity.chatViewModel.state.value
            Log.i(TAG, "uptimeNanos=${SystemClock.elapsedRealtimeNanos()} api=${android.os.Build.VERSION.SDK_INT} stage=$stage " +
                "textFieldValue=${state.editorValue} semanticsSelection=$semanticSelection " +
                "editorInputGeneration=${state.editorInputGeneration} " +
                "editorSelectionCorrectionGeneration=${state.editorSelectionCorrectionGeneration} " +
                "windowFocus=${activity.window.decorView.hasWindowFocus()} acceptingText=" +
                activity.getSystemService(InputMethodManager::class.java).isAcceptingText +
                " imeSession=${currentImeSessionOrNull()?.session} connection=$connection")
        }
    }

    private fun configuredIme(): String = runCatching {
        InstrumentationRegistry.getInstrumentation().uiAutomation.executeShellCommand("settings get secure default_input_method")
            .use { descriptor -> java.io.FileInputStream(descriptor.fileDescriptor).bufferedReader().readText().trim() }
    }.getOrDefault("unavailable")

    private companion object {
        const val TAG = "SereaImeR2"
        const val FIXTURE = "draft"
        const val REPLACEMENT = "draft survives navigation"
        const val IME_ID = "app.serea.android.isolated.test/app.serea.android.diagnostics.RealImeService"
        const val IME_AUTHORITY = "app.serea.android.isolated.test.r2ime"
    }
}
