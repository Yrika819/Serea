package app.serea.android.googleauth

import androidx.lifecycle.ViewModel
import androidx.lifecycle.viewModelScope
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

internal data class GoogleAuthorizationDemoState(
    val gmail: GoogleDemoFeatureState = GoogleDemoFeatureState.DISCONNECTED,
    val calendar: GoogleDemoFeatureState = GoogleDemoFeatureState.DISCONNECTED,
    val disabledInAppFeatures: Set<GoogleFeature> = emptySet(),
    val pendingFeature: GoogleFeature? = null,
    val pendingScopes: Set<GoogleScope> = emptySet(),
    val message: String = "この画面は架空の認可フローを表示します。実アカウントと通信しません。",
    val accountGeneration: Int = 1,
)

/**
 * Drives a synthetic consent UI only. It owns a FakeGoogleAuthorizationGateway directly and has
 * no Play Services, HTTP, account, token persistence, or real-provider dependency.
 */
internal class GoogleAuthorizationDemoViewModel(
    private val gateway: FakeGoogleAuthorizationGateway = FakeGoogleAuthorizationGateway(),
) : ViewModel() {
    private val mutableState = MutableStateFlow(GoogleAuthorizationDemoState())
    val state: StateFlow<GoogleAuthorizationDemoState> = mutableState.asStateFlow()
    private var requestGeneration = 0L

    fun requestConsent(feature: GoogleFeature) {
        val current = mutableState.value
        if (current.pendingFeature != null) {
            mutableState.value = current.copy(message = "先に表示中のデモ承認を完了または取り消してください。")
            return
        }
        val generation = ++requestGeneration
        setFeatureState(feature, GoogleDemoFeatureState.AUTHORIZING)
        viewModelScope.launch {
            applyOutcome(feature, runCatching { gateway.authorize(feature.scopes) }.getOrElse(::failure), generation)
        }
    }

    fun grantPendingConsent() {
        val feature = mutableState.value.pendingFeature ?: return
        val generation = ++requestGeneration
        setFeatureState(feature, GoogleDemoFeatureState.AUTHORIZING)
        viewModelScope.launch {
            gateway.setExpired(false)
            gateway.grant(*feature.scopes.toTypedArray())
            applyOutcome(feature, runCatching { gateway.authorize(feature.scopes) }.getOrElse(::failure), generation)
        }
    }

    fun denyPendingConsent() {
        val feature = mutableState.value.pendingFeature ?: return
        val generation = ++requestGeneration
        setFeatureState(feature, GoogleDemoFeatureState.AUTHORIZING)
        gateway.denyNext(AuthorizationFailure.DENIED)
        viewModelScope.launch {
            applyOutcome(feature, runCatching { gateway.authorize(feature.scopes) }.getOrElse(::failure), generation)
        }
    }

    fun cancelPendingConsent() {
        requestGeneration++
        val current = mutableState.value
        current.pendingFeature?.let { setFeatureState(it, GoogleDemoFeatureState.AUTHORIZATION_REQUIRED) }
        mutableState.value = mutableState.value.copy(
            pendingFeature = null,
            pendingScopes = emptySet(),
            message = "デモ承認を取り消しました。許可範囲は追加されていません。",
        )
    }

    fun simulateTokenExpiry() {
        requestGeneration++
        gateway.setExpired(true)
        mutableState.value = mutableState.value.copy(
            gmail = GoogleDemoFeatureState.EXPIRED,
            calendar = GoogleDemoFeatureState.EXPIRED,
            pendingFeature = null,
            pendingScopes = emptySet(),
            message = "架空の認可状態を期限切れにしました。読み取りは行われません。",
        )
    }

    fun switchDemoAccount() {
        requestGeneration++
        gateway.switchAccount()
        val current = mutableState.value
        mutableState.value = current.copy(
            gmail = if (current.gmail == GoogleDemoFeatureState.DISCONNECTED) current.gmail else GoogleDemoFeatureState.REVOKED,
            calendar = if (current.calendar == GoogleDemoFeatureState.DISCONNECTED) current.calendar else GoogleDemoFeatureState.REVOKED,
            pendingFeature = null,
            pendingScopes = emptySet(),
            accountGeneration = current.accountGeneration + 1,
            message = "架空アカウントを切り替えました。以前の承認結果は失効しました。",
        )
    }

    /** Disables this feature in the demo while leaving the synthetic shared grant untouched. */
    fun disconnectInApp(feature: GoogleFeature) {
        requestGeneration++
        gateway.disableInApp(feature)
        updateFeature(feature, GoogleDemoFeatureState.DISCONNECTED) { state ->
            state.copy(
                disabledInAppFeatures = state.disabledInAppFeatures + feature,
                pendingFeature = state.pendingFeature.takeUnless { it == feature },
                pendingScopes = if (state.pendingFeature == feature) emptySet() else state.pendingScopes,
                message = "${feature.label()}のアプリ内デモ接続を切りました。共有Grantの再利用はしていません。",
            )
        }
    }

    fun reconnectInApp(feature: GoogleFeature) {
        requestGeneration++
        gateway.enableInApp(feature)
        updateFeature(feature, GoogleDemoFeatureState.AUTHORIZATION_REQUIRED) { state ->
            state.copy(
                disabledInAppFeatures = state.disabledInAppFeatures - feature,
                message = "${feature.label()}を再接続するにはデモ承認を要求してください。",
            )
        }
    }

    fun revokeDemoGrant() {
        requestGeneration++
        gateway.revokeGoogleGrant()
        mutableState.value = mutableState.value.copy(
            gmail = GoogleDemoFeatureState.REVOKED,
            calendar = GoogleDemoFeatureState.REVOKED,
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
                setFeatureState(feature, GoogleDemoFeatureState.AUTHORIZATION_REQUIRED)
                mutableState.value = mutableState.value.copy(
                    pendingFeature = feature,
                    pendingScopes = outcome.missingScopes,
                    message = "要求された範囲を確認してからデモ承認を選んでください。",
                )
            }
            is AuthorizationOutcome.Authorized -> {
                // The UI needs only a status. Do not retain even the synthetic token lease.
                outcome.lease.close()
                updateFeature(feature, GoogleDemoFeatureState.AUTHORIZED) { state ->
                    state.copy(
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

    private fun setFeatureState(feature: GoogleFeature, state: GoogleDemoFeatureState) {
        updateFeature(feature, state) { it }
    }

    private fun failure(error: Throwable): AuthorizationOutcome {
        if (error is CancellationException) throw error
        return AuthorizationOutcome.Rejected(AuthorizationFailure.UNAVAILABLE)
    }
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
