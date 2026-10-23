package app.serea.android

import android.content.pm.ActivityInfo
import android.view.ViewConfiguration
import android.view.WindowManager
import android.os.ParcelFileDescriptor
import android.os.Looper
import android.util.Log
import androidx.activity.compose.setContent
import androidx.compose.ui.test.assertCountEquals
import androidx.compose.ui.test.assertIsDisplayed
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
import androidx.compose.ui.test.performTouchInput
import androidx.compose.ui.test.swipeUp
import androidx.compose.ui.test.performTextInput
import androidx.compose.ui.test.performTextClearance
import androidx.compose.ui.test.performImeAction
import androidx.compose.ui.test.performScrollTo
import androidx.compose.ui.test.assertHasClickAction
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.SemanticsActions
import androidx.test.espresso.Espresso
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import app.serea.android.assistant.taskproposal.DemoTaskProposalRepository
import app.serea.android.assistant.taskproposal.TaskProposalDemoScreenForTest
import app.serea.android.calendar.FakeCalendarProvider
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

@RunWith(AndroidJUnit4::class)
class SereaAppShellTest {
    private companion object {
        const val CALENDAR_STAGE_TAG = "SereaCalendarStage"
        const val FONT_SCALE_STAGE_TAG = "SereaFontScale"
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
        var result: CalendarSemanticsSnapshot? = null
        compose.runOnIdle {
            // Evaluate config lambdas, child bounds, and Owner geometry together on the UI thread.
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
            val children = node.children.take(20).joinToString(separator = ";") { child ->
                val text = child.config.getOrElseNullable(SemanticsProperties.Text) { null }
                    ?.joinToString(" ") { it.text }
                val description = child.config.getOrElseNullable(SemanticsProperties.ContentDescription) { null }
                    ?.joinToString(" ")
                val label = text?.takeIf { it.isNotBlank() } ?: description.orEmpty()
                "$label@${child.boundsInRoot}"
            }
            result = CalendarSemanticsSnapshot(
                scrollValue = axis?.value(),
                scrollMax = axis?.maxValue(),
                position = position,
                viewport = viewport,
                loadedEventCount = node.config.getOrElseNullable(CalendarAgendaLoadedEventCountKey) { null },
                agendaItems = agendaItems,
                boundsInRoot = CalendarBoundsSnapshot(root.left, root.top, root.right, root.bottom),
                boundsInWindow = CalendarBoundsSnapshot(window.left, window.top, window.right, window.bottom),
                hasClickAction = SemanticsActions.OnClick in node.config,
                visibleSemantics = children,
            )
        }
        return checkNotNull(result) { "Compose UI thread did not produce a Calendar semantics snapshot." }
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
        val item = if (tag == "calendar_agenda_list") {
            snapshotSemantics(calendarNodeWithText(text))
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
        val textSnapshot = runCatching { snapshotSemantics(calendarTargetTextTarget(text)) }.getOrNull() ?: return false
        val list = runCatching { snapshotSemantics(listInteraction) }.getOrNull() ?: return false
        val root = runCatching { snapshotSemantics(calendarRootNode()) }.getOrNull() ?: return false
        val viewport = list.viewport ?: return false
        val left = maxOf(list.boundsInRoot.left, root.boundsInRoot.left)
        val right = minOf(list.boundsInRoot.right, root.boundsInRoot.right)
        val top = maxOf(list.boundsInRoot.top + viewport.viewportStartOffset, root.boundsInRoot.top)
        val bottom = minOf(list.boundsInRoot.top + viewport.viewportEndOffset, root.boundsInRoot.bottom)
        val bounds = textSnapshot.boundsInRoot
        val insideViewport = bounds.left >= left - 1f && bounds.right <= right + 1f &&
            bounds.top >= top - 1f && bounds.bottom <= bottom + 1f
        val composeAssertDisplayed = runCatching {
            calendarTargetTextNode(text).assertIsDisplayed()
        }.isSuccess
        val displayed = insideViewport && composeAssertDisplayed
        if (!displayed) {
            Log.i(
                CALENDAR_STAGE_TAG,
                "TEXT_NOT_FULLY_DISPLAYED|text=$text|textRoot=${textSnapshot.boundsInRoot}|" +
                    "textWindow=${textSnapshot.boundsInWindow}|listRoot=${list.boundsInRoot}|" +
                    "listWindow=${list.boundsInWindow}|viewport=$viewport|root=${root.boundsInRoot}|" +
                    "insideViewport=$insideViewport|composeAssertDisplayed=$composeAssertDisplayed",
            )
        }
        return displayed
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
        compose.onNodeWithText(text, substring = true, useUnmergedTree = true)

    private fun calendarTargetTextTarget(text: String) = calendarNodeWithText(text, unmerged = true)

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
        when (path) {
            CalendarGesturePath.NODE_EDGE,
            CalendarGesturePath.NODE_INTERIOR -> list.interaction.performTouchInput {
                swipeUp(
                    startY = startY,
                    endY = endY,
                    durationMillis = 400,
                )
            }
            CalendarGesturePath.ROOT_INTERIOR -> {
                val rootInteraction = calendarRootNode().interaction
                val rootBounds = root.boundsInRoot
                rootInteraction.performTouchInput {
                    swipeUp(
                        startY = startRootY - rootBounds.top,
                        endY = endRootY - rootBounds.top,
                        durationMillis = 400,
                    )
                }
            }
        }
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
        compose.onNodeWithTag("task_add").assertIsDisplayed()
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
        assertEquals("unsent task transition draft", compose.activity.chatViewModel.state.value.input)
        assertTrue(viewModel === compose.activity.chatViewModel)
        compose.activityRule.scenario.recreate()
        compose.onNodeWithTag("chat-composer").assertTextContains("unsent task transition draft")
        assertEquals("unsent task transition draft", compose.activity.chatViewModel.state.value.input)
        assertTrue(viewModel === compose.activity.chatViewModel)
    }

    @Test fun chatGmailBackShowsOnlyFictionalMailboxData() {
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

    @Test fun chatDraftSurvivesGmailAndCalendarDestinationChanges() {
        compose.onNodeWithTag("chat-composer").performTextInput("draft survives navigation")
        compose.waitUntil(5_000) { compose.activity.chatViewModel.state.value.input == "draft survives navigation" }
        compose.onNodeWithTag("serea_navigation_bar").assertIsDisplayed()
        select(SereaDestination.GMAIL)
        compose.onNodeWithTag("gmail_search_input").performTextInput("back clears focus")
        compose.onNodeWithTag("gmail_search_input").performImeAction()
        val emptySearchSettled = runCatching { compose.waitUntil(10_000) {
            runCatching {
                compose.onNodeWithTag("gmail_result_count").assertTextEquals("0 件")
                compose.onNodeWithTag("gmail_search_input").assertIsNotFocused()
            }.isSuccess
        } }.isSuccess
        if (!emptySearchSettled) compose.onRoot(useUnmergedTree = true).printToLog("chat-draft-gmail-search-timeout")
        assertTrue("Gmail IME search must settle before navigation and release TextField focus.", emptySearchSettled)
        pressBack()
        compose.onNodeWithTag("gmail_list_heading").assertTextContains("受信箱").assertIsDisplayed()
        pressBack()
        assertEquals("draft survives navigation", compose.activity.chatViewModel.state.value.input)
        compose.onNodeWithTag("chat-composer").assertIsNotFocused()
        select(SereaDestination.CALENDAR)
        compose.onNodeWithTag("calendar_destination").assertIsDisplayed()
        compose.waitUntil(10_000) {
            compose.onAllNodesWithText("DEMO / 架空の予定").fetchSemanticsNodes().isNotEmpty()
        }
        pressBack()
        compose.onNodeWithTag("chat-composer").assertIsDisplayed()
        compose.onNodeWithTag("chat-composer").assertTextContains("draft survives navigation")
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
            val size = compose.onRoot().fetchSemanticsNode().size
            size.width > size.height
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
        compose.activityRule.scenario.onActivity { activity ->
            val manager = activity.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                as android.view.inputmethod.InputMethodManager
            manager.hideSoftInputFromWindow(activity.window.decorView.windowToken, 0)
        }
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
        compose.activityRule.scenario.onActivity { activity ->
            val manager = activity.getSystemService(android.content.Context.INPUT_METHOD_SERVICE)
                as android.view.inputmethod.InputMethodManager
            manager.hideSoftInputFromWindow(activity.window.decorView.windowToken, 0)
        }
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
        try {
            shell("wm size 320x640")
            shell("wm density 160")
            shell("settings put system font_scale 2.0")
            compose.waitUntil(15_000) {
                var fontScale = 1f
                var width = 0
                var densityDpi = 0
                compose.activityRule.scenario.onActivity {
                    fontScale = it.resources.configuration.fontScale
                    width = it.resources.configuration.screenWidthDp
                    densityDpi = it.resources.configuration.densityDpi
                }
                fontScale >= 1.99f && width == 320 && densityDpi == 160
            }
            fontScaleStage("font-scale-applied")
            DestinationContract.all.forEach { contract ->
                compose.onNodeWithTag("destination_${contract.destination.name.lowercase()}")
                    .assertIsDisplayed()
                    .assertTextContains(contract.title)
            }
            compose.onNodeWithTag("chat-composer").assertIsDisplayed()

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
            compose.onRoot(useUnmergedTree = true).printToLog("task-proposal-font-scale")
            throw failure
        } finally {
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
            assertEquals(originalFontScaleSetting, shell("settings get system font_scale"))
            assertEquals(originalSizeOverride, restoredSizeOverride)
            assertEquals(originalDensityOverride, restoredDensityOverride)
            compose.waitUntil(15_000) {
                var width = 0
                var densityDpi = 0
                var fontScale = 0f
                compose.activityRule.scenario.onActivity {
                    width = it.resources.configuration.screenWidthDp
                    densityDpi = it.resources.configuration.densityDpi
                    fontScale = it.resources.configuration.fontScale
                }
                width == originalWidthDp && densityDpi == originalDensityDpi &&
                    kotlin.math.abs(fontScale - originalScale) < 0.02f
            }
        }
    }
}
