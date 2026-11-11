package app.serea.android.googleauth

/** Immutable identity captured when one foreground consent operation is accepted. */
internal data class PrivateGoogleConsentIdentity(
    val requestId: String,
    val generation: Long,
    val authorizationSessionId: String,
)

/** Single-use identity gate. A stale result cannot consume or mutate a newer pending request. */
internal class PrivateGoogleConsentIdentityGate {
    private var pending: PrivateGoogleConsentIdentity? = null

    @Synchronized
    fun begin(identity: PrivateGoogleConsentIdentity): Boolean {
        if (pending != null || identity.requestId.isBlank() || identity.authorizationSessionId.isBlank()) return false
        pending = identity
        return true
    }

    @Synchronized
    fun isPending(identity: PrivateGoogleConsentIdentity): Boolean = pending == identity

    @Synchronized
    fun consume(identity: PrivateGoogleConsentIdentity): Boolean {
        if (pending != identity) return false
        pending = null
        return true
    }

    @Synchronized
    fun cancel(identity: PrivateGoogleConsentIdentity): Boolean = consume(identity)

    @Synchronized
    fun clear() {
        pending = null
    }
}

/** Result parcel returned by the Private Beta consent relay. */
internal data class PrivateGoogleConsentActivityResult(
    val identity: PrivateGoogleConsentIdentity?,
    val resultCode: Int,
    val resultData: android.content.Intent?,
)
