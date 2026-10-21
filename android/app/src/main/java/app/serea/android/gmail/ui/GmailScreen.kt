package app.serea.android.gmail.ui

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.Button
import androidx.compose.material3.Card
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.style.TextOverflow
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import app.serea.android.gmail.GmailMessage
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter
import java.util.Locale

/**
 * Entry point for a host navigation destination. The caller supplies a mock-only
 * [GmailViewModel] and its own Back callback; this composable has no Activity or
 * provider construction dependency.
 */
@Composable
fun GmailScreen(
    viewModel: GmailViewModel,
    onBack: () -> Unit,
    modifier: Modifier = Modifier,
) {
    val state by viewModel.state.collectAsState()
    val route = state.route
    val isRoot = route is GmailRoute.List && route.mode == GmailListMode.INBOX && state.backStack.isEmpty()
    BackHandler(enabled = !isRoot) { viewModel.back() }

    Column(modifier.fillMaxSize().padding(horizontal = 16.dp, vertical = 8.dp)) {
        Row(modifier = Modifier.fillMaxWidth(), verticalAlignment = Alignment.CenterVertically) {
            TextButton(onClick = { if (isRoot) onBack() else viewModel.back() }) {
                Text(if (isRoot) "戻る" else "← 戻る")
            }
            Text(
                "Gmail",
                modifier = Modifier.weight(1f).padding(start = 8.dp),
                style = MaterialTheme.typography.titleLarge,
                fontWeight = FontWeight.SemiBold,
            )
        }

        Surface(
            color = MaterialTheme.colorScheme.secondaryContainer,
            shape = MaterialTheme.shapes.small,
            modifier = Modifier.fillMaxWidth().padding(bottom = 12.dp),
        ) {
            Text(
                "DEMO / 架空のメール — 実アカウントには接続していません",
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 10.dp)
                    .semantics { liveRegion = LiveRegionMode.Polite },
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onSecondaryContainer,
            )
        }

        when (val route = state.route) {
            is GmailRoute.List -> GmailListContent(state, route, viewModel, Modifier.weight(1f))
            is GmailRoute.Message -> GmailMessageContent(
                state = state,
                message = state.detail,
                error = state.detailError,
                onRetry = viewModel::retry,
                onOpenThread = viewModel::openThread,
                modifier = Modifier.weight(1f),
            )
            is GmailRoute.Thread -> GmailThreadContent(state, route, viewModel, Modifier.weight(1f))
        }
    }
}

@Composable
private fun GmailListContent(state: GmailUiState, route: GmailRoute.List, viewModel: GmailViewModel, modifier: Modifier) {
    val keyboardController = LocalSoftwareKeyboardController.current
    val focusManager = LocalFocusManager.current
    val submitSearch = {
        viewModel.submitSearch(state.searchInput)
        keyboardController?.hide()
        focusManager.clearFocus()
    }
    Column(modifier.fillMaxSize()) {
        Row(
            modifier = Modifier.fillMaxWidth(),
            verticalAlignment = Alignment.CenterVertically,
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            OutlinedTextField(
                value = state.searchInput,
                onValueChange = viewModel::updateSearchInput,
                modifier = Modifier.weight(1f).testTag("gmail_search_input"),
                label = { Text("メールを検索") },
                singleLine = true,
                keyboardOptions = KeyboardOptions(imeAction = ImeAction.Search),
                keyboardActions = KeyboardActions(onSearch = { submitSearch() }),
            )
            Button(onClick = { submitSearch() }) { Text("検索") }
        }
        Row(
            modifier = Modifier.fillMaxWidth().padding(top = 4.dp, bottom = 8.dp),
            horizontalArrangement = Arrangement.spacedBy(8.dp),
        ) {
            TextButton(onClick = viewModel::showInbox, enabled = route.mode != GmailListMode.INBOX) { Text("受信箱") }
            if (route.mode == GmailListMode.SEARCH) TextButton(onClick = viewModel::cancelSearch) { Text("検索を取消") }
        }
        Text(
            if (route.mode == GmailListMode.INBOX) "受信箱" else "検索結果",
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.padding(bottom = 2.dp).semantics { heading() }.testTag("gmail_list_heading"),
        )
        if (!state.isLoading && state.listError == null) {
            Text(
                "${state.messages.size} 件",
                style = MaterialTheme.typography.labelLarge,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite },
            )
        }
        when {
            state.isLoading -> Box(Modifier.weight(1f).fillMaxWidth()) {
                LoadingBlock(if (route.mode == GmailListMode.SEARCH) "検索中…" else "読み込み中…")
            }
            state.listError != null && state.messages.isEmpty() -> ErrorBlock(state.listError, viewModel::retry, Modifier.weight(1f))
            state.messages.isEmpty() && route.mode == GmailListMode.INBOX -> EmptyBlock("受信トレイは空です", Modifier.weight(1f))
            state.messages.isEmpty() && state.hasMore -> {
                EmptyBlock("このページにメールはありません。続きを読み込めます。")
                if (state.isLoadingMore) LoadingBlock("続きを読み込み中…")
                else OutlinedButton(onClick = viewModel::loadNextPage) { Text("続きを読み込む") }
            }
            state.messages.isEmpty() -> EmptyBlock("検索結果はありません", Modifier.weight(1f))
            else -> {
                state.listError?.let { InlineError(it, viewModel::retry) }
                LazyColumn(
                    modifier = Modifier.weight(1f).fillMaxWidth(),
                    verticalArrangement = Arrangement.spacedBy(8.dp),
                ) {
                    items(state.messages, key = { it.id.value }) { message ->
                        GmailMessageRow(message) { viewModel.openMessage(message.id) }
                    }
                    if (state.isLoadingMore) item(key = "gmail-page-progress") { LoadingBlock("続きを読み込み中…") }
                    if (state.hasMore) {
                        item(key = "gmail-load-more") {
                            OutlinedButton(
                                onClick = viewModel::loadNextPage,
                                enabled = !state.isLoadingMore && !state.isLoading,
                                modifier = Modifier.fillMaxWidth().padding(vertical = 8.dp),
                            ) { Text("続きを読み込む") }
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun GmailMessageRow(message: GmailMessage, onClick: () -> Unit) {
    Card(onClick = onClick, modifier = Modifier.fillMaxWidth()) {
        Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(4.dp)) {
            Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween, verticalAlignment = Alignment.Top) {
                Text(
                    message.from ?: "差出人不明",
                    modifier = Modifier.weight(1f),
                    style = MaterialTheme.typography.labelLarge,
                    maxLines = 1,
                    overflow = TextOverflow.Ellipsis,
                )
                Text(formatGmailDate(message.internalDateMillis), style = MaterialTheme.typography.labelSmall)
            }
            Text(
                message.subject?.takeIf(String::isNotBlank) ?: "件名なし",
                style = MaterialTheme.typography.titleMedium,
                fontWeight = FontWeight.Medium,
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                modifier = Modifier.testTag("gmail_message_subject"),
            )
            Text(
                message.snippet.orEmpty().ifBlank { "本文プレビューはありません" },
                maxLines = 2,
                overflow = TextOverflow.Ellipsis,
                style = MaterialTheme.typography.bodyMedium,
            )
        }
    }
}

@Composable
private fun GmailMessageContent(
    state: GmailUiState,
    message: GmailMessage?,
    error: GmailUiErrorCode?,
    onRetry: () -> Unit,
    onOpenThread: () -> Unit,
    modifier: Modifier = Modifier,
    threadActionLabel: String = "このスレッドを表示",
    showThreadAction: Boolean = true,
) {
    when {
        state.isLoading && message == null -> Box(modifier.fillMaxSize()) { LoadingBlock("メールを読み込み中…") }
        error != null -> ErrorBlock(error, onRetry, modifier)
        message == null -> ErrorBlock(GmailUiErrorCode.INVALID_DATA, onRetry, modifier)
        else -> {
            val chunks = remember(message.id, message.plainTextBody, message.htmlTextBody) { bodyChunks(message) }
            LazyColumn(modifier = modifier.fillMaxSize(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                item(key = "message-header") {
                    Text(message.subject?.takeIf(String::isNotBlank) ?: "件名なし", style = MaterialTheme.typography.headlineSmall,
                        modifier = Modifier.semantics { heading() })
                    Spacer(Modifier.height(8.dp))
                    MetadataLine("差出人", message.from ?: "不明")
                    MetadataLine("宛先", message.to.takeIf { it.isNotEmpty() }?.joinToString("、") ?: "不明")
                    MetadataLine("日時", formatGmailDate(message.internalDateMillis))
                    if (showThreadAction) {
                        TextButton(onClick = onOpenThread, modifier = Modifier.padding(top = 4.dp)) { Text(threadActionLabel) }
                    }
                    HorizontalDivider()
                    Spacer(Modifier.height(8.dp))
                    Text("本文", style = MaterialTheme.typography.titleMedium, modifier = Modifier.semantics { heading() })
                }
                chunks.forEachIndexed { index, chunk ->
                    item(key = "message-body-$index") {
                        SelectionContainer {
                            Text(chunk, modifier = Modifier.fillMaxWidth().padding(vertical = 2.dp), style = MaterialTheme.typography.bodyLarge)
                        }
                    }
                }
            }
        }
    }
}

@Composable
private fun GmailThreadContent(state: GmailUiState, route: GmailRoute.Thread, viewModel: GmailViewModel, modifier: Modifier) {
    val thread = state.thread
    when {
        state.isLoading && thread == null -> Box(modifier.fillMaxSize()) { LoadingBlock("スレッドを読み込み中…") }
        state.threadError != null -> ErrorBlock(state.threadError, viewModel::retry, modifier)
        thread == null -> ErrorBlock(GmailUiErrorCode.INVALID_DATA, viewModel::retry, modifier)
        route.selectedMessageId != null -> {
            val message = thread.messages.firstOrNull { it.id == route.selectedMessageId }
            if (message == null) {
                ErrorBlock(GmailUiErrorCode.INVALID_DATA, viewModel::showThreadList, modifier)
            } else {
                Column(modifier.fillMaxSize()) {
                    TextButton(onClick = viewModel::showThreadList) { Text("← スレッド一覧") }
                    GmailMessageContent(
                        state = state.copy(isLoading = false),
                        message = message,
                        error = null,
                        onRetry = viewModel::retry,
                        onOpenThread = viewModel::showThreadList,
                        modifier = Modifier.weight(1f),
                        showThreadAction = false,
                    )
                }
            }
        }
        else -> {
            Column(modifier.fillMaxSize()) {
                Text("スレッド内のメール", style = MaterialTheme.typography.titleLarge, modifier = Modifier.semantics { heading() })
                Text("${thread.messages.size} 件", style = MaterialTheme.typography.labelLarge, modifier = Modifier.padding(bottom = 8.dp))
                LazyColumn(modifier = Modifier.weight(1f).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(thread.messages, key = { it.id.value }) { message ->
                        GmailMessageRow(message) { viewModel.selectThreadMessage(message.id) }
                    }
                }
            }
        }
    }
}

@Composable
private fun MetadataLine(label: String, value: String) {
    Column(Modifier.fillMaxWidth().padding(vertical = 2.dp)) {
        Text("$label：", style = MaterialTheme.typography.labelLarge, fontWeight = FontWeight.SemiBold)
        Text(value, modifier = Modifier.fillMaxWidth(), style = MaterialTheme.typography.bodyMedium)
    }
}

@Composable
private fun LoadingBlock(label: String) {
    Column(
        modifier = Modifier.fillMaxWidth().padding(vertical = 32.dp).semantics { liveRegion = LiveRegionMode.Polite },
        horizontalAlignment = Alignment.CenterHorizontally,
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        CircularProgressIndicator()
        Text(label, style = MaterialTheme.typography.bodyLarge)
    }
}

@Composable
private fun EmptyBlock(message: String, modifier: Modifier = Modifier) {
    Column(
        modifier = modifier.fillMaxWidth().verticalScroll(rememberScrollState())
            .semantics { liveRegion = LiveRegionMode.Polite },
    ) {
        Text(message, modifier = Modifier.fillMaxWidth().padding(vertical = 28.dp), style = MaterialTheme.typography.bodyLarge)
    }
}

@Composable
private fun ErrorBlock(code: GmailUiErrorCode, onRetry: () -> Unit, modifier: Modifier = Modifier) {
    Column(
        modifier.fillMaxWidth().verticalScroll(rememberScrollState())
            .semantics { liveRegion = LiveRegionMode.Polite }.padding(vertical = 24.dp),
        verticalArrangement = Arrangement.spacedBy(12.dp),
    ) {
        Text(errorMessage(code), style = MaterialTheme.typography.bodyLarge, color = MaterialTheme.colorScheme.error)
        if (isRetryable(code)) Button(onClick = onRetry) { Text("再試行") }
    }
}

@Composable
private fun InlineError(code: GmailUiErrorCode, onRetry: () -> Unit) {
    Surface(color = MaterialTheme.colorScheme.errorContainer, shape = MaterialTheme.shapes.small, modifier = Modifier.fillMaxWidth()) {
        Row(Modifier.padding(12.dp), verticalAlignment = Alignment.CenterVertically, horizontalArrangement = Arrangement.SpaceBetween) {
            Text(errorMessage(code), modifier = Modifier.weight(1f), color = MaterialTheme.colorScheme.onErrorContainer)
            if (isRetryable(code)) TextButton(onClick = onRetry) { Text("再試行") }
        }
    }
}

internal fun errorMessage(code: GmailUiErrorCode): String = when (code) {
    GmailUiErrorCode.EMPTY_SEARCH -> "検索語を入力してください"
    GmailUiErrorCode.MOCK_PROVIDER_REQUIRED -> "この画面はデモ用の架空メール専用です"
    GmailUiErrorCode.AUTHORIZATION_REQUIRED -> "デモ用メールを読み込めません。状態を確認してください"
    GmailUiErrorCode.AUTHORIZATION_DENIED,
    GmailUiErrorCode.PERMISSION_DENIED -> "メールの閲覧が許可されていません"
    GmailUiErrorCode.AUTHORIZATION_REVOKED -> "閲覧許可が取り消されています"
    GmailUiErrorCode.AUTHORIZATION_EXPIRED -> "閲覧許可の期限が切れています"
    GmailUiErrorCode.MISSING_SCOPE -> "閲覧に必要な許可がありません"
    GmailUiErrorCode.RATE_LIMITED -> "読み込みが集中しています。少し待ってから再試行してください"
    GmailUiErrorCode.TIMEOUT -> "読み込みがタイムアウトしました"
    GmailUiErrorCode.CANCELLED -> "読み込みを取り消しました"
    GmailUiErrorCode.NOT_FOUND -> "メールが見つかりません"
    GmailUiErrorCode.INVALID_DATA -> "メールデータの整合性を確認できませんでした"
    GmailUiErrorCode.TOO_MANY_RESULTS -> "表示できるメール件数の上限に達しました"
    GmailUiErrorCode.UNAVAILABLE -> "メールを読み込めませんでした。再試行してください"
}

private fun isRetryable(code: GmailUiErrorCode): Boolean = code in setOf(
    GmailUiErrorCode.PERMISSION_DENIED,
    GmailUiErrorCode.RATE_LIMITED,
    GmailUiErrorCode.TIMEOUT,
    GmailUiErrorCode.CANCELLED,
    GmailUiErrorCode.UNAVAILABLE,
)

internal fun displayBody(message: GmailMessage): String =
    message.plainTextBody?.takeIf(String::isNotBlank)
        ?: message.htmlTextBody?.takeIf(String::isNotBlank)
        ?: "本文はありません"

internal fun bodyChunks(message: GmailMessage): List<String> {
    val body = displayBody(message)
    if (body.isEmpty()) return listOf("本文はありません")
    val chunks = mutableListOf<String>()
    var start = 0
    while (start < body.length) {
        var end = (start + MAX_BODY_CHUNK_LENGTH).coerceAtMost(body.length)
        if (end < body.length) {
            val newline = body.lastIndexOf('\n', end - 1)
            if (newline >= start + MAX_BODY_CHUNK_LENGTH / 2) end = newline + 1
            if (end < body.length && Character.isHighSurrogate(body[end - 1]) && Character.isLowSurrogate(body[end])) end--
        }
        if (end <= start) end = (start + MAX_BODY_CHUNK_LENGTH).coerceAtMost(body.length)
        chunks += body.substring(start, end)
        start = end
    }
    return chunks
}

internal fun formatGmailDate(epochMillis: Long?): String {
    if (epochMillis == null) return "日時不明"
    return runCatching {
        DATE_FORMATTER.format(Instant.ofEpochMilli(epochMillis).atZone(ZoneId.systemDefault()))
    }.getOrDefault("日時不明")
}

private const val MAX_BODY_CHUNK_LENGTH = 4_000
private val DATE_FORMATTER: DateTimeFormatter = DateTimeFormatter.ofPattern("yyyy/MM/dd HH:mm", Locale.JAPAN)
