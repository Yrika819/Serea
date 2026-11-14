package app.serea.android.googleauth

import android.app.PendingIntent
import android.accounts.Account
import android.content.Intent
import androidx.annotation.MainThread
import com.google.android.gms.auth.api.identity.AuthorizationRequest
import com.google.android.gms.auth.api.identity.AuthorizationResult as PlayAuthorizationResult
import com.google.android.gms.auth.api.identity.Identity
import com.google.android.gms.auth.api.identity.RevokeAccessRequest
import com.google.android.gms.common.ConnectionResult
import com.google.android.gms.common.GoogleApiAvailability
import com.google.android.gms.common.api.CommonStatusCodes
import com.google.android.gms.common.api.ApiException
import java.io.IOException
import java.util.concurrent.CancellationException
import java.util.concurrent.CopyOnWriteArrayList
import java.util.concurrent.atomic.AtomicLong
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.Job
import java.security.MessageDigest
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import kotlin.coroutines.resume

/** OAuth scopes this app has approved for read-only Gmail and Calendar access. */
enum class GoogleScope(val value: String) {
    GMAIL_READ_ONLY("https://www.googleapis.com/auth/gmail.readonly"),
    CALENDAR_EVENTS_READ_ONLY("https://www.googleapis.com/auth/calendar.events.readonly"),
    CALENDAR_LIST_READ_ONLY("https://www.googleapis.com/auth/calendar.calendarlist.readonly"),
}

/** Private Primary consent and token requests are bound to this exact least-privilege set. */
internal object PrivatePrimaryCalendarScopeContract {
    val scopes: Set<GoogleScope> = setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY)
    val rawScopes: Set<String> = scopes.mapTo(linkedSetOf()) { it.value }

    fun validateRequest(feature: GoogleFeature, requested: Set<GoogleScope>): AuthorizationFailure? =
        if (feature == GoogleFeature.CALENDAR && requested == scopes) null else AuthorizationFailure.SCOPE_MISMATCH

    fun validateConsentRequest(requested: Set<GoogleScope>): AuthorizationFailure? =
        if (requested == scopes) null else AuthorizationFailure.SCOPE_MISMATCH

    /** Preserve raw SDK values until checked; never map away an unknown or excess grant. */
    fun validateReturnedScopes(requested: Set<GoogleScope>, granted: Set<String>): AuthorizationFailure? = when {
        requested != scopes -> AuthorizationFailure.SCOPE_MISMATCH
        granted.any { it !in rawScopes } -> AuthorizationFailure.SCOPE_MISMATCH
        else -> null
    }

    /** Token-bearing results require the complete requested set after rejecting every extra raw scope. */
    fun validateTokenScopes(requested: Set<GoogleScope>, granted: Set<String>): AuthorizationFailure? =
        validateReturnedScopes(requested, granted)
            ?: if (granted == rawScopes) null else AuthorizationFailure.DENIED
}

/** App-local feature gate. These gates never revoke Google's shared grant. */
enum class GoogleFeature(val scopes: Set<GoogleScope>) {
    GMAIL(setOf(GoogleScope.GMAIL_READ_ONLY)),
    CALENDAR(setOf(GoogleScope.CALENDAR_EVENTS_READ_ONLY, GoogleScope.CALENDAR_LIST_READ_ONLY)),
}

enum class GoogleServiceState { AVAILABLE, MISSING, OUTDATED }

enum class ScopePermission { GRANTED, NEEDS_CONSENT, DENIED, EXPIRED, NOT_REQUESTED }

enum class AuthorizationFailure {
    NOT_CONFIGURED,
    SERVICE_MISSING,
    SERVICE_OUTDATED,
    DENIED,
    ACCOUNT_CHANGED,
    EXPIRED,
    NETWORK,
    CANCELLED,
    UNAVAILABLE,
    REQUEST_IN_PROGRESS,
    STALE_CONSENT_RESULT,
    FEATURE_DISABLED,
    CONSENT_REQUIRED,
    ACCOUNT_REQUIRED,
    SCOPE_MISMATCH,
}

/** The access token lives only in this short-lived in-memory lease. Its string form never reveals it. */
class AccessTokenLease internal constructor(
    token: String,
    val grantedScopes: Set<GoogleScope>,
    val expiresAtElapsedRealtimeMillis: Long?,
    /** Opaque in-memory binding to one selected-account authorization session. */
    val authorizationSessionId: String? = null,
    private val validityCheck: () -> Boolean = { true },
    private val onAuthenticationRejected: (Job?, String) -> Boolean = { _, _ -> false },
) : AutoCloseable {
    private var tokenValue: String? = token

    fun <T> useToken(block: (String) -> T): T {
        ensureValid()
        return block(checkNotNull(tokenValue) { "Authorization lease is no longer valid." })
    }

    /** Recheck the authorization generation after an asynchronous operation without exposing token bytes. */
    internal fun ensureValid() {
        check(validityCheck()) { "Authorization lease is no longer valid for this feature." }
        if (expiresAtElapsedRealtimeMillis?.let { android.os.SystemClock.elapsedRealtime() >= it } == true) {
            close()
            error("Authorization lease has expired.")
        }
        check(tokenValue != null) { "Authorization lease is no longer valid." }
    }

    /** Invalidates the exact credential generation which received HTTP 401, if still current. */
    internal fun markAuthenticationRejected(sourceJob: Job?, scope: String): Boolean =
        onAuthenticationRejected(sourceJob, scope)

    override fun close() { tokenValue = null }

    override fun toString(): String = "AccessTokenLease(scopes=$grantedScopes, token=<redacted>)"
}

/** Shared grant generations and app-local gates used by the real gateway and deterministic tests. */
internal class GoogleGrantLeaseState {
    private val disabledFeatures = mutableSetOf<GoogleFeature>()
    private val featureVersions = GoogleFeature.entries.associateWith { 0L }.toMutableMap()
    private var authorizationOperationInFlight = false
    private val credentialGenerations = GoogleScope.entries.associateWith { 0L }.toMutableMap()
    private val credentialFingerprints = mutableMapOf<GoogleScope, ByteArray>()

    /** Gateway calls this while holding its consent lock. */
    fun tryBeginAuthorizationOperation(): Boolean {
        if (authorizationOperationInFlight) return false
        authorizationOperationInFlight = true
        return true
    }

    /** Gateway calls this while holding its consent lock, after the remote operation is terminal. */
    fun finishAuthorizationOperation() {
        authorizationOperationInFlight = false
    }

    fun isAuthorizationOperationInFlight(): Boolean = authorizationOperationInFlight

    fun disableInApp(feature: GoogleFeature) {
        if (disabledFeatures.add(feature)) featureVersions[feature] = featureVersions.getValue(feature) + 1
    }

    fun enableInApp(feature: GoogleFeature) {
        if (disabledFeatures.remove(feature)) featureVersions[feature] = featureVersions.getValue(feature) + 1
    }

    fun isEnabledInApp(feature: GoogleFeature): Boolean = feature !in disabledFeatures

    fun disabledFeaturesSnapshot(): Set<GoogleFeature> = disabledFeatures.toSet()

    fun leaseVersions(features: Set<GoogleFeature>): Map<GoogleFeature, Long> =
        features.associateWith { featureVersions.getValue(it) }

    fun isLeaseCurrent(versions: Map<GoogleFeature, Long>): Boolean = versions.all { (feature, version) ->
        feature !in disabledFeatures && featureVersions[feature] == version
    }

    fun invalidateFeatureLeases(feature: GoogleFeature) {
        featureVersions[feature] = featureVersions.getValue(feature) + 1
    }

    /** A successful explicit authorize/consent creates a new credential generation. */
    fun advanceLeaseGeneration(features: Set<GoogleFeature>): Map<GoogleFeature, Long> {
        features.forEach(::invalidateFeatureLeases)
        return leaseVersions(features)
    }

    /** Identifies one successful AccessTokenLease issuance independently of feature gating. */
    fun issueCredentialGenerations(
        scopes: Set<GoogleScope>,
        credentialFingerprint: ByteArray? = null,
    ): Map<GoogleScope, Long> = scopes.associateWith { scope ->
        val previous = credentialFingerprints[scope]
        val sameCredential = credentialFingerprint != null && previous != null &&
            MessageDigest.isEqual(previous, credentialFingerprint)
        if (sameCredential) {
            credentialGenerations.getValue(scope)
        } else {
            (credentialGenerations.getValue(scope) + 1L).also { generation ->
                credentialGenerations[scope] = generation
                if (credentialFingerprint == null) credentialFingerprints.remove(scope)
                else credentialFingerprints[scope] = credentialFingerprint.copyOf()
            }
        }
    }

    /** HTTP authentication failures only invalidate the lease generation that made that call. */
    fun invalidateAfterAuthenticationFailure(
        rejectedCredentialGenerations: Map<GoogleScope, Long>,
        rejectedScope: GoogleScope,
        leaseVersions: Map<GoogleFeature, Long>,
        scopes: Set<GoogleScope>,
        knownPermissions: MutableMap<GoogleScope, ScopePermission>,
    ): Boolean {
        if (rejectedCredentialGenerations.isEmpty() ||
            rejectedCredentialGenerations[rejectedScope] == null ||
            credentialGenerations[rejectedScope] != rejectedCredentialGenerations[rejectedScope] ||
            !isLeaseCurrent(leaseVersions)
        ) return false
        scopes.forEach { knownPermissions[it] = ScopePermission.EXPIRED }
        leaseVersions.keys.forEach(::invalidateFeatureLeases)
        rejectedCredentialGenerations.keys.forEach { scope ->
            credentialGenerations[scope] = credentialGenerations.getValue(scope) + 1L
            credentialFingerprints.remove(scope)
        }
        return true
    }

    fun version(feature: GoogleFeature): Long = featureVersions.getValue(feature)

    /** Invalidate local permissions/leases at revoke initiation; preserve app-local disconnects. */
    fun invalidateGrant(knownPermissions: MutableMap<GoogleScope, ScopePermission>) {
        knownPermissions.clear()
        featureVersions.keys.toList().forEach { feature ->
            featureVersions[feature] = featureVersions.getValue(feature) + 1
        }
        credentialGenerations.keys.toList().forEach { scope ->
            credentialGenerations[scope] = credentialGenerations.getValue(scope) + 1L
        }
        credentialFingerprints.clear()
    }
}

/** The same synchronous cache/lease invalidation used by the real gateway at revoke initiation. */
internal fun invalidateGoogleGrantAtRevokeStart(
    grantState: GoogleGrantLeaseState,
    knownPermissions: MutableMap<GoogleScope, ScopePermission>,
) = grantState.invalidateGrant(knownPermissions)

/** Wait for the underlying Play Services task to become terminal even if its caller is cancelled. */
internal suspend fun <T> awaitGoogleIdentityTaskToTerminal(remoteOperation: suspend () -> T): T =
    withContext(NonCancellable) { remoteOperation() }

sealed interface AuthorizationOutcome {
    data class Authorized(val lease: AccessTokenLease) : AuthorizationOutcome
    data class ConsentRequired(
        val missingScopes: Set<GoogleScope>,
        val requestId: String? = null,
    ) : AuthorizationOutcome
    data class Rejected(val failure: AuthorizationFailure) : AuthorizationOutcome
}

/** UI-facing status does not contain a credential or account identifier. */
data class AuthorizationStatus(
    val service: GoogleServiceState,
    val permissions: Map<GoogleScope, ScopePermission>,
    val failure: AuthorizationFailure? = null,
) {
    fun permits(scopes: Set<GoogleScope>): Boolean = service == GoogleServiceState.AVAILABLE &&
        scopes.all { permissions[it] == ScopePermission.GRANTED }
}

/** Distinguishes an app-local Gmail toggle from revoking the shared Google grant. */
sealed interface DisconnectAction {
    data object DisableInApp : DisconnectAction
    data object RevokeGoogleGrant : DisconnectAction
}

/** Only this interface is passed to Gmail/Calendar adapters; it has no persistence API. */
fun interface GoogleAccessTokenSupplier {
    suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult
}

/** Scope status is separate from token access so read ports can reject before network. */
fun interface GoogleAuthorizationStatusPort {
    fun status(scopes: Set<GoogleScope>): AuthorizationStatus
}

enum class GoogleAuthorizationMode { PLAY_SERVICES, MOCK_ONLY }

/** Public/default variants stay OAuth-off even if a caller passes an enabling constructor flag. */
internal fun ownerPrivateGoogleOAuthEnabled(requestedByLocalPrivateBuild: Boolean): Boolean =
    requestedByLocalPrivateBuild && app.serea.android.BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED

interface GoogleAuthorizationGateway : GoogleAccessTokenSupplier, GoogleAuthorizationStatusPort {
    val mode: GoogleAuthorizationMode
    /**
     * Opaque in-memory account-session binding. Real gateways require an explicitly selected
     * Android Account; changing accounts means creating a new gateway and replacing its provider
     * sessions. Implementations must not expose or log the account name through this value.
     */
    val authorizationSessionId: String? get() = null
    /** In-memory generation changed synchronously when the shared Google grant is invalidated. */
    val googleGrantInvalidationGeneration: Long get() = 0L
    /** Feature-local switch; disabling it leaves the shared Google grant untouched. */
    fun isEnabledInApp(feature: GoogleFeature): Boolean = true
    /** Subscribe to synchronous local invalidation when a shared Google grant revoke starts. */
    fun observeGoogleGrantInvalidation(listener: () -> Unit): AutoCloseable = AutoCloseable { }
    /** 401 invalidation cancels sibling reads while preserving the request that received the 401. */
    fun observeGoogleAuthenticationInvalidation(listener: (Job?, Long) -> Unit): AutoCloseable = AutoCloseable { }
}

/** Private UI surface for foreground consent operations; never exposed to Public composition. */
internal interface PrivateGoogleRealAuthorizationGateway : GoogleAuthorizationGateway {
    suspend fun authorize(scopes: Set<GoogleScope>): AuthorizationOutcome
    fun completeConsentResult(requestId: String, data: Intent?): AuthorizationOutcome
    fun takePendingConsentIntent(requestId: String): PendingIntent?
    fun cancelConsentRequest(requestId: String): Boolean
    fun disableInApp(feature: GoogleFeature)
    suspend fun revokeGoogleGrant(): AuthorizationFailure?
}

/** Thread-safe synchronous notifications used to stop provider requests at grant invalidation. */
internal class GoogleGrantInvalidationObservers {
    private val listeners = CopyOnWriteArrayList<() -> Unit>()
    private val authenticationListeners = CopyOnWriteArrayList<(Job?, Long) -> Unit>()

    fun observe(listener: () -> Unit): AutoCloseable {
        listeners += listener
        return AutoCloseable { listeners.remove(listener) }
    }

    fun notifyInvalidationStarted() {
        listeners.forEach { listener -> runCatching(listener) }
    }

    fun observeAuthenticationInvalidation(listener: (Job?, Long) -> Unit): AutoCloseable {
        authenticationListeners += listener
        return AutoCloseable { authenticationListeners.remove(listener) }
    }

    fun notifyAuthenticationInvalidationStarted(sourceJob: Job?, invalidationGeneration: Long) {
        authenticationListeners.forEach { listener -> runCatching { listener(sourceJob, invalidationGeneration) } }
    }
}

sealed interface TokenSupplyResult {
    data class Available(val lease: AccessTokenLease) : TokenSupplyResult
    data class Unavailable(val failure: AuthorizationFailure) : TokenSupplyResult
    data class UserActionRequired(val missingScopes: Set<GoogleScope>) : TokenSupplyResult
}

/** Single-slot consent handoff. IDs bind scopes, launch, cancellation, and Activity Results together. */
internal class ConsentRequestRegistry {
    internal data class Request(
        val id: String,
        val scopes: Set<GoogleScope>,
        val grantedScopes: Set<GoogleScope>,
        var pendingIntent: PendingIntent?,
        var launched: Boolean = false,
        var completing: Boolean = false,
        val featureVersions: Map<GoogleFeature, Long> = emptyMap(),
    )
    private var active: Request? = null
    @Synchronized fun register(request: Request): Boolean {
        if (active != null) return false
        active = request
        return true
    }
    @Synchronized fun peek(): Request? = active
    @Synchronized fun takeIntent(id: String): PendingIntent? {
        val request = active ?: return null
        if (request.id != id || request.launched) return null
        val intent = request.pendingIntent ?: return null
        request.launched = true
        request.pendingIntent = null
        return intent
    }
    @Synchronized fun complete(id: String): Request? {
        val request = active ?: return null
        if (request.id != id || !request.launched || request.completing) return null
        request.completing = true
        return request
    }
    @Synchronized fun finish(id: String) {
        if (active?.id == id) active = null
    }
    @Synchronized fun cancel(id: String): Boolean {
        if (active?.id != id) return false
        active = null
        return true
    }
    @Synchronized fun cancelFeature(feature: GoogleFeature): Boolean {
        val request = active ?: return false
        if (feature !in request.featureVersions) return false
        active = null
        return true
    }
    @Synchronized fun clear() { active = null }
}

/** Replaces stale grants with the exact scope snapshot returned alongside a consent resolution. */
internal fun recordConsentResolution(
    knownPermissions: MutableMap<GoogleScope, ScopePermission>,
    requested: Set<GoogleScope>,
    granted: Set<GoogleScope>,
) {
    requested.forEach { scope ->
        knownPermissions[scope] = if (scope in granted) ScopePermission.GRANTED else ScopePermission.NEEDS_CONSENT
    }
}

internal fun requestedScopesTouchDisabledFeature(
    scopes: Set<GoogleScope>,
    disabledFeatures: Set<GoogleFeature>,
): Boolean = GoogleFeature.entries.any { feature -> feature in disabledFeatures && scopes.any { it in feature.scopes } }

internal fun googleFeaturesForScopes(scopes: Set<GoogleScope>): Set<GoogleFeature> =
    GoogleFeature.entries.filterTo(linkedSetOf()) { feature -> scopes.any { it in feature.scopes } }

/** A delayed authorization result is usable only for the exact enabled-feature generations it started with. */
internal fun googleAuthorizationOperationFailure(
    grantState: GoogleGrantLeaseState,
    scopes: Set<GoogleScope>,
    featureVersions: Map<GoogleFeature, Long>,
): AuthorizationFailure? {
    val requestedFeatures = googleFeaturesForScopes(scopes)
    return when {
        requestedFeatures.isEmpty() || featureVersions.keys != requestedFeatures -> AuthorizationFailure.STALE_CONSENT_RESULT
        requestedFeatures.any { !grantState.isEnabledInApp(it) } -> AuthorizationFailure.FEATURE_DISABLED
        !grantState.isLeaseCurrent(featureVersions) -> AuthorizationFailure.STALE_CONSENT_RESULT
        else -> null
    }
}

/** Minimal SDK result snapshot. Raw scope strings are retained for exact allowlist checks. */
internal data class GoogleAuthorizationResultSnapshot(
    val hasResolution: Boolean,
    val grantedScopes: Set<String>,
    val accessToken: String?,
    val pendingIntent: PendingIntent?,
) {
    /** SDK snapshots can cross diagnostics and assertion boundaries; credentials are never printable. */
    override fun toString(): String =
        "GoogleAuthorizationResultSnapshot(hasResolution=$hasResolution, grantedScopes=$grantedScopes, " +
            "accessToken=<redacted>, pendingIntent=${if (pendingIntent == null) "absent" else "present"})"
}

/** Isolates Play Services calls so deterministic tests never need a real Google account/service. */
internal interface GoogleAuthorizationOperations {
    suspend fun authorize(
        account: Account,
        scopes: Set<GoogleScope>,
        optOutIncludingGrantedScopes: Boolean,
    ): GoogleAuthorizationResultSnapshot

    fun resultFromIntent(data: Intent?): GoogleAuthorizationResultSnapshot

    suspend fun revoke(account: Account, scopes: Set<GoogleScope>)
}

internal class AndroidGoogleAuthorizationOperations(
    private val context: android.content.Context,
) : GoogleAuthorizationOperations {
    override suspend fun authorize(
        account: Account,
        scopes: Set<GoogleScope>,
        optOutIncludingGrantedScopes: Boolean,
    ): GoogleAuthorizationResultSnapshot {
        val request = AuthorizationRequest.builder()
            .setRequestedScopes(scopes.map { com.google.android.gms.common.api.Scope(it.value) })
            .setAccount(account)
            .setOptOutIncludingGrantedScopes(optOutIncludingGrantedScopes)
            .build()
        val result = awaitGoogleIdentityTaskToTerminal {
            awaitIdentityTask { Identity.getAuthorizationClient(context).authorize(request) }
        }
        return result.snapshot()
    }

    override fun resultFromIntent(data: Intent?): GoogleAuthorizationResultSnapshot =
        Identity.getAuthorizationClient(context).getAuthorizationResultFromIntent(data).snapshot()

    override suspend fun revoke(account: Account, scopes: Set<GoogleScope>) {
        val request = RevokeAccessRequest.builder()
            .setAccount(account)
            .setScopes(scopes.map { com.google.android.gms.common.api.Scope(it.value) })
            .build()
        awaitGoogleIdentityTaskToTerminal {
            awaitIdentityTask { Identity.getAuthorizationClient(context).revokeAccess(request) }
        }
    }

    private fun PlayAuthorizationResult.snapshot() = GoogleAuthorizationResultSnapshot(
        hasResolution = hasResolution(),
        grantedScopes = grantedScopes.toSet(),
        accessToken = accessToken,
        pendingIntent = pendingIntent,
    )

    private suspend fun <T> awaitIdentityTask(task: () -> com.google.android.gms.tasks.Task<T>): T =
        suspendCancellableCoroutine { continuation ->
            try {
                task().addOnCompleteListener { completed ->
                    if (!continuation.isActive) return@addOnCompleteListener
                    val exception = completed.exception
                    if (exception != null) continuation.resumeWith(Result.failure(exception))
                    else continuation.resume(completed.result)
                }
            } catch (exception: Exception) {
                if (continuation.isActive) continuation.resumeWith(Result.failure(exception))
            }
        }
}

/**
 * Real adapter around Google Play services AuthorizationClient. It is disabled unless an owner
 * explicitly enables it at construction. No authorization is initiated by application startup.
 */
internal class PlayServicesGoogleAuthorizationGateway(
    private val context: android.content.Context,
    private val realOAuthEnabled: Boolean = false,
    private val selectedAccount: Account? = null,
    private val authorizationOperations: GoogleAuthorizationOperations = AndroidGoogleAuthorizationOperations(context),
    private val serviceStateOverride: (() -> GoogleServiceState)? = null,
    private val elapsedRealtimeMillis: () -> Long = { android.os.SystemClock.elapsedRealtime() },
) : PrivateGoogleRealAuthorizationGateway {
    /** A constructor flag alone cannot enable OAuth in a public/default build variant. */
    internal val realOAuthEnabledForBuild: Boolean = ownerPrivateGoogleOAuthEnabled(realOAuthEnabled)
    override val mode = GoogleAuthorizationMode.PLAY_SERVICES
    override val authorizationSessionId: String? = selectedAccount?.let { java.util.UUID.randomUUID().toString() }
    private val consentLock = Any()
    private val consentRequests = ConsentRequestRegistry()
    private val knownPermissions = mutableMapOf<GoogleScope, ScopePermission>()
    private val grantState = GoogleGrantLeaseState()
    private val googleGrantGeneration = AtomicLong(0)
    private val grantInvalidationObservers = GoogleGrantInvalidationObservers()

    override val googleGrantInvalidationGeneration: Long
        get() = googleGrantGeneration.get()

    override fun observeGoogleGrantInvalidation(listener: () -> Unit): AutoCloseable =
        grantInvalidationObservers.observe(listener)

    override fun observeGoogleAuthenticationInvalidation(listener: (Job?, Long) -> Unit): AutoCloseable =
        grantInvalidationObservers.observeAuthenticationInvalidation(listener)

    override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
        PrivatePrimaryCalendarScopeContract.validateRequest(feature, scopes)?.let { return TokenSupplyResult.Unavailable(it) }
        if (!realOAuthEnabledForBuild) return TokenSupplyResult.Unavailable(AuthorizationFailure.NOT_CONFIGURED)
        if (selectedAccount == null) return TokenSupplyResult.Unavailable(AuthorizationFailure.ACCOUNT_REQUIRED)
        if (synchronized(consentLock) { !grantState.isEnabledInApp(feature) }) return TokenSupplyResult.Unavailable(AuthorizationFailure.FEATURE_DISABLED)
        val missing = synchronized(consentLock) {
            if (grantState.isAuthorizationOperationInFlight()) {
                return TokenSupplyResult.Unavailable(AuthorizationFailure.REQUEST_IN_PROGRESS)
            }
            scopes.filter { knownPermissions[it] != ScopePermission.GRANTED }.toSet()
        }
        if (missing.isNotEmpty()) return TokenSupplyResult.UserActionRequired(missing)
        if (consentRequests.peek() != null) return TokenSupplyResult.Unavailable(AuthorizationFailure.REQUEST_IN_PROGRESS)
        when (serviceState()) {
            GoogleServiceState.MISSING -> return TokenSupplyResult.Unavailable(AuthorizationFailure.SERVICE_MISSING)
            GoogleServiceState.OUTDATED -> return TokenSupplyResult.Unavailable(AuthorizationFailure.SERVICE_OUTDATED)
            GoogleServiceState.AVAILABLE -> Unit
        }
        return when (val outcome = requestAuthorization(scopes, allowConsent = false)) {
            is AuthorizationOutcome.Authorized -> TokenSupplyResult.Available(outcome.lease)
            is AuthorizationOutcome.ConsentRequired -> TokenSupplyResult.UserActionRequired(outcome.missingScopes)
            is AuthorizationOutcome.Rejected -> TokenSupplyResult.Unavailable(outcome.failure)
        }
    }

    /** Call from a user initiated foreground action. ConsentRequired must be launched by the UI. */
    override suspend fun authorize(scopes: Set<GoogleScope>): AuthorizationOutcome {
        PrivatePrimaryCalendarScopeContract.validateConsentRequest(scopes)?.let { return AuthorizationOutcome.Rejected(it) }
        if (!realOAuthEnabledForBuild) return AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED)
        if (selectedAccount == null) return AuthorizationOutcome.Rejected(AuthorizationFailure.ACCOUNT_REQUIRED)
        synchronized(consentLock) {
            if (requestedScopesTouchDisabledFeature(scopes, grantState.disabledFeaturesSnapshot())) {
                return AuthorizationOutcome.Rejected(AuthorizationFailure.FEATURE_DISABLED)
            }
            if (consentRequests.peek() != null) return AuthorizationOutcome.Rejected(AuthorizationFailure.REQUEST_IN_PROGRESS)
        }
        when (serviceState()) {
            GoogleServiceState.MISSING -> return AuthorizationOutcome.Rejected(AuthorizationFailure.SERVICE_MISSING)
            GoogleServiceState.OUTDATED -> return AuthorizationOutcome.Rejected(AuthorizationFailure.SERVICE_OUTDATED)
            GoogleServiceState.AVAILABLE -> Unit
        }
        return requestAuthorization(scopes, allowConsent = true)
    }

    private suspend fun requestAuthorization(scopes: Set<GoogleScope>, allowConsent: Boolean): AuthorizationOutcome {
        val featureVersions = synchronized(consentLock) {
            if (requestedScopesTouchDisabledFeature(scopes, grantState.disabledFeaturesSnapshot())) {
                return AuthorizationOutcome.Rejected(AuthorizationFailure.FEATURE_DISABLED)
            }
            if (consentRequests.peek() != null || !grantState.tryBeginAuthorizationOperation()) {
                return AuthorizationOutcome.Rejected(AuthorizationFailure.REQUEST_IN_PROGRESS)
            }
            grantState.leaseVersions(googleFeaturesForScopes(scopes))
        }
        return try {
            val result = authorizationOperations.authorize(
                account = checkNotNull(selectedAccount),
                scopes = scopes,
                optOutIncludingGrantedScopes = true,
            )
            currentCoroutineContext().ensureActive()
            mapResult(
                result,
                scopes,
                allowConsent,
                featureVersions,
                advanceGenerationOnSuccess = allowConsent,
            )
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (exception: Exception) {
            currentCoroutineContext().ensureActive()
            synchronized(consentLock) {
                val operationFailure = googleAuthorizationOperationFailure(grantState, scopes, featureVersions)
                val outcome = AuthorizationOutcome.Rejected(operationFailure ?: classify(exception))
                if (operationFailure == null) cacheOutcome(scopes, outcome)
                outcome
            }
        } finally {
            synchronized(consentLock) { grantState.finishAuthorizationOperation() }
        }
    }

    /** Complete a foreground consent flow using the Activity Result Intent. */
    @MainThread
    override fun completeConsentResult(requestId: String, data: Intent?): AuthorizationOutcome {
        if (!realOAuthEnabledForBuild) return AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED)
        if (selectedAccount == null) return AuthorizationOutcome.Rejected(AuthorizationFailure.ACCOUNT_REQUIRED)
        val pending = synchronized(consentLock) {
            consentRequests.complete(requestId)
                ?: return AuthorizationOutcome.Rejected(AuthorizationFailure.STALE_CONSENT_RESULT)
        }
        return try {
            if (PrivatePrimaryCalendarScopeContract.validateConsentRequest(pending.scopes) != null) {
                return AuthorizationOutcome.Rejected(AuthorizationFailure.SCOPE_MISMATCH)
            }
            val result = authorizationOperations.resultFromIntent(data)
            mapResult(
                result,
                pending.scopes,
                allowConsent = false,
                featureVersions = pending.featureVersions,
                advanceGenerationOnSuccess = true,
            )
        } catch (exception: Exception) {
            synchronized(consentLock) {
                val operationFailure = googleAuthorizationOperationFailure(
                    grantState,
                    pending.scopes,
                    pending.featureVersions,
                )
                val outcome = AuthorizationOutcome.Rejected(operationFailure ?: classify(exception))
                if (operationFailure == null) cacheOutcome(pending.scopes, outcome)
                outcome
            }
        } finally {
            consentRequests.finish(requestId)
        }
    }

    /** Consume the matching one-shot resolution. The request remains active until completion/cancel. */
    override fun takePendingConsentIntent(requestId: String): PendingIntent? = consentRequests.takeIntent(requestId)

    /** Explicitly discard a cancelled/failed foreground consent flow. */
    override fun cancelConsentRequest(requestId: String): Boolean = consentRequests.cancel(requestId)

    override fun disableInApp(feature: GoogleFeature): Unit = synchronized(consentLock) {
        grantState.disableInApp(feature)
        consentRequests.cancelFeature(feature)
        Unit
    }
    fun enableInApp(feature: GoogleFeature) = synchronized(consentLock) { grantState.enableInApp(feature) }
    override fun isEnabledInApp(feature: GoogleFeature): Boolean = synchronized(consentLock) {
        grantState.isEnabledInApp(feature)
    }

    override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
        val service = serviceState()
        if (!realOAuthEnabledForBuild) return AuthorizationStatus(
            service,
            scopes.associateWith { ScopePermission.NOT_REQUESTED },
            AuthorizationFailure.NOT_CONFIGURED,
        )
        if (selectedAccount == null) return AuthorizationStatus(
            service,
            scopes.associateWith { ScopePermission.NOT_REQUESTED },
            AuthorizationFailure.ACCOUNT_REQUIRED,
        )
        if (service != GoogleServiceState.AVAILABLE) return AuthorizationStatus(
            service,
            scopes.associateWith { ScopePermission.NOT_REQUESTED },
            if (service == GoogleServiceState.MISSING) AuthorizationFailure.SERVICE_MISSING else AuthorizationFailure.SERVICE_OUTDATED,
        )
        val pending = consentRequests.peek()
        return AuthorizationStatus(service, scopes.associateWith { scope ->
            when {
                pending != null && scope in pending.scopes && scope !in pending.grantedScopes -> ScopePermission.NEEDS_CONSENT
                // The SDK has no passive grant query; unknown scopes are conservatively actionable.
                else -> synchronized(consentLock) { knownPermissions[scope] ?: ScopePermission.NEEDS_CONSENT }
            }
        })
    }

    /** Revoke the selected account's app authorization for this explicitly supplied scope set. */
    override suspend fun revokeGoogleGrant(): AuthorizationFailure? {
        if (!realOAuthEnabledForBuild) return AuthorizationFailure.NOT_CONFIGURED
        val account = selectedAccount ?: return AuthorizationFailure.ACCOUNT_REQUIRED
        val mayRevoke = synchronized(consentLock) {
            if (!grantState.tryBeginAuthorizationOperation()) false
            else {
                // This is the provider result-delivery linearization point. Advance the shared
                // epoch before lease/cache invalidation, while acquire/status remain excluded by
                // consentLock and before observer dispatch can race a buffered return.
                googleGrantGeneration.incrementAndGet()
                // Fail closed immediately, before the Play Services task can suspend.
                invalidateGoogleGrantAtRevokeStart(grantState, knownPermissions)
                consentRequests.clear()
                true
            }
        }
        if (!mayRevoke) return AuthorizationFailure.REQUEST_IN_PROGRESS
        // Keep cancellation synchronous with local fail-closed invalidation, before waiting for
        // Play Services. An already-dispatched read must not continue consuming the response.
        grantInvalidationObservers.notifyInvalidationStarted()
        return try {
            when (serviceState()) {
                GoogleServiceState.MISSING -> return AuthorizationFailure.SERVICE_MISSING
                GoogleServiceState.OUTDATED -> return AuthorizationFailure.SERVICE_OUTDATED
                GoogleServiceState.AVAILABLE -> Unit
            }
            awaitGoogleIdentityTaskToTerminal {
                authorizationOperations.revoke(account, PrivatePrimaryCalendarScopeContract.scopes)
            }
            currentCoroutineContext().ensureActive()
            null
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (exception: Exception) {
            currentCoroutineContext().ensureActive()
            classify(exception)
        }
        finally { synchronized(consentLock) { grantState.finishAuthorizationOperation() } }
    }

    private fun serviceState(): GoogleServiceState = serviceStateOverride?.invoke() ?: when (
        GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(context)
    ) {
        ConnectionResult.SUCCESS -> GoogleServiceState.AVAILABLE
        ConnectionResult.SERVICE_MISSING -> GoogleServiceState.MISSING
        ConnectionResult.SERVICE_VERSION_UPDATE_REQUIRED -> GoogleServiceState.OUTDATED
        else -> GoogleServiceState.MISSING
    }

    private fun mapResult(
        result: GoogleAuthorizationResultSnapshot,
        requested: Set<GoogleScope>,
        allowConsent: Boolean,
        featureVersions: Map<GoogleFeature, Long>,
        advanceGenerationOnSuccess: Boolean = false,
    ): AuthorizationOutcome {
        var invalidatedForUnsafeScopes = false
        val outcome = synchronized(consentLock) {
            googleAuthorizationOperationFailure(grantState, requested, featureVersions)?.let { failure ->
                return@synchronized AuthorizationOutcome.Rejected(failure)
            }
            PrivatePrimaryCalendarScopeContract.validateReturnedScopes(requested, result.grantedScopes)?.let { failure ->
                invalidateForUnsafeScopeResultLocked()
                invalidatedForUnsafeScopes = true
                return@synchronized AuthorizationOutcome.Rejected(failure)
            }
            if (result.hasResolution) {
                val granted = result.grantedScopes.mapTo(linkedSetOf()) { raw ->
                    GoogleScope.entries.single { it.value == raw }
                }
                if (requested.all { it in granted } || result.pendingIntent == null) {
                    return@synchronized AuthorizationOutcome.Rejected(AuthorizationFailure.UNAVAILABLE)
                }
                if (!allowConsent) {
                    recordConsentResolution(knownPermissions, requested, granted)
                    return@synchronized AuthorizationOutcome.ConsentRequired(requested - granted)
                }
                val id = java.util.UUID.randomUUID().toString()
                val request = ConsentRequestRegistry.Request(
                    id = id,
                    scopes = requested.toSet(),
                    grantedScopes = granted,
                    pendingIntent = result.pendingIntent,
                    featureVersions = featureVersions.toMap(),
                )
                if (!consentRequests.register(request)) {
                    return@synchronized AuthorizationOutcome.Rejected(AuthorizationFailure.REQUEST_IN_PROGRESS)
                }
                recordConsentResolution(knownPermissions, requested, granted)
                return@synchronized AuthorizationOutcome.ConsentRequired(requested - granted, id)
            }
            val granted = result.grantedScopes.mapTo(linkedSetOf()) { raw -> GoogleScope.entries.single { it.value == raw } }
            val token = result.accessToken
            PrivatePrimaryCalendarScopeContract.validateTokenScopes(requested, result.grantedScopes)?.let { failure ->
                invalidateForUnsafeScopeResultLocked()
                invalidatedForUnsafeScopes = true
                return@synchronized AuthorizationOutcome.Rejected(failure)
            }
            if (token.isNullOrBlank()) {
                return@synchronized AuthorizationOutcome.Rejected(AuthorizationFailure.EXPIRED)
                    .also { cacheOutcome(requested, it) }
            }
            val leaseFeatureVersions = if (advanceGenerationOnSuccess) {
                grantState.advanceLeaseGeneration(featureVersions.keys)
            } else featureVersions
            val credentialFingerprint = java.security.MessageDigest.getInstance("SHA-256")
                .digest(token.toByteArray(Charsets.UTF_8))
            val credentialGenerations = grantState.issueCredentialGenerations(requested, credentialFingerprint)
            val lease = AccessTokenLease(
                token,
                granted,
                elapsedRealtimeMillis() + 5 * 60 * 1000L,
                validityCheck = {
                    synchronized(consentLock) { grantState.isLeaseCurrent(leaseFeatureVersions) }
                },
                authorizationSessionId = authorizationSessionId,
                onAuthenticationRejected = { sourceJob, rawScope ->
                    val rejectedScope = GoogleScope.entries.firstOrNull { it.value == rawScope }
                    if (rejectedScope == null) {
                        false
                    } else {
                        val invalidationGeneration = synchronized(consentLock) {
                            if (!grantState.invalidateAfterAuthenticationFailure(
                                    credentialGenerations,
                                    rejectedScope,
                                    leaseFeatureVersions,
                                    requested,
                                    knownPermissions,
                                )
                            ) {
                                null
                            } else {
                                // This generation is also the provider's result-delivery barrier. Advance
                                // it before notifying observers, matching provider-wide revoke ordering.
                                googleGrantGeneration.incrementAndGet()
                            }
                        }
                        if (invalidationGeneration != null) {
                            grantInvalidationObservers.notifyAuthenticationInvalidationStarted(sourceJob, invalidationGeneration)
                        }
                        invalidationGeneration != null
                    }
                },
            )
            AuthorizationOutcome.Authorized(lease).also { cacheOutcome(requested, it) }
        }
        if (invalidatedForUnsafeScopes) grantInvalidationObservers.notifyInvalidationStarted()
        return outcome
    }

    /** Called only while holding consentLock; unsafe scope data invalidates every cached local grant. */
    private fun invalidateForUnsafeScopeResultLocked() {
        googleGrantGeneration.incrementAndGet()
        invalidateGoogleGrantAtRevokeStart(grantState, knownPermissions)
        consentRequests.clear()
    }

    private fun cacheOutcome(scopes: Set<GoogleScope>, outcome: AuthorizationOutcome) = synchronized(consentLock) {
        when (outcome) {
            is AuthorizationOutcome.Authorized -> scopes.forEach { scope ->
                knownPermissions[scope] = if (scope in outcome.lease.grantedScopes) ScopePermission.GRANTED else ScopePermission.DENIED
            }
            is AuthorizationOutcome.ConsentRequired -> outcome.missingScopes.forEach { knownPermissions[it] = ScopePermission.NEEDS_CONSENT }
            is AuthorizationOutcome.Rejected -> when (outcome.failure) {
                AuthorizationFailure.DENIED -> scopes.forEach { knownPermissions[it] = ScopePermission.DENIED }
                AuthorizationFailure.EXPIRED -> scopes.forEach { knownPermissions[it] = ScopePermission.EXPIRED }
                AuthorizationFailure.CONSENT_REQUIRED -> scopes.forEach { knownPermissions[it] = ScopePermission.NEEDS_CONSENT }
                else -> Unit // Network, service, cancellation, and feature-gate failures are not grant changes.
            }
        }
    }

    private fun classify(exception: Exception): AuthorizationFailure = when (exception) {
        is CancellationException -> AuthorizationFailure.CANCELLED
        is IOException -> AuthorizationFailure.NETWORK
        is ApiException -> when (exception.statusCode) {
            ConnectionResult.SERVICE_MISSING -> AuthorizationFailure.SERVICE_MISSING
            ConnectionResult.SERVICE_VERSION_UPDATE_REQUIRED -> AuthorizationFailure.SERVICE_OUTDATED
            ConnectionResult.NETWORK_ERROR -> AuthorizationFailure.NETWORK
            CommonStatusCodes.SIGN_IN_REQUIRED -> AuthorizationFailure.CONSENT_REQUIRED
            CommonStatusCodes.CANCELED -> AuthorizationFailure.CANCELLED
            else -> AuthorizationFailure.UNAVAILABLE
        }
        else -> AuthorizationFailure.UNAVAILABLE
    }
}
