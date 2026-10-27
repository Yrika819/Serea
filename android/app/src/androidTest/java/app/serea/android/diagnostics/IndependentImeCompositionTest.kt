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
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.SemanticsActions
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import app.serea.android.MainActivity
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performTextInput
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
        var session = awaitImeSession(callbackCountBefore)
        var connection = session.connectionIdentity
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "B.initial", connection, null)
        }
        logSemantics("B.initial", connection)
        compose.activityRule.scenario.onActivity { activity -> log(activity, "B.set-draft", connection, imeSetComposingText(session, FIXTURE)) }
        logSemantics("B.set-draft", connection)
        // Refresh only the Android-owned session token; Test B deliberately does not wait for
        // TextFieldValue or ViewModel state between the IME operations.
        session = currentImeSession()
        connection = session.connectionIdentity
        compose.activityRule.scenario.onActivity { activity -> log(activity, "B.replace", connection, imeSetComposingText(session, REPLACEMENT)) }
        logSemantics("B.replace", connection)
        session = currentImeSession()
        connection = session.connectionIdentity
        compose.activityRule.scenario.onActivity { activity -> log(activity, "B.finish", connection, imeFinishComposingText(session)) }
        logSemantics("B.finish", connection)
        awaitValue("B.settled", connection) { it.text == REPLACEMENT && it.composition == null && it.selection == TextRange(REPLACEMENT.length) }
        assertTrue("Android did not start a new input session: ${imeEvents()}", imeEvents().count { it.contains(" onStartInput restarting=") } > callbackCountBefore)
    }

    @Test fun C_realImeCompositionWithSynchronizationBoundaries() {
        val callbackCountBefore = imeEvents().count { it.contains(" onStartInput restarting=") }
        activateRealIme()
        focusComposer()
        var session = awaitImeSession(callbackCountBefore)
        var connection = session.connectionIdentity
        assertMissingExpectedPackageIsRejected(session)
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "C.initial", connection, null)
            log(activity, "C.set-draft", connection, imeSetComposingText(session, FIXTURE))
        }
        awaitValue("C.draft-composing", connection) { it.text == FIXTURE && it.composition != null && it.selection == TextRange(FIXTURE.length) }
        session = currentImeSession()
        connection = session.connectionIdentity
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "C.replace", connection, imeSetComposingText(session, REPLACEMENT))
        }
        awaitValue("C.replacement-composing", connection) { it.text == REPLACEMENT && it.composition != null && it.selection == TextRange(REPLACEMENT.length) }
        session = currentImeSession()
        connection = session.connectionIdentity
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "C.finish", connection, imeFinishComposingText(session))
        }
        awaitValue("C.committed", connection) { it.text == REPLACEMENT && it.composition == null && it.selection == TextRange(REPLACEMENT.length) }
        assertTrue("Android did not start a new input session: ${imeEvents()}", imeEvents().count { it.contains(" onStartInput restarting=") } > callbackCountBefore)
    }

    @Test fun D_realImeNavigationAndActivityRecreation() {
        var callbackCount = imeEvents().count { it.contains(" onStartInput restarting=") }
        activateRealIme()
        focusComposer()
        var session = awaitImeSession(callbackCount)
        assertTrue("D initial IME composition was rejected", imeSetComposingText(session, REPLACEMENT))
        awaitValue("D.initial-composing", session.connectionIdentity) {
            it.text == REPLACEMENT && it.composition != null && it.selection == TextRange(REPLACEMENT.length)
        }
        session = currentImeSession()
        assertTrue("D initial composition commit was rejected", imeFinishComposingText(session))
        awaitValue("D.initial-committed", session.connectionIdentity) {
            it.text == REPLACEMENT && it.composition == null && it.selection == TextRange(REPLACEMENT.length)
        }
        session = currentImeSession()
        assertTrue("D initial selection was rejected", imeSetSelection(session, 5, 5))
        awaitValue("D.selection-5", session.connectionIdentity) {
            it.text == REPLACEMENT && it.composition == null && it.selection == TextRange(5, 5)
        }

        compose.onNodeWithTag("destination_gmail").performClick()
        compose.onNodeWithTag("gmail_search_input").performTextInput("back clears focus")
        compose.onNodeWithTag("gmail_search_input").performImeAction()
        compose.waitUntil(10_000) {
            runCatching {
                compose.onNodeWithTag("gmail_result_count").assertExists()
                compose.onNodeWithTag("gmail_search_input").assertIsNotFocused()
            }.isSuccess
        }
        pressBack()
        compose.waitUntil(10_000) {
            runCatching { compose.onNodeWithTag("gmail_list_heading").assertExists() }.isSuccess
        }
        pressBack()
        compose.waitUntil(10_000) {
            runCatching { compose.onNodeWithTag("chat-composer").assertExists() }.isSuccess &&
                compose.activity.chatViewModel.state.value.editorValue.text == REPLACEMENT
        }
        assertEquals("Gmail return lost the saved composer caret", TextRange(5, 5), compose.activity.chatViewModel.state.value.editorValue.selection)
        callbackCount = imeEvents().count { it.contains(" onStartInput restarting=") }
        compose.onNodeWithTag("chat-composer")
            .performSemanticsAction(SemanticsActions.RequestFocus) { requestFocus -> requestFocus() }
        compose.onNodeWithTag("chat-composer").assertIsFocused()
        session = awaitImeSession(callbackCount)
        assertTrue("Real IME insertion after Gmail return was rejected", imeCommitText(session, "!"))
        awaitValue("D.gmail-reinserted", session.connectionIdentity) {
            it.text == "draft! survives navigation" && it.selection == TextRange(6, 6)
        }

        compose.onNodeWithTag("destination_calendar").performClick()
        compose.onNodeWithTag("calendar_destination").assertExists()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("DEMO / 架空の予定").fetchSemanticsNodes().isNotEmpty()
        }
        pressBack()
        compose.waitUntil(10_000) {
            runCatching { compose.onNodeWithTag("chat-composer").assertExists() }.isSuccess
        }
        assertEquals("Calendar return lost the saved composer caret", TextRange(6, 6), compose.activity.chatViewModel.state.value.editorValue.selection)
        callbackCount = imeEvents().count { it.contains(" onStartInput restarting=") }
        compose.onNodeWithTag("chat-composer")
            .performSemanticsAction(SemanticsActions.RequestFocus) { requestFocus -> requestFocus() }
        compose.onNodeWithTag("chat-composer").assertIsFocused()
        session = awaitImeSession(callbackCount)
        assertTrue("Real IME insertion after Calendar return was rejected", imeCommitText(session, "?"))
        awaitValue("D.calendar-reinserted", session.connectionIdentity) {
            it.text == "draft!? survives navigation" && it.selection == TextRange(7, 7)
        }

        session = currentImeSession()
        assertTrue("Activity recreation caret setup was rejected", imeSetSelection(session, 8, 8))
        awaitValue("D.recreation-selection", session.connectionIdentity) {
            it.text == "draft!? survives navigation" && it.selection == TextRange(8, 8)
        }
        compose.activityRule.scenario.recreate()
        compose.waitUntil(10_000) {
            runCatching { compose.onNodeWithTag("chat-composer").assertExists() }.isSuccess &&
                compose.activity.chatViewModel.state.value.editorValue.text == "draft!? survives navigation" &&
                compose.activity.chatViewModel.state.value.editorValue.selection == TextRange(8, 8)
        }
        callbackCount = imeEvents().count { it.contains(" onStartInput restarting=") }
        compose.onNodeWithTag("chat-composer")
            .performSemanticsAction(SemanticsActions.RequestFocus) { requestFocus -> requestFocus() }
        compose.onNodeWithTag("chat-composer").assertIsFocused()
        session = awaitImeSession(callbackCount)
        assertTrue("Real IME insertion after Activity recreation was rejected", imeCommitText(session, "!"))
        awaitValue("D.recreation-reinserted", session.connectionIdentity) {
            it.text == "draft!? !survives navigation" && it.selection == TextRange(9, 9)
        }
        compose.activityRule.scenario.onActivity { activity ->
            log(activity, "D.navigation-and-recreation", session.connectionIdentity, true)
        }
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
            if (device.wait(Until.hasObject(By.text("Wait")), 1_000)) {
                Log.w(TAG, "FOREGROUND_SYSTEM_ANR_DIALOG found=Wait; requesting system UI recovery")
                device.findObject(By.text("Wait")).click()
                device.waitForIdle(10_000)
            }
            device.pressHome()
            val target = "$TARGET_PACKAGE/$TARGET_ACTIVITY"
            val launch = device.executeShellCommand("am start -W -f 0x30000000 -n $target")
            Log.i(TAG, "FOREGROUND_START result=$launch")
            Log.i(TAG, "FOREGROUND_AFTER task=${device.executeShellCommand("dumpsys activity activities | grep -E 'topResumedActivity|mResumedActivity' | tail -1")} " +
                "focus=${device.executeShellCommand("dumpsys window windows | grep -E 'mCurrentFocus|mFocusedApp' | tail -2")}")
            assertTrue("UiAutomator could not find the foreground Chat task", device.wait(Until.hasObject(By.pkg(TARGET_PACKAGE)), 15_000))
        }
        compose.waitUntil(20_000) {
            compose.activityRule.scenario.onActivity { activity ->
                hasWindowFocus = activity.window.decorView.hasWindowFocus()
            }
            hasWindowFocus
        }
        val node = compose.onNodeWithTag("chat-composer").fetchSemanticsNode()
        compose.activityRule.scenario.onActivity { activity ->
            val editors = device.findObjects(By.clazz("android.widget.EditText"))
            Log.i(TAG, "FOREGROUND_ACCESSIBILITY_EDITORS count=${editors.size} bounds=${editors.map { it.visibleBounds }} " +
                "composerBounds=${node.boundsInRoot} display=${device.displayWidth}x${device.displayHeight} " +
                "activity=${objectIdentity(activity)} windowFocus=${activity.window.decorView.hasWindowFocus()}")
            assertTrue("Chat Activity must own Android Window Focus before composer tap", activity.window.decorView.hasWindowFocus())
            assertNotNull("UiAutomator did not expose the Composer EditText", editors.firstOrNull())
            editors.first().click()
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

    private fun pressBack() {
        compose.activityRule.scenario.onActivity { it.onBackPressedDispatcher.onBackPressed() }
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

    private data class ImeSession(
        val serviceIdentity: String,
        val connectionIdentity: String,
        val session: Long,
        val editorPackage: String,
        val inputType: Int,
        val fieldId: Int,
        val fieldName: String,
    )

    private data class ImeOperationObservation(
        val operationInvoked: Boolean,
        val accepted: Boolean,
        val reason: String,
        val expectedPackage: String,
        val expectedService: String,
        val expectedSession: Long,
        val expectedConnection: String,
        val expectedInputType: Int,
        val service: String,
        val session: Long,
        val connection: String,
        val editorPackage: String,
        val inputType: Int,
    )

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
            bundle.getInt("inputType"),
            bundle.getInt("fieldId"),
            requireNotNull(bundle.getString("fieldName")),
        )
        assertEquals("IME EditorInfo.packageName does not identify the isolated Chat target", TARGET_PACKAGE, session.editorPackage)
        assertTrue("IME session generation must be positive", session.session > 0)
        assertTrue("IME session must expose an active connection", session.connectionIdentity != "null")
        assertTrue("IME EditorInfo must identify a text editor, not a generic focused view: $session", session.inputType != android.text.InputType.TYPE_NULL)
        Log.i(TAG, "REAL_IME_SESSION service=${session.serviceIdentity} generation=${session.session} " +
            "connection=${session.connectionIdentity} editorPackage=${session.editorPackage} inputType=${session.inputType} " +
            "fieldId=${session.fieldId} fieldName=${session.fieldName}")
        return session
    }

    private fun imeEvents(): List<String> = imeCall("snapshot").getStringArrayList("events")?.toList().orEmpty()

    private fun imeSetComposingText(session: ImeSession, text: String): Boolean = runImeOperation(
        "setComposingText",
        session,
    ) {
        putString("text", text)
        putInt("cursor", 1)
    }

    private fun imeFinishComposingText(session: ImeSession): Boolean = runImeOperation(
        "finishComposingText",
        session,
    ) { }

    private fun imeSetSelection(session: ImeSession, start: Int, end: Int): Boolean = runImeOperation(
        "setSelection",
        session,
    ) {
        putInt("start", start)
        putInt("end", end)
    }

    private fun imeCommitText(session: ImeSession, text: String): Boolean = runImeOperation(
        "commitText",
        session,
    ) {
        putString("text", text)
        putInt("cursor", 1)
    }

    private fun operationObservation(result: Bundle, expectedInputType: Int) = ImeOperationObservation(
        operationInvoked = result.getBoolean("operationInvoked"),
        accepted = result.getBoolean("accepted"),
        reason = result.getString("operationReason") ?: "MISSING",
        expectedPackage = result.getString("operationExpectedPackage") ?: "null",
        expectedService = result.getString("operationExpectedService") ?: "null",
        expectedSession = result.getLong("operationExpectedSession"),
        expectedConnection = result.getString("operationExpectedConnection") ?: "null",
        expectedInputType = expectedInputType,
        service = result.getString("operationService") ?: "null",
        session = result.getLong("operationSession"),
        connection = result.getString("operationConnection") ?: "null",
        editorPackage = result.getString("operationEditorPackage") ?: "null",
        inputType = result.getInt("operationInputType", android.text.InputType.TYPE_NULL),
    )

    private fun runImeOperation(
        method: String,
        session: ImeSession,
        request: Bundle.() -> Unit,
    ): Boolean {
        val arguments = Bundle().apply {
            putString("expectedEditorPackage", TARGET_PACKAGE)
            putString("expectedService", session.serviceIdentity)
            putLong("expectedSession", session.session)
            putString("expectedConnection", session.connectionIdentity)
            request.invoke(this)
        }
        val result = imeCall(method, arguments)
        val observation = operationObservation(result, session.inputType)
        logImeOperation(method, observation)
        assertTrue("$method was not invoked: $observation", observation.operationInvoked)
        assertTrue("$method InputConnection result was rejected: $observation", observation.accepted)
        assertEquals("$method request package mismatch: $observation", TARGET_PACKAGE, observation.expectedPackage)
        assertEquals("$method request service changed: $observation", session.serviceIdentity, observation.expectedService)
        assertEquals("$method request session changed: $observation", session.session, observation.expectedSession)
        assertEquals("$method request connection changed: $observation", session.connectionIdentity, observation.expectedConnection)
        assertEquals("$method operation used another service: $observation", session.serviceIdentity, observation.service)
        assertEquals("$method operation used another session: $observation", session.session, observation.session)
        assertEquals("$method operation used another connection: $observation", session.connectionIdentity, observation.connection)
        assertEquals("$method operation used another editor package: $observation", TARGET_PACKAGE, observation.editorPackage)
        assertEquals("$method operation input type changed: $observation", session.inputType, observation.inputType)
        assertEquals("$method invocation reason mismatch: $observation", "INVOKED_ACCEPTED", observation.reason)
        return observation.accepted
    }

    private fun assertMissingExpectedPackageIsRejected(session: ImeSession) {
        val request = Bundle().apply {
            putString("text", "diagnostic guard probe")
            putInt("cursor", 1)
            putLong("expectedSession", session.session)
            putString("expectedService", session.serviceIdentity)
            putString("expectedConnection", session.connectionIdentity)
            // Deliberately omit expectedEditorPackage to prove the guard refuses to invoke.
        }
        val result = imeCall("setComposingText", request)
        val observation = operationObservation(result, session.inputType)
        logImeOperation("setComposingText-missing-package-probe", observation)
        assertEquals("Missing package must have a bounded rejection reason: $observation", "MISSING_EXPECTED_PACKAGE", observation.reason)
        assertEquals("Guard rejection must not invoke InputConnection: $observation", false, observation.operationInvoked)
        assertEquals("Guard rejection must not report an InputConnection result: $observation", false, observation.accepted)
        assertEquals("Guard probe must omit expected package: $observation", "null", observation.expectedPackage)
        assertEquals("Guard probe expected service changed: $observation", session.serviceIdentity, observation.expectedService)
        assertEquals("Guard probe expected session changed: $observation", session.session, observation.expectedSession)
        assertEquals("Guard probe expected connection changed: $observation", session.connectionIdentity, observation.expectedConnection)
        assertEquals("Guard probe actual service changed: $observation", session.serviceIdentity, observation.service)
        assertEquals("Guard probe must inspect the current Android session", session.session, observation.session)
        assertEquals("Guard probe must inspect the current Android connection", session.connectionIdentity, observation.connection)
        assertEquals("Guard probe must inspect the current editor package", TARGET_PACKAGE, observation.editorPackage)
        assertEquals("Guard probe input type changed: $observation", session.inputType, observation.inputType)
    }

    private fun logImeOperation(method: String, operation: ImeOperationObservation) {
        Log.i(TAG, "REAL_IME_OPERATION method=$method operationInvoked=${operation.operationInvoked} " +
            "accepted=${operation.accepted} reason=${operation.reason} " +
            "expectedPackage=${operation.expectedPackage} actualPackage=${operation.editorPackage} " +
            "expectedService=${operation.expectedService} actualService=${operation.service} " +
            "expectedSession=${operation.expectedSession} actualSession=${operation.session} " +
            "expectedConnection=${operation.expectedConnection} actualConnection=${operation.connection} " +
            "expectedInputType=${operation.expectedInputType} actualInputType=${operation.inputType}")
    }

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
        runCatching {
            compose.waitUntil(5_000) {
                reached = condition(snapshot())
                reached
            }
        }
        val finalState = snapshot()
        val semanticSelection = runCatching {
            compose.onNodeWithTag("chat-composer").fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange]
        }.getOrNull()
        log(compose.activityRule.scenario, stage, connection, semanticSelection)
        if (!reached) {
            Log.e(TAG, "DIAGNOSTIC_STATE_TIMEOUT stage=$stage actual=$finalState " +
                "semanticsSelection=$semanticSelection connection=$connection")
        }
        assertTrue("State did not settle at $stage; actual=$finalState semanticsSelection=$semanticSelection", reached)
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
