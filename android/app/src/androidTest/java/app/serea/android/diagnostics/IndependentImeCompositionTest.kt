package app.serea.android.diagnostics

import android.os.SystemClock
import android.os.Bundle
import android.os.ParcelFileDescriptor
import android.util.Log
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.InputMethodManager
import android.view.View
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import app.serea.android.MainActivity
import androidx.compose.ui.test.performClick
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Assume.assumeTrue
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
            connection = activity.window.decorView.findFocus()?.onCreateInputConnection(EditorInfo())
            log(activity, "A.initial", objectIdentity(connection), null)
        }
        assumeTrue("Manual onCreateInputConnection returned null; baseline unavailable.", connection != null)
        val directConnectionId = objectIdentity(connection)
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
        val callbackCountBefore = imeEvents().count { it.contains(" onStartInput restarting=") }
        activateRealIme()
        focusComposer()
        val session = awaitImeSession(callbackCountBefore)
        val connection = session.connectionIdentity
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "B.initial", connection, null)
        }
        logSemantics("B.initial", connection)
        compose.activityRule.scenario.onActivity { activity -> log(activity, "B.set-draft", connection, imeSetComposingText(session, FIXTURE)) }
        logSemantics("B.set-draft", connection)
        compose.activityRule.scenario.onActivity { activity -> log(activity, "B.replace", connection, imeSetComposingText(session, REPLACEMENT)) }
        logSemantics("B.replace", connection)
        compose.activityRule.scenario.onActivity { activity -> log(activity, "B.finish", connection, imeFinishComposingText(session)) }
        logSemantics("B.finish", connection)
        awaitValue("B.settled", connection) { it.text == REPLACEMENT && it.composition == null && it.selection == TextRange(REPLACEMENT.length) }
        assertTrue("Android did not start a new input session: ${imeEvents()}", imeEvents().count { it.contains(" onStartInput restarting=") } > callbackCountBefore)
    }

    @Test fun C_realImeCompositionWithSynchronizationBoundaries() {
        val callbackCountBefore = imeEvents().count { it.contains(" onStartInput restarting=") }
        activateRealIme()
        focusComposer()
        awaitImeSession(callbackCountBefore)
        val connection = currentImeSession().connectionIdentity
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "C.initial", connection, null)
            log(activity, "C.set-draft", connection, imeSetComposingText(currentImeSession(), FIXTURE))
        }
        awaitValue("C.draft-composing", connection) { it.text == FIXTURE && it.composition != null && it.selection == TextRange(FIXTURE.length) }
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "C.replace", connection, imeSetComposingText(currentImeSession(), REPLACEMENT))
        }
        awaitValue("C.replacement-composing", connection) { it.text == REPLACEMENT && it.composition != null && it.selection == TextRange(REPLACEMENT.length) }
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "C.finish", connection, imeFinishComposingText(currentImeSession()))
        }
        awaitValue("C.committed", connection) { it.text == REPLACEMENT && it.composition == null && it.selection == TextRange(REPLACEMENT.length) }
        assertTrue("Android did not start a new input session: ${imeEvents()}", imeEvents().count { it.contains(" onStartInput restarting=") } > callbackCountBefore)
    }

    private fun focusComposer() {
        compose.waitUntil(30_000) { runCatching { compose.onNodeWithTag("chat-composer").assertExists() }.isSuccess }
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        var hasWindowFocus = false
        compose.activityRule.scenario.onActivity { activity ->
            hasWindowFocus = activity.window.decorView.hasWindowFocus()
            Log.i(TAG, "FOREGROUND_INITIAL activity=${objectIdentity(activity)} state=${activity.lifecycle.currentState} " +
                "windowFocus=$hasWindowFocus task=${device.executeShellCommand("dumpsys activity activities | grep -E 'topResumedActivity|mResumedActivity' | tail -1")}")
        }
        if (!hasWindowFocus) {
            device.pressHome()
            val target = "$TARGET_PACKAGE/$TARGET_ACTIVITY"
            val launch = device.executeShellCommand("am start -W -n $target")
            Log.i(TAG, "FOREGROUND_START result=$launch")
            assertTrue("UiAutomator could not find the foreground Chat task", device.wait(Until.hasObject(By.pkg(TARGET_PACKAGE)), 15_000))
        }
        compose.waitUntil(20_000) {
            compose.activityRule.scenario.onActivity { activity ->
                hasWindowFocus = activity.window.decorView.hasWindowFocus()
            }
            hasWindowFocus
        }
        val node = compose.onNodeWithTag("chat-composer").fetchSemanticsNode()
        var rootLocation = IntArray(2)
        compose.activityRule.scenario.onActivity { activity ->
            val view: View = activity.window.decorView.findFocus() ?: activity.window.decorView
            view.getLocationOnScreen(rootLocation)
            val rect = node.boundsInRoot
            val x = (rootLocation[0] + rect.center.x).toInt()
            val y = (rootLocation[1] + rect.center.y).toInt()
            Log.i(TAG, "FOREGROUND_TAP activity=${objectIdentity(activity)} task=${device.executeShellCommand("dumpsys activity activities | grep -E 'topResumedActivity|mResumedActivity' | tail -1")}" +
                " bounds=${rect.left},${rect.top},${rect.right},${rect.bottom} screen=${rootLocation[0]},${rootLocation[1]} target=$x,$y windowFocus=${activity.window.decorView.hasWindowFocus()}")
            assertTrue("Chat Activity must own Android Window Focus before composer tap", activity.window.decorView.hasWindowFocus())
            assertTrue("Composer tap missed the display", device.click(x, y))
        }
        compose.onNodeWithTag("chat-composer").assertIsFocused()
        compose.waitForIdle()
        compose.activityRule.scenario.onActivity { activity ->
            Log.i(TAG, "INITIAL api=${android.os.Build.VERSION.SDK_INT} package=$TARGET_PACKAGE activity=${objectIdentity(activity)} " +
                "lifecycle=${activity.lifecycle.currentState} windowFocus=${activity.window.decorView.hasWindowFocus()} " +
                "composerFocused=${runCatching { compose.onNodeWithTag("chat-composer").assertIsFocused(); true }.getOrDefault(false)} " +
                "text=${activity.chatViewModel.state.value.editorValue} imeConfigured=${configuredIme()}")
            assertTrue("Each independent case must start from an empty composer.", activity.chatViewModel.state.value.editorValue.text.isEmpty())
        }
    }

    private fun activateRealIme() {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        Log.i(TAG, "IME_ENABLE_RESULT=${shellCommand("ime enable $IME_ID")}")
        Log.i(TAG, "IME_SET_RESULT=${shellCommand("ime set $IME_ID")}")
        assertEquals("Test IME was not selected by Android", IME_ID, configuredIme())
    }

    private fun shellCommand(command: String): String {
        val descriptor = InstrumentationRegistry.getInstrumentation().uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(descriptor).bufferedReader().use { it.readText() }.trim()
    }

    private data class ImeSession(val serviceIdentity: String, val connectionIdentity: String, val session: Long, val editorPackage: String)

    private fun awaitImeSession(callbackCountBefore: Int): ImeSession {
        var result: ImeSession? = null
        compose.waitUntil(10_000) {
            result = currentImeSessionOrNull()
            result != null && imeEvents().count { it.contains(" onStartInput restarting=") } > callbackCountBefore
        }
        assertNotNull("System did not bind test IME; events=${imeEvents()}", result)
        return result!!
    }

    private fun currentImeSession(): ImeSession = requireNotNull(currentImeSessionOrNull()) { "Bound test IME has no active InputConnection" }

    private fun currentImeSessionOrNull(): ImeSession? {
        val bundle = imeCall("snapshot")
        if (!bundle.getBoolean("serviceBound") || !bundle.getBoolean("connectionBound")) return null
        val session = ImeSession(
            requireNotNull(bundle.getString("serviceIdentity")),
            requireNotNull(bundle.getString("connectionIdentity")),
            bundle.getLong("session"),
            requireNotNull(bundle.getString("editorPackage")),
        )
        assertEquals("IME EditorInfo.packageName does not identify the isolated Chat target", TARGET_PACKAGE, session.editorPackage)
        assertTrue("IME session generation must be positive", session.session > 0)
        assertTrue("IME session must expose an active connection", session.connectionIdentity != "null")
        Log.i(TAG, "REAL_IME_SESSION service=${session.serviceIdentity} generation=${session.session} " +
            "connection=${session.connectionIdentity} editorPackage=${session.editorPackage}")
        return session
    }

    private fun imeEvents(): List<String> = imeCall("snapshot").getStringArrayList("events")?.toList().orEmpty()

    private fun imeSetComposingText(session: ImeSession, text: String): Boolean = imeCall(
        "setComposingText",
        Bundle().apply {
            putString("text", text)
            putInt("cursor", 1)
            putLong("expectedSession", session.session)
            putString("expectedConnection", session.connectionIdentity)
        },
    ).getBoolean("accepted")

    private fun imeFinishComposingText(session: ImeSession): Boolean = imeCall(
        "finishComposingText",
        Bundle().apply {
            putLong("expectedSession", session.session)
            putString("expectedConnection", session.connectionIdentity)
        },
    ).getBoolean("accepted")

    private fun imeCall(method: String, extras: Bundle? = null): Bundle {
        val resolver = InstrumentationRegistry.getInstrumentation().context.contentResolver
        val client = requireNotNull(resolver.acquireContentProviderClient(IME_AUTHORITY)) {
            "Could not connect to the test IME diagnostics provider."
        }
        return try {
            requireNotNull(client.call(method, null, extras)) { "Test IME diagnostics provider returned no result for $method" }
        } finally {
            @Suppress("DEPRECATION")
            client.release()
        }
    }

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
            "stage=$stage result=$accepted activity=${objectIdentity(activity)} " +
            "focused=${objectIdentity(focused)} imeSession=${currentImeSessionOrNull()?.session} " +
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
        const val TAG = "SereaImeR25"
        const val FIXTURE = "draft"
        const val REPLACEMENT = "draft survives navigation"
        const val IME_ID = "app.serea.android.r25testime/app.serea.android.diagnostics.RealImeService"
        const val IME_AUTHORITY = "app.serea.android.r25testime.diagnostics"
        const val TARGET_PACKAGE = "app.serea.android.isolated"
        const val TARGET_ACTIVITY = "app.serea.android.MainActivity"

        fun objectIdentity(value: Any?): String = value?.let {
            "${it.javaClass.name}@${System.identityHashCode(it)}"
        } ?: "null"
    }
}
