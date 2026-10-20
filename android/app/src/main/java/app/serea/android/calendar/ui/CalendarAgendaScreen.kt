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
import androidx.compose.foundation.lazy.items
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
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.unit.dp
import app.serea.android.calendar.CalendarDescriptor
import app.serea.android.calendar.CalendarEvent
import app.serea.android.calendar.CalendarEventKey
import app.serea.android.calendar.CalendarId
import app.serea.android.calendar.EventTime
import app.serea.android.calendar.FakeCalendarProvider
import java.time.Clock
import java.time.LocalDate
import java.time.ZoneId

/**
 * Independent navigation entry point for the read-only Calendar demo.
 * The provider type is deliberately FakeCalendarProvider: this UI cannot start OAuth or select
 * the Google HTTP adapter. Navigation can call this from any host destination.
 */
@Composable
fun CalendarAgendaScreen(
    onBack: () -> Unit,
    provider: FakeCalendarProvider = remember { FakeCalendarProvider() },
    clock: Clock = provider.clock,
    displayZone: ZoneId = ZoneId.systemDefault(),
    modifier: Modifier = Modifier,
) {
    var savedMode by rememberSaveable { mutableStateOf(CalendarAgendaMode.TODAY.name) }
    var savedDate by rememberSaveable { mutableStateOf("") }
    var savedCalendarId by rememberSaveable { mutableStateOf<String?>(null) }
    var savedDetailCalendarId by rememberSaveable { mutableStateOf<String?>(null) }
    var savedDetailEventId by rememberSaveable { mutableStateOf<String?>(null) }
    var dateInput by rememberSaveable { mutableStateOf("") }
    val query = remember(savedMode, savedDate) {
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
            authorizationPort = provider,
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
                    Text(if (inDetail) "戻る" else "閉じる")
                }
                Spacer(Modifier.weight(1f))
                Text(
                    "DEMO / 架空の予定",
                    modifier = Modifier.semantics { contentDescription = "DEMO / 架空の予定。Google Calendarには接続していません。" },
                    style = MaterialTheme.typography.labelLarge,
                    color = MaterialTheme.colorScheme.tertiary,
                    fontWeight = FontWeight.Bold,
                )
            }
            Text("Mock Only · Google Calendar未接続", style = MaterialTheme.typography.labelMedium)

            if (!inDetail) {
                AgendaModeSelector(
                    selected = state.query.mode,
                    dateInput = dateInput,
                    enabled = agendaControlsEnabled,
                    onMode = { mode ->
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
                    },
                    onDateInput = { value ->
                        dateInput = value
                        runCatching { LocalDate.parse(value) }.getOrNull()?.let { date ->
                            savedMode = CalendarAgendaMode.DATE.name
                            savedDate = date.toString()
                            controller.selectDate(date)
                        }
                    },
                )
                CalendarSelector(
                    calendars = state.calendars,
                    selectedId = state.selectedCalendarId,
                    enabled = agendaControlsEnabled,
                    onSelect = { id ->
                        savedCalendarId = id
                        controller.selectCalendar(id)
                    },
                )
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
) {
    Column(verticalArrangement = Arrangement.spacedBy(4.dp)) {
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
                modifier = Modifier.fillMaxWidth(),
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
) {
    if (calendars.isEmpty()) return
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
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
        modifier = Modifier.fillMaxWidth().padding(vertical = 32.dp),
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
        modifier = Modifier.fillMaxWidth().padding(vertical = 20.dp),
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
    onEvent: (CalendarEvent) -> Unit,
) {
    Column(modifier = Modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
        Text(CalendarAgendaJapaneseText.agendaHeading(state.query, clock, displayZone), style = MaterialTheme.typography.titleLarge)
        when {
            state.calendars.isEmpty() -> Text("カレンダーがありません。")
            state.events.isEmpty() -> Text("このカレンダーに予定はありません。", style = MaterialTheme.typography.bodyLarge)
            else -> LazyColumn(
                modifier = Modifier.weight(1f).fillMaxWidth(),
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
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
    LazyColumn(
        modifier = Modifier.fillMaxSize(),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        item {
            Text(CalendarAgendaJapaneseText.title(event.title), style = MaterialTheme.typography.headlineSmall)
        }
        item {
            DetailField("開始・終了", CalendarAgendaJapaneseText.eventTime(event, displayZone))
            val allDay = event.time as? EventTime.AllDay
            if (allDay != null) {
                Text(
                    "終了日は ${CalendarAgendaJapaneseText.date(allDay.endDateExclusive)}（含みません）",
                    style = MaterialTheme.typography.bodyMedium,
                )
            }
        }
        item {
            DetailField("予定の状態", CalendarAgendaJapaneseText.status(event.status))
            if (event.status == app.serea.android.calendar.EventStatus.CANCELLED) {
                Text("取消済みの予定です。Agenda一覧には表示されません。", style = MaterialTheme.typography.bodyMedium)
            }
        }
        item {
            DetailField("対象カレンダー", calendar?.displayName?.takeIf { it.isNotBlank() } ?: "名称なし")
        }
        item {
            DetailField("タイムゾーン", event.time?.let {
                when (it) {
                    is EventTime.AllDay -> it.timeZone.id
                    is EventTime.Timed -> it.timeZone.id
                }
            } ?: calendar?.timeZone?.id ?: "不明")
            DetailField("表示タイムゾーン", displayZone.id)
        }
        item { DetailField("場所", CalendarAgendaJapaneseText.location(event.location)) }
        item { DetailField("説明", CalendarAgendaJapaneseText.description(event.description)) }
    }
}

@Composable
private fun DetailField(label: String, value: String) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(vertical = 4.dp),
        verticalArrangement = Arrangement.spacedBy(2.dp),
    ) {
        Text(label, style = MaterialTheme.typography.labelLarge, color = MaterialTheme.colorScheme.primary)
        Text(value, style = MaterialTheme.typography.bodyLarge)
    }
}
