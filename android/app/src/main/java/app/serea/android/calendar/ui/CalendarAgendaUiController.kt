package app.serea.android.calendar.ui

import app.serea.android.calendar.CalendarAgendaService
import app.serea.android.calendar.CalendarApiFailureCode
import app.serea.android.calendar.CalendarAuthorizationPort
import app.serea.android.calendar.CalendarAuthorizationState
import app.serea.android.calendar.CalendarAuthorizationStatus
import app.serea.android.calendar.CalendarDateRange
import app.serea.android.calendar.CalendarEvent
import app.serea.android.calendar.CalendarEventKey
import app.serea.android.calendar.CalendarEventSource
import app.serea.android.calendar.CalendarPermissionPolicy
import app.serea.android.calendar.CalendarReadFailure
import app.serea.android.calendar.CalendarReadPort
import app.serea.android.calendar.CalendarEventPage
import app.serea.android.calendar.EventStatus
import app.serea.android.calendar.EventTime
import app.serea.android.calendar.CalendarDescriptor
import java.time.Clock
import java.time.Duration
import java.time.Instant
import java.time.LocalDate
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Job
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.isActive
import kotlinx.coroutines.launch

enum class CalendarAgendaMode { TODAY, TOMORROW, UPCOMING, DATE }

data class CalendarAgendaQuery(
    val mode: CalendarAgendaMode = CalendarAgendaMode.TODAY,
    val selectedDate: LocalDate? = null,
) {
    init { require((mode == CalendarAgendaMode.DATE) == (selectedDate != null)) }
}

enum class CalendarAgendaFailure {
    PERMISSION_REQUIRED,
    MOCK_ONLY_REQUIRED,
    DENIED,
    EXPIRED,
    MISSING_SCOPE,
    TIME_ZONE_UNKNOWN,
    TIMEOUT,
    RATE_LIMITED,
    PARTIAL_FAILURE,
    EVENT_UNAVAILABLE,
    INVALID_RESPONSE,
    TOO_MANY_RESULTS,
    UNAVAILABLE,
}

sealed interface CalendarAgendaUiState {
    val query: CalendarAgendaQuery
    val selectedCalendarId: String?
    val calendars: List<CalendarDescriptor>

    data class Loading(
        override val query: CalendarAgendaQuery,
        override val selectedCalendarId: String?,
        override val calendars: List<CalendarDescriptor> = emptyList(),
        val detailKey: CalendarEventKey? = null,
    ) : CalendarAgendaUiState

    data class Agenda(
        override val query: CalendarAgendaQuery,
        override val selectedCalendarId: String?,
        override val calendars: List<CalendarDescriptor>,
        val events: List<CalendarEvent>,
        val mockOnly: Boolean,
    ) : CalendarAgendaUiState

    data class Failure(
        override val query: CalendarAgendaQuery,
        override val selectedCalendarId: String?,
        override val calendars: List<CalendarDescriptor>,
        val reason: CalendarAgendaFailure,
        val detailKey: CalendarEventKey? = null,
        val mockOnly: Boolean = true,
    ) : CalendarAgendaUiState

    data class EventDetail(
        override val query: CalendarAgendaQuery,
        override val selectedCalendarId: String?,
        override val calendars: List<CalendarDescriptor>,
        val event: CalendarEvent,
        val mockOnly: Boolean,
    ) : CalendarAgendaUiState
}

/**
 * Small read-only coordinator for the independent Calendar screen. It accepts read ports only;
 * the published Composable supplies FakeCalendarProvider and never constructs an HTTP adapter.
 */
internal class CalendarAgendaUiController(
    private val readPort: CalendarReadPort,
    private val authorizationPort: CalendarAuthorizationPort,
    private val clock: Clock,
    private val displayZone: ZoneId,
    private val scope: CoroutineScope,
    initialQuery: CalendarAgendaQuery = CalendarAgendaQuery(),
    initialCalendarId: String? = null,
    initialDetailKey: CalendarEventKey? = null,
) {
    private val mutableState = MutableStateFlow<CalendarAgendaUiState>(
        CalendarAgendaUiState.Loading(initialQuery, initialCalendarId, detailKey = initialDetailKey),
    )
    val state: StateFlow<CalendarAgendaUiState> = mutableState.asStateFlow()

    private var query = initialQuery
    private var selectedCalendarId = initialCalendarId
    private var detailKey: CalendarEventKey? = initialDetailKey
    private var requestGeneration = 0L
    private var activeJob: Job? = null
    private var lastAgenda: CalendarAgendaUiState.Agenda? = null

    init { loadCurrentRoute() }

    fun selectMode(mode: CalendarAgendaMode) {
        val next = when (mode) {
            CalendarAgendaMode.DATE -> query.takeIf { it.mode == mode } ?: return
            else -> CalendarAgendaQuery(mode)
        }
        if (next == query && detailKey == null) return
        query = next
        detailKey = null
        loadAgenda()
    }

    fun selectDate(date: LocalDate) {
        val next = CalendarAgendaQuery(CalendarAgendaMode.DATE, date)
        if (next == query && detailKey == null) return
        query = next
        detailKey = null
        loadAgenda()
    }

    fun selectCalendar(calendarId: String) {
        if (selectedCalendarId == calendarId && detailKey == null) return
        selectedCalendarId = calendarId
        detailKey = null
        loadAgenda()
    }

    fun openEvent(key: CalendarEventKey) {
        detailKey = key
        loadDetail(key)
    }

    fun backToAgenda() {
        detailKey = null
        requestGeneration++
        activeJob?.cancel()
        activeJob = null
        val cached = lastAgenda
        if (cached != null && cached.query == query && cached.selectedCalendarId == selectedCalendarId) {
            mutableState.value = cached
        } else {
            loadAgenda()
        }
    }

    /** A second retry while loading is ignored; selection changes still replace/cancel a request. */
    fun retry() {
        if (mutableState.value is CalendarAgendaUiState.Loading) return
        loadCurrentRoute()
    }

    fun close() {
        requestGeneration++
        activeJob?.cancel()
        activeJob = null
    }

    private fun loadCurrentRoute() {
        val key = detailKey
        if (key == null) loadAgenda() else loadDetail(key)
    }

    private fun loadAgenda() {
        val requestedQuery = query
        val requestedCalendarId = selectedCalendarId
        val previousCalendars = mutableState.value.calendars
        val generation = replaceRequest(
            CalendarAgendaUiState.Loading(requestedQuery, requestedCalendarId, previousCalendars),
        )
        activeJob = scope.launch {
            try {
                val authorization = requireReadAccess(requireCalendarList = true)
                val calendars = readPort.listCalendars()
                currentCoroutineContext().ensureActive()
                val selected = calendars.firstOrNull { it.id.value == requestedCalendarId }
                    ?: calendars.firstOrNull()
                selectedCalendarId = selected?.id?.value
                mutableState.value = CalendarAgendaUiState.Loading(
                    requestedQuery, selectedCalendarId, calendars,
                )
                val events = if (selected == null) emptyList() else {
                    val range = calendarAgendaRange(requestedQuery, clock, displayZone)
                    CalendarAgendaService(
                        CalendarAgendaSelectedReadPort(readPort, selected), clock, displayZone,
                    ).eventsInRange(range)
                        .asSequence()
                        .filter { it.key.calendarId == selected.id }
                        .filter { isInAgenda(it, requestedQuery, clock, displayZone) }
                        .distinctBy { it.stableIdentity() }
                        .toList()
                }
                if (events.any { it.source != CalendarEventSource.MOCK }) {
                    throw UiCalendarFailure(CalendarAgendaFailure.MOCK_ONLY_REQUIRED)
                }
                currentCoroutineContext().ensureActive()
                if (generation != requestGeneration) return@launch
                val ready = CalendarAgendaUiState.Agenda(
                    requestedQuery,
                    selected?.id?.value,
                    calendars,
                    events,
                    authorization.state == CalendarAuthorizationState.MOCK_ONLY,
                )
                lastAgenda = ready
                mutableState.value = ready
            } catch (cancelled: CancellationException) {
                if (generation == requestGeneration && currentCoroutineContext().isActive) {
                    mutableState.value = CalendarAgendaUiState.Failure(
                        requestedQuery,
                        selectedCalendarId,
                        mutableState.value.calendars,
                        CalendarAgendaFailure.UNAVAILABLE,
                    )
                }
                throw cancelled
            } catch (failure: Exception) {
                if (generation == requestGeneration) {
                    val reason = failure.toAgendaFailure()
                    mutableState.value = CalendarAgendaUiState.Failure(
                        requestedQuery,
                        selectedCalendarId,
                        mutableState.value.calendars,
                        reason,
                    )
                }
            }
        }
    }

    private fun loadDetail(key: CalendarEventKey) {
        val requestedQuery = query
        val requestedCalendarId = selectedCalendarId
        val calendars = mutableState.value.calendars
        val generation = replaceRequest(
            CalendarAgendaUiState.Loading(requestedQuery, requestedCalendarId, calendars, detailKey = key),
        )
        activeJob = scope.launch {
            try {
                if (requestedCalendarId == null || key.calendarId.value != requestedCalendarId) {
                    throw UiCalendarFailure(CalendarAgendaFailure.EVENT_UNAVAILABLE)
                }
                val authorization = requireReadAccess(requireCalendarList = true)
                val calendars = readPort.listCalendars()
                currentCoroutineContext().ensureActive()
                mutableState.value = CalendarAgendaUiState.Loading(
                    requestedQuery, requestedCalendarId, calendars, detailKey = key,
                )
                val event = readPort.getEvent(key)
                currentCoroutineContext().ensureActive()
                if (generation != requestGeneration) return@launch
                if (event.key != key) throw CalendarReadFailure(
                    CalendarApiFailureCode.INVALID_RESPONSE,
                    "Calendar provider returned an event for another key.",
                )
                if (event.source != CalendarEventSource.MOCK) {
                    throw UiCalendarFailure(CalendarAgendaFailure.MOCK_ONLY_REQUIRED)
                }
                CalendarPermissionPolicy.assertCalendarEventSafe(event)
                mutableState.value = CalendarAgendaUiState.EventDetail(
                    requestedQuery, requestedCalendarId, calendars, event,
                    authorization.state == CalendarAuthorizationState.MOCK_ONLY,
                )
            } catch (cancelled: CancellationException) {
                if (generation == requestGeneration && currentCoroutineContext().isActive) {
                    mutableState.value = CalendarAgendaUiState.Failure(
                        requestedQuery,
                        requestedCalendarId,
                        mutableState.value.calendars,
                        CalendarAgendaFailure.UNAVAILABLE,
                        detailKey = key,
                    )
                }
                throw cancelled
            } catch (failure: Exception) {
                if (generation == requestGeneration) {
                    mutableState.value = CalendarAgendaUiState.Failure(
                        requestedQuery,
                        requestedCalendarId,
                        mutableState.value.calendars,
                        failure.toAgendaFailure(isDetail = true),
                        detailKey = key,
                    )
                }
            }
        }
    }

    private suspend fun requireReadAccess(requireCalendarList: Boolean): CalendarAuthorizationStatus {
        val status = authorizationPort.status()
        when (status.state) {
            CalendarAuthorizationState.MOCK_ONLY -> Unit
            CalendarAuthorizationState.AUTHORIZED -> throw UiCalendarFailure(CalendarAgendaFailure.MOCK_ONLY_REQUIRED)
            CalendarAuthorizationState.DENIED, CalendarAuthorizationState.REVOKED ->
                throw UiCalendarFailure(CalendarAgendaFailure.DENIED)
            CalendarAuthorizationState.EXPIRED -> throw UiCalendarFailure(CalendarAgendaFailure.EXPIRED)
            CalendarAuthorizationState.NOT_CONFIGURED,
            CalendarAuthorizationState.NOT_CONNECTED,
            CalendarAuthorizationState.AUTHORIZATION_REQUIRED,
            CalendarAuthorizationState.FEATURE_DISABLED -> throw UiCalendarFailure(CalendarAgendaFailure.PERMISSION_REQUIRED)
            CalendarAuthorizationState.ERROR -> throw UiCalendarFailure(CalendarAgendaFailure.UNAVAILABLE)
        }
        val requiredScopes = buildSet {
            add(CalendarPermissionPolicy.EVENTS_READ_ONLY)
            if (requireCalendarList) add(CalendarPermissionPolicy.CALENDAR_LIST_READ_ONLY)
        }
        if (status.missingScopes.any { it in requiredScopes }) {
            throw UiCalendarFailure(CalendarAgendaFailure.MISSING_SCOPE)
        }
        return status
    }

    private fun replaceRequest(loading: CalendarAgendaUiState.Loading): Long {
        requestGeneration++
        activeJob?.cancel()
        mutableState.value = loading
        return requestGeneration
    }

    private class UiCalendarFailure(val reason: CalendarAgendaFailure) : RuntimeException()

    private fun Throwable.toAgendaFailure(isDetail: Boolean = false): CalendarAgendaFailure = when (this) {
        is UiCalendarFailure -> reason
        is CalendarReadFailure -> when (code) {
            CalendarApiFailureCode.PERMISSION_DENIED -> CalendarAgendaFailure.DENIED
            CalendarApiFailureCode.MISSING_SCOPE -> CalendarAgendaFailure.MISSING_SCOPE
            CalendarApiFailureCode.TIME_ZONE_UNKNOWN -> CalendarAgendaFailure.TIME_ZONE_UNKNOWN
            CalendarApiFailureCode.TIMEOUT -> CalendarAgendaFailure.TIMEOUT
            CalendarApiFailureCode.RATE_LIMITED -> CalendarAgendaFailure.RATE_LIMITED
            CalendarApiFailureCode.PARTIAL_FAILURE,
            CalendarApiFailureCode.SYNC_TOKEN_EXPIRED -> CalendarAgendaFailure.PARTIAL_FAILURE
            CalendarApiFailureCode.NOT_FOUND -> if (isDetail) CalendarAgendaFailure.EVENT_UNAVAILABLE else CalendarAgendaFailure.UNAVAILABLE
            CalendarApiFailureCode.TOO_MANY_EVENTS,
            CalendarApiFailureCode.TOO_MANY_PAGES -> CalendarAgendaFailure.TOO_MANY_RESULTS
            CalendarApiFailureCode.INVALID_RESPONSE -> CalendarAgendaFailure.INVALID_RESPONSE
            CalendarApiFailureCode.CANCELLED -> CalendarAgendaFailure.UNAVAILABLE
            CalendarApiFailureCode.UNAVAILABLE -> CalendarAgendaFailure.UNAVAILABLE
        }
        else -> CalendarAgendaFailure.UNAVAILABLE
    }
}

/** Limits a selected-calendar query before page data is read from the underlying port. */
private class CalendarAgendaSelectedReadPort(
    private val delegate: CalendarReadPort,
    private val selected: CalendarDescriptor,
) : CalendarReadPort {
    override suspend fun listCalendars(): List<CalendarDescriptor> = listOf(selected)

    override suspend fun getCalendar(calendarId: app.serea.android.calendar.CalendarId): CalendarDescriptor {
        requireSelected(calendarId)
        return delegate.getCalendar(calendarId)
    }

    override suspend fun getEvent(key: CalendarEventKey): CalendarEvent {
        requireSelected(key.calendarId)
        return delegate.getEvent(key)
    }

    override suspend fun listEventsPage(
        calendarId: app.serea.android.calendar.CalendarId,
        range: CalendarDateRange,
        pageToken: String?,
    ): CalendarEventPage {
        requireSelected(calendarId)
        return delegate.listEventsPage(calendarId, range, pageToken)
    }

    private fun requireSelected(calendarId: app.serea.android.calendar.CalendarId) {
        if (calendarId != selected.id) throw CalendarReadFailure(
            CalendarApiFailureCode.INVALID_RESPONSE,
            "Calendar request escaped the selected calendar.",
        )
    }
}

/** Half-open local-day windows are built with ZoneId rules, including 23/25-hour DST dates. */
internal fun calendarAgendaRange(query: CalendarAgendaQuery, clock: Clock, zone: ZoneId): CalendarDateRange {
    val now = clock.instant()
    return when (query.mode) {
        CalendarAgendaMode.TODAY -> localDayRange(now.atZone(zone).toLocalDate(), zone)
        CalendarAgendaMode.TOMORROW -> localDayRange(now.atZone(zone).toLocalDate().plusDays(1), zone)
        CalendarAgendaMode.DATE -> localDayRange(checkNotNull(query.selectedDate), zone)
        CalendarAgendaMode.UPCOMING -> CalendarDateRange(now, now.plus(Duration.ofDays(30)))
    }
}

internal fun normalizeSpecifiedDateInput(value: String, clock: Clock, zone: ZoneId): String =
    runCatching { LocalDate.parse(value) }.getOrNull()?.toString()
        ?: clock.instant().atZone(zone).toLocalDate().toString()

private fun localDayRange(date: LocalDate, zone: ZoneId) = CalendarDateRange(
    date.atStartOfDay(zone).toInstant(),
    date.plusDays(1).atStartOfDay(zone).toInstant(),
)

private fun isInAgenda(event: CalendarEvent, query: CalendarAgendaQuery, clock: Clock, @Suppress("UNUSED_PARAMETER") displayZone: ZoneId): Boolean =
    when (query.mode) {
        CalendarAgendaMode.UPCOMING -> when (val time = event.time) {
            is EventTime.Timed -> time.start >= clock.instant()
            is EventTime.AllDay -> time.startDate.atStartOfDay(time.timeZone).toInstant() >= clock.instant()
            null -> false
        }
        else -> event.status != EventStatus.CANCELLED && event.time != null
    }

object CalendarAgendaJapaneseText {
    private val dateFormatter = DateTimeFormatter.ofPattern("yyyy年M月d日(E)", Locale.JAPAN)
    private val dateTimeFormatter = DateTimeFormatter.ofPattern("yyyy年M月d日(E) HH:mm XXX", Locale.JAPAN)

    fun safeMessage(failure: CalendarAgendaFailure): String = when (failure) {
        CalendarAgendaFailure.PERMISSION_REQUIRED -> "予定を表示する権限が必要です。"
        CalendarAgendaFailure.MOCK_ONLY_REQUIRED -> "この画面はDEMO / 架空の予定だけを表示します。"
        CalendarAgendaFailure.DENIED -> "予定を表示する権限がありません。"
        CalendarAgendaFailure.EXPIRED -> "予定を表示する権限の有効期限が切れています。"
        CalendarAgendaFailure.MISSING_SCOPE -> "予定の閲覧に必要な権限が不足しています。"
        CalendarAgendaFailure.TIME_ZONE_UNKNOWN -> "予定のタイムゾーンを確認できないため表示できません。"
        CalendarAgendaFailure.TIMEOUT -> "読み込みがタイムアウトしました。再試行してください。"
        CalendarAgendaFailure.RATE_LIMITED -> "読み込みが一時的に制限されています。しばらくしてから再試行してください。"
        CalendarAgendaFailure.PARTIAL_FAILURE -> "予定をすべて読み込めませんでした。再試行してください。"
        CalendarAgendaFailure.EVENT_UNAVAILABLE -> "予定を取得できません。削除または取消済みの可能性があります。"
        CalendarAgendaFailure.INVALID_RESPONSE -> "予定の応答を確認できませんでした。再試行してください。"
        CalendarAgendaFailure.TOO_MANY_RESULTS -> "予定が多いため表示できません。期間を短くして再試行してください。"
        CalendarAgendaFailure.UNAVAILABLE -> "予定を読み込めませんでした。再試行してください。"
    }

    fun date(date: LocalDate): String = date.format(dateFormatter)

    fun agendaHeading(query: CalendarAgendaQuery, clock: Clock, zone: ZoneId): String = when (query.mode) {
        CalendarAgendaMode.TODAY -> "今日の予定 · ${date(clock.instant().atZone(zone).toLocalDate())}"
        CalendarAgendaMode.TOMORROW -> "明日の予定 · ${date(clock.instant().atZone(zone).toLocalDate().plusDays(1))}"
        CalendarAgendaMode.UPCOMING -> "今後の予定"
        CalendarAgendaMode.DATE -> "${date(checkNotNull(query.selectedDate))}の予定"
    }

    fun eventTime(event: CalendarEvent, displayZone: ZoneId): String = when (val time = event.time) {
        is EventTime.AllDay -> {
            val lastInclusiveDate = time.endDateExclusive.minusDays(1)
            if (lastInclusiveDate == time.startDate) "終日 · ${date(time.startDate)}"
            else "終日 · ${date(time.startDate)} ～ ${date(lastInclusiveDate)}"
        }
        is EventTime.Timed -> "${time.start.atZone(displayZone).format(dateTimeFormatter)} ～ ${time.end.atZone(displayZone).format(dateTimeFormatter)}"
        null -> "日時を表示できません"
    }

    fun allDayEndBoundary(event: CalendarEvent): String? =
        (event.time as? EventTime.AllDay)?.endDateExclusive?.let(::date)

    fun status(status: EventStatus): String = when (status) {
        EventStatus.CONFIRMED -> "確定"
        EventStatus.TENTATIVE -> "仮の予定"
        EventStatus.CANCELLED -> "取消済み"
        EventStatus.UNKNOWN -> "状態不明"
    }

    fun title(title: String?): String = title?.takeIf { it.isNotBlank() } ?: "（タイトルなし）"

    fun description(value: String?): String = value?.takeIf { it.isNotBlank() } ?: "説明はありません"

    fun location(value: String?): String = value?.takeIf { it.isNotBlank() } ?: "場所はありません"
}
