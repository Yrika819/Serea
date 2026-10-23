package app.serea.android.googleauth

import app.serea.android.gmail.GmailAuthorizationPort
import app.serea.android.gmail.GmailAuthorizationState
import app.serea.android.gmail.GmailAuthorizationStatus
import app.serea.android.gmail.GmailPermissionPolicy
import app.serea.android.gmail.transport.GmailAccessToken
import app.serea.android.gmail.transport.GmailAccessTokenPort

/** Gmail's shared authorization bridge exposes only gmail.readonly to its HTTP adapter. */
class GoogleGmailAuthorizationAdapter(
    private val gateway: GoogleAuthorizationGateway,
    private val googleHttpEnabledByOwner: Boolean = false,
) : GmailAuthorizationPort, GmailAccessTokenPort {
    private val authorizationSessionId = gateway.authorizationSessionId
    private val gmailScopes = setOf(GoogleScope.GMAIL_READ_ONLY)

    override suspend fun status(): GmailAuthorizationStatus {
        val googleStatus = gateway.status(gmailScopes)
        val missing = gmailScopes.filter { googleStatus.permissions[it] != ScopePermission.GRANTED }
            .mapTo(linkedSetOf()) { it.value }
        val permission = googleStatus.permissions[GoogleScope.GMAIL_READ_ONLY] ?: ScopePermission.NOT_REQUESTED
        val state = when {
            gateway.mode == GoogleAuthorizationMode.MOCK_ONLY -> GmailAuthorizationState.MOCK_ONLY
            !googleHttpEnabledByOwner -> GmailAuthorizationState.NOT_CONFIGURED
            googleStatus.failure == AuthorizationFailure.NOT_CONFIGURED -> GmailAuthorizationState.NOT_CONFIGURED
            authorizationSessionId == null -> GmailAuthorizationState.NOT_CONNECTED
            gateway.authorizationSessionId != authorizationSessionId -> GmailAuthorizationState.REVOKED
            !gateway.isEnabledInApp(GoogleFeature.GMAIL) -> GmailAuthorizationState.ERROR
            googleStatus.failure == AuthorizationFailure.ACCOUNT_CHANGED -> GmailAuthorizationState.REVOKED
            googleStatus.failure == AuthorizationFailure.ACCOUNT_REQUIRED -> GmailAuthorizationState.NOT_CONNECTED
            googleStatus.service == GoogleServiceState.MISSING ||
                googleStatus.failure == AuthorizationFailure.SERVICE_MISSING -> GmailAuthorizationState.NOT_CONNECTED
            googleStatus.service == GoogleServiceState.OUTDATED ||
                googleStatus.failure == AuthorizationFailure.SERVICE_OUTDATED -> GmailAuthorizationState.ERROR
            permission == ScopePermission.GRANTED -> GmailAuthorizationState.AUTHORIZED
            permission == ScopePermission.DENIED || googleStatus.failure == AuthorizationFailure.DENIED -> GmailAuthorizationState.DENIED
            permission == ScopePermission.EXPIRED || googleStatus.failure == AuthorizationFailure.EXPIRED -> GmailAuthorizationState.EXPIRED
            else -> GmailAuthorizationState.AUTHORIZATION_REQUIRED
        }
        return GmailAuthorizationStatus(state, missing, googleStatus.failure?.name)
    }

    override suspend fun accessToken(scope: String): GmailAccessToken? {
        if (scope != GmailPermissionPolicy.GMAIL_READ_ONLY ||
            !googleHttpEnabledByOwner ||
            gateway.mode != GoogleAuthorizationMode.PLAY_SERVICES ||
            authorizationSessionId == null ||
            gateway.authorizationSessionId != authorizationSessionId ||
            !gateway.isEnabledInApp(GoogleFeature.GMAIL)
        ) return null
        val result = gateway.acquire(GoogleFeature.GMAIL, setOf(GoogleScope.GMAIL_READ_ONLY))
        val lease = (result as? TokenSupplyResult.Available)?.lease ?: return null
        if (GoogleScope.GMAIL_READ_ONLY !in lease.grantedScopes ||
            lease.authorizationSessionId != authorizationSessionId
        ) {
            lease.close()
            return null
        }
        return GmailAccessToken(lease)
    }
}
