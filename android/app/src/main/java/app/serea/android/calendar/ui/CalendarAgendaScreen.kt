package app.serea.android.calendar.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.LazyListState
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.horizontalScroll
import androidx.compose.foundation.shape.RoundedCornerShape
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.derivedStateOf
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.SemanticsPropertyKey
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import app.serea.android.calendar.CalendarDescriptor
import app.serea.android.calendar.CalendarEvent
import app.serea.android.calendar.CalendarEventKey
import app.serea.android.calendar.CalendarAuthorizationPort
import app.serea.android.calendar.CalendarReadPort
import app.serea.android.calendar.CalendarId
import app.serea.android.calendar.EventTime
import app.serea.android.calendar.FakeCalendarProvider
import java.time.Clock
import java.time.LocalDate
import java.time.ZoneId

/** Exposes the real lazy-list position to Compose tests without adding accessibility text. */
internal val CalendarLazyListPositionKey =
    SemanticsPropertyKey<Pair<Int, Int>>("CalendarLazyListPosition")

/** Immutable layout data for test diagnostics; no LazyListState or snapshot lambda escapes. */
internal data class CalendarLazyListViewportSnapshot(
    val totalItems: Int,
    val visibleItemIndices: List<Int>,
    val viewportStartOffset: Int,
    val viewportEndOffset: Int,
    val canScrollForward: Boolean,
)

internal val CalendarLazyListViewportKey =
    SemanticsPropertyKey<CalendarLazyListViewportSnapshot>("CalendarLazyListViewport")

internal val CalendarAgendaLoadedEventCountKey =
    SemanticsPropertyKey<Int>("CalendarAgendaLoadedEventCount")

/** Synthetic agenda page results and their realized LazyColumn positions for bounded UI tests. */
internal data class CalendarAgendaItemsSnapshot(
    val eventCount: Int,
    val eventItemIndices: Map<String, Int>,
)

internal val CalendarAgendaItemsKey =
    SemanticsPropertyKey<CalendarAgendaItemsSnapshot>("CalendarAgendaItems")

/**
 * Independent navigation entry point for read-only Calendar. The composition root supplies
 * session-bound ports; this screen never creates authorization or network adapters.
 */
@Composable
fun CalendarAgendaScreen(
    onBack: () -> Unit,
    provider: CalendarReadPort = remember { FakeCalendarProvider() },
    authorizationPort: CalendarAuthorizationPort = provider as? CalendarAuthorizationPort
        ?: error("Calendar provider must expose its authorization port."),
    clock: Clock = (provider as? FakeCalendarProvider)?.clock ?: Clock.systemUTC(),
    displayZone: ZoneId = ZoneId.systemDefault(),
    modifier: Modifier = Modifier,
) {
    val configuration = LocalConfiguration.current
    val compactControls = configuration.orientation == android.content.res.Configuration.ORIENTATION_LANDSCAPE ||
        configuration.fontScale >= 1.5f
    var savedMode by rememberSaveable { mutableStateOf(CalendarAgendaMode.TODAY.name) }
    var savedDate by rememberSaveable { mutableStateOf("") }
    var savedCalendarId by rememberSaveable { mutableStateOf<String?>(null) }
    var savedDetailCalendarId by rememberSaveable { mutableStateOf<String?>(null) }
    var savedDetailEventId by rememberSaveable { mutableStateOf<String?>(null) }
    var dateInput by rememberSaveable { mutableStateOf("") }
    val agendaListState = rememberLazyListState()
    val query: CalendarAgendaQuery = remember(savedMode, savedDate) {
        when (runCatching { CalendarAgendaMode.valueOf(savedMode) }.getOrDefault(CalendarAgendaMode.TODAY)) {
            CalendarAgendaMode.TODAY -> CalendarAgendaQuery(CalendarAgendaMode.TODAY)
            CalendarAgendaMode.TOMORROW -> CalendarAgendaQuery(CalendarAgendaMode.TOMORROW)
            CalendarAgendaMode.UPCOMING -> CalendarAgendaQuery(CalendarAgendaMode.UPCOMING)
            CalendarAgendaMode.DATE -> {
                val date = runCatching { LocalDate.parse(savedDate) }
                    .getOrElse { clock.instant().atZone(displayZone).toLocalDate() }
                CalendarAgendaQuery(CalendarAgendaMode.DATE, date)
            }
        }
    }
    val detailKey = remember(savedDetailCalendarId, savedDetailEventId) {
        if (savedDetailCalendarId == null || savedDetailEventId == null) null else runCatching {
            CalendarEventKey(CalendarId(savedDetailCalendarId!!), app.serea.android.calendar.EventId(savedDetailEventId!!))
        }.getOrNull()
    }
    val scope = rememberCoroutineScope()
    val controller = remember(provider, clock, displayZone) {
        CalendarAgendaUiController(
            readPort = provider,
            authorizationPort = authorizationPort,
            clock = clock,
            displayZone = displayZone,
            scope = scope,
            initialQuery = query,
            initialCalendarId = savedCalendarId,
            initialDetailKey = detailKey,
        )
    }
    DisposableEffect(controller) {
        onDispose { controller.close() }
    }
    val state by controller.state.collectAsState()
    val isDemoProvider = when (state) {
        is CalendarAgendaUiState.Agenda -> (state as CalendarAgendaUiState.Agenda).mockOnly
        is CalendarAgendaUiState.EventDetail -> (state as CalendarAgendaUiState.EventDetail).mockOnly
        is CalendarAgendaUiState.Failure -> (state as CalendarAgendaUiState.Failure).mockOnly
        is CalendarAgendaUiState.Loading -> true
    }
    var previousAgendaScope by remember { mutableStateOf(state.query to state.selectedCalendarId) }
    LaunchedEffect(state.query, state.selectedCalendarId) {
        val currentScope = state.query to state.selectedCalendarId
        if (currentScope != previousAgendaScope) agendaListState.requestScrollToItem(0)
        previousAgendaScope = currentScope
    }
    val inDetail = when (state) {
        is CalendarAgendaUiState.EventDetail -> true
        is CalendarAgendaUiState.Loading -> (state as CalendarAgendaUiState.Loading).detailKey != null
        is CalendarAgendaUiState.Failure -> (state as CalendarAgendaUiState.Failure).detailKey != null
        is CalendarAgendaUiState.Agenda -> false
    }
    val agendaControlsEnabled = state !is CalendarAgendaUiState.Loading

    LaunchedEffect(state.selectedCalendarId) {
        if (state.selectedCalendarId != null) savedCalendarId = state.selectedCalendarId
    }
    LaunchedEffect(state) {
        when (state) {
            is CalendarAgendaUiState.EventDetail -> {
                val event = (state as CalendarAgendaUiState.EventDetail).event
                savedDetailCalendarId = event.key.calendarId.value
                savedDetailEventId = event.key.eventId.value
            }
            is CalendarAgendaUiState.Agenda -> {
                savedDetailCalendarId = null
                savedDetailEventId = null
            }
            is CalendarAgendaUiState.Failure -> {
                val key = (state as CalendarAgendaUiState.Failure).detailKey
                if (key != null) {
                    savedDetailCalendarId = key.calendarId.value
                    savedDetailEventId = key.eventId.value
                }
            }
            is CalendarAgendaUiState.Loading -> Unit
        }
    }

    val leaveDetail = {
        savedDetailCalendarId = null
        savedDetailEventId = null
        controller.backToAgenda()
    }
    BackHandler(enabled = inDetail, onBack = leaveDetail)

    Scaffold(modifier = modifier.fillMaxSize()) { padding ->
        Column(
            modifier = Modifier.fillMaxSize().padding(padding).padding(horizontal = 16.dp, vertical = 8.dp),
            verticalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            Row(verticalAlignment = Alignment.CenterVertically, modifier = Modifier.fillMaxWidth()) {
                TextButton(onClick = { if (inDetail) leaveDetail() else onBack() }) {
                    Text("戻る")
                }
                Spacer(Modifier.weight(1f))
                Text(
                    if (isDemoProvider) "DEMO / 架空の予定" else "Google Calendar · 読み取り専用",
                    modifier = Modifier.semantics {
                        contentDescription = if (isDemoProvider) {
                            "DEMO / 架空の予定。Google Calendarには接続していません。"
                        } else {
                            "Google Calendarから取得した読み取り専用の予定です。"
                        }
                        heading()
                    },
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.tertiary,
                    fontWeight = FontWeight.Bold,
                )
            }
            Text(
                if (isDemoProvider) "デモのみ · Google Calendar未接続" else "Google Calendarから取得 · 変更はできません",
                style = MaterialTheme.typography.labelMedium,
            )

            if (!inDetail) {
                val onMode: (CalendarAgendaMode) -> Unit = { mode ->
                    if (mode == CalendarAgendaMode.DATE) {
                        savedMode = mode.name
                        dateInput = normalizeSpecifiedDateInput(dateInput, clock, displayZone)
                        val date = LocalDate.parse(dateInput)
                        savedDate = date.toString()
                        controller.selectDate(date)
                    } else {
                        savedMode = mode.name
                        savedDate = ""
                        controller.selectMode(mode)
                    }
                }
                val onDateInput: (String) -> Unit = { value ->
                    dateInput = value
                    runCatching { LocalDate.parse(value) }.getOrNull()?.let { date ->
                        savedMode = CalendarAgendaMode.DATE.name
                        savedDate = date.toString()
                        controller.selectDate(date)
                    }
                }
                val onCalendar: (String) -> Unit = { id ->
                    savedCalendarId = id
                    controller.selectCalendar(id)
                }
                if (compactControls) {
                    Row(
                        modifier = Modifier.fillMaxWidth(),
                        horizontalArrangement = Arrangement.spacedBy(8.dp),
                        verticalAlignment = Alignment.CenterVertically,
                    ) {
                        AgendaModeSelector(
                            selected = state.query.mode,
                            dateInput = dateInput,
                            enabled = agendaControlsEnabled,
                            onMode = onMode,
                            onDateInput = onDateInput,
                            modifier = Modifier.weight(1f),
                        )
                        CalendarSelector(
                            calendars = state.calendars,
                            selectedId = state.selectedCalendarId,
                            enabled = agendaControlsEnabled,
                            onSelect = onCalendar,
                            modifier = Modifier.weight(1f),
                        )
                    }
                } else {
                    AgendaModeSelector(
                        selected = state.query.mode,
                        dateInput = dateInput,
                        enabled = agendaControlsEnabled,
                        onMode = onMode,
                        onDateInput = onDateInput,
                    )
                    CalendarSelector(
                        calendars = state.calendars,
                        selectedId = state.selectedCalendarId,
                        enabled = agendaControlsEnabled,
                        onSelect = onCalendar,
                    )
                }
            }

            Box(modifier = Modifier.weight(1f).fillMaxWidth()) {
                when (val current = state) {
                    is CalendarAgendaUiState.Loading -> LoadingContent(current.detailKey != null)
                    is CalendarAgendaUiState.Failure -> FailureContent(current, controller::retry)
                    is CalendarAgendaUiState.EventDetail -> EventDetailContent(
                        current.event,
                        current.calendars.firstOrNull { it.id == current.event.key.calendarId },
                        displayZone,
                    )
                    is CalendarAgendaUiState.Agenda -> AgendaContent(
                        current,
                        clock,
                        displayZone,
                        agendaListState,
                        onEvent = { event ->
                            savedDetailCalendarId = event.key.calendarId.value
                            savedDetailEventId = event.key.eventId.value
                            controller.openEvent(event.key)
                        },
                    )
                }
            }
        }
    }
}

@Composable
private fun AgendaModeSelector(
    selected: CalendarAgendaMode,
    dateInput: String,
    enabled: Boolean,
    onMode: (CalendarAgendaMode) -> Unit,
    onDateInput: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(4.dp)) {
        Row(
            modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            FilterChip(selected == CalendarAgendaMode.TODAY, onClick = { onMode(CalendarAgendaMode.TODAY) }, enabled = enabled, label = { Text("今日") })
            FilterChip(selected == CalendarAgendaMode.TOMORROW, onClick = { onMode(CalendarAgendaMode.TOMORROW) }, enabled = enabled, label = { Text("明日") })
            FilterChip(selected == CalendarAgendaMode.UPCOMING, onClick = { onMode(CalendarAgendaMode.UPCOMING) }, enabled = enabled, label = { Text("今後") })
            FilterChip(selected == CalendarAgendaMode.DATE, onClick = { onMode(CalendarAgendaMode.DATE) }, enabled = enabled, label = { Text("指定日") })
        }
        if (selected == CalendarAgendaMode.DATE) {
            val validDate = runCatching { LocalDate.parse(dateInput) }.isSuccess
            OutlinedTextField(
                value = dateInput,
                onValueChange = onDateInput,
                modifier = Modifier.fillMaxWidth().testTag("calendar_date_input"),
                enabled = enabled,
                label = { Text("指定日 (YYYY-MM-DD)") },
                placeholder = { Text("2026-10-21") },
                singleLine = true,
                isError = dateInput.isNotEmpty() && !validDate,
                supportingText = if (dateInput.isNotEmpty() && !validDate) ({ Text("YYYY-MM-DD形式で入力してください") }) else null,
                keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Ascii),
            )
        }
    }
}

@Composable
private fun CalendarSelector(
    calendars: List<CalendarDescriptor>,
    selectedId: String?,
    enabled: Boolean,
    onSelect: (String) -> Unit,
    modifier: Modifier = Modifier,
) {
    if (calendars.isEmpty()) return
    Column(modifier = modifier, verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text("表示するカレンダー", style = MaterialTheme.typography.labelLarge)
        Row(
            modifier = Modifier.fillMaxWidth().horizontalScroll(rememberScrollState()),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            calendars.forEach { calendar ->
                val name = calendar.displayName?.takeIf { it.isNotBlank() } ?: "名称なし"
                FilterChip(
                    selected = calendar.id.value == selectedId,
                    onClick = { onSelect(calendar.id.value) },
                    enabled = enabled,
                    label = { Text(name, maxLines = 2) },
                )
            }
        }
    }
}

@Composable
private fun LoadingContent(detail: Boolean) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(vertical = 32.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        CircularProgressIndicator(modifier = Modifier.size(36.dp))
        Text(if (detail) "予定の詳細を読み込み中" else "予定を読み込み中", style = MaterialTheme.typography.bodyLarge)
    }
}

@Composable
private fun FailureContent(state: CalendarAgendaUiState.Failure, onRetry: () -> Unit) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(vertical = 20.dp)
            .semantics { liveRegion = LiveRegionMode.Polite },
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            CalendarAgendaJapaneseText.safeMessage(state.reason),
            style = MaterialTheme.typography.bodyLarge,
            color = MaterialTheme.colorScheme.error,
        )
        TextButton(onClick = onRetry) { Text("再試行") }
    }
}

@Composable
private fun AgendaContent(
    state: CalendarAgendaUiState.Agenda,
    clock: Clock,
    displayZone: ZoneId,
    listState: LazyListState,
    onEvent: (CalendarEvent) -> Unit,
) {
    val eventItemIndices = buildMap {
        var itemIndex = 1 // Agenda heading is item zero.
        state.events.groupBy { eventGroupDate(it, state.query, clock, displayZone) }
            .toSortedMap()
            .values
            .forEach { events ->
                itemIndex++ // Date heading.
                events.forEach { event ->
                    put(event.key.eventId.value, itemIndex++)
                }
            }
    }
    val listPosition by remember(listState) {
        derivedStateOf {
            listState.firstVisibleItemIndex to listState.firstVisibleItemScrollOffset
        }
    }
    LazyColumn(
        modifier = Modifier.fillMaxSize()
            .semantics {
                this[CalendarLazyListPositionKey] = listPosition
                this[CalendarLazyListViewportKey] = CalendarLazyListViewportSnapshot(
                    totalItems = listState.layoutInfo.totalItemsCount,
                    visibleItemIndices = listState.layoutInfo.visibleItemsInfo.map { it.index }.toList(),
                    viewportStartOffset = listState.layoutInfo.viewportStartOffset,
                    viewportEndOffset = listState.layoutInfo.viewportEndOffset,
                    canScrollForward = listState.canScrollForward,
                )
                this[CalendarAgendaLoadedEventCountKey] = state.events.size
                this[CalendarAgendaItemsKey] = CalendarAgendaItemsSnapshot(
                    eventCount = state.events.size,
                    eventItemIndices = eventItemIndices.toMap(),
                )
            }
            .testTag("calendar_agenda_list"),
        state = listState,
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        item {
            Text(
                CalendarAgendaJapaneseText.agendaHeading(state.query, clock, displayZone),
                style = MaterialTheme.typography.titleLarge,
                modifier = Modifier.semantics { heading() },
            )
        }
        when {
            state.calendars.isEmpty() -> item {
                Text("カレンダーがありません。", modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
            }
            state.events.isEmpty() -> item {
                Text(
                    "このカレンダーに予定はありません。",
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
                )
            }
            else -> {
                state.events.groupBy { eventGroupDate(it, state.query, clock, displayZone) }.toSortedMap().forEach { (date, events) ->
                    item(key = "date-${date}") {
                        Text(CalendarAgendaJapaneseText.date(date), style = MaterialTheme.typography.titleMedium)
                    }
                    items(events, key = { it.stableIdentity() }) { event ->
                        EventCard(
                            event,
                            state.calendars.firstOrNull { it.id == event.key.calendarId },
                            displayZone,
                            onClick = { onEvent(event) },
                        )
                    }
                }
            }
        }
    }
}

private fun eventGroupDate(
    event: CalendarEvent,
    query: CalendarAgendaQuery,
    clock: Clock,
    displayZone: ZoneId,
): LocalDate {
    when (query.mode) {
        CalendarAgendaMode.TODAY -> return clock.instant().atZone(displayZone).toLocalDate()
        CalendarAgendaMode.TOMORROW -> return clock.instant().atZone(displayZone).toLocalDate().plusDays(1)
        CalendarAgendaMode.DATE -> return checkNotNull(query.selectedDate)
        CalendarAgendaMode.UPCOMING -> Unit
    }
    return when (val time = event.time) {
    is EventTime.AllDay -> time.startDate
    is EventTime.Timed -> time.start.atZone(displayZone).toLocalDate()
    null -> LocalDate.MIN
    }
}

@Composable
private fun EventCard(
    event: CalendarEvent,
    calendar: CalendarDescriptor?,
    displayZone: ZoneId,
    onClick: () -> Unit,
) {
    val calendarName = calendar?.displayName?.takeIf { it.isNotBlank() } ?: "名称なし"
    Card(
        onClick = onClick,
        modifier = Modifier.fillMaxWidth().semantics(mergeDescendants = true) { },
        shape = RoundedCornerShape(16.dp),
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceVariant),
    ) {
        Column(
            modifier = Modifier.fillMaxWidth().padding(16.dp),
            verticalArrangement = Arrangement.spacedBy(4.dp),
        ) {
            Text(CalendarAgendaJapaneseText.title(event.title), style = MaterialTheme.typography.titleMedium, fontWeight = FontWeight.SemiBold)
            Text(CalendarAgendaJapaneseText.eventTime(event, displayZone), style = MaterialTheme.typography.bodyMedium)
            Text("カレンダー · $calendarName", style = MaterialTheme.typography.bodySmall)
            Text(CalendarAgendaJapaneseText.status(event.status), style = MaterialTheme.typography.labelMedium)
        }
    }
}

@Composable
private fun EventDetailContent(
    event: CalendarEvent,
    calendar: CalendarDescriptor?,
    displayZone: ZoneId,
) {
    val listState = rememberLazyListState()
    val listPosition by remember(listState) {
        derivedStateOf {
            listState.firstVisibleItemIndex to listState.firstVisibleItemScrollOffset
        }
    }
    LazyColumn(
        state = listState,
        modifier = Modifier.fillMaxSize()
            .semantics {
                this[CalendarLazyListPositionKey] = listPosition
                this[CalendarLazyListViewportKey] = CalendarLazyListViewportSnapshot(
                    totalItems = listState.layoutInfo.totalItemsCount,
                    visibleItemIndices = listState.layoutInfo.visibleItemsInfo.map { it.index }.toList(),
                    viewportStartOffset = listState.layoutInfo.viewportStartOffset,
                    viewportEndOffset = listState.layoutInfo.viewportEndOffset,
                    canScrollForward = listState.canScrollForward,
                )
            }
            .testTag("calendar_event_detail_list"),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            Column(Modifier.fillMaxWidth().testTag("calendar_event_detail_item_0")) {
                Text(
                    CalendarAgendaJapaneseText.title(event.title),
                    style = MaterialTheme.typography.headlineSmall,
                    modifier = Modifier.semantics { heading() },
                )
            }
        }
        item {
            Column(Modifier.fillMaxWidth().testTag("calendar_event_detail_item_1")) {
                DetailField("開始・終了", CalendarAgendaJapaneseText.eventTime(event, displayZone))
                val allDay = event.time as? EventTime.AllDay
                if (allDay != null) {
                    Text(
                        "終了日は ${CalendarAgendaJapaneseText.date(allDay.endDateExclusive)}（含みません）",
                        style = MaterialTheme.typography.bodyMedium,
                    )
                }
            }
        }
        item {
            Column(Modifier.fillMaxWidth().testTag("calendar_event_detail_item_2")) {
                DetailField("予定の状態", CalendarAgendaJapaneseText.status(event.status))
                if (event.status == app.serea.android.calendar.EventStatus.CANCELLED) {
                    Text("取消済みの予定です。Agenda一覧には表示されません。", style = MaterialTheme.typography.bodyMedium)
                }
            }
        }
        item {
            DetailField(
                "対象カレンダー",
                calendar?.displayName?.takeIf { it.isNotBlank() } ?: "名称なし",
                modifier = Modifier.testTag("calendar_event_detail_item_3"),
            )
        }
        item {
            Column(Modifier.fillMaxWidth().testTag("calendar_event_detail_item_4")) {
                DetailField("タイムゾーン", event.time?.let {
                    when (it) {
                        is EventTime.AllDay -> it.timeZone.id
                        is EventTime.Timed -> it.timeZone.id
                    }
                } ?: calendar?.timeZone?.id ?: "不明")
                DetailField("表示タイムゾーン", displayZone.id)
            }
        }
        item {
            DetailField(
                "場所",
                CalendarAgendaJapaneseText.location(event.location),
                modifier = Modifier.testTag("calendar_event_detail_item_5"),
            )
        }
        item {
            DetailField(
                "説明",
                CalendarAgendaJapaneseText.description(event.description),
                modifier = Modifier.testTag("calendar_event_detail_item_6"),
            )
        }
    }
}

@Composable
private fun DetailField(label: String, value: String, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        Text(value, style = MaterialTheme.typography.bodyLarge)
    }
}
