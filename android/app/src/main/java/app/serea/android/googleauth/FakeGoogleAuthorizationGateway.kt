package app.serea.android.googleauth

/** In-memory consent provider for deterministic tests and local development. */
class FakeGoogleAuthorizationGateway(
    initialScopes: Set<GoogleScope> = emptySet(),
    private val fixtureToken: String = "fixture-access-token-never-real",
) : GoogleAccessTokenSupplier {
    private val grantedScopes = initialScopes.toMutableSet()
    private var service = GoogleServiceState.AVAILABLE
    private var nextFailure: AuthorizationFailure? = null
    private var enabled = true
    private var expired = false
    private var accountChanged = false
    private val disabledInApp = mutableSetOf<GoogleScope>()
    var revokeCount: Int = 0
        private set

    fun grant(vararg scopes: GoogleScope) { grantedScopes += scopes }
    fun denyNext(failure: AuthorizationFailure = AuthorizationFailure.DENIED) { nextFailure = failure }
    fun setGooglePlayServices(state: GoogleServiceState) { service = state }
    fun setEnabled(value: Boolean) { enabled = value }
    fun setExpired(value: Boolean) { expired = value }
    fun switchAccount() { accountChanged = true }

    override suspend fun acquire(scopes: Set<GoogleScope>): TokenSupplyResult {
        when (val outcome = authorize(scopes)) {
            is AuthorizationOutcome.Authorized -> return TokenSupplyResult.Available(outcome.lease)
            is AuthorizationOutcome.ConsentRequired -> return TokenSupplyResult.UserActionRequired(outcome.missingScopes)
            is AuthorizationOutcome.Rejected -> return TokenSupplyResult.Unavailable(outcome.failure)
        }
    }

    suspend fun authorize(scopes: Set<GoogleScope>): AuthorizationOutcome {
        if (!enabled) return AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED)
        if (scopes.isEmpty()) return AuthorizationOutcome.Rejected(AuthorizationFailure.UNAVAILABLE)
        when (service) {
            GoogleServiceState.MISSING -> return AuthorizationOutcome.Rejected(AuthorizationFailure.SERVICE_MISSING)
            GoogleServiceState.OUTDATED -> return AuthorizationOutcome.Rejected(AuthorizationFailure.SERVICE_OUTDATED)
            GoogleServiceState.AVAILABLE -> Unit
        }
        if (accountChanged) {
            accountChanged = false
            grantedScopes.clear()
            return AuthorizationOutcome.Rejected(AuthorizationFailure.ACCOUNT_CHANGED)
        }
        nextFailure?.let { failure ->
            nextFailure = null
            return AuthorizationOutcome.Rejected(failure)
        }
        if (expired) return AuthorizationOutcome.Rejected(AuthorizationFailure.EXPIRED)
        val missing = scopes - grantedScopes
        return if (missing.isNotEmpty()) AuthorizationOutcome.ConsentRequired(missing)
        else AuthorizationOutcome.Authorized(AccessTokenLease(fixtureToken, grantedScopes.toSet(), null))
    }

    suspend fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
        val failure = when {
            !enabled -> AuthorizationFailure.NOT_CONFIGURED
            service == GoogleServiceState.MISSING -> AuthorizationFailure.SERVICE_MISSING
            service == GoogleServiceState.OUTDATED -> AuthorizationFailure.SERVICE_OUTDATED
            expired -> AuthorizationFailure.EXPIRED
            else -> null
        }
        val permissions = scopes.associateWith { scope ->
            when {
                failure != null -> ScopePermission.NOT_REQUESTED
                scope in grantedScopes -> ScopePermission.GRANTED
                else -> ScopePermission.NEEDS_CONSENT
            }
        }
        return AuthorizationStatus(service, permissions, failure)
    }

    /** Disabling one app feature leaves the shared Google grant alone. */
    fun disableInApp(feature: GoogleScope) { disabledInApp += feature }
    fun enableInApp(feature: GoogleScope) { disabledInApp -= feature }
    fun isEnabledInApp(feature: GoogleScope): Boolean = feature !in disabledInApp

    /** Models Google revokeAccess, which invalidates every grant held by this app. */
    fun revokeGoogleGrant() {
        revokeCount++
        grantedScopes.clear()
        disabledInApp.clear()
        expired = false
    }
}
