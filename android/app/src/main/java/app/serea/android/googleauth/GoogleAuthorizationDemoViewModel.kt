package app.serea.android.googleauth

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
import app.serea.android.calendar.CalendarProviderSession
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch

/** State shown by the local-only Google authorization walkthrough in Settings. */
internal enum class GoogleDemoFeatureState {
    DISCONNECTED,
    AUTHORIZATION_REQUIRED,
    AUTHORIZING,
    AUTHORIZED,
    EXPIRED,
    REVOKED,
    ERROR,
}

/** Synthetic account choices contain no real account names, email addresses, or identifiers. */
internal enum class GoogleDemoAccount(val displayName: String) {
    ACCOUNT_A("架空アカウント A"),
    ACCOUNT_B("架空アカウント B"),
}

internal data class GoogleAuthorizationDemoState(
    val gmail: GoogleDemoFeatureState = GoogleDemoFeatureState.DISCONNECTED,
    val calendar: GoogleDemoFeatureState = GoogleDemoFeatureState.DISCONNECTED,
    val disabledInAppFeatures: Set<GoogleFeature> = emptySet(),
    val inFlightFeature: GoogleFeature? = null,
    val pendingFeature: GoogleFeature? = null,
    val pendingScopes: Set<GoogleScope> = emptySet(),
    val message: String = "この画面は架空の認可フローを表示します。実アカウントと通信しません。",
    val selectedAccount: GoogleDemoAccount = GoogleDemoAccount.ACCOUNT_A,
    val accountGeneration: Int = 1,
)

/**
 * Drives a synthetic consent UI only. It owns a FakeGoogleAuthorizationGateway directly and has
 * no Play Services, HTTP, account, token persistence, or real-provider dependency.
 */
internal class GoogleAuthorizationDemoViewModel(
    private val gateway: FakeGoogleAuthorizationGateway = FakeGoogleAuthorizationGateway(),
    private val calendarSessionFactory: CalendarDemoSessionFactory = FakeCalendarDemoSessionFactory(),
) : ViewModel() {
    private val mutableState = MutableStateFlow(GoogleAuthorizationDemoState())
    val state: StateFlow<GoogleAuthorizationDemoState> = mutableState.asStateFlow()
    private var activeCalendarSession: CalendarProviderSession? = null
    private val mutableCalendarDemoSession = MutableStateFlow<CalendarProviderSession?>(null)
    /** Exposed only after synthetic Calendar consent; every account/revoke/disconnect closes it. */
    val calendarDemoSession: StateFlow<CalendarProviderSession?> = mutableCalendarDemoSession.asStateFlow()
    private var requestGeneration = 0L
    private val explicitConfirmationRequired = mutableSetOf<GoogleFeature>()

    fun requestConsent(feature: GoogleFeature) {
        if (feature in explicitConfirmationRequired) {
            val current = mutableState.value
            if (feature in current.disabledInAppFeatures) {
                mutableState.value = current.copy(
                    message = "${feature.label()}はアプリ内で切断されています。再接続してからデモ承認を要求してください。",
                )
                return
            }
            if (current.inFlightFeature != null || current.pendingFeature != null) {
                mutableState.value = current.copy(message = "先に表示中のデモ認可操作を完了または取り消してください。")
                return
            }
            requestGeneration++
            mutableState.value = current.withFeatureState(feature, GoogleDemoFeatureState.AUTHORIZATION_REQUIRED).copy(
                pendingFeature = feature,
                pendingScopes = feature.scopes,
                message = "再接続のため、共有デモ権限をこの画面で確認してください。",
            )
            return
        }
        runAuthorization(feature, allowPendingConsent = false) { gateway.authorize(feature.scopes) }
    }

    fun grantPendingConsent() {
        val feature = mutableState.value.pendingFeature ?: return
        runAuthorization(feature, allowPendingConsent = true) {
            gateway.setExpired(false)
            gateway.grant(*feature.scopes.toTypedArray())
            gateway.authorize(feature.scopes)
        }
    }

    fun denyPendingConsent() {
        val feature = mutableState.value.pendingFeature ?: return
        runAuthorization(feature, allowPendingConsent = true) {
            gateway.denyNext(AuthorizationFailure.DENIED)
            gateway.authorize(feature.scopes)
        }
    }

    fun cancelPendingConsent() {
        val current = mutableState.value
        val feature = current.pendingFeature ?: current.inFlightFeature
        requestGeneration++
        val updated = feature?.let { current.withFeatureState(it, GoogleDemoFeatureState.AUTHORIZATION_REQUIRED) } ?: current
        mutableState.value = updated.copy(
            inFlightFeature = null,
            pendingFeature = null,
            pendingScopes = emptySet(),
            message = "デモ認可操作を取り消しました。許可範囲は追加されていません。",
        )
    }

    fun simulateTokenExpiry() {
        requestGeneration++
        invalidateCalendarDemoSession()
        gateway.setExpired(true)
        mutableState.value = mutableState.value.copy(
            gmail = GoogleDemoFeatureState.EXPIRED,
            calendar = GoogleDemoFeatureState.EXPIRED,
            inFlightFeature = null,
            pendingFeature = null,
            pendingScopes = emptySet(),
            message = "架空の認可状態を期限切れにしました。読み取りは行われません。",
        )
    }

    fun selectDemoAccount(account: GoogleDemoAccount) {
        val current = mutableState.value
        if (current.selectedAccount == account) return
        requestGeneration++
        invalidateCalendarDemoSession()
        gateway.selectSyntheticAccount()
        explicitConfirmationRequired.clear()
        mutableState.value = current.copy(
            gmail = if (current.gmail == GoogleDemoFeatureState.DISCONNECTED) current.gmail else GoogleDemoFeatureState.REVOKED,
            calendar = if (current.calendar == GoogleDemoFeatureState.DISCONNECTED) current.calendar else GoogleDemoFeatureState.REVOKED,
            inFlightFeature = null,
            pendingFeature = null,
            pendingScopes = emptySet(),
            selectedAccount = account,
            accountGeneration = current.accountGeneration + 1,
            message = "${account.displayName}を選択しました。以前の承認結果は失効し、引き継がれていません。",
        )
    }

    fun switchDemoAccount() = selectDemoAccount(
        if (mutableState.value.selectedAccount == GoogleDemoAccount.ACCOUNT_A) {
            GoogleDemoAccount.ACCOUNT_B
        } else {
            GoogleDemoAccount.ACCOUNT_A
        },
    )

    /** Disables this feature in the demo while leaving the synthetic shared grant untouched. */
    fun disconnectInApp(feature: GoogleFeature) {
        val current = mutableState.value
        if (current.inFlightFeature == feature || current.pendingFeature == feature) requestGeneration++
        if (feature == GoogleFeature.CALENDAR) invalidateCalendarDemoSession()
        gateway.disableInApp(feature)
        updateFeature(feature, GoogleDemoFeatureState.DISCONNECTED) { state ->
            state.copy(
                disabledInAppFeatures = state.disabledInAppFeatures + feature,
                inFlightFeature = state.inFlightFeature.takeUnless { it == feature },
                pendingFeature = state.pendingFeature.takeUnless { it == feature },
                pendingScopes = if (state.pendingFeature == feature) emptySet() else state.pendingScopes,
                message = "${feature.label()}のアプリ内デモ接続を切りました。共有Grantの再利用はしていません。",
            )
        }
    }

    fun reconnectInApp(feature: GoogleFeature) {
        val current = mutableState.value
        if (current.inFlightFeature == feature || current.pendingFeature == feature) requestGeneration++
        gateway.enableInApp(feature)
        explicitConfirmationRequired += feature
        updateFeature(feature, GoogleDemoFeatureState.AUTHORIZATION_REQUIRED) { state ->
            state.copy(
                disabledInAppFeatures = state.disabledInAppFeatures - feature,
                inFlightFeature = state.inFlightFeature.takeUnless { it == feature },
                pendingFeature = state.pendingFeature.takeUnless { it == feature },
                pendingScopes = if (state.pendingFeature == feature) emptySet() else state.pendingScopes,
                message = "${feature.label()}を再接続するにはデモ承認を要求してください。",
            )
        }
    }

    fun revokeDemoGrant() {
        requestGeneration++
        invalidateCalendarDemoSession()
        gateway.revokeGoogleGrant()
        explicitConfirmationRequired.clear()
        mutableState.value = mutableState.value.copy(
            gmail = GoogleDemoFeatureState.REVOKED,
            calendar = GoogleDemoFeatureState.REVOKED,
            inFlightFeature = null,
            pendingFeature = null,
            pendingScopes = emptySet(),
            message = "架空の共有Grantを取り消しました。実Google権限には影響しません。",
        )
    }

    private fun applyOutcome(feature: GoogleFeature, outcome: AuthorizationOutcome, generation: Long) {
        if (generation != requestGeneration) {
            (outcome as? AuthorizationOutcome.Authorized)?.lease?.close()
            return
        }
        when (outcome) {
            is AuthorizationOutcome.ConsentRequired -> {
                updateFeature(feature, GoogleDemoFeatureState.AUTHORIZATION_REQUIRED) { state -> state.copy(
                    inFlightFeature = null,
                    pendingFeature = feature,
                    pendingScopes = outcome.missingScopes,
                    message = "要求された範囲を確認してからデモ承認を選んでください。",
                ) }
            }
            is AuthorizationOutcome.Authorized -> {
                // The UI needs only a status. Do not retain even the synthetic token lease.
                outcome.lease.close()
                if (feature == GoogleFeature.CALENDAR) {
                    invalidateCalendarDemoSession()
                    activeCalendarSession = runCatching {
                        calendarSessionFactory.openAfterSyntheticConsent(
                            account = mutableState.value.selectedAccount,
                            accountGeneration = mutableState.value.accountGeneration,
                        )
                    }.getOrNull()
                    if (activeCalendarSession == null) {
                        updateFeature(feature, GoogleDemoFeatureState.ERROR) { state ->
                            state.copy(
                                inFlightFeature = null,
                                pendingFeature = null,
                                pendingScopes = emptySet(),
                                message = "Calendarの架空セッションを作成できませんでした。読み取りは無効です。",
                            )
                        }
                        return
                    }
                    mutableCalendarDemoSession.value = activeCalendarSession
                }
                explicitConfirmationRequired -= feature
                updateFeature(feature, GoogleDemoFeatureState.AUTHORIZED) { state ->
                    state.copy(
                        inFlightFeature = null,
                        pendingFeature = null,
                        pendingScopes = emptySet(),
                        message = "${feature.label()}の架空の読み取り専用デモ承認が有効です。実データは取得していません。",
                    )
                }
            }
            is AuthorizationOutcome.Rejected -> {
                val status = outcome.toDemoFeatureState()
                updateFeature(feature, status) { state ->
                    state.copy(
                        inFlightFeature = null,
                        pendingFeature = null,
                        pendingScopes = emptySet(),
                        message = "デモ認可の状態: ${outcome.failure.name}。実Googleには接続していません。",
                    )
                }
            }
        }
    }

    private fun updateFeature(
        feature: GoogleFeature,
        status: GoogleDemoFeatureState,
        transform: (GoogleAuthorizationDemoState) -> GoogleAuthorizationDemoState,
    ) {
        val current = mutableState.value
        val updated = when (feature) {
            GoogleFeature.GMAIL -> current.copy(gmail = status)
            GoogleFeature.CALENDAR -> current.copy(calendar = status)
        }
        mutableState.value = transform(updated)
    }

    private fun runAuthorization(
        feature: GoogleFeature,
        allowPendingConsent: Boolean,
        operation: suspend () -> AuthorizationOutcome,
    ) {
        val generation = beginAuthorization(feature, allowPendingConsent) ?: return
        viewModelScope.launch {
            try {
                // A queued fake operation may be invalidated before this coroutine starts.
                // Check before it can mutate the synthetic gateway (for example grant scopes).
                if (!isCurrentAuthorization(feature, generation)) return@launch
                val outcome = runCatching { operation() }.getOrElse(::failure)
                applyOutcome(feature, outcome, generation)
            } finally {
                finishAbandonedAuthorization(feature, generation)
            }
        }
    }

    private fun beginAuthorization(feature: GoogleFeature, allowPendingConsent: Boolean): Long? {
        val current = mutableState.value
        if (feature in current.disabledInAppFeatures) {
            mutableState.value = current.copy(
                message = "${feature.label()}はアプリ内で切断されています。再接続してからデモ承認を要求してください。",
            )
            return null
        }
        val canStart = current.inFlightFeature == null && if (allowPendingConsent) {
            current.pendingFeature == feature
        } else {
            current.pendingFeature == null
        }
        if (!canStart) {
            mutableState.value = current.copy(message = "先に表示中のデモ認可操作を完了または取り消してください。")
            return null
        }
        val generation = ++requestGeneration
        mutableState.value = current.withFeatureState(feature, GoogleDemoFeatureState.AUTHORIZING).copy(
            inFlightFeature = feature,
            pendingFeature = if (allowPendingConsent) null else current.pendingFeature,
            pendingScopes = if (allowPendingConsent) emptySet() else current.pendingScopes,
        )
        return generation
    }

    private fun isCurrentAuthorization(feature: GoogleFeature, generation: Long): Boolean =
        generation == requestGeneration && mutableState.value.inFlightFeature == feature

    private fun finishAbandonedAuthorization(feature: GoogleFeature, generation: Long) {
        if (!isCurrentAuthorization(feature, generation)) return
        val current = mutableState.value
        mutableState.value = current.withFeatureState(feature, GoogleDemoFeatureState.AUTHORIZATION_REQUIRED).copy(
            inFlightFeature = null,
            pendingFeature = null,
            pendingScopes = emptySet(),
            message = "デモ認可操作は完了しませんでした。再試行するには承認を要求してください。",
        )
    }

    private fun failure(error: Throwable): AuthorizationOutcome {
        if (error is CancellationException) throw error
        return AuthorizationOutcome.Rejected(AuthorizationFailure.UNAVAILABLE)
    }

    private fun invalidateCalendarDemoSession() {
        activeCalendarSession?.close()
        activeCalendarSession = null
        mutableCalendarDemoSession.value = null
    }

    override fun onCleared() {
        invalidateCalendarDemoSession()
        super.onCleared()
    }
}

private fun GoogleAuthorizationDemoState.withFeatureState(
    feature: GoogleFeature,
    featureState: GoogleDemoFeatureState,
): GoogleAuthorizationDemoState = when (feature) {
    GoogleFeature.GMAIL -> copy(gmail = featureState)
    GoogleFeature.CALENDAR -> copy(calendar = featureState)
}

internal fun AuthorizationOutcome.toDemoFeatureState(): GoogleDemoFeatureState = when (this) {
    is AuthorizationOutcome.ConsentRequired -> GoogleDemoFeatureState.AUTHORIZATION_REQUIRED
    is AuthorizationOutcome.Authorized -> GoogleDemoFeatureState.AUTHORIZED
    is AuthorizationOutcome.Rejected -> when (failure) {
        AuthorizationFailure.EXPIRED -> GoogleDemoFeatureState.EXPIRED
        AuthorizationFailure.ACCOUNT_CHANGED,
        AuthorizationFailure.STALE_CONSENT_RESULT -> GoogleDemoFeatureState.REVOKED
        AuthorizationFailure.FEATURE_DISABLED -> GoogleDemoFeatureState.DISCONNECTED
        AuthorizationFailure.CANCELLED -> GoogleDemoFeatureState.AUTHORIZATION_REQUIRED
        else -> GoogleDemoFeatureState.ERROR
    }
}

private fun GoogleFeature.label(): String = when (this) {
    GoogleFeature.GMAIL -> "Gmail"
    GoogleFeature.CALENDAR -> "Calendar"
}
