package app.serea.android.googleauth

import android.accounts.Account
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import app.serea.android.BuildConfig
import app.serea.android.calendar.CalendarProviderSession
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.receiveAsFlow
import kotlinx.coroutines.launch

internal enum class PrivateGoogleAuthStatus {
    DISCONNECTED,
    ACCOUNT_SELECTION_PENDING,
    ACCOUNT_SELECTED,
    AUTHORIZING,
    CONSENT_PENDING,
    AUTHORIZED,
    CANCELLED,
    REVOKED,
    ERROR,
}

internal enum class SyntheticGoogleAccount(val label: String) {
    A("架空アカウント A"),
    B("架空アカウント B"),
}

internal data class PrivateGoogleConsentPreview(
    val id: Long,
    val generation: Long,
    val scopes: Set<GoogleScope>,
)

internal data class PrivateGoogleAuthorizationUiState(
    val enabledInThisVariant: Boolean,
    val realOAuthEnabled: Boolean,
    val hasSelectedAccount: Boolean = false,
    val syntheticAccount: SyntheticGoogleAccount? = null,
    val status: PrivateGoogleAuthStatus = PrivateGoogleAuthStatus.DISCONNECTED,
    val pendingAccountSelectionId: Long? = null,
    val pendingAccountSelectionGeneration: Long? = null,
    val pendingMockConsent: PrivateGoogleConsentPreview? = null,
    val pendingRealConsentRequestId: String? = null,
    val pendingRealConsentGeneration: Long? = null,
    val message: String,
)

internal data class PendingGoogleConsentActivityResult(
    val requestId: String,
    val generation: Long,
    val pendingIntent: PendingIntent,
)

internal data class PendingGoogleAccountSelection(val requestId: Long, val generation: Long)

/**
 * Private-variant-only authorization UI state. Public variants never construct a real gateway;
 * the normal Private Beta configuration also stays Mock-only unless its explicit local build flag
 * is enabled. Provider composition in this phase is deliberately Fake-only, even after consent.
 */
internal class PrivateGoogleAuthorizationViewModel(
    context: Context?,
    private val enabledInThisVariant: Boolean,
    private val realOAuthEnabled: Boolean,
    private val fakeGatewayFactory: () -> FakeGoogleAuthorizationGateway = { FakeGoogleAuthorizationGateway() },
) : ViewModel() {
    private val appContext = context?.applicationContext
    private val calendarComposition = PrivateGoogleCalendarComposition()
    private var fakeGateway = fakeGatewayFactory()
    private var realGateway: PlayServicesGoogleAuthorizationGateway? = null
    private var generation = 0L
    private var nextPreviewId = 0L
    private var nextAccountSelectionId = 0L
    private val mutableCalendarSession = MutableStateFlow<CalendarProviderSession?>(null)
    val calendarSession: StateFlow<CalendarProviderSession?> = mutableCalendarSession.asStateFlow()
    private val consentLaunches = Channel<PendingGoogleConsentActivityResult>(Channel.BUFFERED)
    val pendingConsentLaunches = consentLaunches.receiveAsFlow()
    private val mutableState = MutableStateFlow(
        PrivateGoogleAuthorizationUiState(
            enabledInThisVariant = enabledInThisVariant,
            realOAuthEnabled = realOAuthEnabled,
            message = if (!enabledInThisVariant) {
                "Owner専用Google UIはこのBuild Variantでは無効です。"
            } else if (realOAuthEnabled) {
                "Ownerの明示操作時だけ認証を開始します。Google通信ProviderはMockのままです。"
            } else {
                "Mock専用です。実Googleアカウント、OAuth、Token、通信は使いません。"
            },
        ),
    )
    val state: StateFlow<PrivateGoogleAuthorizationUiState> = mutableState.asStateFlow()

    /** Claims one explicit AccountPicker Activity Result operation. */
    fun beginRealAccountSelection(): PendingGoogleAccountSelection? {
        if (!enabledInThisVariant || !realOAuthEnabled ||
            mutableState.value.pendingAccountSelectionId != null ||
            mutableState.value.status == PrivateGoogleAuthStatus.AUTHORIZING ||
            mutableState.value.status == PrivateGoogleAuthStatus.CONSENT_PENDING
        ) return null
        generation++
        closeCalendarSession()
        cancelPendingRealConsent()
        realGateway?.disableInApp(GoogleFeature.CALENDAR)
        realGateway = null
        val request = PendingGoogleAccountSelection(++nextAccountSelectionId, generation)
        mutableState.value = mutableState.value.copy(
            hasSelectedAccount = false,
            syntheticAccount = null,
            status = PrivateGoogleAuthStatus.ACCOUNT_SELECTION_PENDING,
            pendingAccountSelectionId = request.requestId,
            pendingAccountSelectionGeneration = request.generation,
            pendingMockConsent = null,
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
            message = "Android AccountPickerを待っています。認可はまだ開始していません。",
        )
        return request
    }

    /** Called only for the current AccountPicker result; the account name is never copied to UI state. */
    fun onRealAccountSelected(requestId: Long, requestGeneration: Long, account: Account?) {
        if (!enabledInThisVariant || !realOAuthEnabled ||
            mutableState.value.pendingAccountSelectionId != requestId ||
            mutableState.value.pendingAccountSelectionGeneration != requestGeneration ||
            generation != requestGeneration
        ) return
        mutableState.value = mutableState.value.copy(
            pendingAccountSelectionId = null,
            pendingAccountSelectionGeneration = null,
        )
        if (account == null || account.type != GOOGLE_ACCOUNT_TYPE) {
            realGateway = null
            mutableState.value = mutableState.value.copy(
                hasSelectedAccount = false,
                syntheticAccount = null,
                pendingAccountSelectionId = null,
                pendingAccountSelectionGeneration = null,
                pendingMockConsent = null,
                status = PrivateGoogleAuthStatus.CANCELLED,
                message = "アカウント選択は取り消されました。認証は開始していません。",
            )
            return
        }
        val context = appContext ?: run {
            mutableState.value = mutableState.value.copy(
                status = PrivateGoogleAuthStatus.ERROR,
                message = "Application Contextを取得できません。認証を開始していません。",
            )
            return
        }
        realGateway?.disableInApp(GoogleFeature.CALENDAR)
        realGateway = PlayServicesGoogleAuthorizationGateway(
            context = context,
            realOAuthEnabled = true,
            selectedAccount = Account(account.name, GOOGLE_ACCOUNT_TYPE),
        ).also { it.disableInApp(GoogleFeature.GMAIL) }
        mutableState.value = mutableState.value.copy(
            hasSelectedAccount = true,
            syntheticAccount = null,
            pendingAccountSelectionId = null,
            pendingAccountSelectionGeneration = null,
            pendingMockConsent = null,
            status = PrivateGoogleAuthStatus.ACCOUNT_SELECTED,
            message = "アカウントを選択しました。Calendar認可は別の明示操作が必要です。",
        )
    }

    /** Fail closed if Android cannot return the request identity with an AccountPicker result. */
    fun cancelCurrentRealAccountSelection() {
        if (mutableState.value.pendingAccountSelectionId == null) return
        generation++
        mutableState.value = mutableState.value.copy(
            hasSelectedAccount = false,
            pendingAccountSelectionId = null,
            pendingAccountSelectionGeneration = null,
            status = PrivateGoogleAuthStatus.CANCELLED,
            message = "AccountPickerの結果を照合できません。認証は開始していません。",
        )
    }

    /** Mock account selection is local UI state; it contains no account identifier or PII. */
    fun selectSyntheticAccount(account: SyntheticGoogleAccount) {
        if (!enabledInThisVariant || realOAuthEnabled) return
        generation++
        closeCalendarSession()
        cancelPendingRealConsent()
        fakeGateway = fakeGatewayFactory().also { it.disableInApp(GoogleFeature.GMAIL) }
        mutableState.value = mutableState.value.copy(
            hasSelectedAccount = true,
            syntheticAccount = account,
            pendingMockConsent = null,
            status = PrivateGoogleAuthStatus.ACCOUNT_SELECTED,
            message = "${account.label}を選択しました。これは架空アカウントです。",
        )
    }

    /** No account selection or consent is initiated from startup or recomposition. */
    fun requestPrimaryCalendarConsent() {
        if (!enabledInThisVariant || !mutableState.value.hasSelectedAccount) return
        if (mutableState.value.status == PrivateGoogleAuthStatus.AUTHORIZING ||
            mutableState.value.status == PrivateGoogleAuthStatus.CONSENT_PENDING
        ) return
        val currentGeneration = generation
        val requestedScopes = setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)
        // Claim the one authorization slot synchronously before scheduling gateway work.
        mutableState.value = mutableState.value.copy(
            status = PrivateGoogleAuthStatus.AUTHORIZING,
            pendingMockConsent = null,
            message = "Calendar認可を準備しています。",
        )
        viewModelScope.launch {
            if (realOAuthEnabled) {
                val gateway = realGateway ?: run {
                    setFailure("選択済みAccount sessionがありません。", currentGeneration)
                    return@launch
                }
                when (val outcome = gateway.authorize(requestedScopes)) {
                    is AuthorizationOutcome.Authorized -> {
                        outcome.lease.close()
                        markAuthorized(gateway, currentGeneration)
                    }
                    is AuthorizationOutcome.ConsentRequired -> {
                        val requestId = outcome.requestId ?: run {
                            setFailure("認可要求IDがありません。安全のため停止しました。", currentGeneration)
                            return@launch
                        }
                        val pendingIntent = gateway.takePendingConsentIntent(requestId) ?: run {
                            gateway.cancelConsentRequest(requestId)
                            setFailure("認可画面を安全に開始できませんでした。", currentGeneration)
                            return@launch
                        }
                        if (currentGeneration != generation) {
                            gateway.cancelConsentRequest(requestId)
                            return@launch
                        }
                        mutableState.value = mutableState.value.copy(
                            status = PrivateGoogleAuthStatus.CONSENT_PENDING,
                            pendingRealConsentRequestId = requestId,
                            pendingRealConsentGeneration = currentGeneration,
                            message = "Calendar予定の読み取り専用認可を待っています。",
                        )
                        consentLaunches.send(PendingGoogleConsentActivityResult(requestId, currentGeneration, pendingIntent))
                    }
                    is AuthorizationOutcome.Rejected -> setFailure(
                        "Calendar認可を開始できません: ${outcome.failure}", currentGeneration,
                    )
                }
            } else {
                when (val outcome = fakeGateway.authorize(requestedScopes)) {
                    is AuthorizationOutcome.ConsentRequired -> {
                        val preview = PrivateGoogleConsentPreview(
                            id = ++nextPreviewId,
                            generation = currentGeneration,
                            scopes = outcome.missingScopes,
                        )
                        mutableState.value = mutableState.value.copy(
                            status = PrivateGoogleAuthStatus.CONSENT_PENDING,
                            pendingMockConsent = preview,
                            message = "Mock認可内容を確認してください。実Googleへは要求されません。",
                        )
                    }
                    is AuthorizationOutcome.Authorized -> {
                        outcome.lease.close()
                        markAuthorized(fakeGateway, currentGeneration)
                    }
                    is AuthorizationOutcome.Rejected -> setFailure(
                        "Mock認可を開始できません: ${outcome.failure}", currentGeneration,
                    )
                }
            }
        }
    }

    fun approveMockConsent(preview: PrivateGoogleConsentPreview) {
        if (!isCurrent(preview) || realOAuthEnabled) return
        // Consume the exact preview before any coroutine can suspend, preventing duplicate taps.
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            status = PrivateGoogleAuthStatus.AUTHORIZING,
            message = "Mock認可を処理しています。",
        )
        fakeGateway.grant(*preview.scopes.toTypedArray())
        viewModelScope.launch {
            when (val outcome = fakeGateway.authorize(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY))) {
                is AuthorizationOutcome.Authorized -> {
                    outcome.lease.close()
                    markAuthorized(fakeGateway, preview.generation)
                }
                is AuthorizationOutcome.ConsentRequired -> setFailure("Mock Scopeが不足しています。", preview.generation)
                is AuthorizationOutcome.Rejected -> setFailure("Mock認可を拒否しました: ${outcome.failure}", preview.generation)
            }
        }
    }

    fun denyMockConsent(preview: PrivateGoogleConsentPreview) {
        if (!isCurrent(preview) || realOAuthEnabled) return
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            status = PrivateGoogleAuthStatus.AUTHORIZING,
            message = "Mock認可の拒否を処理しています。",
        )
        fakeGateway.denyNext(AuthorizationFailure.DENIED)
        viewModelScope.launch {
            val outcome = fakeGateway.authorize(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY))
            if (outcome is AuthorizationOutcome.Authorized) outcome.lease.close()
            if (preview.generation == generation) {
                mutableState.value = mutableState.value.copy(
                    pendingMockConsent = null,
                    status = PrivateGoogleAuthStatus.ERROR,
                    message = "Mock認可を拒否しました。ProviderはFakeのままです。",
                )
            }
        }
    }

    fun cancelMockConsent(preview: PrivateGoogleConsentPreview) {
        if (!isCurrent(preview) || realOAuthEnabled) return
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            status = PrivateGoogleAuthStatus.CANCELLED,
            message = "Mock認可を取り消しました。権限は追加されていません。",
        )
    }

    /** Activity Result callbacks are bound to both request ID and selected-account generation. */
    fun completeRealConsentResult(requestId: String, requestGeneration: Long, resultCode: Int, data: Intent?) {
        val gateway = realGateway ?: return
        if (!enabledInThisVariant || !realOAuthEnabled || requestGeneration != generation ||
            mutableState.value.pendingRealConsentRequestId != requestId ||
            mutableState.value.pendingRealConsentGeneration != requestGeneration ||
            mutableState.value.status != PrivateGoogleAuthStatus.CONSENT_PENDING
        ) {
            gateway.cancelConsentRequest(requestId)
            return
        }
        mutableState.value = mutableState.value.copy(
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
        )
        if (resultCode != android.app.Activity.RESULT_OK) {
            gateway.cancelConsentRequest(requestId)
            mutableState.value = mutableState.value.copy(
                status = PrivateGoogleAuthStatus.CANCELLED,
                message = "認可画面を取り消しました。Calendar ProviderはFakeのままです。",
            )
            return
        }
        val outcome = gateway.completeConsentResult(requestId, data)
        when (outcome) {
            is AuthorizationOutcome.Authorized -> {
                outcome.lease.close()
                markAuthorized(gateway, requestGeneration)
            }
            is AuthorizationOutcome.ConsentRequired -> setFailure("Calendarの読み取りScopeが承認されませんでした。", requestGeneration)
            is AuthorizationOutcome.Rejected -> setFailure("Calendar認可結果を拒否しました: ${outcome.failure}", requestGeneration)
        }
    }

    /** Opens only a synthetic Calendar session. Real HTTP stays disabled in this phase. */
    fun openFakePrimaryCalendarPreview(): CalendarProviderSession? {
        if (mutableState.value.status != PrivateGoogleAuthStatus.AUTHORIZED) return null
        val gateway: GoogleAuthorizationGateway = if (realOAuthEnabled) realGateway ?: return null else fakeGateway
        closeCalendarSession()
        return calendarComposition.openAuthorizedPreview(gateway).also {
            mutableCalendarSession.value = it
        }
    }

    fun disconnectCalendarInApp() {
        generation++
        if (realOAuthEnabled) realGateway?.disableInApp(GoogleFeature.CALENDAR)
        else fakeGateway.disableInApp(GoogleFeature.CALENDAR)
        closeCalendarSession()
        cancelPendingRealConsent()
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            status = PrivateGoogleAuthStatus.REVOKED,
            message = "Calendarはアプリ内で切断されました。Google共有Grantは変更していません。ProviderはFakeです。",
        )
    }

    private fun markAuthorized(gateway: GoogleAuthorizationGateway, requestGeneration: Long) {
        if (requestGeneration != generation) return
        // Provider session is deliberately not opened until the user asks for the Mock preview.
        val scopes = setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)
        val status = gateway.status(scopes)
        if (!status.permits(scopes)) {
            setFailure("必要なCalendar read-only Scopeを確認できません。", requestGeneration)
            return
        }
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
            status = PrivateGoogleAuthStatus.AUTHORIZED,
            message = if (realOAuthEnabled) {
                "OAuth認可状態を確認しました。Google HTTPは無効で、Calendar表示はMock Providerです。"
            } else {
                "Mock認可に成功しました。Calendar表示は架空データです。"
            },
        )
    }

    private fun setFailure(message: String, requestGeneration: Long) {
        if (requestGeneration != generation) return
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
            status = PrivateGoogleAuthStatus.ERROR,
            message = message,
        )
    }

    private fun isCurrent(preview: PrivateGoogleConsentPreview): Boolean =
        enabledInThisVariant && !realOAuthEnabled &&
            mutableState.value.pendingMockConsent == preview && preview.generation == generation

    private fun cancelPendingRealConsent() {
        mutableState.value.pendingRealConsentRequestId?.let { realGateway?.cancelConsentRequest(it) }
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
        )
    }

    private fun closeCalendarSession() {
        calendarComposition.closeCurrent()
        mutableCalendarSession.value = null
    }

    override fun onCleared() {
        closeCalendarSession()
        realGateway?.disableInApp(GoogleFeature.CALENDAR)
        consentLaunches.close()
        calendarComposition.close()
        super.onCleared()
    }

    companion object {
        const val GOOGLE_ACCOUNT_TYPE = "com.google"
    }
}

internal class PrivateGoogleAuthorizationViewModelFactory(
    context: Context,
) : ViewModelProvider.Factory {
    private val appContext = context.applicationContext

    @Suppress("UNCHECKED_CAST")
    override fun <T : ViewModel> create(modelClass: Class<T>): T {
        require(modelClass.isAssignableFrom(PrivateGoogleAuthorizationViewModel::class.java))
        return PrivateGoogleAuthorizationViewModel(
            context = appContext,
            enabledInThisVariant = BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED,
            realOAuthEnabled = BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED,
        ) as T
    }
}
