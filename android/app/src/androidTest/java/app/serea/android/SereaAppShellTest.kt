package app.serea.android

import android.content.pm.ActivityInfo
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.ViewConfiguration
import android.view.WindowManager
import android.os.ParcelFileDescriptor
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import android.net.Uri
import android.os.Bundle
import android.view.inputmethod.InputMethodManager
import androidx.activity.compose.setContent
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
import androidx.compose.ui.test.assertIsFocused
import androidx.compose.ui.test.assertIsNotFocused
import androidx.compose.ui.test.assertIsSelected
import androidx.compose.ui.test.assertIsNotEnabled
import androidx.compose.ui.test.assertTextContains
import androidx.compose.ui.test.assertTextEquals
import androidx.compose.ui.test.assertIsEnabled
import androidx.compose.ui.test.junit4.createAndroidComposeRule
import androidx.compose.ui.test.onAllNodesWithTag
import androidx.compose.ui.test.onAllNodesWithText
import androidx.compose.ui.test.onFirst
import androidx.compose.ui.test.onNodeWithTag
import androidx.compose.ui.test.onNodeWithText
import androidx.compose.ui.test.onRoot
import androidx.compose.ui.test.isRoot
import androidx.compose.ui.test.printToLog
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.ExperimentalTestApi
import androidx.compose.ui.test.performTextInputSelection
import androidx.compose.ui.text.TextRange
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.SemanticsActions
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.espresso.Espresso
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.uiautomator.By
import androidx.test.uiautomator.UiDevice
import androidx.test.uiautomator.Until
import app.serea.android.assistant.taskproposal.DemoTaskProposalRepository
import app.serea.android.assistant.taskproposal.TaskProposalDemoScreenForTest
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.calendar.FakeCalendarProvider
import app.serea.android.gmail.GmailProviderKind
import app.serea.android.calendar.ui.CalendarAgendaItemsKey
import app.serea.android.calendar.ui.CalendarLazyListPositionKey
import app.serea.android.calendar.ui.CalendarLazyListViewportKey
import app.serea.android.calendar.ui.CalendarAgendaLoadedEventCountKey
import kotlinx.coroutines.CompletableDeferred
import org.junit.Before
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import java.time.Instant
import java.time.ZoneId
import kotlin.math.ceil
import kotlin.math.roundToInt

@RunWith(AndroidJUnit4::class)
class SereaAppShellTest {
    private companion object {
        const val CALENDAR_STAGE_TAG = "SereaCalendarStage"
        const val FONT_SCALE_STAGE_TAG = "SereaFontScale"
        const val TASKS_DESTINATION_STAGE_TAG = "SereaTasksDestination"
        const val COMPOSER_IME_DIAGNOSTICS_TAG = "SereaComposerIme"
        const val TEST_IME_ID = "app.serea.android.r25testime/app.serea.android.diagnostics.RealImeService"
        const val TEST_IME_AUTHORITY = "app.serea.android.r25testime.diagnostics"
        const val ISOLATED_PACKAGE = "app.serea.android.isolated"
        const val COMPOSER_DRAFT = "draft"
        const val COMPOSER_REPLACEMENT = "draft survives navigation"
    }

    private data class TestImeSession(
        val service: String,
        val connection: String,
        val generation: Long,
        val editorPackage: String,
        val inputType: Int,
        val fieldId: Int,
    )

    private fun imeSnapshot(): Bundle {
        val client = requireNotNull(
            InstrumentationRegistry.getInstrumentation().context.contentResolver
                .acquireContentProviderClient(Uri.parse("content://$TEST_IME_AUTHORITY")),
        ) { "Required Test IME diagnostics provider is unavailable." }
        return try {
            requireNotNull(client.call("snapshot", null, null)) { "Test IME returned no session snapshot." }
        } finally {
            @Suppress("DEPRECATION")
            client.release()
        }
    }

    private fun awaitTestImeSession(startEventsBefore: Int): TestImeSession {
        var found: TestImeSession? = null
        compose.waitUntil(10_000) {
            val state = imeSnapshot()
            val events = state.getStringArrayList("events").orEmpty()
            if (state.getBoolean("serviceBound") && state.getBoolean("connectionBound") &&
                events.count { it.contains(" onStartInput restarting=") } > startEventsBefore &&
                state.getString("editorPackage") == ISOLATED_PACKAGE &&
                state.getInt("inputType") != android.text.InputType.TYPE_NULL
            ) {
                found = TestImeSession(
                    requireNotNull(state.getString("serviceIdentity")),
                    requireNotNull(state.getString("connectionIdentity")),
                    state.getLong("session"),
                    requireNotNull(state.getString("editorPackage")),
                    state.getInt("inputType"),
                    state.getInt("fieldId"),
                )
                true
            } else false
        }
        val session = requireNotNull(found) { "Android did not bind the required Test IME InputConnection." }
        assertTrue("IME session must have a live system InputConnection", session.connection != "null")
        assertTrue("IME EditorInfo must describe a text field", session.inputType != android.text.InputType.TYPE_NULL)
        assertEquals("IME must be bound to the isolated Chat target", ISOLATED_PACKAGE, session.editorPackage)
        Log.i(COMPOSER_IME_DIAGNOSTICS_TAG, "REAL_IME_SESSION uptimeNanos=${SystemClock.elapsedRealtimeNanos()} $session")
        Log.i(COMPOSER_IME_DIAGNOSTICS_TAG,
            "REAL_IME_CALLBACKS uptimeNanos=${SystemClock.elapsedRealtimeNanos()} events=" +
                statefulImeEvents().joinToString(" || "))
        return session
    }

    private fun statefulImeEvents(): List<String> = imeSnapshot().getStringArrayList("events").orEmpty().toList()

    /** Read the currently Android-bound Test IME session immediately before an operation. */
    private fun currentTestImeSession(): TestImeSession {
        val state = imeSnapshot()
        assertTrue("Test IME service is not currently bound", state.getBoolean("serviceBound"))
        assertTrue("Test IME has no current Android InputConnection", state.getBoolean("connectionBound"))
        val session = TestImeSession(
            service = requireNotNull(state.getString("serviceIdentity")),
            connection = requireNotNull(state.getString("connectionIdentity")),
            generation = state.getLong("session"),
            editorPackage = requireNotNull(state.getString("editorPackage")),
            inputType = state.getInt("inputType"),
            fieldId = state.getInt("fieldId"),
        )
        assertTrue("Test IME service identity is unexpected: $session",
            session.service.startsWith("app.serea.android.diagnostics.RealImeService@"))
        assertTrue("Test IME must expose a positive input session: $session", session.generation > 0)
        assertTrue("Test IME must expose a live system InputConnection: $session", session.connection != "null")
        assertEquals("Test IME must remain bound to isolated Chat", ISOLATED_PACKAGE, session.editorPackage)
        assertTrue("EditorInfo must describe a text field: $session",
            session.inputType != android.text.InputType.TYPE_NULL)
        return session
    }

    private fun tapComposerThroughSystemInput() {
        val bounds = compose.onNodeWithTag("chat-composer").fetchSemanticsNode().boundsInWindow
        val origin = IntArray(2)
        compose.activityRule.scenario.onActivity { activity ->
            activity.window.decorView.getLocationOnScreen(origin)
        }
        val x = origin[0] + bounds.center.x.toInt()
        val y = origin[1] + bounds.center.y.toInt()
        val uiAutomatorAccepted = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()).click(x, y)
        val shellResult = if (uiAutomatorAccepted) {
            "NOT_USED"
        } else {
            shell("input tap $x $y && echo SEREA_TOUCH_DISPATCHED")
        }
        val accepted = uiAutomatorAccepted || shellResult.endsWith("SEREA_TOUCH_DISPATCHED")
        Log.i(COMPOSER_IME_DIAGNOSTICS_TAG,
            "SYSTEM_TOUCH uptimeNanos=${SystemClock.elapsedRealtimeNanos()} bounds=$bounds " +
                "screenOrigin=${origin.toList()} screenPoint=($x,$y) " +
                "uiAutomatorAccepted=$uiAutomatorAccepted shellResult=$shellResult accepted=$accepted")
        assertTrue("Neither UiAutomator nor Android shell dispatched a touchscreen tap to the Chat Composer.", accepted)
    }

    private fun awaitComposerWindowFocus(stage: String) {
        var snapshot = "MainActivity unavailable"
        val focused = runCatching {
            compose.waitUntil(10_000) {
                var hasWindowFocus = false
                compose.activityRule.scenario.onActivity { activity ->
                    val decor = activity.window.decorView
                    hasWindowFocus = decor.hasWindowFocus()
                    snapshot = "stage=$stage activity=${activity.javaClass.simpleName}@${System.identityHashCode(activity)} " +
                        "lifecycle=${activity.lifecycle.currentState} windowFocus=$hasWindowFocus " +
                        "decorFocus=${decor.hasFocus()} attached=${decor.isAttachedToWindow} visible=${decor.isShown}"
                }
                hasWindowFocus
            }
        }.isSuccess
        Log.i(COMPOSER_IME_DIAGNOSTICS_TAG, "WINDOW_FOCUS uptimeNanos=${SystemClock.elapsedRealtimeNanos()} $snapshot")
        assertTrue("Chat Composer Activity did not acquire Android window focus: $snapshot", focused)
    }

    /** Bring a resumed but unfocused target task to the foreground before sending a real touch. */
    private fun ensureChatWindowFocus(stage: String) {
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        var hasWindowFocus = false
        var activitySnapshot = "MainActivity unavailable"
        compose.activityRule.scenario.onActivity { activity ->
            hasWindowFocus = activity.window.decorView.hasWindowFocus()
            activitySnapshot = "activity=${activity.javaClass.simpleName}@${System.identityHashCode(activity)} " +
                "lifecycle=${activity.lifecycle.currentState} windowFocus=$hasWindowFocus " +
                "attached=${activity.window.decorView.isAttachedToWindow} visible=${activity.window.decorView.isShown}"
        }
        Log.i(COMPOSER_IME_DIAGNOSTICS_TAG,
            "FOREGROUND_INITIAL uptimeNanos=${SystemClock.elapsedRealtimeNanos()} stage=$stage $activitySnapshot " +
                "task=${shell("dumpsys activity activities | grep -E 'topResumedActivity|mResumedActivity' | tail -1")}")
        if (!hasWindowFocus) {
            if (device.wait(Until.hasObject(By.text("Wait")), 1_000)) {
                Log.w(COMPOSER_IME_DIAGNOSTICS_TAG, "FOREGROUND_SYSTEM_ANR_DIALOG stage=$stage found=Wait")
                device.findObject(By.text("Wait")).click()
                device.waitForIdle(10_000)
            }
            device.pressHome()
            val target = "$ISOLATED_PACKAGE/app.serea.android.MainActivity"
            val launch = device.executeShellCommand("am start -W -f 0x30000000 -n $target")
            Log.i(COMPOSER_IME_DIAGNOSTICS_TAG,
                "FOREGROUND_START uptimeNanos=${SystemClock.elapsedRealtimeNanos()} stage=$stage result=$launch")
            assertTrue("Android did not confirm foreground Chat Activity launch: $launch", launch.contains("Status: ok"))
            assertTrue("UiAutomator could not find the foreground Chat task",
                device.wait(Until.hasObject(By.pkg(ISOLATED_PACKAGE)), 15_000))
        }
        val focused = runCatching {
            compose.waitUntil(20_000) {
                compose.activityRule.scenario.onActivity { activity ->
                    hasWindowFocus = activity.window.decorView.hasWindowFocus()
                    activitySnapshot = "activity=${activity.javaClass.simpleName}@${System.identityHashCode(activity)} " +
                        "lifecycle=${activity.lifecycle.currentState} windowFocus=$hasWindowFocus " +
                        "attached=${activity.window.decorView.isAttachedToWindow} visible=${activity.window.decorView.isShown}"
                }
                hasWindowFocus
            }
        }.isSuccess
        Log.i(COMPOSER_IME_DIAGNOSTICS_TAG,
            "FOREGROUND_AFTER uptimeNanos=${SystemClock.elapsedRealtimeNanos()} stage=$stage $activitySnapshot " +
                "task=${shell("dumpsys activity activities | grep -E 'topResumedActivity|mResumedActivity' | tail -1")} " +
                "windows=${shell("dumpsys window windows | grep -E 'mCurrentFocus|mFocusedApp' | tail -2")}")
        assertTrue("Chat Activity did not acquire Android window focus after foreground recovery: $activitySnapshot", focused)
    }

    private fun imeOperation(
        method: String,
        baselineSession: TestImeSession,
        text: String? = null,
        selectionStart: Int? = null,
        selectionEnd: Int? = null,
    ): Boolean {
        // A previous InputConnection may have been replaced by Android. Bind every operation
        // to a fresh IME-owned identity, then require the provider to report that exact identity
        // at invocation time. This reads no Chat/ViewModel/Semantics state.
        val session = currentTestImeSession()
        assertEquals("The baseline session must describe the same isolated editor.",
            ISOLATED_PACKAGE, baselineSession.editorPackage)
        val args = Bundle().apply {
            putString("expectedEditorPackage", session.editorPackage)
            putString("expectedService", session.service)
            putLong("expectedSession", session.generation)
            putString("expectedConnection", session.connection)
            putInt("expectedInputType", session.inputType)
            if (text != null) {
                putString("text", text)
                putInt("cursor", 1)
            }
            if (selectionStart != null && selectionEnd != null) {
                putInt("start", selectionStart)
                putInt("end", selectionEnd)
            }
        }
        val client = requireNotNull(
            InstrumentationRegistry.getInstrumentation().context.contentResolver
                .acquireContentProviderClient(Uri.parse("content://$TEST_IME_AUTHORITY")),
        ) { "Required Test IME diagnostics provider is unavailable." }
        val result = try {
            requireNotNull(client.call(method, null, args)) { "Test IME returned no result for $method." }
        } finally {
            @Suppress("DEPRECATION")
            client.release()
        }
        val accepted = result.getBoolean("accepted")
        val invoked = result.getBoolean("operationInvoked")
        val reason = result.getString("operationReason")
        val expectedService = result.getString("operationExpectedService")
        val expectedSession = result.getLong("operationExpectedSession")
        val expectedConnection = result.getString("operationExpectedConnection")
        val expectedInputType = result.getInt("operationExpectedInputType", android.text.InputType.TYPE_NULL)
        val operationSession = result.getLong("operationSession")
        val operationConnection = requireNotNull(result.getString("operationConnection"))
        val operationService = requireNotNull(result.getString("operationService"))
        val operationEditorPackage = requireNotNull(result.getString("operationEditorPackage"))
        val operationInputType = result.getInt("operationInputType")
        assertTrue("Test IME must invoke the Android-managed connection for $method: $result", invoked)
        assertEquals("Test IME must accept $method", "INVOKED_ACCEPTED", reason)
        assertTrue("IME operation must use a live system InputConnection.", operationConnection != "null")
        assertEquals("IME operation must use the current service", session.service, expectedService)
        assertEquals("IME operation must use the current session", session.generation, expectedSession)
        assertEquals("IME operation must use the current connection", session.connection, expectedConnection)
        assertEquals("IME operation must use the current input type", session.inputType, expectedInputType)
        assertEquals("IME operation must invoke the current service", session.service, operationService)
        assertEquals("IME operation must invoke the current InputConnection", session.connection, operationConnection)
        assertEquals("IME operation must invoke the current session", session.generation, operationSession)
        assertEquals("IME operation must target the isolated Chat editor.", session.editorPackage, operationEditorPackage)
        assertEquals("IME operation must use the active editor input type", session.inputType, operationInputType)
        Log.i(COMPOSER_IME_DIAGNOSTICS_TAG,
            "REAL_IME_OPERATION uptimeNanos=${SystemClock.elapsedRealtimeNanos()} method=$method " +
                "invoked=$invoked accepted=$accepted reason=$reason " +
                "baselineService=${baselineSession.service} baselineSession=${baselineSession.generation} " +
                "baselineConnection=${baselineSession.connection} " +
                "service=$operationService session=$operationSession " +
                "connection=$operationConnection editorPackage=$operationEditorPackage " +
                "inputType=$operationInputType fixture=$text")
        assertTrue("Android-managed Test IME rejected $method for active session $operationSession " +
            "after snapshot $session.", accepted)
        return accepted
    }

    private fun logComposerImeDiagnostics(stage: String, inputConnection: InputConnection?) {
        val semanticSelection = runCatching {
            compose.onNodeWithTag("chat-composer").fetchSemanticsNode()
                .config[SemanticsProperties.TextSelectionRange]
        }.getOrNull()
        compose.activityRule.scenario.onActivity { activity ->
            val editorValue = activity.chatViewModel.state.value.editorValue
            val decor = activity.window.decorView
            val focusedView = decor.findFocus()
            val inputMethodManager = activity.getSystemService(InputMethodManager::class.java)
            Log.i(
                COMPOSER_IME_DIAGNOSTICS_TAG,
                "uptimeMs=${SystemClock.uptimeMillis()} api=${android.os.Build.VERSION.SDK_INT} " +
                    "activity=${System.identityHashCode(activity)} stage=$stage editorValue=$editorValue " +
                    "inputGeneration=${activity.chatViewModel.state.value.editorInputGeneration} " +
                    "selectionCorrectionGeneration=${activity.chatViewModel.state.value.editorSelectionCorrectionGeneration} " +
                    "semanticSelection=$semanticSelection windowFocus=${decor.hasWindowFocus()} " +
                    "focusedView=${focusedView?.javaClass?.name}@${focusedView?.let { System.identityHashCode(it) }} " +
                    "acceptingText=${inputMethodManager.isAcceptingText} " +
                    "inputConnection=${inputConnection?.let { System.identityHashCode(it) }}",
            )
        }
    }

    private fun logImmediateComposerImeDiagnostics(
        activity: MainActivity,
        stage: String,
        inputConnection: InputConnection,
        accepted: Boolean,
    ) {
        val editorValue = activity.chatViewModel.state.value.editorValue
        val decor = activity.window.decorView
        val focusedView = decor.findFocus()
        val inputMethodManager = activity.getSystemService(InputMethodManager::class.java)
        Log.i(
            COMPOSER_IME_DIAGNOSTICS_TAG,
            "uptimeMs=${SystemClock.uptimeMillis()} stage=$stage accepted=$accepted editorValue=$editorValue " +
                "inputGeneration=${activity.chatViewModel.state.value.editorInputGeneration} " +
                "selectionCorrectionGeneration=${activity.chatViewModel.state.value.editorSelectionCorrectionGeneration} " +
                "windowFocus=${decor.hasWindowFocus()} " +
                "focusedView=${focusedView?.javaClass?.name}@${focusedView?.let { System.identityHashCode(it) }} " +
                "acceptingText=${inputMethodManager.isAcceptingText} " +
                "inputConnection=${System.identityHashCode(inputConnection)}",
        )
    }

    private fun awaitComposerImeState(
        stage: String,
        inputConnection: InputConnection?,
        condition: () -> Boolean,
    ) {
        val settled = runCatching { compose.waitUntil(5_000, condition) }.isSuccess
        logComposerImeDiagnostics(stage, inputConnection)
        assertTrue("Composer IME state did not settle at $stage.", settled)
    }

    @get:Rule val compose = createAndroidComposeRule<MainActivity>()

    private enum class CalendarGesturePath { NODE_EDGE, NODE_INTERIOR, ROOT_INTERIOR }
    private var calendarGestureProbeDone = false
    private var calendarDetailSemanticsProbeDone = false
    private var calendarGesturePath = CalendarGesturePath.NODE_INTERIOR

    private data class CalendarBoundsSnapshot(
        val left: Float,
        val top: Float,
        val right: Float,
        val bottom: Float,
    ) {
        val centerX: Float get() = (left + right) / 2f
        val width: Float get() = right - left
        val height: Float get() = bottom - top
        override fun toString() = "[$left,$top,$right,$bottom]"
    }

    /** Contains only copied values; the Compose semantics node and its state-reading lambdas stay local. */
    private data class CalendarSemanticsSnapshot(
        val scrollValue: Float?,
        val scrollMax: Float?,
        val position: Pair<Int, Int>?,
        val viewport: app.serea.android.calendar.ui.CalendarLazyListViewportSnapshot?,
        val loadedEventCount: Int?,
        val agendaItems: app.serea.android.calendar.ui.CalendarAgendaItemsSnapshot?,
        val boundsInRoot: CalendarBoundsSnapshot,
        val boundsInWindow: CalendarBoundsSnapshot,
        val hasClickAction: Boolean,
        val visibleSemantics: String,
    )

    private data class CalendarSemanticsTarget(
        val interaction: androidx.compose.ui.test.SemanticsNodeInteraction,
        val nodes: androidx.compose.ui.test.SemanticsNodeInteractionCollection,
    )

    /** Select a semantics node on the test thread; read snapshot-backed values on the UI thread. */
    private fun snapshotSemantics(target: CalendarSemanticsTarget): CalendarSemanticsSnapshot {
        compose.waitForIdle()
        // The public collection API selects without serializing lazy, snapshot-backed properties
        // into SemanticsNodeInteraction's last-seen failure message.
        val nodes = target.nodes.fetchSemanticsNodes()
        check(nodes.size == 1) { "Expected one Calendar semantics node, found ${nodes.size}." }
        val node = nodes.single()
        // Keep semantics reads on the Compose test thread. Reading this node on the UI thread and
        // then performing gestures on the instrumentation thread can bind SnapshotStateObserver
        // to different threads after ActivityScenario recreation (notably on API 35).
        val axis = node.config.getOrElseNullable(SemanticsProperties.VerticalScrollAxisRange) { null }
        val position = node.config.getOrElseNullable(CalendarLazyListPositionKey) { null }?.let { it.first to it.second }
        val viewport = node.config.getOrElseNullable(CalendarLazyListViewportKey) { null }?.let {
            it.copy(visibleItemIndices = it.visibleItemIndices.toList())
        }
        val agendaItems = node.config.getOrElseNullable(CalendarAgendaItemsKey) { null }?.let {
            it.copy(eventItemIndices = it.eventItemIndices.toMap())
        }
        val root = node.boundsInRoot
        val window = node.boundsInWindow
        return CalendarSemanticsSnapshot(
            scrollValue = axis?.value(),
            scrollMax = axis?.maxValue(),
            position = position,
            viewport = viewport,
            loadedEventCount = node.config.getOrElseNullable(CalendarAgendaLoadedEventCountKey) { null },
            agendaItems = agendaItems,
            boundsInRoot = CalendarBoundsSnapshot(root.left, root.top, root.right, root.bottom),
            boundsInWindow = CalendarBoundsSnapshot(window.left, window.top, window.right, window.bottom),
            hasClickAction = SemanticsActions.OnClick in node.config,
            // Traversing SemanticsNode.children evaluates collapsed semantics through the
            // SnapshotStateObserver. That observer is already bound to the Compose UI thread;
            // recursively reading child nodes from the instrumentation thread causes an
            // API-dependent cross-thread observer failure. Keep the snapshot to the explicit
            // values above, which the gestures and assertions use.
            visibleSemantics = "child traversal omitted to preserve SnapshotStateObserver thread affinity",
        )
    }

    private fun calendarNodeWithTag(tag: String) = CalendarSemanticsTarget(
        interaction = compose.onNodeWithTag(tag),
        nodes = compose.onAllNodesWithTag(tag),
    )

    private fun calendarNodeWithText(text: String, substring: Boolean = true, unmerged: Boolean = false) =
        CalendarSemanticsTarget(
            interaction = compose.onNodeWithText(text, substring = substring, useUnmergedTree = unmerged),
            nodes = compose.onAllNodesWithText(text, substring = substring, useUnmergedTree = unmerged),
        )

    private fun calendarRootNode() = CalendarSemanticsTarget(
        interaction = compose.onRoot(),
        nodes = compose.onAllNodes(isRoot()),
    )

    @Before fun waitUntilChatCanAcceptInput() {
        calendarGestureProbeDone = false
        calendarDetailSemanticsProbeDone = false
        calendarGesturePath = CalendarGesturePath.NODE_INTERIOR
        compose.waitUntil(30_000) {
            runCatching { compose.onNodeWithTag("chat-composer").assertIsEnabled() }.isSuccess
        }
    }

    private fun select(destination: SereaDestination) = compose.onNodeWithTag(
        "destination_${destination.name.lowercase()}",
    ).performClick()

    private fun pressBack() {
        compose.activityRule.scenario.onActivity { it.onBackPressedDispatcher.onBackPressed() }
    }

    private fun calendarStage(name: String, outcome: String) {
        val testThread = Thread.currentThread()
        val uiThread = Looper.getMainLooper().thread
        Log.i(
            CALENDAR_STAGE_TAG,
            "$outcome|$name|testThread=${testThread.name}:${testThread.id}|" +
                "uiThread=${uiThread.name}:${uiThread.id}|onUiThread=${testThread === uiThread}",
        )
    }

    private fun fontScaleStage(name: String) {
        Log.i(FONT_SCALE_STAGE_TAG, "STAGE=$name")
    }

    /** Scrolls a real LazyColumn and uses the loaded item map to bound the search and detect a skip. */
    private fun scrollAgendaUntilDisplayed(tag: String, text: String) {
        val list = calendarNodeWithTag(tag)
        val fixtureTarget = if (tag == "calendar_agenda_list") {
            FakeCalendarProvider.Fixtures.events.singleOrNull { it.title == text }
        } else {
            null
        }
        if (tag == "calendar_agenda_list") {
            assertTrue("Calendar search target must exist exactly once in the fixture: $text", fixtureTarget != null)
        }
        compose.waitUntil(10_000) {
            runCatching {
                val snapshot = snapshotSemantics(list)
                snapshot.boundsInRoot.height > 0f
            }.getOrDefault(false)
        }
        if (fixtureTarget != null) {
            runCatching {
                compose.waitUntil(10_000) {
                    fixtureTarget.key.eventId.value in
                        snapshotSemantics(list).agendaItems?.eventItemIndices.orEmpty()
                }
            }
        }
        val initial = snapshotSemantics(list)
        if (tag == "calendar_event_detail_list" && !calendarDetailSemanticsProbeDone &&
            (initial.scrollMax ?: 0f) > 0f
        ) {
            calendarDetailSemanticsProbeDone = true
            verifyCalendarSemanticsScrollToIndex(list)
        }
        val targetIndex = fixtureTarget?.let { initial.agendaItems?.eventItemIndices?.get(it.key.eventId.value) }
        if (fixtureTarget != null && targetIndex == null) {
            throw AssertionError(
                "Calendar target exists in the fake fixture but was not loaded into the agenda: " +
                    "target=${fixtureTarget.key.eventId.value} calendar=${fixtureTarget.key.calendarId.value} " +
                    "loadedEvents=${initial.loadedEventCount} items=${initial.viewport?.totalItems} " +
                    "position=${initial.position} viewport=${initial.viewport} bounds=${initial.boundsInRoot} " +
                    "visible=${initial.visibleSemantics}",
            )
        }
        if (isCalendarTargetTextFullyVisible(list, text)) {
            logVisibleCalendarItemBounds(tag, text)
            return
        }
        if (targetIndex != null && initial.viewport?.visibleItemIndices?.minOrNull()?.let { it > targetIndex } == true) {
            throw AssertionError(
                "Calendar target was already passed before the search began: target=$text " +
                    "targetItemIndex=$targetIndex position=${initial.position} viewport=${initial.viewport} " +
                    "loadedEvents=${initial.loadedEventCount} bounds=${initial.boundsInRoot} " +
                    "visible=${initial.visibleSemantics}",
            )
        }
        if (tag == "calendar_agenda_list" && !calendarGestureProbeDone) {
            if ((initial.scrollMax ?: 0f) > 0f) {
                calendarGestureProbeDone = true
                calendarGesturePath = diagnoseCalendarGesturePath(list)
            }
        }
        val initiallyVisibleItems = initial.viewport?.visibleItemIndices?.size?.coerceAtLeast(1)
            ?: ceil(initial.boundsInRoot.height / 64f).toInt().coerceAtLeast(1)
        val fallbackLoadedItems = initial.viewport?.totalItems ?:
            ceil((initial.scrollMax ?: 0f) / initial.boundsInRoot.height.coerceAtLeast(1f)).toInt() + 1
        val initialFirstVisible = initial.position?.first ?: initial.viewport?.visibleItemIndices?.minOrNull() ?: 0
        val initialTargetDistance = targetIndex?.let { it - initialFirstVisible }
            ?: fallbackLoadedItems
        val maximumSwipes = (
            ceil(initialTargetDistance.coerceAtLeast(1).toDouble() / initiallyVisibleItems).toInt() * 2 + 2
        ).coerceIn(1, 128)
        var last = initial
        var searchBudget = maximumSwipes
        var stopReason = "finite search budget exhausted"
        var swipeIndex = 0
        while (swipeIndex < searchBudget && swipeIndex < 128) {
            if (isCalendarTargetTextFullyVisible(list, text)) {
                logVisibleCalendarItemBounds(tag, text)
                return
            }
            val before = snapshotSemantics(list)
            val beforeValue = before.scrollValue ?: 0f
            val beforeMax = before.scrollMax ?: 0f
            assertTrue("$tag must be vertically scrollable before searching for $text", beforeMax > 0f)
            if (before.viewport?.canScrollForward == false || beforeValue >= beforeMax - 1f) {
                stopReason = "scroll range ended before target"
                break
            }

            val swipeHeight = before.boundsInRoot.height
            val touchSlop = ViewConfiguration.get(compose.activity).scaledTouchSlop.toFloat()
            assertTrue(
                "$tag must expose enough visible height for a real scroll gesture " +
                    "(height=$swipeHeight, touchSlop=$touchSlop)",
                swipeHeight > touchSlop * 2f + 2f,
            )
            val gestureAdvanced = performCalendarGesture(
                list, calendarGesturePath, swipeHeight, "$tag|loop=${swipeIndex + 1}",
            )
            compose.waitForIdle()
            val after = snapshotSemantics(list)
            swipeIndex += 1
            val afterValue = after.scrollValue ?: 0f
            val afterMax = after.scrollMax ?: 0f
            val lazyListPositionAdvanced = when {
                before.position != null && after.position != null -> before.position != after.position
                before.viewport != null && after.viewport != null ->
                    before.viewport.visibleItemIndices != after.viewport.visibleItemIndices
                else -> false
            }
            Log.i(
                CALENDAR_STAGE_TAG,
                "SWIPE|$tag|index=$swipeIndex|from=$beforeValue|to=$afterValue|max=$afterMax|" +
                    "position=${before.position}->${after.position}|" +
                    "visibleIndices=${before.viewport?.visibleItemIndices}->${after.viewport?.visibleItemIndices}|" +
                    "viewport=${after.viewport}|" +
                    "loadedEvents=${after.loadedEventCount}",
            )
            assertTrue(
                "$tag Touch swipe $swipeIndex did not advance the real LazyList position " +
                    "(from=$beforeValue/${before.position}, to=$afterValue/${after.position}, " +
                    "max=$afterMax, viewport=${after.viewport})",
                gestureAdvanced && lazyListPositionAdvanced && afterValue > beforeValue + 0.5f,
            )
            last = after
            val activeTargetIndex = if (fixtureTarget != null) {
                after.agendaItems?.eventItemIndices?.get(fixtureTarget.key.eventId.value)
            } else {
                targetIndex
            }
            if (targetIndex != null && activeTargetIndex == null) {
                throw AssertionError(
                    "Calendar target disappeared from loaded agenda data during search: target=$text " +
                        "targetItemIndex=$targetIndex position=${after.position} viewport=${after.viewport} " +
                        "loadedEvents=${after.loadedEventCount} bounds=${after.boundsInRoot} " +
                        "visible=${after.visibleSemantics}",
                )
            }
            if (activeTargetIndex != null &&
                after.viewport?.visibleItemIndices?.minOrNull()?.let { it > activeTargetIndex } == true
            ) {
                throw AssertionError(
                    "Calendar search passed the loaded target without displaying it: target=$text " +
                        "targetItemIndex=$activeTargetIndex position=${after.position} viewport=${after.viewport} " +
                        "loadedEvents=${after.loadedEventCount} bounds=${after.boundsInRoot} " +
                        "visible=${after.visibleSemantics}",
                )
            }
            if (activeTargetIndex != null && after.viewport?.visibleItemIndices?.contains(activeTargetIndex) == true) {
                logCalendarTargetVisibilityAtItemRange(tag, text, activeTargetIndex, after)
            }
            val currentVisibleItems = after.viewport?.visibleItemIndices?.size?.coerceAtLeast(1)
                ?: initiallyVisibleItems
            val currentFirstVisible = after.position?.first ?: after.viewport?.visibleItemIndices?.minOrNull() ?: 0
            val currentItemCount = maxOf(after.viewport?.totalItems ?: 0, activeTargetIndex?.plus(1) ?: 0)
            val currentDistance = activeTargetIndex?.let { (it - currentFirstVisible).coerceAtLeast(1) }
                ?: (currentItemCount - currentFirstVisible).coerceAtLeast(1)
            val remainingSwipes = ceil(currentDistance.toDouble() / currentVisibleItems).toInt() * 2 + 2
            searchBudget = maxOf(searchBudget, swipeIndex + remainingSwipes).coerceAtMost(128)
            if (after.viewport?.canScrollForward == false || afterValue >= afterMax - 1f) {
                stopReason = "scroll range ended before target"
                break
            }
        }
        // The final Touch can expose the target at the same time it reaches canScrollForward=false.
        // Re-read the Text Semantics and its clipped bounds after that layout before declaring the
        // finite gesture search exhausted.
        if (isCalendarTargetTextFullyVisible(list, text)) {
            logVisibleCalendarItemBounds(tag, text)
            return
        }
        if (targetIndex != null) {
            list.interaction.performSemanticsAction(SemanticsActions.ScrollToIndex) { scrollToIndex ->
                scrollToIndex(targetIndex)
            }
            compose.waitForIdle()
            last = snapshotSemantics(list)
            Log.i(
                CALENDAR_STAGE_TAG,
                "SEMANTICS_SCROLL_TO_TARGET|tag=$tag|target=$text|targetItemIndex=$targetIndex|" +
                    "position=${last.position}|viewport=${last.viewport}|bounds=${last.boundsInRoot}",
            )
            if (last.viewport?.visibleItemIndices?.contains(targetIndex) == true &&
                isCalendarTargetTextFullyVisible(list, text)
            ) {
                logVisibleCalendarItemBounds(tag, text)
                return
            }
            stopReason = "finite Touch search ended; ScrollToIndex did not fully expose target Text"
        } else if (tag == "calendar_event_detail_list") {
            // Detail labels are not backed by the agenda event-id map. Enumerate the finite
            // LazyColumn semantics range until the real unmerged Text node is composed, then use
            // its ancestor CollectionItemInfo rather than guessing from visible item indices.
            val itemCount = checkNotNull(last.viewport) {
                "Calendar detail list must expose its LazyColumn item count."
            }.totalItems
            var actualTargetIndex: Int? = null
            for (requestedIndex in 0 until itemCount) {
                scrollCalendarToSemanticsIndex(list, requestedIndex)
                last = snapshotSemantics(list)
                val discoveredIndex = calendarTextCollectionItemIndex(text, tag) ?: continue
                actualTargetIndex = discoveredIndex
                Log.i(
                    CALENDAR_STAGE_TAG,
                    "SEMANTICS_TEXT_INDEX_DISCOVERED|tag=$tag|target=$text|requestedIndex=$requestedIndex|" +
                        "actualItemIndex=$discoveredIndex|viewport=" + last.viewport +
                        "|scroll=" + last.scrollValue + "/" + last.scrollMax,
                )
                scrollCalendarToSemanticsIndex(list, discoveredIndex)
                last = snapshotSemantics(list)
                val confirmedIndex = calendarTextCollectionItemIndex(text, tag)
                assertEquals(
                    "Calendar detail Text CollectionItemInfo index must remain stable after ScrollToIndex",
                    discoveredIndex,
                    confirmedIndex,
                )
                if (last.viewport?.visibleItemIndices?.contains(discoveredIndex) == true &&
                    isCalendarTargetTextFullyVisible(list, text)
                ) {
                    logVisibleCalendarItemBounds(tag, text)
                    return
                }
                stopReason = "ScrollToIndex resolved item " + discoveredIndex +
                    " but target Text bounds were not fully visible"
                break
            }
            if (actualTargetIndex == null) {
                stopReason = "finite ScrollToIndex scan found no exact Text CollectionItemInfo in 0 until " +
                    itemCount + "; lastViewport=" + last.viewport
            }
        }
        throw AssertionError(
            "Calendar target was not displayed after a finite search: target=$text " +
                "fixturePresent=${fixtureTarget != null} stopReason=$stopReason swipes=$swipeIndex/$searchBudget " +
                "loadedEvents=${last.loadedEventCount} " +
                "targetItemIndex=$targetIndex position=${last.position} viewport=${last.viewport} " +
                "scroll=${last.scrollValue}/${last.scrollMax} bounds=${last.boundsInRoot} " +
                "visible=${last.visibleSemantics}",
        )
    }

    private fun logVisibleCalendarItemBounds(tag: String, text: String) {
        // EventCard merges its descendants. Read the unmerged Text node for actual glyph bounds,
        // and the merged card separately for its user-operable click semantics.
        val textSemantics = snapshotSemantics(calendarTargetTextTarget(text))
        val semanticsItemIndex = calendarTextCollectionItemIndex(text, tag)
        val item = if (tag == "calendar_agenda_list") {
            snapshotSemantics(calendarNodeWithText(text))
        } else if (semanticsItemIndex != null) {
            snapshotSemantics(calendarNodeWithTag("calendar_event_detail_item_$semanticsItemIndex"))
        } else {
            textSemantics
        }
        val list = snapshotSemantics(calendarNodeWithTag(tag))
        val root = snapshotSemantics(calendarRootNode())
        val viewport = checkNotNull(list.viewport) { "$tag must expose its real LazyColumn viewport." }
        val viewportTop = list.boundsInRoot.top + viewport.viewportStartOffset
        val viewportBottom = list.boundsInRoot.top + viewport.viewportEndOffset
        val visibleTop = maxOf(textSemantics.boundsInRoot.top, viewportTop, root.boundsInRoot.top)
        val visibleBottom = minOf(textSemantics.boundsInRoot.bottom, viewportBottom, root.boundsInRoot.bottom)
        val visibleHeight = (visibleBottom - visibleTop).coerceAtLeast(0f)
        val visibleFraction = visibleHeight / textSemantics.boundsInRoot.height.coerceAtLeast(1f)
        val viewportLeft = maxOf(list.boundsInRoot.left, root.boundsInRoot.left)
        val viewportRight = minOf(list.boundsInRoot.right, root.boundsInRoot.right)
        val textFullyVisible = textSemantics.boundsInRoot.left >= viewportLeft - 1f &&
            textSemantics.boundsInRoot.right <= viewportRight + 1f &&
            textSemantics.boundsInRoot.top >= viewportTop - 1f &&
            textSemantics.boundsInRoot.bottom <= viewportBottom + 1f &&
            textSemantics.boundsInRoot.top >= root.boundsInRoot.top - 1f &&
            textSemantics.boundsInRoot.bottom <= root.boundsInRoot.bottom + 1f
        if (tag == "calendar_agenda_list") {
            assertTrue("Calendar agenda target must expose its tappable event semantics: $text", item.hasClickAction)
        } else {
            assertTrue(
                "Calendar detail Text bounds must resolve to one indexed LazyColumn item semantics node: $text",
                semanticsItemIndex != null,
            )
            assertTrue(
                "Calendar detail Text item index must be in the actual visible LazyColumn range: " +
                    "$text index=$semanticsItemIndex visible=${viewport.visibleItemIndices}",
                semanticsItemIndex in viewport.visibleItemIndices,
            )
        }
        assertTrue(
            "Calendar target text semantics must intersect the clipped visible LazyColumn viewport: " +
                "target=$text text=${textSemantics.boundsInRoot} item=${item.boundsInRoot} " +
                "viewport=$viewport list=${list.boundsInRoot} " +
                "root=${root.boundsInRoot}",
            visibleHeight > 0f,
        )
        assertTrue(
            "Calendar target Text must be fully inside the visible viewport before it is treated as readable: " +
                "target=$text text=${textSemantics.boundsInRoot} item=${item.boundsInRoot} " +
                "viewport=$viewport list=${list.boundsInRoot} root=${root.boundsInRoot} " +
                "visibleFraction=$visibleFraction",
            textFullyVisible,
        )
        Log.i(
            CALENDAR_STAGE_TAG,
            "TEXT_COLLECTION_ITEM_INDEX|tag=$tag|text=$text|index=$semanticsItemIndex|" +
                "itemRoot=${item.boundsInRoot}|itemWindow=${item.boundsInWindow}",
        )
        Log.i(
            CALENDAR_STAGE_TAG,
            "VISIBLE_ITEM|tag=$tag|text=$text|textRoot=${textSemantics.boundsInRoot}|" +
                "textWindow=${textSemantics.boundsInWindow}|itemRoot=${item.boundsInRoot}|itemWindow=${item.boundsInWindow}|" +
                "listRoot=${list.boundsInRoot}|listWindow=${list.boundsInWindow}|root=${root.boundsInRoot}|" +
                "position=${list.position}|viewport=$viewport|visibleFraction=$visibleFraction|" +
                "textFullyVisible=$textFullyVisible|tappable=${item.hasClickAction}|" +
                "loadedEvents=${list.loadedEventCount}",
        )
    }

    private fun isCalendarTargetTextFullyVisible(
        listInteraction: CalendarSemanticsTarget,
        text: String,
    ): Boolean {
        val textTarget = calendarTargetTextTarget(text)
        val textMatches = textTarget.nodes.fetchSemanticsNodes()
        if (textMatches.isEmpty()) {
            if (text == "説明") {
                val partialMatches = compose.onAllNodesWithText(
                    text,
                    substring = true,
                    useUnmergedTree = true,
                ).fetchSemanticsNodes()
                var candidateDetails: List<String> = emptyList()
                if (partialMatches.isNotEmpty()) {
                    compose.runOnIdle {
                        candidateDetails = partialMatches.map { node ->
                            val labels = node.config.getOrElseNullable(SemanticsProperties.Text) { null }
                                ?.joinToString(" ") { it.text }
                                .orEmpty()
                            "$labels@${node.boundsInRoot}"
                        }
                    }
                }
                Log.i(
                    CALENDAR_STAGE_TAG,
                    "EXACT_TEXT_SEMANTICS_MISSING|target=$text|partialMatches=${partialMatches.size}|" +
                        "candidates=$candidateDetails",
                )
            }
            return false
        }
        check(textMatches.size == 1) {
            "Expected one exact unmerged Calendar Text semantics node for '$text', found ${textMatches.size}."
        }
        // Snapshot-backed Semantics failures must surface here with their cause; treating one as
        // an invisible node hid the distinction between missing, ambiguous, and unreadable Text.
        val textSnapshot = snapshotSemantics(textTarget)
        val list = snapshotSemantics(listInteraction)
        val root = snapshotSemantics(calendarRootNode())
        val viewport = list.viewport ?: return false
        val left = maxOf(list.boundsInRoot.left, root.boundsInRoot.left)
        val right = minOf(list.boundsInRoot.right, root.boundsInRoot.right)
        val top = maxOf(list.boundsInRoot.top + viewport.viewportStartOffset, root.boundsInRoot.top)
        val bottom = minOf(list.boundsInRoot.top + viewport.viewportEndOffset, root.boundsInRoot.bottom)
        val bounds = textSnapshot.boundsInRoot
        val insideViewport = bounds.left >= left - 1f && bounds.right <= right + 1f &&
            bounds.top >= top - 1f && bounds.bottom <= bottom + 1f
        val assertionFailure = try {
            calendarTargetTextNode(text).assertIsDisplayed()
            null
        } catch (failure: AssertionError) {
            failure
        }
        val composeAssertDisplayed = assertionFailure == null
        val displayed = insideViewport && composeAssertDisplayed
        if (!displayed) {
            Log.i(
                CALENDAR_STAGE_TAG,
                "TEXT_NOT_FULLY_DISPLAYED|text=$text|textRoot=${textSnapshot.boundsInRoot}|" +
                    "textWindow=${textSnapshot.boundsInWindow}|listRoot=${list.boundsInRoot}|" +
                    "listWindow=${list.boundsInWindow}|viewport=$viewport|root=${root.boundsInRoot}|" +
                    "insideViewport=$insideViewport|composeAssertDisplayed=$composeAssertDisplayed|" +
                    "assertionFailure=${assertionFailure?.message}",
            )
        }
        return displayed
    }

    /** Resolves Text to its list item from merged/unmerged nodes or the explicit detail item node. */
    private fun calendarTextCollectionItemIndex(text: String, tag: String): Int? {
        if (tag == "calendar_event_detail_list") {
            return calendarDetailTextItemIndex(text)
        }
        val targets = listOf(
            "merged" to calendarNodeWithText(text, substring = false),
            "unmerged" to calendarTargetTextTarget(text),
        )
        val matchingNodes = targets.mapNotNull { (tree, target) ->
            val matches = target.nodes.fetchSemanticsNodes()
            check(matches.size <= 1) {
                "Expected at most one exact $tree Calendar Text semantics node for '$text', found ${matches.size}."
            }
            matches.singleOrNull()?.let { tree to it }
        }
        if (matchingNodes.isEmpty()) return null
        var indices = emptyList<Pair<String, Int>>()
        compose.runOnIdle {
            indices = matchingNodes.mapNotNull { (tree, matchedNode) ->
                var node: androidx.compose.ui.semantics.SemanticsNode? = matchedNode
                while (node != null) {
                    val item = node.config.getOrElseNullable(SemanticsProperties.CollectionItemInfo) { null }
                    if (item != null) {
                        return@mapNotNull tree to item.rowIndex
                    }
                    node = node.parent
                }
                null
            }
        }
        val distinctIndices = indices.map { it.second }.distinct()
        check(distinctIndices.size <= 1) {
            "Merged and unmerged Calendar Text semantics resolved to different LazyColumn indices for '$text': $indices"
        }
        return distinctIndices.singleOrNull()
    }

    /**
     * Compose 1.7.8 does not expose CollectionItemInfo on this detail Text's semantics parent
     * chain. Resolve its actual item through an explicit item semantics tag and require the
     * unmerged Text bounds to be contained by exactly one currently visible indexed item node.
     */
    private fun calendarDetailTextItemIndex(text: String): Int? {
        val textSnapshot = runCatching { snapshotSemantics(calendarTargetTextTarget(text)) }.getOrNull()
            ?: return null
        val list = snapshotSemantics(calendarNodeWithTag("calendar_event_detail_list"))
        val visibleIndices = checkNotNull(list.viewport).visibleItemIndices
        val matchingIndices = visibleIndices.filter { index ->
            val item = runCatching {
                snapshotSemantics(calendarNodeWithTag("calendar_event_detail_item_$index"))
            }.getOrNull() ?: return@filter false
            val textBounds = textSnapshot.boundsInRoot
            val itemBounds = item.boundsInRoot
            textBounds.left >= itemBounds.left - 1f && textBounds.right <= itemBounds.right + 1f &&
                textBounds.top >= itemBounds.top - 1f && textBounds.bottom <= itemBounds.bottom + 1f
        }
        check(matchingIndices.size <= 1) {
            "Calendar detail Text must resolve to one indexed item by Semantics bounds: " +
                "text=$text visible=$visibleIndices matching=$matchingIndices"
        }
        return matchingIndices.singleOrNull()
    }

    private fun scrollCalendarToSemanticsIndex(list: CalendarSemanticsTarget, index: Int) {
        list.interaction.performSemanticsAction(SemanticsActions.ScrollToIndex) { scrollToIndex ->
            scrollToIndex(index)
        }
        compose.waitForIdle()
    }

    /** Records card visibility separately from its merged and unmerged Text semantics bounds. */
    private fun logCalendarTargetVisibilityAtItemRange(
        tag: String,
        text: String,
        targetItemIndex: Int,
        list: CalendarSemanticsSnapshot,
    ) {
        val viewport = list.viewport ?: return
        val textSnapshot = runCatching { snapshotSemantics(calendarTargetTextTarget(text)) }.getOrNull()
        val cardSnapshot = runCatching { snapshotSemantics(calendarNodeWithText(text)) }.getOrNull()
        if (textSnapshot == null) {
            Log.i(
                CALENDAR_STAGE_TAG,
                "TARGET_ITEM_RANGE_REACHED|tag=$tag|target=$text|targetItemIndex=$targetItemIndex|" +
                    "textNode=absent|cardBounds=${cardSnapshot?.boundsInRoot}|" +
                    "position=${list.position}|viewport=$viewport|listBounds=${list.boundsInRoot}",
            )
            return
        }
        val viewportTop = list.boundsInRoot.top + viewport.viewportStartOffset
        val viewportBottom = list.boundsInRoot.top + viewport.viewportEndOffset
        val clippedTop = maxOf(textSnapshot.boundsInRoot.top, viewportTop)
        val clippedBottom = minOf(textSnapshot.boundsInRoot.bottom, viewportBottom)
        val visibleHeight = (clippedBottom - clippedTop).coerceAtLeast(0f)
        val textHeight = textSnapshot.boundsInRoot.height.coerceAtLeast(1f)
        Log.i(
            CALENDAR_STAGE_TAG,
            "TARGET_ITEM_RANGE_REACHED|tag=$tag|target=$text|targetItemIndex=$targetItemIndex|" +
                "textRoot=${textSnapshot.boundsInRoot}|textWindow=${textSnapshot.boundsInWindow}|" +
                "cardBounds=${cardSnapshot?.boundsInRoot}|listBounds=${list.boundsInRoot}|" +
                "viewport=$viewport|visibleTextFraction=${visibleHeight / textHeight}|" +
                "textIntersectsViewport=${visibleHeight > 0f}|position=${list.position}",
        )
    }

    private fun calendarTargetTextNode(text: String) =
        compose.onNodeWithText(text, substring = false, useUnmergedTree = true)

    private fun calendarTargetTextTarget(text: String) =
        calendarNodeWithText(text, substring = false, unmerged = true)

    /** Separates scroll semantics from touch hit testing before searching a long agenda. */
    private fun diagnoseCalendarGesturePath(list: CalendarSemanticsTarget): CalendarGesturePath {
        val node = snapshotSemantics(list)
        val root = snapshotSemantics(calendarRootNode())
        val destination = snapshotSemantics(calendarNodeWithTag("calendar_destination"))
        val bounds = node.boundsInRoot
        val rootBounds = root.boundsInRoot
        val destinationBounds = destination.boundsInRoot
        val visibleLeft = maxOf(bounds.left, rootBounds.left, destinationBounds.left)
        val visibleTop = maxOf(bounds.top, rootBounds.top, destinationBounds.top)
        val visibleRight = minOf(bounds.right, rootBounds.right, destinationBounds.right)
        val visibleBottom = minOf(bounds.bottom, rootBounds.bottom, destinationBounds.bottom)
        val height = bounds.height
        val edgeStartY = height - 1f
        val edgeEndY = 1f
        val interiorStartY = height * 0.8f
        val interiorEndY = height * 0.2f
        Log.i(
            CALENDAR_STAGE_TAG,
            "GEOMETRY|boundsInRoot=${node.boundsInRoot}|boundsInWindow=${node.boundsInWindow}|" +
                "root=${root.boundsInRoot}|destination=${destination.boundsInRoot}|" +
                "visibleIntersection=[$visibleLeft,$visibleTop,$visibleRight,$visibleBottom]|" +
                "size=${bounds.width}x$height|rootCenterX=${rootBounds.centerX}|listCenterX=${bounds.centerX}|" +
                "windowOffset=${node.boundsInWindow.top - node.boundsInRoot.top}|" +
                "scroll=${node.scrollValue}/${node.scrollMax}|firstVisible=${node.position}|" +
                "viewport=${node.viewport}|loadedEvents=${node.loadedEventCount}|" +
                "edgeLocal=$edgeStartY->$edgeEndY|" +
                "edgeRoot=${bounds.centerX},${bounds.top + edgeStartY}->${bounds.centerX},${bounds.top + edgeEndY}|" +
                "interiorLocal=$interiorStartY->$interiorEndY",
        )
        assertTrue("Calendar list must intersect its visible root and destination bounds", visibleRight > visibleLeft && visibleBottom > visibleTop)
        assertTrue("Root-center input must fall inside the Calendar list horizontally", rootBounds.centerX in visibleLeft..visibleRight)
        assertTrue("Current 1px edge start must be inside the Calendar list bounds", edgeStartY in 0f..height)
        assertTrue("Current 1px edge end must be inside the Calendar list bounds", edgeEndY in 0f..height)
        assertTrue("Current edge gesture points must be inside the clipped visible list region",
            bounds.centerX in visibleLeft..visibleRight &&
                bounds.top + edgeStartY in visibleTop..visibleBottom &&
                bounds.top + edgeEndY in visibleTop..visibleBottom)
        assertTrue("Interior gesture points must be inside the clipped visible list region",
            bounds.centerX in visibleLeft..visibleRight &&
                bounds.top + interiorStartY in visibleTop..visibleBottom &&
                bounds.top + interiorEndY in visibleTop..visibleBottom)

        val edgeProgress = performCalendarGesture(
            list,
            CalendarGesturePath.NODE_EDGE,
            height,
            "probe-edge",
        )
        if (edgeProgress) {
            verifyCalendarSemanticsScrollToIndex(list)
            return CalendarGesturePath.NODE_EDGE
        }

        val interiorProgress = performCalendarGesture(
            list,
            CalendarGesturePath.NODE_INTERIOR,
            height,
            "probe-node-interior",
        )
        if (interiorProgress) {
            verifyCalendarSemanticsScrollToIndex(list)
            return CalendarGesturePath.NODE_INTERIOR
        }

        val rootProgress = performCalendarGesture(
            list,
            CalendarGesturePath.ROOT_INTERIOR,
            height,
            "probe-root-interior",
        )
        if (rootProgress) {
            verifyCalendarSemanticsScrollToIndex(list)
            return CalendarGesturePath.ROOT_INTERIOR
        }
        verifyCalendarSemanticsScrollToIndex(list)
        throw AssertionError(
            "Calendar list moved through Semantics ScrollToIndex but none of the three real Touch Gesture paths moved it; " +
                "boundsInRoot=${node.boundsInRoot}, boundsInWindow=${node.boundsInWindow}, " +
                "root=${root.boundsInRoot}, visibleIntersection=[$visibleLeft,$visibleTop,$visibleRight,$visibleBottom], " +
                "position=${node.position}, viewport=${node.viewport}, loadedEvents=${node.loadedEventCount}",
        )
    }

    private fun verifyCalendarSemanticsScrollToIndex(list: CalendarSemanticsTarget) {
        val before = snapshotSemantics(list)
        val viewportBefore = checkNotNull(before.viewport)
        val visibleBefore = viewportBefore.visibleItemIndices
        assertTrue("Calendar list must expose at least one visible item", visibleBefore.isNotEmpty())
        val firstBefore = visibleBefore.minOrNull()!!
        val lastBefore = visibleBefore.maxOrNull()!!
        val targetIndex = if (viewportBefore.canScrollForward && lastBefore + 1 < viewportBefore.totalItems) {
            lastBefore + 1
        } else {
            (firstBefore - 1).coerceAtLeast(0)
        }
        assertTrue("Calendar Semantics probe must select a different item", targetIndex !in visibleBefore)
        list.interaction.performSemanticsAction(SemanticsActions.ScrollToIndex) { scrollToIndex ->
            scrollToIndex(targetIndex)
        }
        compose.waitForIdle()
        val after = snapshotSemantics(list)
        val visibleAfter = checkNotNull(after.viewport?.visibleItemIndices)
        Log.i(
            CALENDAR_STAGE_TAG,
            "SEMANTICS_SCROLL_TO_INDEX|target=$targetIndex|" +
                "firstVisible=$firstBefore->${visibleAfter.minOrNull()}|" +
                "visibleIndices=$visibleBefore->$visibleAfter|" +
                "viewport=${after.viewport}",
        )
        assertTrue(
            "Semantics ScrollToIndex must make the requested Calendar item visible",
            targetIndex in visibleAfter && visibleAfter != visibleBefore,
        )
        list.interaction.performSemanticsAction(SemanticsActions.ScrollToIndex) { scrollToIndex ->
            scrollToIndex(firstBefore)
        }
        compose.waitForIdle()
        val restored = snapshotSemantics(list)
        val restoredVisible = checkNotNull(restored.viewport?.visibleItemIndices)
        Log.i(
            CALENDAR_STAGE_TAG,
            "SEMANTICS_SCROLL_TO_INDEX_RESTORE|target=$firstBefore|" +
                "visibleIndices=$visibleBefore->$restoredVisible",
        )
        assertTrue(
            "Semantics ScrollToIndex probe must restore the starting item to visibility",
            firstBefore in restoredVisible,
        )
    }

    private fun performCalendarGesture(
        list: CalendarSemanticsTarget,
        path: CalendarGesturePath,
        height: Float,
        label: String,
    ): Boolean {
        val before = snapshotSemantics(list)
        val beforeValue = checkNotNull(before.scrollValue)
        val beforeMax = checkNotNull(before.scrollMax)
        val positionBefore = before.position
        val visibleIndicesBefore = before.viewport?.visibleItemIndices
        val startY = if (path == CalendarGesturePath.NODE_EDGE) height - 1f else height * 0.8f
        val endY = if (path == CalendarGesturePath.NODE_EDGE) 1f else height * 0.2f
        val startRootY = before.boundsInRoot.top + startY
        val endRootY = before.boundsInRoot.top + endY
        val startWindowY = before.boundsInWindow.top + startY
        val endWindowY = before.boundsInWindow.top + endY
        val root = snapshotSemantics(calendarRootNode())
        Log.i(
            CALENDAR_STAGE_TAG,
            "GESTURE_TARGET|$label|path=$path|local=${before.boundsInRoot.width / 2f},$startY->${before.boundsInRoot.width / 2f},$endY|" +
                "root=${before.boundsInRoot.centerX},$startRootY->${before.boundsInRoot.centerX},$endRootY|" +
                "rootViewportCenterX=${root.boundsInRoot.centerX}|" +
                "window=${before.boundsInWindow.centerX},$startWindowY->${before.boundsInWindow.centerX},$endWindowY|" +
                "node=${before.boundsInRoot}|windowBounds=${before.boundsInWindow}|" +
                "scroll=$beforeValue/$beforeMax|" +
                "firstVisible=$positionBefore|viewport=${before.viewport}|loadedEvents=${before.loadedEventCount}",
        )
        val touchBounds = if (path == CalendarGesturePath.ROOT_INTERIOR) root.boundsInWindow else before.boundsInWindow
        val touchStartY = if (path == CalendarGesturePath.ROOT_INTERIOR) startRootY +
            (root.boundsInWindow.top - root.boundsInRoot.top) else before.boundsInWindow.top + startY
        val touchEndY = if (path == CalendarGesturePath.ROOT_INTERIOR) endRootY +
            (root.boundsInWindow.top - root.boundsInRoot.top) else before.boundsInWindow.top + endY
        val device = UiDevice.getInstance(InstrumentationRegistry.getInstrumentation())
        val touchAccepted = device.swipe(
            touchBounds.centerX.roundToInt(), touchStartY.roundToInt(),
            touchBounds.centerX.roundToInt(), touchEndY.roundToInt(), 24,
        )
        assertTrue("UiAutomator did not dispatch the requested Calendar touchscreen swipe.", touchAccepted)
        compose.waitForIdle()
        val after = snapshotSemantics(list)
        val afterValue = checkNotNull(after.scrollValue)
        val afterMax = checkNotNull(after.scrollMax)
        val afterPosition = after.position
        val visibleIndicesAfter = after.viewport?.visibleItemIndices
        val lazyListPositionAdvanced = when {
            positionBefore != null && afterPosition != null -> afterPosition != positionBefore
            visibleIndicesBefore != null && visibleIndicesAfter != null -> visibleIndicesAfter != visibleIndicesBefore
            else -> false
        }
        val advanced = afterValue > beforeValue + 0.5f && lazyListPositionAdvanced
        Log.i(
            CALENDAR_STAGE_TAG,
            "GESTURE_RESULT|$label|path=$path|from=$beforeValue|to=$afterValue|" +
                "max=$afterMax|advanced=$advanced|" +
                "firstVisible=$positionBefore->$afterPosition|" +
                "visibleIndices=$visibleIndicesBefore->$visibleIndicesAfter|" +
                "viewport=${after.viewport}|loadedEvents=${after.loadedEventCount}",
        )
        return advanced
    }

    private fun shell(command: String): String {
        val descriptor = InstrumentationRegistry.getInstrumentation().uiAutomation.executeShellCommand(command)
        return ParcelFileDescriptor.AutoCloseInputStream(descriptor).bufferedReader().use { it.readText() }.trim()
    }

    @Test fun chatIsInitialDestinationAndAllRoutesAreAccessible() {
        compose.onNodeWithText("Serea").assertIsDisplayed()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
        compose.onAllNodesWithTag("destination_chat").assertCountEquals(1)
        DestinationContract.all.forEach { contract ->
            compose.onAllNodesWithTag("destination_${contract.destination.name.lowercase()}").assertCountEquals(1)
        }
    }

    @Test fun chatTasksBackReturnsToTheSameChatViewModel() {
        val viewModel = compose.activity.chatViewModel
        compose.onNodeWithTag("chat-composer").performTextInput("unsent task transition draft")
        compose.onNodeWithTag("open_tasks").performClick()
        val taskScreenDisplayed = runCatching {
            compose.waitUntil(10_000) {
                !compose.activity.shellTaskViewModel.state.value.loading &&
                    runCatching {
                        compose.onNodeWithTag("tasks_heading").assertIsDisplayed()
                        compose.onNodeWithTag("task_add").assertIsDisplayed()
                    }.isSuccess
            }
        }.isSuccess
        if (!taskScreenDisplayed) {
            Log.i(
                TASKS_DESTINATION_STAGE_TAG,
                "TASKS_DESTINATION|state=${compose.activity.shellTaskViewModel.state.value} " +
                    "route=${compose.activity.toString()}",
            )
            compose.onRoot(useUnmergedTree = true).printToLog("chat-tasks-destination")
        }
        assertTrue("Tasks destination heading and add action must both become displayed", taskScreenDisplayed)
        compose.onNodeWithTag("task_add").assertIsDisplayed()
        pressBack()
        val chatScreenRestored = runCatching {
            compose.waitUntil(5_000) {
                runCatching { compose.onNodeWithTag("chat-composer").assertIsDisplayed() }.isSuccess
            }
        }.isSuccess
        if (!chatScreenRestored) {
            val chatSelected = runCatching {
                compose.onNodeWithTag("destination_chat").assertIsSelected()
                true
            }.getOrDefault(false)
            val tasksSelected = runCatching {
                compose.onNodeWithTag("destination_tasks").assertIsSelected()
                true
            }.getOrDefault(false)
            Log.i(
                TASKS_DESTINATION_STAGE_TAG,
                "CHAT_BACK_TIMEOUT|chatSelected=$chatSelected tasksSelected=$tasksSelected " +
                    "chatInputLength=${compose.activity.chatViewModel.state.value.input.length} " +
                    "taskLoading=${compose.activity.shellTaskViewModel.state.value.loading}",
            )
            compose.onRoot(useUnmergedTree = true).printToLog("chat-tasks-back")
        }
        assertTrue("Chat composer must become displayed after returning from Tasks", chatScreenRestored)
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
        assertEquals("unsent task transition draft", compose.activity.chatViewModel.state.value.input)
        assertTrue(viewModel === compose.activity.chatViewModel)
        compose.activityRule.scenario.recreate()
        compose.onNodeWithTag("chat-composer").assertTextContains("unsent task transition draft")
        assertEquals("unsent task transition draft", compose.activity.chatViewModel.state.value.input)
        assertTrue(viewModel === compose.activity.chatViewModel)
    }

    @Test fun chatGmailBackShowsOnlyFictionalMailboxData() {
        assertEquals(GmailProviderKind.FAKE, compose.activity.shellProviders.gmail.kind)
        select(SereaDestination.GMAIL)
        compose.onNodeWithText("DEMO / 架空のメール — 実アカウントには接続していません").assertIsDisplayed()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Project Finch schedule").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Project Finch schedule").assertIsDisplayed()
        compose.onNodeWithText("Project Finch schedule").performClick()
        compose.onNodeWithText("The made-up Finch review starts at noon.").assertIsDisplayed()
        pressBack() // Gmail detail -> Gmail list.
        compose.onNodeWithTag("gmail_list_heading").assertTextContains("受信箱").assertIsDisplayed()
        pressBack() // Gmail destination -> Chat.
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun gmailSearchAndMessageDetailUseTheFakeProviderFixtures() {
        compose.waitUntil(10_000) { !compose.activity.chatViewModel.state.value.isRestoring }
        val chatConversationBefore = compose.activity.chatViewModel.state.value.conversation
        select(SereaDestination.GMAIL)
        compose.waitUntil(10_000) {
            runCatching { compose.onNodeWithTag("gmail_result_count").assertTextContains("7 件") }.isSuccess
        }
        compose.onNodeWithText("7 件").assertIsDisplayed()
        compose.onNodeWithTag("gmail_search_input").performTextInput("Garden plan")
        compose.onNodeWithTag("gmail_search_input").assertTextContains("Garden plan")
        compose.onNodeWithTag("gmail_search_input").performImeAction()
        val firstSearchLoaded = runCatching { compose.waitUntil(10_000) {
            runCatching {
                compose.onNodeWithTag("gmail_result_count").assertTextEquals("1 件")
                compose.onNodeWithTag("gmail_search_input").assertIsNotFocused()
            }.isSuccess
        } }.isSuccess
        if (!firstSearchLoaded) compose.onRoot(useUnmergedTree = true).printToLog("gmail-search-result-timeout")
        assertTrue("Gmail IME search must load its single fictional Garden plan message and clear field focus.", firstSearchLoaded)
        compose.onNodeWithTag("gmail_message_subject").assertTextContains("Garden plan")
        compose.activityRule.scenario.recreate()
        compose.onNodeWithTag("gmail_search_input").assertTextContains("Garden plan").assertIsDisplayed()
        compose.onNodeWithText("検索を取消").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("受信箱").fetchSemanticsNodes().isNotEmpty() &&
                runCatching { compose.onNodeWithTag("gmail_result_count").assertTextEquals("7 件") }.isSuccess
        }
        compose.onNodeWithTag("gmail_result_count").assertTextEquals("7 件").assertIsDisplayed()
        compose.onNodeWithTag("gmail_list_heading").assertTextEquals("受信箱")
        compose.onAllNodesWithTag("gmail_message_subject").onFirst().assertTextContains("Project Finch schedule")
        compose.onNodeWithTag("gmail_search_input").performTextInput("Garden plan")
        compose.onNodeWithTag("gmail_search_input").assertTextContains("Garden plan")
        compose.onNodeWithTag("gmail_search_submit").assertIsDisplayed().assertIsEnabled()
        compose.onNodeWithTag("gmail_search_input").performImeAction()
        val repeatedSearchLoaded = runCatching { compose.waitUntil(10_000) {
            runCatching { compose.onNodeWithTag("gmail_result_count").assertTextEquals("1 件") }.isSuccess
        } }.isSuccess
        if (!repeatedSearchLoaded) compose.onRoot(useUnmergedTree = true).printToLog("gmail-repeat-search-result-timeout")
        assertTrue("Gmail search after cancellation must settle to exactly one fictional Garden plan message.", repeatedSearchLoaded)
        compose.onNodeWithTag("gmail_message_subject").assertTextContains("Garden plan")
        compose.onNodeWithTag("gmail_search_input").assertIsNotFocused()
        compose.onNodeWithTag("gmail_message_subject").performClick()
        compose.onNodeWithText("The imaginary basil bed is ready.").assertIsDisplayed()
        assertEquals(chatConversationBefore, compose.activity.chatViewModel.state.value.conversation)
    }

    @Test fun chatCalendarBackShowsFictionalAgendaAndDetail() {
        assertEquals(CalendarProviderKind.FAKE, compose.activity.shellProviders.calendar.kind)
        select(SereaDestination.CALENDAR)
        compose.onNodeWithTag("calendar_destination").assertIsDisplayed()
        compose.onNodeWithText("DEMO / 架空の予定").assertIsDisplayed()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("表示するカレンダー").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Demo Global").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Date-line test").fetchSemanticsNodes().isNotEmpty()
        }
        scrollAgendaUntilDisplayed("calendar_agenda_list", "Date-line test")
        compose.onNodeWithText("Date-line test").performClick()
        calendarTargetTextNode("Date-line test").assertIsDisplayed()
        pressBack() // Event detail -> agenda.
        compose.onNodeWithText("表示するカレンダー").assertIsDisplayed()
        pressBack() // Calendar destination -> Chat.
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun chatSettingsBackKeepsCredentialScreenshotProtectionScoped() {
        compose.onNodeWithTag("open_settings").performClick()
        compose.onNodeWithText("Provider候補: Ollama Cloud").performScrollTo().assertIsDisplayed()
        compose.waitUntil(5_000) {
            compose.activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE != 0
        }
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
        compose.waitUntil(5_000) {
            compose.activity.window.attributes.flags and WindowManager.LayoutParams.FLAG_SECURE == 0
        }
    }

    @Test fun taskProposalDemoIsFakeOnlyUnapprovedAndCanClose() {
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        compose.onNodeWithText("DEMO / 架空のタスク").assertIsDisplayed()
        compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_approve").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_close").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("chat-composer").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun taskProposalApprovalBlocksBackAndNavigationUntilResultIsKnown() {
        val repository = DemoTaskProposalRepository()
        val mutationStarted = CompletableDeferred<Unit>()
        val allowMutation = CompletableDeferred<Unit>()
        repository.beforeMutation = {
            mutationStarted.complete(Unit)
            allowMutation.await()
        }
        compose.activityRule.scenario.onActivity { activity ->
            activity.setContent {
                SereaTheme {
                    SereaAppShell(
                        chatViewModel = activity.chatViewModel,
                        credentials = activity.shellCredentials,
                        usage = activity.shellUsage,
                        taskViewModel = activity.shellTaskViewModel,
                        taskProposalDemoContent = { modifier, onClose, onPendingChanged ->
                            TaskProposalDemoScreenForTest(repository, modifier, onClose, onPendingChanged)
                        },
                    )
                }
            }
        }
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        compose.onNodeWithTag("task_proposal_approve").performScrollTo().performClick()
        compose.waitUntil(5_000) { mutationStarted.isCompleted }
        compose.waitUntil(5_000) {
            runCatching { compose.onNodeWithTag("destination_chat").assertIsNotEnabled() }.isSuccess
        }

        pressBack()
        compose.onNodeWithTag("task_proposal_demo").assertIsDisplayed()
        compose.waitUntil(5_000) {
            runCatching { compose.onNodeWithTag("task_proposal_status").assertTextContains("処理しています", substring = true) }.isSuccess
        }
        compose.onNodeWithTag("task_proposal_status").assertTextContains("処理しています", substring = true)
        compose.onNodeWithTag("destination_chat").assertIsNotEnabled()
        assertEquals(0, repository.writeCount())

        allowMutation.complete(Unit)
        compose.waitUntil(5_000) { repository.writeCount() == 1 }
        compose.waitUntil(5_000) {
            runCatching {
                compose.onNodeWithTag("task_proposal_status")
                    .assertTextEquals("承認済みです。変更を適用しました。")
            }.isSuccess
        }
        compose.waitUntil(5_000) {
            runCatching { compose.onNodeWithTag("destination_chat").assertIsEnabled() }.isSuccess
        }
        compose.onNodeWithTag("task_proposal_status").assertTextEquals("承認済みです。変更を適用しました。")
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun taskProposalDemoDoesNotWriteUntilExplicitApproval() {
        compose.waitUntil(10_000) { !compose.activity.shellTaskViewModel.state.value.loading }
        val tasksBefore = compose.activity.shellTaskViewModel.state.value.tasks.toList()
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("明示的に承認するまで、タスクは変更されません。").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_close").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithTag("chat-composer").fetchSemanticsNodes().isNotEmpty()
        }
        compose.activityRule.scenario.onActivity { it.shellTaskViewModel.refresh() }
        compose.waitUntil(10_000) { !compose.activity.shellTaskViewModel.state.value.loading }
        assertEquals(null, compose.activity.shellTaskViewModel.state.value.error)
        assertEquals(tasksBefore, compose.activity.shellTaskViewModel.state.value.tasks)
    }

    @Test fun backStackReturnsThroughPriorDestinations() {
        select(SereaDestination.GMAIL)
        select(SereaDestination.CALENDAR)
        compose.onNodeWithText("DEMO / 架空の予定").assertIsDisplayed()
        pressBack()
        compose.onNodeWithTag("gmail_list_heading").assertTextContains("受信箱").assertIsDisplayed()
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @Test fun activityRecreationRestoresRouteAndBackStackButNoProposalSession() {
        select(SereaDestination.GMAIL)
        select(SereaDestination.SETTINGS)
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("Provider候補: Ollama Cloud").performScrollTo().assertIsDisplayed()
        pressBack()
        compose.onNodeWithText("DEMO / 架空のメール — 実アカウントには接続していません").assertIsDisplayed()
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
    }

    @OptIn(ExperimentalTestApi::class)
    @Test fun chatDraftSurvivesGmailAndCalendarDestinationChanges() {
        val composer = compose.onNodeWithTag("chat-composer")
        assertTrue(SemanticsActions.SetText in composer.fetchSemanticsNode().config)
        val previousIme = shell("settings get secure default_input_method")
        assertTrue("Could not read the disposable emulator's current IME setting.", previousIme.isNotBlank())
        val restoreIme = if (previousIme == TEST_IME_ID) "com.google.android.inputmethod.latin/.LatinIME" else previousIme
        try {
            assertTrue("Test IME APK was not installed by the Required Gate harness.",
                shell("pm path app.serea.android.r25testime").contains("package:"))
            val startsBefore = imeSnapshot().getStringArrayList("events").orEmpty()
                .count { it.contains(" onStartInput restarting=") }
            shell("ime enable $TEST_IME_ID")
            shell("ime set $TEST_IME_ID")
            assertEquals("Android did not select the isolated Test IME.", TEST_IME_ID,
                shell("settings get secure default_input_method"))
            ensureChatWindowFocus("initial real IME session")
            tapComposerThroughSystemInput()
            awaitComposerWindowFocus("initial real IME session")
            composer.assertIsFocused()
            var session = awaitTestImeSession(startsBefore)
            assertTrue("Android IME callback did not identify the active Composer editor.",
                imeSnapshot().getStringArrayList("events").orEmpty().any {
                    it.contains(" onStartInput restarting=") && it.contains("package=$ISOLATED_PACKAGE")
                })
            awaitComposerImeState("R4-initial-state", null) {
                compose.activity.chatViewModel.state.value.editorValue.let {
                    it.text.isEmpty() && it.composition == null && it.selection == TextRange.Zero
                }
            }

            // B: consecutive operations use only Android's currently bound InputConnection.
            logComposerImeDiagnostics("R4-B-initial", null)
            imeOperation("setComposingText", session, COMPOSER_DRAFT)
            logComposerImeDiagnostics("R4-B-after-draft-call", null)
            imeOperation("setComposingText", session, COMPOSER_REPLACEMENT)
            logComposerImeDiagnostics("R4-B-after-replace-call", null)
            imeOperation("finishComposingText", session)
            logComposerImeDiagnostics("R4-B-after-finish-call", null)
            awaitComposerImeState("R4-B-committed", null) {
                compose.activity.chatViewModel.state.value.editorValue.let { value ->
                    value.text == COMPOSER_REPLACEMENT && value.composition == null &&
                        value.selection == TextRange(COMPOSER_REPLACEMENT.length)
                }
            }
            compose.waitUntil(5_000) {
                composer.fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange] ==
                    TextRange(COMPOSER_REPLACEMENT.length)
            }

            // C: reset to a verified empty state, then put Compose/ViewModel observation boundaries
            // between separate operations sent through new snapshots of the same live IME session.
            composer.performTextClearance()
            awaitComposerImeState("R4-C-reset", null) {
                compose.activity.chatViewModel.state.value.editorValue.let {
                    it.text.isEmpty() && it.composition == null && it.selection == TextRange.Zero
                }
            }
            val startsAfterReset = imeSnapshot().getStringArrayList("events").orEmpty()
                .count { it.contains(" onStartInput restarting=") }
            shell("ime set $restoreIme")
            shell("ime set $TEST_IME_ID")
            assertEquals(TEST_IME_ID, shell("settings get secure default_input_method"))
            // Committing B and clearing the editor can leave the Activity resumed while Android
            // has removed Window focus. Recover that state, then verify the fresh session created
            // by switching IMEs. The Composer already has a real input session here; another
            // injected screen tap is redundant and can fail even while that session is live.
            ensureChatWindowFocus("synchronized real IME session")
            awaitComposerWindowFocus("synchronized real IME session")
            composer.assertIsFocused()
            // Window focus can cause Android to replace the IME InputConnection after an
            // onStartInput callback. Capture the session only after this focus boundary so the
            // operation cannot accidentally target the just-finished connection.
            session = awaitTestImeSession(startsAfterReset)
            imeOperation("setComposingText", session, COMPOSER_DRAFT)
            awaitComposerImeState("R4-C-draft-composing", null) {
                compose.activity.chatViewModel.state.value.editorValue.let {
                    it.text == COMPOSER_DRAFT && it.composition != null && it.selection == TextRange(COMPOSER_DRAFT.length)
                }
            }
            assertEquals(TextRange(COMPOSER_DRAFT.length),
                composer.fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange])
            imeOperation("setComposingText", session, COMPOSER_REPLACEMENT)
            awaitComposerImeState("R4-C-replacement-composing", null) {
                compose.activity.chatViewModel.state.value.editorValue.let {
                    it.text == COMPOSER_REPLACEMENT && it.composition != null &&
                        it.selection == TextRange(COMPOSER_REPLACEMENT.length)
                }
            }
            imeOperation("finishComposingText", session)
            awaitComposerImeState("R4-C-committed", null) {
                compose.activity.chatViewModel.state.value.editorValue.let {
                    it.text == COMPOSER_REPLACEMENT && it.composition == null &&
                        it.selection == TextRange(COMPOSER_REPLACEMENT.length)
                }
            }
            imeOperation("setSelection", session, selectionStart = 5, selectionEnd = 5)
            awaitComposerImeState("R4-selection-5", null) {
                compose.activity.chatViewModel.state.value.editorValue.selection == TextRange(5, 5) &&
                    composer.fetchSemanticsNode().config[SemanticsProperties.TextSelectionRange] == TextRange(5, 5)
            }

            select(SereaDestination.GMAIL)
            compose.onNodeWithTag("gmail_search_input").performTextInput("back clears focus")
            compose.onNodeWithTag("gmail_search_input").performImeAction()
            val emptySearchSettled = runCatching { compose.waitUntil(10_000) {
                runCatching {
                    compose.onNodeWithTag("gmail_result_count").assertTextEquals("0 件")
                    compose.onNodeWithTag("gmail_search_input").assertIsNotFocused()
                }.isSuccess
            } }.isSuccess
            assertTrue("Gmail IME search must settle before navigation and release TextField focus.", emptySearchSettled)
            pressBack()
            compose.onNodeWithTag("gmail_list_heading").assertTextContains("受信箱").assertIsDisplayed()
            pressBack()
            assertEquals(COMPOSER_REPLACEMENT, compose.activity.chatViewModel.state.value.input)
            assertEquals(TextRange(5, 5), compose.activity.chatViewModel.state.value.editorValue.selection)
            val startsBeforeGmailReturn = imeSnapshot().getStringArrayList("events").orEmpty()
                .count { it.contains(" onStartInput restarting=") }
            composer.performSemanticsAction(SemanticsActions.RequestFocus) { it() }
            composer.assertIsFocused()
            session = awaitTestImeSession(startsBeforeGmailReturn)
            imeOperation("commitText", session, "!")
            awaitComposerImeState("R4-gmail-return-insertion", null) {
                compose.activity.chatViewModel.state.value.editorValue.let {
                    it.text == "draft! survives navigation" && it.selection == TextRange(6, 6)
                }
            }

            select(SereaDestination.CALENDAR)
            compose.onNodeWithTag("calendar_destination").assertIsDisplayed()
            compose.waitUntil(10_000) { compose.onAllNodesWithText("DEMO / 架空の予定").fetchSemanticsNodes().isNotEmpty() }
            pressBack()
            composer.assertIsDisplayed()
            assertEquals(TextRange(6, 6), compose.activity.chatViewModel.state.value.editorValue.selection)
            val startsBeforeCalendarReturn = imeSnapshot().getStringArrayList("events").orEmpty()
                .count { it.contains(" onStartInput restarting=") }
            composer.performSemanticsAction(SemanticsActions.RequestFocus) { it() }
            composer.assertIsFocused()
            session = awaitTestImeSession(startsBeforeCalendarReturn)
            imeOperation("commitText", session, "?")
            awaitComposerImeState("R4-calendar-return-insertion", null) {
                compose.activity.chatViewModel.state.value.editorValue.let {
                    it.text == "draft!? survives navigation" && it.selection == TextRange(7, 7)
                }
            }

            imeOperation("setSelection", session, selectionStart = 8, selectionEnd = 8)
            awaitComposerImeState("R4-pre-recreation-selection", null) {
                compose.activity.chatViewModel.state.value.editorValue.selection == TextRange(8, 8)
            }
            compose.activityRule.scenario.recreate()
            compose.waitUntil(10_000) {
                compose.activity.chatViewModel.state.value.input == "draft!? survives navigation" &&
                    runCatching { compose.onNodeWithTag("chat-composer").assertIsDisplayed() }.isSuccess
            }
            val recreatedComposer = compose.onNodeWithTag("chat-composer")
            recreatedComposer.assertIsNotFocused()
            assertEquals("Activity recreation must preserve the composer selection.", TextRange(8, 8),
                compose.activity.chatViewModel.state.value.editorValue.selection)
            val startsBeforeRecreatedFocus = imeSnapshot().getStringArrayList("events").orEmpty()
                .count { it.contains(" onStartInput restarting=") }
            recreatedComposer.performSemanticsAction(SemanticsActions.RequestFocus) { requestFocus -> requestFocus() }
            recreatedComposer.assertIsFocused()
            session = awaitTestImeSession(startsBeforeRecreatedFocus)
            imeOperation("commitText", session, "!")
            awaitComposerImeState("R4-recreation-insertion", null) {
                compose.activity.chatViewModel.state.value.editorValue.let {
                    it.text == "draft!? !survives navigation" && it.selection == TextRange(9, 9)
                }
            }
        } finally {
            shell("ime set $restoreIme")
            shell("ime disable $TEST_IME_ID")
            assertEquals("The Required IME test did not restore the original emulator IME.", restoreIme,
                shell("settings get secure default_input_method"))
            Log.i(COMPOSER_IME_DIAGNOSTICS_TAG,
                "REAL_IME_CLEANUP uptimeNanos=${SystemClock.elapsedRealtimeNanos()} restored=$restoreIme disabled=$TEST_IME_ID")
        }
    }

    @Test fun navigationDestinationsRemainSemanticallyAvailable() {
        val navigation = compose.onNodeWithTag("serea_navigation_bar")
        navigation.assertIsDisplayed()
        DestinationContract.all.forEach { contract ->
            compose.onNodeWithTag("destination_${contract.destination.name.lowercase()}")
                .assertIsDisplayed()
                .assertHasClickAction()
        }
        compose.onNodeWithTag("destination_chat").assertIsSelected()
        DestinationContract.all.drop(1).forEach { contract ->
            select(contract.destination)
            compose.onNodeWithTag("serea_navigation_bar").assertIsDisplayed()
            compose.onNodeWithTag("destination_${contract.destination.name.lowercase()}")
                .assertIsDisplayed()
                .assertIsSelected()
        }
    }

    @Test fun screenHeadingsAreExposedToScreenReaders() {
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithText("Serea").fetchSemanticsNode().config)

        select(SereaDestination.TASKS)
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithTag("tasks_heading").fetchSemanticsNode().config)
        select(SereaDestination.GMAIL)
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithTag("gmail_list_heading").fetchSemanticsNode().config)
        select(SereaDestination.CALENDAR)
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithText("DEMO / 架空の予定", substring = true).fetchSemanticsNode().config)
        select(SereaDestination.SETTINGS)
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithTag("destination_title").fetchSemanticsNode().config)
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        assertTrue(SemanticsProperties.Heading in compose.onNodeWithText("DEMO / 架空のタスク").fetchSemanticsNode().config)
    }

    @Test fun calendarDetailAndSelectedCalendarSurviveAnotherDestination() {
        calendarStage("orientation-landscape", "START")
        compose.activityRule.scenario.onActivity { activity ->
            activity.requestedOrientation = ActivityInfo.SCREEN_ORIENTATION_LANDSCAPE
        }
        compose.waitUntil(10_000) {
            runCatching {
                val size = compose.onRoot().fetchSemanticsNode().size
                size.width > size.height
            }.getOrDefault(false)
        }
        calendarStage("orientation-landscape", "FINISH")

        calendarStage("calendar-destination-semantics", "START")
        select(SereaDestination.CALENDAR)
        compose.onNodeWithTag("calendar_destination").assertIsDisplayed()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("表示するカレンダー").fetchSemanticsNodes().isNotEmpty()
        }
        calendarStage("calendar-destination-semantics", "FINISH")
        calendarStage("agenda-semantics", "START")
        compose.onNodeWithText("Demo Work").performClick()
        compose.waitUntil(10_000) {
            runCatching {
                compose.onNodeWithText("Demo Work").assertIsSelected()
                compose.onNodeWithText("今後").assertIsEnabled()
            }.isSuccess
        }
        val afterCalendarSelection = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        val calendarSelectionPosition = checkNotNull(afterCalendarSelection.position)
        Log.i(
            CALENDAR_STAGE_TAG,
            "AFTER_CALENDAR_SELECTION|scroll=${afterCalendarSelection.scrollValue}|" +
                "firstVisible=$calendarSelectionPosition",
        )
        assertEquals("Calendar selection must reset the first visible list position", 0 to 0, calendarSelectionPosition)
        compose.onNodeWithText("今後").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Demo Work").assertIsSelected()
        compose.onNodeWithText("今後").assertIsSelected()
        compose.waitUntil(10_000) {
            runCatching {
                val snapshot = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
                (snapshot.scrollMax ?: 0f) > 0f && (snapshot.scrollValue ?: Float.MAX_VALUE) <= 1f
            }.getOrDefault(false)
        }
        val afterUpcomingSelection = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        val upcomingSelectionPosition = checkNotNull(afterUpcomingSelection.position)
        Log.i(
            CALENDAR_STAGE_TAG,
            "AFTER_UPCOMING_SELECTION|scroll=${afterUpcomingSelection.scrollValue}|" +
                "max=${afterUpcomingSelection.scrollMax}|firstVisible=$upcomingSelectionPosition|" +
                "viewport=${afterUpcomingSelection.viewport}|loadedEvents=${afterUpcomingSelection.loadedEventCount}",
        )
        assertEquals("Upcoming selection must reset the first visible list position", 0 to 0, upcomingSelectionPosition)
        calendarStage("agenda-semantics", "FINISH")
        calendarStage("scroll-event-50", "START")
        scrollAgendaUntilDisplayed("calendar_agenda_list", "Fictional paged event 50")
        calendarTargetTextNode("Fictional paged event 50").assertIsDisplayed()
        calendarStage("scroll-event-50", "FINISH")
        val agendaBeforeDetail = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        val agendaOffsetBeforeDetailValue = checkNotNull(agendaBeforeDetail.scrollValue)
        val agendaPositionBeforeDetail = checkNotNull(agendaBeforeDetail.position)
        assertTrue("Calendar agenda must have several scroll positions", checkNotNull(agendaBeforeDetail.scrollMax) > 0f)
        assertTrue("The long agenda test must begin below its first event", agendaOffsetBeforeDetailValue > 0f)
        compose.onNodeWithText("Fictional paged event 50").performClick()
        calendarTargetTextNode("Fictional paged event 50").assertIsDisplayed()
        calendarStage("event-detail-back", "START")
        pressBack()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
        }
        val agendaAfterDetail = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        assertEquals(agendaOffsetBeforeDetailValue, checkNotNull(agendaAfterDetail.scrollValue), 1f)
        assertEquals(agendaPositionBeforeDetail, agendaAfterDetail.position)
        calendarStage("event-detail-back", "FINISH")

        calendarStage("destination-back", "START")
        select(SereaDestination.GMAIL)
        pressBack()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
        }
        val agendaAfterDestination = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        assertEquals(agendaOffsetBeforeDetailValue, checkNotNull(agendaAfterDestination.scrollValue), 1f)
        assertEquals(agendaPositionBeforeDetail, agendaAfterDestination.position)
        calendarStage("destination-back", "FINISH")

        calendarStage("activity-recreation", "START")
        compose.activityRule.scenario.recreate()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
        }
        val agendaAfterRecreation = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        assertEquals(agendaOffsetBeforeDetailValue, checkNotNull(agendaAfterRecreation.scrollValue), 1f)
        assertEquals(agendaPositionBeforeDetail, agendaAfterRecreation.position)
        calendarStage("activity-recreation", "FINISH")

        // Both query and calendar changes are checked against long lists, avoiding a false pass
        // caused by a short list clamping a stale scroll offset to zero.
        calendarStage("date-query-and-ime", "START")
        compose.onNodeWithText("指定日").performClick()
        compose.onNodeWithTag("calendar_date_input").performTextClearance()
        val displayZone = ZoneId.systemDefault()
        val firstBulkDate = Instant.parse("2026-10-22T16:00:00Z").atZone(displayZone).toLocalDate().toString()
        val secondBulkDate = Instant.parse("2026-10-23T16:00:00Z").atZone(displayZone).toLocalDate().toString()
        compose.onNodeWithTag("calendar_date_input").performTextInput(firstBulkDate)
        assertEquals(
            firstBulkDate,
            compose.onNodeWithTag("calendar_date_input").fetchSemanticsNode()
                .config[SemanticsProperties.EditableText].text,
        )
        Espresso.closeSoftKeyboard()
        compose.waitForIdle()
        // Drain platform input-method window and accessibility events before the following physical swipe.
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()).waitForIdle()
        compose.waitUntil(10_000) {
            runCatching {
                val snapshot = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
                (snapshot.scrollMax ?: 0f) > 0f && snapshot.scrollValue == 0f
            }.getOrDefault(false)
        }
        val agendaOffsetAfterQueryChange = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        val agendaPositionAfterQueryChange = checkNotNull(agendaOffsetAfterQueryChange.position)
        assertTrue("date query must retain a scrollable Work agenda", checkNotNull(agendaOffsetAfterQueryChange.scrollMax) > 0f)
        assertEquals(0f, checkNotNull(agendaOffsetAfterQueryChange.scrollValue), 1f)
        assertEquals("date query must reset the LazyListState index and offset", 0 to 0, agendaPositionAfterQueryChange)
        calendarStage("date-query-and-ime", "FINISH")

        calendarStage("date-change-scroll", "START")
        scrollAgendaUntilDisplayed("calendar_agenda_list", "Fictional paged event 40")
        calendarTargetTextNode("Fictional paged event 40").assertIsDisplayed()
        val workOffsetBeforeDateChange = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        assertTrue("date-change test must start below the first Work event", checkNotNull(workOffsetBeforeDateChange.scrollValue) > 0f)

        compose.onNodeWithTag("calendar_date_input").performTextClearance()
        compose.onNodeWithTag("calendar_date_input").performTextInput(secondBulkDate)
        assertEquals(
            secondBulkDate,
            compose.onNodeWithTag("calendar_date_input").fetchSemanticsNode()
                .config[SemanticsProperties.EditableText].text,
        )
        Espresso.closeSoftKeyboard()
        compose.waitForIdle()
        // Drain platform input-method window and accessibility events before the following physical swipe.
        UiDevice.getInstance(InstrumentationRegistry.getInstrumentation()).waitForIdle()
        compose.waitUntil(10_000) {
            runCatching {
                val snapshot = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
                (snapshot.scrollMax ?: 0f) > 0f && snapshot.scrollValue == 0f
            }.getOrDefault(false)
        }
        val agendaOffsetAfterDateChange = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        val agendaPositionAfterDateChange = checkNotNull(agendaOffsetAfterDateChange.position)
        assertTrue("changing the selected date must keep a scrollable Work agenda", checkNotNull(agendaOffsetAfterDateChange.scrollMax) > 0f)
        assertEquals(0f, checkNotNull(agendaOffsetAfterDateChange.scrollValue), 1f)
        assertEquals("date selection must reset the LazyListState index and offset", 0 to 0, agendaPositionAfterDateChange)
        calendarStage("date-change-scroll", "FINISH")

        calendarStage("calendar-change-scroll", "START")
        scrollAgendaUntilDisplayed("calendar_agenda_list", "Fictional paged event 64")
        calendarTargetTextNode("Fictional paged event 64").assertIsDisplayed()
        val workOffsetBeforeCalendarChange = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        assertTrue("calendar switch test must start below the first Work event", checkNotNull(workOffsetBeforeCalendarChange.scrollValue) > 0f)
        compose.onNodeWithText("今後").performClick()
        compose.onNodeWithText("Demo Global").performClick()
        compose.waitUntil(10_000) {
            runCatching {
                val snapshot = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
                (snapshot.scrollMax ?: 0f) > 0f && snapshot.scrollValue == 0f
            }.getOrDefault(false)
        }
        val agendaOffsetAfterCalendarChange = snapshotSemantics(calendarNodeWithTag("calendar_agenda_list"))
        val agendaPositionAfterCalendarChange = checkNotNull(agendaOffsetAfterCalendarChange.position)
        assertTrue("Demo Global must also provide a long agenda for reset verification", checkNotNull(agendaOffsetAfterCalendarChange.scrollMax) > 0f)
        assertEquals(0f, checkNotNull(agendaOffsetAfterCalendarChange.scrollValue), 1f)
        assertEquals("calendar selection must reset the LazyListState index and offset", 0 to 0, agendaPositionAfterCalendarChange)
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Date-line test").fetchSemanticsNodes().isNotEmpty()
        }
        scrollAgendaUntilDisplayed("calendar_agenda_list", "Date-line test")
        compose.onNodeWithText("Date-line test").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_event_detail_list").fetchSemanticsNodes().isNotEmpty()
        }
        scrollAgendaUntilDisplayed("calendar_event_detail_list", "説明")
        calendarTargetTextNode("説明").assertIsDisplayed()
        val detailBefore = snapshotSemantics(calendarNodeWithTag("calendar_event_detail_list"))
        val detailScrollBeforeValue = checkNotNull(detailBefore.scrollValue)
        assertTrue("calendar detail must be scrollable in this viewport", checkNotNull(detailBefore.scrollMax) > 0f)
        assertTrue("calendar detail must have a non-zero saved offset", detailScrollBeforeValue > 0f)
        calendarStage("calendar-change-scroll", "FINISH")

        calendarStage("detail-destination-back", "START")
        select(SereaDestination.GMAIL)
        compose.onNodeWithTag("gmail_list_heading").assertIsDisplayed()
        pressBack()

        compose.waitUntil(10_000) {
            compose.onAllNodesWithTag("calendar_event_detail_list").fetchSemanticsNodes().isNotEmpty()
        }
        val detailAfter = snapshotSemantics(calendarNodeWithTag("calendar_event_detail_list"))
        assertTrue(checkNotNull(detailAfter.scrollMax) > 0f)
        assertEquals(detailScrollBeforeValue, checkNotNull(detailAfter.scrollValue), 1f)
        scrollAgendaUntilDisplayed("calendar_event_detail_list", "Date-line test")
        calendarTargetTextNode("Date-line test").assertIsDisplayed()
        scrollAgendaUntilDisplayed("calendar_event_detail_list", "説明")
        calendarTargetTextNode("説明").assertIsDisplayed()
        scrollAgendaUntilDisplayed("calendar_event_detail_list", "開始・終了")
        calendarTargetTextNode("開始・終了").assertIsDisplayed()
        pressBack()
        compose.onNodeWithText("Demo Global").assertIsSelected()
        calendarStage("detail-destination-back", "FINISH")
    }

    @Test fun taskEditorDraftSurvivesActivityRecreation() {
        compose.waitUntil(10_000) { !compose.activity.shellTaskViewModel.state.value.loading }
        select(SereaDestination.TASKS)
        compose.onNodeWithTag("task_add").performClick()
        compose.onNodeWithTag("task_title").performTextInput("unsaved task draft")
        compose.activityRule.scenario.onActivity { activity ->
            val manager = activity.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                as android.view.inputmethod.InputMethodManager
            manager.hideSoftInputFromWindow(activity.window.decorView.windowToken, 0)
        }
        compose.waitForIdle()

        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("新しいタスク").assertIsDisplayed()
        compose.onNodeWithTag("task_title").assertTextContains("unsaved task draft")
    }

    @Test fun taskProposalRotationStartsANewUnapprovedFakeReview() {
        select(SereaDestination.TASK_PROPOSAL_DEMO)
        compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_approve").performScrollTo().performClick()
        compose.waitUntil(5_000) {
            compose.onAllNodesWithText("Fake Repository 書き込み回数: 1").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Fake Repository 書き込み回数: 1").performScrollTo().assertIsDisplayed()
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("DEMO / 架空のタスク").assertIsDisplayed()
        compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
        compose.onNodeWithText("明示的に承認するまで、タスクは変更されません。").performScrollTo().assertIsDisplayed()
        compose.onNodeWithTag("task_proposal_approve").performScrollTo().assertIsEnabled()
    }

    @Test fun gmailAndCalendarDetailsRestoreAcrossActivityRecreation() {
        select(SereaDestination.GMAIL)
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Project Finch schedule").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Project Finch schedule").performClick()
        compose.onNodeWithText("The made-up Finch review starts at noon.").assertIsDisplayed()
        compose.activityRule.scenario.recreate()
        compose.onNodeWithText("The made-up Finch review starts at noon.").assertIsDisplayed()
        pressBack()
        compose.onNodeWithTag("gmail_list_heading").assertTextContains("受信箱").assertIsDisplayed()
        pressBack()

        select(SereaDestination.CALENDAR)
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("表示するカレンダー").fetchSemanticsNodes().isNotEmpty()
        }
        compose.onNodeWithText("Demo Global").performClick()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("Date-line test").fetchSemanticsNodes().isNotEmpty()
        }
        scrollAgendaUntilDisplayed("calendar_agenda_list", "Date-line test")
        compose.onNodeWithText("Date-line test").performClick()
        compose.activityRule.scenario.recreate()
        calendarTargetTextNode("Date-line test").assertIsDisplayed()
    }

    @Test fun fontScaleTwoKeepsTaskProposalReviewAndFooterReachable() {
        fontScaleStage("start")
        val originalScale = compose.activity.resources.configuration.fontScale
        val originalWidthDp = compose.activity.resources.configuration.screenWidthDp
        val originalDensityDpi = compose.activity.resources.configuration.densityDpi
        val originalFontScaleSetting = shell("settings get system font_scale")
        check(originalFontScaleSetting == "null" || originalFontScaleSetting.toFloatOrNull() != null) {
            "Unable to read the original system font scale setting"
        }
        val originalSizeOverride = shell("wm size").lineSequence()
            .firstOrNull { it.startsWith("Override size:") }
            ?.substringAfter(':')?.trim()?.takeIf { it.isNotEmpty() }
        val originalDensityOverride = shell("wm density").lineSequence()
            .firstOrNull { it.startsWith("Override density:") }
            ?.substringAfter(':')?.trim()?.takeIf { it.isNotEmpty() }
        var lastActivityConfiguration = "not-observed"
        fun activityConfigurationMatches(expectedScale: Float, expectedWidth: Int, expectedDensityDpi: Int): Boolean {
            return try {
                var fontScale = 0f
                var width = 0
                var densityDpi = 0
                compose.activityRule.scenario.onActivity {
                    fontScale = it.resources.configuration.fontScale
                    width = it.resources.configuration.screenWidthDp
                    densityDpi = it.resources.configuration.densityDpi
                }
                lastActivityConfiguration = "fontScale=$fontScale,width=$width,densityDpi=$densityDpi"
                kotlin.math.abs(fontScale - expectedScale) < 0.02f &&
                    width == expectedWidth && densityDpi == expectedDensityDpi
            } catch (failure: NullPointerException) {
                // Android destroys and recreates the Activity for a system font/size change.
                // ActivityScenario can observe the short DESTROYED interval between those callbacks.
                if (failure.message == "Cannot run onActivity since Activity has been destroyed already") {
                    lastActivityConfiguration = "activity-destroyed-during-recreation"
                    false
                } else {
                    throw failure
                }
            }
        }
        var primaryFailure: Throwable? = null
        try {
            shell("wm size 320x640")
            shell("wm density 160")
            shell("settings put system font_scale 2.0")
            compose.waitUntil(15_000) {
                activityConfigurationMatches(expectedScale = 2f, expectedWidth = 320, expectedDensityDpi = 160)
            }
            fontScaleStage("font-scale-applied")
            DestinationContract.all.forEach { contract ->
                compose.onNodeWithTag("destination_${contract.destination.name.lowercase()}")
                    .assertIsDisplayed()
                    .assertTextContains(contract.title)
            }
            compose.onNodeWithTag("chat-composer").assertIsDisplayed()
            fontScaleStage("compact-composer-visible")
            compose.onNodeWithText("その他の操作").performClick()
            fontScaleStage("compact-overflow-open")
            compose.onNodeWithText("新しい会話").assertIsDisplayed()
            fontScaleStage("compact-new-conversation-visible")
            compose.onNodeWithText("履歴").assertIsDisplayed()
            fontScaleStage("compact-history-visible")
            compose.onNodeWithText("新しい会話").performClick()
            fontScaleStage("compact-new-conversation-selected")
            compose.waitUntil(10_000) {
                compose.onAllNodesWithText("新しい会話").fetchSemanticsNodes().isEmpty()
            }
            compose.onNodeWithTag("chat-composer").assertIsDisplayed()
            fontScaleStage("compact-overflow-dismissed")

            select(SereaDestination.TASKS)
            fontScaleStage("tasks-destination-selected")
            compose.waitUntil(10_000) { !compose.activity.shellTaskViewModel.state.value.loading }
            compose.onNodeWithTag("tasks_heading").assertIsDisplayed()
            compose.onNodeWithTag("task_add").assertIsDisplayed()

            select(SereaDestination.SETTINGS)
            fontScaleStage("settings-destination-selected")
            compose.onNodeWithTag("destination_title").assertTextContains("設定").assertIsDisplayed()
            compose.onNodeWithText("Provider候補: Ollama Cloud").performScrollTo().assertIsDisplayed()
            compose.onNodeWithText("DEMO · Cloud送信: 無効", substring = false).performScrollTo().assertIsDisplayed()
            pressBack()
            compose.onNodeWithTag("tasks_heading").assertIsDisplayed()
            pressBack()
            compose.onNodeWithTag("chat-composer").assertIsDisplayed()

            select(SereaDestination.GMAIL)
            fontScaleStage("gmail-destination-selected")
            val gmailHeadingDisplayed = runCatching { compose.waitUntil(10_000) {
                runCatching { compose.onNodeWithTag("gmail_list_heading").assertIsDisplayed() }.isSuccess
            } }.isSuccess
            if (!gmailHeadingDisplayed) compose.onRoot(useUnmergedTree = true).printToLog("font-scale-gmail-heading")
            assertTrue("The Gmail heading must remain displayed after returning through Tasks and Settings at font scale 2.0.", gmailHeadingDisplayed)
            compose.onNodeWithTag("gmail_list_heading").assertIsDisplayed()
            compose.onNodeWithTag("gmail_search_input").assertIsDisplayed()
            compose.waitUntil(10_000) {
                compose.onAllNodesWithText("7 件").fetchSemanticsNodes().isNotEmpty()
            }
            compose.onAllNodesWithTag("gmail_message_subject").onFirst().performScrollTo().assertIsDisplayed()
            fontScaleStage("gmail-message-visible")

            select(SereaDestination.CALENDAR)
            fontScaleStage("calendar-destination-selected")
            compose.onNodeWithText("DEMO / 架空の予定").assertIsDisplayed()
            compose.waitUntil(10_000) {
                compose.onAllNodesWithTag("calendar_agenda_list").fetchSemanticsNodes().isNotEmpty()
            }
            scrollAgendaUntilDisplayed("calendar_agenda_list", "Design review")
            fontScaleStage("calendar-design-review-unmerged-text-visible")
            calendarTargetTextNode("Design review").assertIsDisplayed()

            select(SereaDestination.TASK_PROPOSAL_DEMO)
            fontScaleStage("task-proposal-destination-selected")
            compose.onNodeWithText("DEMO / 架空のタスク").assertIsDisplayed()
            listOf("create", "update", "complete", "delete").forEach { operation ->
                fontScaleStage("check-operation-$operation")
                compose.onNodeWithTag("task_proposal_operation_$operation").performScrollTo().assertIsDisplayed()
                fontScaleStage("operation-$operation-visible")
            }
            compose.waitUntil(10_000) {
                compose.onAllNodesWithTag("task_proposal_approve").fetchSemanticsNodes().isNotEmpty()
            }
            compose.waitUntil(10_000) {
                runCatching { compose.onNodeWithTag("task_proposal_approve").performScrollTo().assertIsDisplayed() }.isSuccess
            }
            fontScaleStage("check-review-close")
            compose.onNodeWithTag("task_proposal_close").performScrollTo().assertIsDisplayed()
            fontScaleStage("review-close-visible")
            fontScaleStage("check-fake-repository-write-count")
            compose.onNodeWithText("Fake Repository 書き込み回数: 0").performScrollTo().assertIsDisplayed()
            fontScaleStage("fake-repository-write-count-visible")
            compose.onNodeWithTag("task_proposal_approve").performScrollTo().assertIsDisplayed()
            fontScaleStage("check-reset-button")
            compose.onNodeWithText("デモをリセット").performScrollTo().assertIsDisplayed().performClick()
            fontScaleStage("reset-button-clicked")
            compose.waitUntil(10_000) {
                runCatching { compose.onNodeWithTag("task_proposal_close").performScrollTo().assertIsDisplayed() }.isSuccess
            }
        } catch (failure: Throwable) {
            primaryFailure = failure
            runCatching { compose.onRoot(useUnmergedTree = true).printToLog("task-proposal-font-scale") }
        } finally {
            try {
                runCatching {
                    if (originalFontScaleSetting == "null") {
                        shell("settings delete system font_scale")
                    } else {
                        shell("settings put system font_scale $originalFontScaleSetting")
                    }
                }
                runCatching {
                    if (originalSizeOverride == null) shell("wm size reset") else shell("wm size $originalSizeOverride")
                }
                runCatching {
                    if (originalDensityOverride == null) shell("wm density reset") else shell("wm density $originalDensityOverride")
                }
                val restoredSizeOverride = shell("wm size").lineSequence()
                    .firstOrNull { it.startsWith("Override size:") }
                    ?.substringAfter(':')?.trim()?.takeIf { it.isNotEmpty() }
                val restoredDensityOverride = shell("wm density").lineSequence()
                    .firstOrNull { it.startsWith("Override density:") }
                    ?.substringAfter(':')?.trim()?.takeIf { it.isNotEmpty() }
                val restoredFontScaleSetting = shell("settings get system font_scale")
                val originalSettingIsPlatformDefault = originalFontScaleSetting == "null" &&
                    originalScale == 1f && restoredFontScaleSetting.toFloatOrNull() == 1f
                assertTrue(
                    "System font scale setting was not restored: original=$originalFontScaleSetting " +
                        "restored=$restoredFontScaleSetting effectiveOriginal=$originalScale",
                    originalFontScaleSetting == restoredFontScaleSetting || originalSettingIsPlatformDefault,
                )
                assertEquals(originalSizeOverride, restoredSizeOverride)
                assertEquals(originalDensityOverride, restoredDensityOverride)
                compose.waitUntil(15_000) {
                    activityConfigurationMatches(
                        expectedScale = originalScale,
                        expectedWidth = originalWidthDp,
                        expectedDensityDpi = originalDensityDpi,
                    )
                }
            } catch (cleanupFailure: Throwable) {
                fontScaleStage("cleanup-failed:${cleanupFailure::class.java.simpleName}:$lastActivityConfiguration")
                val earlierFailure = primaryFailure
                if (earlierFailure == null) primaryFailure = cleanupFailure
                else earlierFailure.addSuppressed(cleanupFailure)
            }
        }
        primaryFailure?.let { throw it }
    }
}
