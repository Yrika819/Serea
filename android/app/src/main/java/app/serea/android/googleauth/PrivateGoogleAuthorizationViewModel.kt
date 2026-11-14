package app.serea.android.googleauth

import android.accounts.Account
import android.app.PendingIntent
import android.content.Context
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
    REVOKING,
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
    /** Shown only to identify the account in the explicit revoke confirmation; never persisted/logged. */
    val selectedAccountDisplayName: String? = null,
    val revokeConfirmationPending: Boolean = false,
    val syntheticAccount: SyntheticGoogleAccount? = null,
    val status: PrivateGoogleAuthStatus = PrivateGoogleAuthStatus.DISCONNECTED,
    val pendingAccountSelectionId: Long? = null,
    val pendingAccountSelectionGeneration: Long? = null,
    val pendingAccountSelectionToken: String? = null,
    val pendingMockConsent: PrivateGoogleConsentPreview? = null,
    val pendingRealConsentRequestId: String? = null,
    val pendingRealConsentGeneration: Long? = null,
    val pendingRealConsentSessionId: String? = null,
    val message: String,
) {
    /** Account labels, consent/session IDs, and request tokens are never printable diagnostics. */
    override fun toString(): String =
        "PrivateGoogleAuthorizationUiState(enabledInThisVariant=$enabledInThisVariant, " +
            "realOAuthEnabled=$realOAuthEnabled, hasSelectedAccount=$hasSelectedAccount, " +
            "selectedAccountDisplayName=<redacted>, revokeConfirmationPending=$revokeConfirmationPending, " +
            "syntheticAccount=$syntheticAccount, status=$status, pendingIdentity=<redacted>, message=<redacted>)"
}

internal data class PendingGoogleConsentActivityResult(
    val identity: PrivateGoogleConsentIdentity,
    val pendingIntent: PendingIntent,
)

internal data class PendingGoogleAccountSelection(val requestId: Long, val generation: Long, val requestToken: String)

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
    private val realGatewayFactory: (Context?, String) -> PrivateGoogleRealAuthorizationGateway = { context, accountName ->
        PlayServicesGoogleAuthorizationGateway(
            context = requireNotNull(context) { "Application Context is unavailable." },
            realOAuthEnabled = true,
            selectedAccount = Account(accountName, GOOGLE_ACCOUNT_TYPE),
        )
    },
) : ViewModel() {
    private val appContext = context?.applicationContext
    private val calendarComposition = PrivateGoogleCalendarComposition()
    private var fakeGateway = fakeGatewayFactory()
    private var realGateway: PrivateGoogleRealAuthorizationGateway? = null
    private val consentIdentityGate = PrivateGoogleConsentIdentityGate()
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
            mutableState.value.status == PrivateGoogleAuthStatus.CONSENT_PENDING ||
            mutableState.value.status == PrivateGoogleAuthStatus.REVOKING ||
            mutableState.value.revokeConfirmationPending
        ) return null
        generation++
        closeCalendarSession()
        cancelPendingRealConsent()
        realGateway?.disableInApp(GoogleFeature.CALENDAR)
        realGateway = null
        val request = PendingGoogleAccountSelection(
            requestId = ++nextAccountSelectionId,
            generation = generation,
            requestToken = java.util.UUID.randomUUID().toString(),
        )
        mutableState.value = mutableState.value.copy(
            hasSelectedAccount = false,
            selectedAccountDisplayName = null,
            revokeConfirmationPending = false,
            syntheticAccount = null,
            status = PrivateGoogleAuthStatus.ACCOUNT_SELECTION_PENDING,
            pendingAccountSelectionId = request.requestId,
            pendingAccountSelectionGeneration = request.generation,
            pendingAccountSelectionToken = request.requestToken,
            pendingMockConsent = null,
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
            pendingRealConsentSessionId = null,
            message = "Android AccountPickerを待っています。認可はまだ開始していません。",
        )
        return request
    }

    /** The selected account label is exposed only so the explicit revoke dialog names its target. */
    fun onRealAccountSelected(
        requestId: Long,
        requestGeneration: Long,
        requestToken: String?,
        accountName: String?,
        accountType: String?,
    ) {
        if (!enabledInThisVariant || !realOAuthEnabled ||
            mutableState.value.pendingAccountSelectionId != requestId ||
            mutableState.value.pendingAccountSelectionGeneration != requestGeneration ||
            mutableState.value.pendingAccountSelectionToken != requestToken ||
            generation != requestGeneration
        ) return
        mutableState.value = mutableState.value.copy(
            pendingAccountSelectionId = null,
            pendingAccountSelectionGeneration = null,
            pendingAccountSelectionToken = null,
        )
        if (accountName.isNullOrBlank() || accountType != GOOGLE_ACCOUNT_TYPE) {
            realGateway = null
            mutableState.value = mutableState.value.copy(
                hasSelectedAccount = false,
                selectedAccountDisplayName = null,
                syntheticAccount = null,
                pendingAccountSelectionId = null,
                pendingAccountSelectionGeneration = null,
                pendingAccountSelectionToken = null,
                pendingMockConsent = null,
                status = PrivateGoogleAuthStatus.CANCELLED,
                message = "アカウント選択は取り消されました。認証は開始していません。",
            )
            return
        }
        realGateway?.disableInApp(GoogleFeature.CALENDAR)
        realGateway = runCatching { realGatewayFactory(appContext, accountName) }
            .getOrElse {
                mutableState.value = mutableState.value.copy(
                    status = PrivateGoogleAuthStatus.ERROR,
                    message = "認可Sessionを安全に作成できません。認証を開始していません。",
                )
                return
            }.also { it.disableInApp(GoogleFeature.GMAIL) }
        mutableState.value = mutableState.value.copy(
            hasSelectedAccount = true,
            selectedAccountDisplayName = accountName,
            syntheticAccount = null,
            pendingAccountSelectionId = null,
            pendingAccountSelectionGeneration = null,
            pendingAccountSelectionToken = null,
            pendingMockConsent = null,
            status = PrivateGoogleAuthStatus.ACCOUNT_SELECTED,
            message = "アカウントを選択しました。Calendar認可は別の明示操作が必要です。",
        )
    }

    /** Fail closed if Android cannot return the request identity with an AccountPicker result. */
    fun cancelRealAccountSelection(requestId: Long?, requestGeneration: Long?, requestToken: String?): Boolean {
        if (requestId == null || requestGeneration == null || requestToken.isNullOrBlank() ||
            mutableState.value.pendingAccountSelectionId != requestId ||
            mutableState.value.pendingAccountSelectionGeneration != requestGeneration ||
            mutableState.value.pendingAccountSelectionToken != requestToken ||
            generation != requestGeneration
        ) return false
        generation++
        mutableState.value = mutableState.value.copy(
            hasSelectedAccount = false,
            selectedAccountDisplayName = null,
            pendingAccountSelectionId = null,
            pendingAccountSelectionGeneration = null,
            pendingAccountSelectionToken = null,
            status = PrivateGoogleAuthStatus.CANCELLED,
            message = "AccountPickerの結果を照合できません。認証は開始していません。",
        )
        return true
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
        if (mutableState.value.revokeConfirmationPending || mutableState.value.status == PrivateGoogleAuthStatus.REVOKING) return
        if (mutableState.value.status == PrivateGoogleAuthStatus.AUTHORIZING ||
            mutableState.value.status == PrivateGoogleAuthStatus.CONSENT_PENDING
        ) return
        val currentGeneration = generation
        val requestedScopes = PrivatePrimaryCalendarScopeContract.scopes
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
                        val sessionId = gateway.authorizationSessionId
                        if (sessionId.isNullOrBlank()) {
                            gateway.cancelConsentRequest(requestId)
                            setFailure("認可Sessionを照合できません。安全のため停止しました。", currentGeneration)
                            return@launch
                        }
                        val identity = PrivateGoogleConsentIdentity(requestId, currentGeneration, sessionId)
                        if (!consentIdentityGate.begin(identity)) {
                            gateway.cancelConsentRequest(requestId)
                            setFailure("別の認可要求が処理中です。", currentGeneration)
                            return@launch
                        }
                        mutableState.value = mutableState.value.copy(
                            status = PrivateGoogleAuthStatus.CONSENT_PENDING,
                            pendingRealConsentRequestId = requestId,
                            pendingRealConsentGeneration = currentGeneration,
                            pendingRealConsentSessionId = sessionId,
                            message = "Calendar予定の読み取り専用認可を待っています。",
                        )
                        consentLaunches.send(PendingGoogleConsentActivityResult(identity, pendingIntent))
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
            when (val outcome = fakeGateway.authorize(PrivatePrimaryCalendarScopeContract.scopes)) {
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
            val outcome = fakeGateway.authorize(PrivatePrimaryCalendarScopeContract.scopes)
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

    /** Only the one-shot identity captured by the Consent Relay can complete a request. */
    fun completeRealConsentResult(result: PrivateGoogleConsentActivityResult) {
        val identity = result.identity ?: return
        val gateway = realGateway ?: return
        if (!enabledInThisVariant || !realOAuthEnabled || !consentIdentityGate.isPending(identity) ||
            identity.generation != generation ||
            mutableState.value.pendingRealConsentRequestId != identity.requestId ||
            mutableState.value.pendingRealConsentGeneration != identity.generation ||
            mutableState.value.pendingRealConsentSessionId != identity.authorizationSessionId ||
            gateway.authorizationSessionId != identity.authorizationSessionId ||
            mutableState.value.status != PrivateGoogleAuthStatus.CONSENT_PENDING
        ) {
            return
        }
        if (!consentIdentityGate.consume(identity)) return
        mutableState.value = mutableState.value.copy(
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
            pendingRealConsentSessionId = null,
        )
        if (result.resultCode != android.app.Activity.RESULT_OK) {
            gateway.cancelConsentRequest(identity.requestId)
            mutableState.value = mutableState.value.copy(
                status = PrivateGoogleAuthStatus.CANCELLED,
                message = "認可画面を取り消しました。Calendar ProviderはFakeのままです。",
            )
            return
        }
        if (result.resultData == null) {
            gateway.cancelConsentRequest(identity.requestId)
            mutableState.value = mutableState.value.copy(
                status = PrivateGoogleAuthStatus.ERROR,
                message = "認可結果がありません。Calendar ProviderはFakeのままです。",
            )
            return
        }
        val outcome = gateway.completeConsentResult(identity.requestId, result.resultData)
        when (outcome) {
            is AuthorizationOutcome.Authorized -> {
                outcome.lease.close()
                markAuthorized(gateway, identity.generation)
            }
            is AuthorizationOutcome.ConsentRequired -> setFailure("Calendarの読み取りScopeが承認されませんでした。", identity.generation)
            is AuthorizationOutcome.Rejected -> setFailure("Calendar認可結果を拒否しました: ${outcome.failure}", identity.generation)
        }
    }

    /** Fail only the matching pending request if launching the Private relay fails. */
    fun failConsentLaunch(identity: PrivateGoogleConsentIdentity) {
        if (!consentIdentityGate.cancel(identity) || generation != identity.generation ||
            mutableState.value.pendingRealConsentRequestId != identity.requestId ||
            mutableState.value.pendingRealConsentGeneration != identity.generation ||
            mutableState.value.pendingRealConsentSessionId != identity.authorizationSessionId
        ) return
        realGateway?.cancelConsentRequest(identity.requestId)
        mutableState.value = mutableState.value.copy(
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
            pendingRealConsentSessionId = null,
            status = PrivateGoogleAuthStatus.ERROR,
            message = "認可画面を安全に開始できませんでした。",
        )
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
        if (mutableState.value.status == PrivateGoogleAuthStatus.REVOKING) return
        generation++
        if (realOAuthEnabled) realGateway?.disableInApp(GoogleFeature.CALENDAR)
        else fakeGateway.disableInApp(GoogleFeature.CALENDAR)
        closeCalendarSession()
        cancelPendingRealConsent()
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            revokeConfirmationPending = false,
            status = PrivateGoogleAuthStatus.REVOKED,
            message = "Calendarはアプリ内で切断されました。Google共有Grantは変更していません。ProviderはFakeです。",
        )
    }

    /** Opens a confirmation dialog without changing the Google grant or local session. */
    fun requestGoogleGrantRevoke(): Boolean {
        val state = mutableState.value
        if (!enabledInThisVariant || !realOAuthEnabled || !state.hasSelectedAccount || realGateway == null ||
            state.revokeConfirmationPending || state.status in setOf(
                PrivateGoogleAuthStatus.ACCOUNT_SELECTION_PENDING,
                PrivateGoogleAuthStatus.AUTHORIZING,
                PrivateGoogleAuthStatus.CONSENT_PENDING,
                PrivateGoogleAuthStatus.REVOKING,
            )
        ) return false
        mutableState.value = state.copy(revokeConfirmationPending = true)
        return true
    }

    fun cancelGoogleGrantRevoke() {
        if (mutableState.value.revokeConfirmationPending) {
            mutableState.value = mutableState.value.copy(revokeConfirmationPending = false)
        }
    }

    /** Locally invalidates the selected session before starting the account-bound Google revoke. */
    fun confirmGoogleGrantRevoke(): Boolean {
        val state = mutableState.value
        val gateway = realGateway
        if (!state.revokeConfirmationPending || !state.hasSelectedAccount || gateway == null ||
            state.status == PrivateGoogleAuthStatus.REVOKING
        ) return false

        val targetSessionId = gateway.authorizationSessionId ?: return false
        generation++
        val revokeGeneration = generation
        gateway.disableInApp(GoogleFeature.CALENDAR)
        gateway.disableInApp(GoogleFeature.GMAIL)
        closeCalendarSession()
        cancelPendingRealConsent()
        consentIdentityGate.clear()
        mutableState.value = mutableState.value.copy(
            hasSelectedAccount = false,
            revokeConfirmationPending = false,
            status = PrivateGoogleAuthStatus.REVOKING,
            message = "選択中のGoogle Accountの認可を取り消しています。ローカルSessionは停止済みです。",
        )

        viewModelScope.launch {
            val failure = try {
                gateway.revokeGoogleGrant()
            } catch (cancelled: kotlinx.coroutines.CancellationException) {
                AuthorizationFailure.CANCELLED
            } catch (_: Exception) {
                AuthorizationFailure.UNAVAILABLE
            }
            if (generation != revokeGeneration || realGateway !== gateway ||
                gateway.authorizationSessionId != targetSessionId ||
                mutableState.value.status != PrivateGoogleAuthStatus.REVOKING
            ) return@launch

            realGateway = null
            mutableState.value = mutableState.value.copy(
                hasSelectedAccount = false,
                selectedAccountDisplayName = null,
                pendingAccountSelectionId = null,
                pendingAccountSelectionGeneration = null,
                pendingAccountSelectionToken = null,
                pendingRealConsentRequestId = null,
                pendingRealConsentGeneration = null,
                pendingRealConsentSessionId = null,
                status = if (failure == null) PrivateGoogleAuthStatus.REVOKED else PrivateGoogleAuthStatus.ERROR,
                message = if (failure == null) {
                    "Google認可を取り消しました。再利用時は改めて認可が必要です。"
                } else {
                    "ローカル接続は停止しましたが、Google側の取消は確認できませんでした (${failure.name})。自動再試行はしません。"
                },
            )
        }
        return true
    }

    private fun markAuthorized(gateway: GoogleAuthorizationGateway, requestGeneration: Long) {
        if (requestGeneration != generation) return
        // Provider session is deliberately not opened until the user asks for the Mock preview.
        val scopes = PrivatePrimaryCalendarScopeContract.scopes
        val status = gateway.status(scopes)
        if (!status.permits(scopes)) {
            setFailure("必要なCalendar read-only Scopeを確認できません。", requestGeneration)
            return
        }
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
            pendingRealConsentSessionId = null,
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
        cancelPendingRealConsent()
        closeCalendarSession()
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
            pendingRealConsentSessionId = null,
            status = PrivateGoogleAuthStatus.ERROR,
            message = message,
        )
    }

    private fun isCurrent(preview: PrivateGoogleConsentPreview): Boolean =
        enabledInThisVariant && !realOAuthEnabled &&
            mutableState.value.pendingMockConsent == preview && preview.generation == generation

    private fun cancelPendingRealConsent() {
        val state = mutableState.value
        val requestId = state.pendingRealConsentRequestId
        val requestGeneration = state.pendingRealConsentGeneration
        val sessionId = state.pendingRealConsentSessionId
        if (requestId != null && requestGeneration != null && sessionId != null) {
            val identity = PrivateGoogleConsentIdentity(requestId, requestGeneration, sessionId)
            if (consentIdentityGate.cancel(identity)) realGateway?.cancelConsentRequest(identity.requestId)
        }
        mutableState.value = mutableState.value.copy(
            pendingMockConsent = null,
            pendingRealConsentRequestId = null,
            pendingRealConsentGeneration = null,
            pendingRealConsentSessionId = null,
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
