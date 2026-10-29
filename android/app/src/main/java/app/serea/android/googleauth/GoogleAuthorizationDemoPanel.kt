package app.serea.android.googleauth

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
import androidx.compose.material3.FilterChip
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedButton
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import app.serea.android.calendar.CalendarProviderSession

/** Local walkthrough for consent UX; every action uses a fake gateway and synthetic scope data. */
@Composable
internal fun GoogleAuthorizationDemoPanel(
    viewModel: GoogleAuthorizationDemoViewModel = viewModel(),
    onOpenCalendar: ((CalendarProviderSession) -> Unit)? = null,
) {
    val state by viewModel.state.collectAsState()
    val calendarSession by viewModel.calendarDemoSession.collectAsState()
    val pendingPreview = state.pendingConsentPreview
    val pending = pendingPreview?.feature
    val authorizationBusy = state.inFlightFeature != null || pending != null

    Column(
        modifier = Modifier.fillMaxWidth().testTag("google_auth_demo_section"),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            "Google接続の動作デモ",
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.semantics { heading() }.testTag("google_auth_demo_heading"),
        )
        Text("${state.selectedAccount.displayName} · 実Googleアカウント、OAuth画面、ネットワーク、メールや予定データは使用しません。")
        Text("Calendarの利用目的: 架空の予定一覧と詳細を表示するだけです。予定の作成・変更・削除は行いません。")
        Text(
            "Calendar scope: ${GoogleScope.CALENDAR_EVENTS_READ_ONLY.value}\n" +
                "Calendar一覧を表示する場合: ${GoogleScope.CALENDAR_LIST_READ_ONLY.value}",
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.testTag("google_auth_demo_calendar_scope"),
        )
        Text("合成アカウントを選択", style = MaterialTheme.typography.labelLarge)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            GoogleDemoAccount.entries.forEach { account ->
                FilterChip(
                    selected = state.selectedAccount == account,
                    onClick = { viewModel.selectDemoAccount(account) },
                    label = { Text(account.displayName) },
                    modifier = Modifier.testTag("google_auth_demo_account_${account.name.lowercase()}"),
                )
            }
        }
        Text(
            "Gmail: ${state.gmail.label(GoogleFeature.GMAIL !in state.disabledInAppFeatures)} · " +
                "Calendar: ${state.calendar.label(GoogleFeature.CALENDAR !in state.disabledInAppFeatures)}",
            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }.testTag("google_auth_demo_status"),
        )
        Text(
            state.message,
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }.testTag("google_auth_demo_message"),
        )
        FeatureActions(
            title = "Gmail読み取りデモ",
            feature = GoogleFeature.GMAIL,
            status = state.gmail,
            isEnabledInApp = GoogleFeature.GMAIL !in state.disabledInAppFeatures,
            isAuthorizationBusy = authorizationBusy,
            onRequest = viewModel::requestConsent,
            onDisconnect = viewModel::disconnectInApp,
            onReconnect = viewModel::reconnectInApp,
        )
        if (state.calendar == GoogleDemoFeatureState.AUTHORIZED && calendarSession != null && onOpenCalendar != null) {
            OutlinedButton(
                onClick = { calendarSession?.let { onOpenCalendar(it) } },
                modifier = Modifier.testTag("google_auth_demo_open_calendar"),
            ) { Text("架空の予定を表示") }
        }
        FeatureActions(
            title = "Calendar読み取りデモ",
            feature = GoogleFeature.CALENDAR,
            status = state.calendar,
            isEnabledInApp = GoogleFeature.CALENDAR !in state.disabledInAppFeatures,
            isAuthorizationBusy = authorizationBusy,
            onRequest = viewModel::requestConsent,
            onDisconnect = viewModel::disconnectInApp,
            onReconnect = viewModel::reconnectInApp,
        )
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            OutlinedButton(
                onClick = viewModel::switchDemoAccount,
                modifier = Modifier.weight(1f).testTag("google_auth_demo_switch_account"),
            ) { Text("架空アカウントを切替") }
            OutlinedButton(
                onClick = viewModel::revokeDemoGrant,
                modifier = Modifier.weight(1f).testTag("google_auth_demo_revoke"),
            ) { Text("共有デモ権限を取消") }
        }
        if (state.inFlightFeature != null) {
            OutlinedButton(
                onClick = viewModel::cancelAuthorization,
                modifier = Modifier.testTag("google_auth_demo_cancel_authorization"),
            ) { Text("認可操作を取り消す") }
        }
        OutlinedButton(
            onClick = viewModel::simulateTokenExpiry,
            enabled = state.gmail == GoogleDemoFeatureState.AUTHORIZED ||
                state.calendar == GoogleDemoFeatureState.AUTHORIZED,
            modifier = Modifier.testTag("google_auth_demo_expire"),
        ) { Text("架空の認可状態を期限切れにする") }
    }

    if (pendingPreview != null) {
        val previewFeature = pendingPreview.feature
        AlertDialog(
            onDismissRequest = { viewModel.cancelPendingConsent(pendingPreview) },
            title = { Text("${previewFeature.label()}の認可プレビュー") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text("選択中: ${pendingPreview.selectedAccount.displayName}")
                    if (previewFeature == GoogleFeature.CALENDAR) {
                        Text("目的: Calendarの予定一覧・詳細を表示する")
                        Text("許可する操作: 読み取り専用。予定の作成、変更、削除はしません。")
                        Text("Chatへ渡す場合は別途明示確認が必要です。Cloud送信はこの許可に含まれません。")
                    } else {
                        Text("目的: 合成メールの一覧と詳細を表示する")
                        Text("許可する操作: 読み取り専用。メールの送信、変更、削除はしません。")
                        Text("Chatへ渡す場合は別途明示確認が必要です。Cloud送信はこの許可に含まれません。")
                    }
                    Text("この合成画面で確認する読み取り専用範囲:")
                    pendingPreview.consentScopes.sortedBy { it.value }.forEach { scope ->
                        Text("${scope.label()}: ${scope.value}", style = MaterialTheme.typography.bodySmall)
                    }
                    Text("実Googleには要求されず、実際のメールや予定も読み取りません。")
                }
            },
            confirmButton = {
                TextButton(
                    onClick = { viewModel.grantPendingConsent(pendingPreview) },
                    modifier = Modifier.testTag("google_auth_demo_consent_grant"),
                ) { Text("合成データで許可") }
            },
            dismissButton = {
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    TextButton(
                        onClick = { viewModel.denyPendingConsent(pendingPreview) },
                        modifier = Modifier.testTag("google_auth_demo_consent_deny"),
                    ) { Text("拒否") }
                    TextButton(
                        onClick = { viewModel.cancelPendingConsent(pendingPreview) },
                        modifier = Modifier.testTag("google_auth_demo_consent_cancel"),
                    ) { Text("取消") }
                }
            },
        )
    }
}

@Composable
private fun FeatureActions(
    title: String,
    feature: GoogleFeature,
    status: GoogleDemoFeatureState,
    isEnabledInApp: Boolean,
    isAuthorizationBusy: Boolean,
    onRequest: (GoogleFeature) -> Unit,
    onDisconnect: (GoogleFeature) -> Unit,
    onReconnect: (GoogleFeature) -> Unit,
) {
    Column(verticalArrangement = Arrangement.spacedBy(2.dp)) {
        Text(title, style = MaterialTheme.typography.labelLarge)
        Text("状態: ${status.label(isEnabledInApp)}", style = MaterialTheme.typography.bodySmall)
        Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
            Button(
                onClick = { onRequest(feature) },
                enabled = isEnabledInApp && !isAuthorizationBusy,
                modifier = Modifier.weight(1f).testTag("google_auth_demo_request_${feature.name.lowercase()}"),
            ) {
                Text(
                    if (status == GoogleDemoFeatureState.EXPIRED) "再接続して認可内容を確認" else "認可内容を確認",
                )
            }
            if (!isEnabledInApp) {
                TextButton(
                    onClick = { onReconnect(feature) },
                    modifier = Modifier.testTag("google_auth_demo_reconnect_${feature.name.lowercase()}"),
                ) { Text("アプリ内で再接続") }
            } else {
                TextButton(
                    onClick = { onDisconnect(feature) },
                    modifier = Modifier.testTag("google_auth_demo_disconnect_${feature.name.lowercase()}"),
                ) { Text("アプリ内切断") }
            }
        }
    }
}

internal fun GoogleDemoFeatureState.label(isEnabledInApp: Boolean): String {
    if (!isEnabledInApp) return "アプリ内切断（デモ）"
    return when (this) {
        GoogleDemoFeatureState.DISCONNECTED -> "未接続（デモ）"
        GoogleDemoFeatureState.AUTHORIZATION_REQUIRED -> "承認が必要（デモ）"
        GoogleDemoFeatureState.AUTHORIZING -> "認可中（デモ）"
        GoogleDemoFeatureState.AUTHORIZED -> "承認済み（デモのみ）"
        GoogleDemoFeatureState.EXPIRED -> "期限切れ（デモ）"
        GoogleDemoFeatureState.REVOKED -> "失効（デモ）"
        GoogleDemoFeatureState.ERROR -> "利用不可（デモ）"
    }
}

private fun GoogleFeature.label(): String = when (this) {
    GoogleFeature.GMAIL -> "Gmail"
    GoogleFeature.CALENDAR -> "Calendar"
}

private fun GoogleScope.label(): String = when (this) {
    GoogleScope.GMAIL_READ_ONLY -> "Gmail 読み取り専用"
    GoogleScope.CALENDAR_EVENTS_READ_ONLY -> "Calendar 予定読み取り専用"
    GoogleScope.CALENDAR_LIST_READ_ONLY -> "Calendar 一覧読み取り専用"
}
