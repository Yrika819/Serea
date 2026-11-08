package app.serea.android.googleauth

import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.result.IntentSenderRequest
import androidx.activity.result.contract.ActivityResultContracts
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
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp

/** Owner-only UI. Default Private Beta uses synthetic Account/consent results and Fake providers. */
@Composable
internal fun PrivateGoogleAuthorizationPanel(
    viewModel: PrivateGoogleAuthorizationViewModel,
    onOpenCalendar: ((app.serea.android.calendar.CalendarProviderSession) -> Unit)? = null,
) {
    val state by viewModel.state.collectAsState()

    val accountPicker = rememberLauncherForActivityResult(PrivateGoogleAccountPickerContract()) { result ->
        val requestId = result.requestId
        val requestGeneration = result.generation
        if (requestId != null && requestGeneration != null) {
            viewModel.onRealAccountSelected(requestId, requestGeneration, result.account)
        } else {
            viewModel.cancelCurrentRealAccountSelection()
        }
    }
    val activeRequestId by rememberUpdatedState(state.pendingRealConsentRequestId)
    val activeRequestGeneration by rememberUpdatedState(state.pendingRealConsentGeneration)
    val consentResult = rememberLauncherForActivityResult(ActivityResultContracts.StartIntentSenderForResult()) { result ->
        val requestId = activeRequestId
        val requestGeneration = activeRequestGeneration
        if (requestId != null && requestGeneration != null) {
            viewModel.completeRealConsentResult(
                requestId = requestId,
                requestGeneration = requestGeneration,
                resultCode = result.resultCode,
                data = result.data,
            )
        }
    }
    LaunchedEffect(viewModel) {
        viewModel.pendingConsentLaunches.collect { request ->
            if (request.generation == viewModel.state.value.pendingRealConsentGeneration &&
                request.requestId == viewModel.state.value.pendingRealConsentRequestId
            ) {
                consentResult.launch(IntentSenderRequest.Builder(request.pendingIntent.intentSender).build())
            }
        }
    }

    Column(
        modifier = Modifier.fillMaxWidth().testTag("private_google_auth_section"),
        verticalArrangement = Arrangement.spacedBy(8.dp),
    ) {
        Text(
            "Owner用Google認証（Private Beta）",
            style = MaterialTheme.typography.titleMedium,
            modifier = Modifier.semantics { heading() }.testTag("private_google_auth_heading"),
        )
        Text(
            "対象: Primary Calendar · 読み取り専用\nScope: ${GoogleScope.CALENDAR_EVENTS_READ_ONLY.value}",
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.testTag("private_google_auth_scope"),
        )
        Text("Gmailは無効です。Calendar一覧Scope、書込み、Cloud送信は有効化しません。")
        Text(
            state.message,
            style = MaterialTheme.typography.bodySmall,
            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }
                .testTag("private_google_auth_message"),
        )
        Text(
            "状態: ${state.status.label()}${if (state.realOAuthEnabled) " · 実OAuth設定あり" else " · Mock only"}",
            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }
                .testTag("private_google_auth_status"),
        )

        if (!state.realOAuthEnabled) {
            Text("架空アカウントを選択", style = MaterialTheme.typography.labelLarge)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                SyntheticGoogleAccount.entries.forEach { account ->
                    FilterChip(
                        selected = state.syntheticAccount == account,
                        onClick = { viewModel.selectSyntheticAccount(account) },
                        label = { Text(account.label) },
                        modifier = Modifier.testTag("private_google_synthetic_account_${account.name.lowercase()}"),
                    )
                }
            }
        } else {
            Button(
                onClick = {
                    viewModel.beginRealAccountSelection()?.let(accountPicker::launch)
                },
                enabled = state.enabledInThisVariant && state.pendingAccountSelectionId == null &&
                    state.status != PrivateGoogleAuthStatus.CONSENT_PENDING &&
                    state.status != PrivateGoogleAuthStatus.AUTHORIZING,
                modifier = Modifier.testTag("private_google_select_account"),
            ) { Text("Googleアカウントを選ぶ") }
        }

        Button(
            onClick = viewModel::requestPrimaryCalendarConsent,
            enabled = state.hasSelectedAccount && state.enabledInThisVariant &&
                state.status != PrivateGoogleAuthStatus.CONSENT_PENDING &&
                state.status != PrivateGoogleAuthStatus.AUTHORIZING,
            modifier = Modifier.testTag("private_google_request_calendar_consent"),
        ) { Text("Calendar読み取り認可を確認") }

        if (state.status == PrivateGoogleAuthStatus.AUTHORIZED && onOpenCalendar != null) {
            OutlinedButton(
                onClick = { viewModel.openFakePrimaryCalendarPreview()?.let(onOpenCalendar) },
                modifier = Modifier.testTag("private_google_open_fake_calendar"),
            ) { Text("架空のPrimary予定を表示") }
        }
        if (state.hasSelectedAccount) {
            TextButton(
                onClick = viewModel::disconnectCalendarInApp,
                modifier = Modifier.testTag("private_google_disconnect_calendar"),
            ) { Text("Calendarをアプリ内で切断") }
        }
    }

    val preview = state.pendingMockConsent
    if (preview != null) {
        AlertDialog(
            onDismissRequest = { viewModel.cancelMockConsent(preview) },
            title = { Text("Calendar認可のMock確認") },
            text = {
                Column(verticalArrangement = Arrangement.spacedBy(6.dp)) {
                    Text("架空アカウントのみを使用します。実Googleには接続しません。")
                    Text("要求範囲: ${preview.scopes.singleOrNull()?.value ?: "不正なScope"}")
                    Text("Calendarイベントの読み取り専用です。書込みとCalendar一覧アクセスは含みません。")
                }
            },
            confirmButton = {
                TextButton(
                    onClick = { viewModel.approveMockConsent(preview) },
                    modifier = Modifier.testTag("private_google_mock_consent_approve"),
                ) { Text("Mockで許可") }
            },
            dismissButton = {
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    TextButton(
                        onClick = { viewModel.denyMockConsent(preview) },
                        modifier = Modifier.testTag("private_google_mock_consent_deny"),
                    ) { Text("拒否") }
                    TextButton(
                        onClick = { viewModel.cancelMockConsent(preview) },
                        modifier = Modifier.testTag("private_google_mock_consent_cancel"),
                    ) { Text("取消") }
                }
            },
        )
    }
}

private fun PrivateGoogleAuthStatus.label(): String = when (this) {
    PrivateGoogleAuthStatus.DISCONNECTED -> "未接続"
    PrivateGoogleAuthStatus.ACCOUNT_SELECTION_PENDING -> "Account選択待ち"
    PrivateGoogleAuthStatus.ACCOUNT_SELECTED -> "Account選択済み・未認可"
    PrivateGoogleAuthStatus.AUTHORIZING -> "認可処理中"
    PrivateGoogleAuthStatus.CONSENT_PENDING -> "認可結果待ち"
    PrivateGoogleAuthStatus.AUTHORIZED -> "認可状態を確認済み"
    PrivateGoogleAuthStatus.CANCELLED -> "取消"
    PrivateGoogleAuthStatus.REVOKED -> "アプリ内切断"
    PrivateGoogleAuthStatus.ERROR -> "利用不可"
}
