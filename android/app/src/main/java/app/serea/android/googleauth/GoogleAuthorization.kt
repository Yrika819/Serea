package app.serea.android.googleauth

import android.app.PendingIntent
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
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlin.coroutines.resume

/** OAuth scopes this app has approved for read-only Gmail and Calendar access. */
enum class GoogleScope(val value: String) {
    GMAIL_READ_ONLY("https://www.googleapis.com/auth/gmail.readonly"),
    CALENDAR_EVENTS_READ_ONLY("https://www.googleapis.com/auth/calendar.events.readonly"),
    CALENDAR_LIST_READ_ONLY("https://www.googleapis.com/auth/calendar.calendarlist.readonly"),
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
}

/** The access token lives only in this short-lived in-memory lease. Its string form never reveals it. */
class AccessTokenLease internal constructor(
    token: String,
    val grantedScopes: Set<GoogleScope>,
    val expiresAtElapsedRealtimeMillis: Long?,
) : AutoCloseable {
    private var tokenValue: String? = token

    fun <T> useToken(block: (String) -> T): T {
        if (expiresAtElapsedRealtimeMillis?.let { android.os.SystemClock.elapsedRealtime() >= it } == true) {
            close()
            error("Authorization lease has expired.")
        }
        val token = checkNotNull(tokenValue) { "Authorization lease is no longer valid." }
        return block(token)
    }

    override fun close() { tokenValue = null }

    override fun toString(): String = "AccessTokenLease(scopes=$grantedScopes, token=<redacted>)"
}

sealed interface AuthorizationOutcome {
    data class Authorized(val lease: AccessTokenLease) : AuthorizationOutcome
    data class ConsentRequired(
        val missingScopes: Set<GoogleScope>,
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
    suspend fun acquire(scopes: Set<GoogleScope>): TokenSupplyResult
}

sealed interface TokenSupplyResult {
    data class Available(val lease: AccessTokenLease) : TokenSupplyResult
    data class Unavailable(val failure: AuthorizationFailure) : TokenSupplyResult
    data class UserActionRequired(val missingScopes: Set<GoogleScope>) : TokenSupplyResult
}

/**
 * Real adapter around Google Play services AuthorizationClient. It is disabled unless an owner
 * explicitly enables it at construction. No authorization is initiated by application startup.
 */
class PlayServicesGoogleAuthorizationGateway(
    private val context: android.content.Context,
    private val realOAuthEnabled: Boolean = false,
) : GoogleAccessTokenSupplier {
    @Volatile private var pendingConsent: PendingIntent? = null
    @Volatile private var pendingScopes: Set<GoogleScope> = emptySet()

    override suspend fun acquire(scopes: Set<GoogleScope>): TokenSupplyResult {
        if (!realOAuthEnabled) return TokenSupplyResult.Unavailable(AuthorizationFailure.NOT_CONFIGURED)
        if (scopes.isEmpty()) return TokenSupplyResult.Unavailable(AuthorizationFailure.UNAVAILABLE)
        when (serviceState()) {
            GoogleServiceState.MISSING -> return TokenSupplyResult.Unavailable(AuthorizationFailure.SERVICE_MISSING)
            GoogleServiceState.OUTDATED -> return TokenSupplyResult.Unavailable(AuthorizationFailure.SERVICE_OUTDATED)
            GoogleServiceState.AVAILABLE -> Unit
        }
        return when (val outcome = authorize(scopes)) {
            is AuthorizationOutcome.Authorized -> TokenSupplyResult.Available(outcome.lease)
            is AuthorizationOutcome.ConsentRequired -> TokenSupplyResult.UserActionRequired(outcome.missingScopes)
            is AuthorizationOutcome.Rejected -> TokenSupplyResult.Unavailable(outcome.failure)
        }
    }

    /** Call from a user initiated foreground action. ConsentRequired must be launched by the UI. */
    suspend fun authorize(scopes: Set<GoogleScope>): AuthorizationOutcome {
        if (!realOAuthEnabled) return AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED)
        if (scopes.isEmpty()) return AuthorizationOutcome.Rejected(AuthorizationFailure.UNAVAILABLE)
        when (serviceState()) {
            GoogleServiceState.MISSING -> return AuthorizationOutcome.Rejected(AuthorizationFailure.SERVICE_MISSING)
            GoogleServiceState.OUTDATED -> return AuthorizationOutcome.Rejected(AuthorizationFailure.SERVICE_OUTDATED)
            GoogleServiceState.AVAILABLE -> Unit
        }
        val request = AuthorizationRequest.builder()
            .setRequestedScopes(scopes.map { com.google.android.gms.common.api.Scope(it.value) })
            .build()
        return try {
            val result = await { Identity.getAuthorizationClient(context).authorize(request) }
            mapResult(result, scopes)
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (exception: Exception) {
            AuthorizationOutcome.Rejected(classify(exception))
        }
    }

    /** Complete a foreground consent flow using the Activity Result Intent. */
    @MainThread
    fun completeConsentResult(data: Intent?): AuthorizationOutcome {
        if (!realOAuthEnabled) return AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED)
        return try {
            val result = Identity.getAuthorizationClient(context).getAuthorizationResultFromIntent(data)
            val requested = pendingScopes
            pendingScopes = emptySet()
            mapResult(result, requested)
        } catch (exception: Exception) {
            AuthorizationOutcome.Rejected(classify(exception))
        }
    }

    /** Consume the one-shot consent resolution after authorize() returns ConsentRequired. */
    fun takePendingConsentIntent(): PendingIntent? = pendingConsent.also { pendingConsent = null }

    suspend fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
        val service = serviceState()
        if (!realOAuthEnabled) return AuthorizationStatus(
            service,
            scopes.associateWith { ScopePermission.NOT_REQUESTED },
            AuthorizationFailure.NOT_CONFIGURED,
        )
        if (service != GoogleServiceState.AVAILABLE) return AuthorizationStatus(
            service,
            scopes.associateWith { ScopePermission.NOT_REQUESTED },
            if (service == GoogleServiceState.MISSING) AuthorizationFailure.SERVICE_MISSING else AuthorizationFailure.SERVICE_OUTDATED,
        )
        return when (val result = authorize(scopes)) {
            is AuthorizationOutcome.Authorized -> {
                val granted = result.lease.grantedScopes
                val status = AuthorizationStatus(service, scopes.associateWith {
                    if (it in granted) ScopePermission.GRANTED else ScopePermission.NOT_REQUESTED
                })
                result.lease.close()
                status
            }
            is AuthorizationOutcome.ConsentRequired -> AuthorizationStatus(
                service,
                scopes.associateWith { if (it in result.missingScopes) ScopePermission.NEEDS_CONSENT else ScopePermission.GRANTED },
            )
            is AuthorizationOutcome.Rejected -> AuthorizationStatus(
                service,
                scopes.associateWith { ScopePermission.DENIED },
                result.failure,
            )
        }
    }

    /** Google revokeAccess revokes the app's whole grant, including other requested services. */
    suspend fun revokeGoogleGrant(): AuthorizationFailure? {
        if (!realOAuthEnabled) return AuthorizationFailure.NOT_CONFIGURED
        when (serviceState()) {
            GoogleServiceState.MISSING -> return AuthorizationFailure.SERVICE_MISSING
            GoogleServiceState.OUTDATED -> return AuthorizationFailure.SERVICE_OUTDATED
            GoogleServiceState.AVAILABLE -> Unit
        }
        return try {
            await { Identity.getAuthorizationClient(context).revokeAccess(RevokeAccessRequest.builder().build()) }
            null
        } catch (exception: Exception) { classify(exception) }
    }

    private fun serviceState(): GoogleServiceState = when (
        GoogleApiAvailability.getInstance().isGooglePlayServicesAvailable(context)
    ) {
        ConnectionResult.SUCCESS -> GoogleServiceState.AVAILABLE
        ConnectionResult.SERVICE_MISSING -> GoogleServiceState.MISSING
        ConnectionResult.SERVICE_VERSION_UPDATE_REQUIRED -> GoogleServiceState.OUTDATED
        else -> GoogleServiceState.MISSING
    }

    private fun mapResult(result: PlayAuthorizationResult, requested: Set<GoogleScope>): AuthorizationOutcome {
        if (result.hasResolution()) {
            val granted = result.grantedScopes.mapNotNull { raw -> GoogleScope.entries.firstOrNull { it.value == raw } }.toSet()
            pendingConsent = result.pendingIntent
            pendingScopes = requested
            return AuthorizationOutcome.ConsentRequired(requested - granted)
        }
        val granted = result.grantedScopes.mapNotNull { raw -> GoogleScope.entries.firstOrNull { it.value == raw } }.toSet()
        val token = result.accessToken
        if (requested.any { it !in granted }) return AuthorizationOutcome.Rejected(AuthorizationFailure.DENIED)
        if (token.isNullOrBlank()) return AuthorizationOutcome.Rejected(AuthorizationFailure.EXPIRED)
        return AuthorizationOutcome.Authorized(
            AccessTokenLease(token, granted, android.os.SystemClock.elapsedRealtime() + 5 * 60 * 1000L),
        )
    }

    private fun classify(exception: Exception): AuthorizationFailure = when (exception) {
        is CancellationException -> AuthorizationFailure.CANCELLED
        is IOException -> AuthorizationFailure.NETWORK
        is ApiException -> when (exception.statusCode) {
            ConnectionResult.SERVICE_MISSING -> AuthorizationFailure.SERVICE_MISSING
            ConnectionResult.SERVICE_VERSION_UPDATE_REQUIRED -> AuthorizationFailure.SERVICE_OUTDATED
            ConnectionResult.NETWORK_ERROR -> AuthorizationFailure.NETWORK
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
