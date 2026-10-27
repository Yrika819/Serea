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
import kotlinx.coroutines.NonCancellable
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
}

/** The access token lives only in this short-lived in-memory lease. Its string form never reveals it. */
class AccessTokenLease internal constructor(
    token: String,
    val grantedScopes: Set<GoogleScope>,
    val expiresAtElapsedRealtimeMillis: Long?,
    /** Opaque in-memory binding to one selected-account authorization session. */
    val authorizationSessionId: String? = null,
    private val validityCheck: () -> Boolean = { true },
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

    override fun close() { tokenValue = null }

    override fun toString(): String = "AccessTokenLease(scopes=$grantedScopes, token=<redacted>)"
}

/** Shared grant generations and app-local gates used by the real gateway and deterministic tests. */
internal class GoogleGrantLeaseState {
    private val disabledFeatures = mutableSetOf<GoogleFeature>()
    private val featureVersions = GoogleFeature.entries.associateWith { 0L }.toMutableMap()
    private var authorizationOperationInFlight = false

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

    fun version(feature: GoogleFeature): Long = featureVersions.getValue(feature)

    /** Invalidate local permissions/leases at revoke initiation; preserve app-local disconnects. */
    fun invalidateGrant(knownPermissions: MutableMap<GoogleScope, ScopePermission>) {
        knownPermissions.clear()
        featureVersions.keys.toList().forEach { feature ->
            featureVersions[feature] = featureVersions.getValue(feature) + 1
        }
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

interface GoogleAuthorizationGateway : GoogleAccessTokenSupplier, GoogleAuthorizationStatusPort {
    val mode: GoogleAuthorizationMode
    /**
     * Opaque in-memory account-session binding. Real gateways require an explicitly selected
     * Android Account; changing accounts means creating a new gateway and replacing its provider
     * sessions. Implementations must not expose or log the account name through this value.
     */
    val authorizationSessionId: String? get() = null
    /** Feature-local switch; disabling it leaves the shared Google grant untouched. */
    fun isEnabledInApp(feature: GoogleFeature): Boolean = true
    /** Subscribe to synchronous local invalidation when a shared Google grant revoke starts. */
    fun observeGoogleGrantInvalidation(listener: () -> Unit): AutoCloseable = AutoCloseable { }
}

/** Thread-safe synchronous notifications used to stop provider requests at grant invalidation. */
internal class GoogleGrantInvalidationObservers {
    private val listeners = CopyOnWriteArrayList<() -> Unit>()

    fun observe(listener: () -> Unit): AutoCloseable {
        listeners += listener
        return AutoCloseable { listeners.remove(listener) }
    }

    fun notifyInvalidationStarted() {
        listeners.forEach { listener -> runCatching(listener) }
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

/**
 * Real adapter around Google Play services AuthorizationClient. It is disabled unless an owner
 * explicitly enables it at construction. No authorization is initiated by application startup.
 */
class PlayServicesGoogleAuthorizationGateway(
    private val context: android.content.Context,
    private val realOAuthEnabled: Boolean = false,
    private val selectedAccount: Account? = null,
) : GoogleAuthorizationGateway {
    override val mode = GoogleAuthorizationMode.PLAY_SERVICES
    override val authorizationSessionId: String? = selectedAccount?.let { java.util.UUID.randomUUID().toString() }
    private val consentLock = Any()
    private val consentRequests = ConsentRequestRegistry()
    private val knownPermissions = mutableMapOf<GoogleScope, ScopePermission>()
    private val grantState = GoogleGrantLeaseState()
    private val grantInvalidationObservers = GoogleGrantInvalidationObservers()

    override fun observeGoogleGrantInvalidation(listener: () -> Unit): AutoCloseable =
        grantInvalidationObservers.observe(listener)

    override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
        if (!realOAuthEnabled) return TokenSupplyResult.Unavailable(AuthorizationFailure.NOT_CONFIGURED)
        if (selectedAccount == null) return TokenSupplyResult.Unavailable(AuthorizationFailure.ACCOUNT_REQUIRED)
        if (synchronized(consentLock) { !grantState.isEnabledInApp(feature) }) return TokenSupplyResult.Unavailable(AuthorizationFailure.FEATURE_DISABLED)
        if (scopes.isEmpty() || !feature.scopes.containsAll(scopes)) return TokenSupplyResult.Unavailable(AuthorizationFailure.UNAVAILABLE)
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
    suspend fun authorize(scopes: Set<GoogleScope>): AuthorizationOutcome {
        if (!realOAuthEnabled) return AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED)
        if (selectedAccount == null) return AuthorizationOutcome.Rejected(AuthorizationFailure.ACCOUNT_REQUIRED)
        if (scopes.isEmpty()) return AuthorizationOutcome.Rejected(AuthorizationFailure.UNAVAILABLE)
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
            val request = AuthorizationRequest.builder()
                .setRequestedScopes(scopes.map { com.google.android.gms.common.api.Scope(it.value) })
                .setAccount(checkNotNull(selectedAccount))
                .build()
            val result = awaitGoogleIdentityTaskToTerminal {
                await { Identity.getAuthorizationClient(context).authorize(request) }
            }
            currentCoroutineContext().ensureActive()
            mapResult(result, scopes, allowConsent, featureVersions)
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
    fun completeConsentResult(requestId: String, data: Intent?): AuthorizationOutcome {
        if (!realOAuthEnabled) return AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED)
        if (selectedAccount == null) return AuthorizationOutcome.Rejected(AuthorizationFailure.ACCOUNT_REQUIRED)
        val pending = synchronized(consentLock) {
            consentRequests.complete(requestId)
                ?: return AuthorizationOutcome.Rejected(AuthorizationFailure.STALE_CONSENT_RESULT)
        }
        return try {
            val result = Identity.getAuthorizationClient(context).getAuthorizationResultFromIntent(data)
            mapResult(result, pending.scopes, allowConsent = false, pending.featureVersions)
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
    fun takePendingConsentIntent(requestId: String): PendingIntent? = consentRequests.takeIntent(requestId)

    /** Explicitly discard a cancelled/failed foreground consent flow. */
    fun cancelConsentRequest(requestId: String): Boolean = consentRequests.cancel(requestId)

    fun disableInApp(feature: GoogleFeature) = synchronized(consentLock) {
        grantState.disableInApp(feature)
        consentRequests.cancelFeature(feature)
    }
    fun enableInApp(feature: GoogleFeature) = synchronized(consentLock) { grantState.enableInApp(feature) }
    override fun isEnabledInApp(feature: GoogleFeature): Boolean = synchronized(consentLock) {
        grantState.isEnabledInApp(feature)
    }

    override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
        val service = serviceState()
        if (!realOAuthEnabled) return AuthorizationStatus(
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

    /** Google revokeAccess revokes the app's whole grant, including other requested services. */
    suspend fun revokeGoogleGrant(): AuthorizationFailure? {
        if (!realOAuthEnabled) return AuthorizationFailure.NOT_CONFIGURED
        if (selectedAccount == null) return AuthorizationFailure.ACCOUNT_REQUIRED
        val mayRevoke = synchronized(consentLock) {
            if (consentRequests.peek() != null || !grantState.tryBeginAuthorizationOperation()) false
            else {
                // Fail closed immediately, before the Play Services task can suspend.
                invalidateGoogleGrantAtRevokeStart(grantState, knownPermissions)
                true
            }
        }
        if (!mayRevoke) return AuthorizationFailure.REQUEST_IN_PROGRESS
        // Keep cancellation synchronous with local fail-closed invalidation, before waiting for
        // Play Services. An already-dispatched read must not continue consuming the response.
        grantInvalidationObservers.notifyInvalidationStarted()
        return try {
            consentRequests.clear()
            when (serviceState()) {
                GoogleServiceState.MISSING -> return AuthorizationFailure.SERVICE_MISSING
                GoogleServiceState.OUTDATED -> return AuthorizationFailure.SERVICE_OUTDATED
                GoogleServiceState.AVAILABLE -> Unit
            }
            awaitGoogleIdentityTaskToTerminal {
                await { Identity.getAuthorizationClient(context).revokeAccess(RevokeAccessRequest.builder().build()) }
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

    private fun serviceState(): GoogleServiceState = when (
        GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(context)
    ) {
        ConnectionResult.SUCCESS -> GoogleServiceState.AVAILABLE
        ConnectionResult.SERVICE_MISSING -> GoogleServiceState.MISSING
        ConnectionResult.SERVICE_VERSION_UPDATE_REQUIRED -> GoogleServiceState.OUTDATED
        else -> GoogleServiceState.MISSING
    }

    private fun mapResult(
        result: PlayAuthorizationResult,
        requested: Set<GoogleScope>,
        allowConsent: Boolean,
        featureVersions: Map<GoogleFeature, Long>,
    ): AuthorizationOutcome = synchronized(consentLock) {
        googleAuthorizationOperationFailure(grantState, requested, featureVersions)?.let { failure ->
            return@synchronized AuthorizationOutcome.Rejected(failure)
        }
        if (result.hasResolution()) {
            val granted = result.grantedScopes.mapNotNull { raw -> GoogleScope.entries.firstOrNull { it.value == raw } }.toSet()
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
        val granted = result.grantedScopes.mapNotNull { raw -> GoogleScope.entries.firstOrNull { it.value == raw } }.toSet()
        val token = result.accessToken
        if (requested.any { it !in granted }) {
            return@synchronized AuthorizationOutcome.Rejected(AuthorizationFailure.DENIED)
                .also { cacheOutcome(requested, it) }
        }
        if (token.isNullOrBlank()) {
            return@synchronized AuthorizationOutcome.Rejected(AuthorizationFailure.EXPIRED)
                .also { cacheOutcome(requested, it) }
        }
        val lease = AccessTokenLease(
            token,
            granted,
            android.os.SystemClock.elapsedRealtime() + 5 * 60 * 1000L,
            validityCheck = {
                synchronized(consentLock) { grantState.isLeaseCurrent(featureVersions) }
            },
            authorizationSessionId = authorizationSessionId,
        )
        AuthorizationOutcome.Authorized(lease).also { cacheOutcome(requested, it) }
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

    private suspend fun <T> await(task: () -> com.google.android.gms.tasks.Task<T>): T =
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
