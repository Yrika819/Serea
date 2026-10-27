package app.serea.android.googleauth

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.Button
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

/** Local walkthrough for consent UX; every action uses a fake gateway and synthetic scope data. */
@Composable
internal fun GoogleAuthorizationDemoPanel(
    viewModel: GoogleAuthorizationDemoViewModel = viewModel(),
) {
    val state by viewModel.state.collectAsState()
    val pending = state.pendingFeature
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
        Text("架空アカウント ${if (state.accountGeneration % 2 == 1) "A" else "B"} · 実Googleアカウント、OAuth画面、ネットワーク、メールや予定データは使用しません。")
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
                onClick = viewModel::cancelPendingConsent,
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

    if (pending != null) {
        AlertDialog(
            onDismissRequest = viewModel::cancelPendingConsent,
            title = { Text("${pending.label()}のデモ承認") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text("この合成画面で確認する読み取り専用範囲:")
                    state.pendingScopes.sortedBy { it.value }.forEach { scope ->
                        Text(scope.value, style = MaterialTheme.typography.bodySmall)
                    }
                    Text("実Googleには要求されず、実際のメールや予定も読み取りません。")
                }
            },
            confirmButton = {
                TextButton(
                    onClick = viewModel::grantPendingConsent,
                    modifier = Modifier.testTag("google_auth_demo_consent_grant"),
                ) { Text("デモで許可") }
            },
            dismissButton = {
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    TextButton(
                        onClick = viewModel::denyPendingConsent,
                        modifier = Modifier.testTag("google_auth_demo_consent_deny"),
                    ) { Text("拒否") }
                    TextButton(
                        onClick = viewModel::cancelPendingConsent,
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
            ) { Text("デモ承認を要求") }
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
