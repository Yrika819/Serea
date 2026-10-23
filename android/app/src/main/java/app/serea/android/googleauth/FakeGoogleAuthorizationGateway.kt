package app.serea.android.googleauth

/** In-memory consent provider for deterministic tests and local development. */
class FakeGoogleAuthorizationGateway(
    initialScopes: Set<GoogleScope> = emptySet(),
    private val fixtureToken: String = "fixture-access-token-never-real",
) : GoogleAuthorizationGateway {
    override val mode = GoogleAuthorizationMode.MOCK_ONLY
    override var authorizationSessionId: String = java.util.UUID.randomUUID().toString()
        private set
    private val grantedScopes = initialScopes.toMutableSet()
    private var service = GoogleServiceState.AVAILABLE
    private var nextFailure: AuthorizationFailure? = null
    private var enabled = true
    private var expired = false
    private var accountChanged = false
    private val disabledInApp = mutableSetOf<GoogleFeature>()
    private val featureVersions = GoogleFeature.entries.associateWith { 0L }.toMutableMap()
    var revokeCount: Int = 0
        private set

    fun grant(vararg scopes: GoogleScope) { grantedScopes += scopes }
    fun denyNext(failure: AuthorizationFailure = AuthorizationFailure.DENIED) { nextFailure = failure }
    fun setGooglePlayServices(state: GoogleServiceState) { service = state }
    fun setEnabled(value: Boolean) { enabled = value }
    fun setExpired(value: Boolean) { expired = value }
    fun switchAccount() {
        accountChanged = true
        grantedScopes.clear()
        authorizationSessionId = java.util.UUID.randomUUID().toString()
    }

    override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
        if (!enabled) return TokenSupplyResult.Unavailable(AuthorizationFailure.NOT_CONFIGURED)
        if (feature in disabledInApp) return TokenSupplyResult.Unavailable(AuthorizationFailure.FEATURE_DISABLED)
        if (scopes.isEmpty() || !feature.scopes.containsAll(scopes)) return TokenSupplyResult.Unavailable(AuthorizationFailure.UNAVAILABLE)
        when (val outcome = authorize(scopes)) {
            is AuthorizationOutcome.Authorized -> return TokenSupplyResult.Available(outcome.lease)
            is AuthorizationOutcome.ConsentRequired -> return TokenSupplyResult.UserActionRequired(outcome.missingScopes)
            is AuthorizationOutcome.Rejected -> return TokenSupplyResult.Unavailable(outcome.failure)
        }
    }

    suspend fun authorize(scopes: Set<GoogleScope>): AuthorizationOutcome {
        if (!enabled) return AuthorizationOutcome.Rejected(AuthorizationFailure.NOT_CONFIGURED)
        if (scopes.isEmpty()) return AuthorizationOutcome.Rejected(AuthorizationFailure.UNAVAILABLE)
        if (GoogleFeature.entries.any { feature -> feature in disabledInApp && scopes.any { it in feature.scopes } }) {
            return AuthorizationOutcome.Rejected(AuthorizationFailure.FEATURE_DISABLED)
        }
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
        else {
            val features = GoogleFeature.entries.filter { feature -> scopes.any { it in feature.scopes } }
            if (features.any { it in disabledInApp }) return AuthorizationOutcome.Rejected(AuthorizationFailure.FEATURE_DISABLED)
            val versions = features.associateWith { featureVersions.getValue(it) }
            val sessionId = authorizationSessionId
            AuthorizationOutcome.Authorized(AccessTokenLease(
                fixtureToken,
                grantedScopes.toSet(),
                null,
                validityCheck = {
                    authorizationSessionId == sessionId &&
                        versions.all { (feature, version) -> feature !in disabledInApp && featureVersions[feature] == version }
                },
                authorizationSessionId = sessionId,
            ))
        }
    }

    override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
        val failure = when {
            !enabled -> AuthorizationFailure.NOT_CONFIGURED
            service == GoogleServiceState.MISSING -> AuthorizationFailure.SERVICE_MISSING
            service == GoogleServiceState.OUTDATED -> AuthorizationFailure.SERVICE_OUTDATED
            accountChanged -> AuthorizationFailure.ACCOUNT_CHANGED
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
    fun disableInApp(feature: GoogleFeature) {
        if (disabledInApp.add(feature)) featureVersions[feature] = featureVersions.getValue(feature) + 1
    }
    fun enableInApp(feature: GoogleFeature) {
        if (disabledInApp.remove(feature)) featureVersions[feature] = featureVersions.getValue(feature) + 1
    }
    override fun isEnabledInApp(feature: GoogleFeature): Boolean = feature !in disabledInApp

    /** Models Google revokeAccess, which invalidates every grant held by this app. */
    fun revokeGoogleGrant() {
        revokeCount++
        grantedScopes.clear()
        disabledInApp.clear()
        featureVersions.keys.forEach { feature -> featureVersions[feature] = featureVersions.getValue(feature) + 1 }
        expired = false
    }
}
