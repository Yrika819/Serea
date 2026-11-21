package app.serea.android.googleauth

internal sealed interface PrivateCalendarProviderDecision {
    data object Fake : PrivateCalendarProviderDecision
    data class Live(val sessionId: String) : PrivateCalendarProviderDecision
}

/**
 * Resolves the product route before a provider or HTTP client is constructed.
 * A disabled HTTP gate is the only path that selects Fake; an enabled but invalid
 * Live configuration fails closed instead of presenting synthetic data as Live.
 */
internal fun selectPrivateCalendarProvider(
    applicationId: String,
    buildType: String,
    privateUiEnabled: Boolean,
    httpEnabled: Boolean,
    oauthEnabled: Boolean,
    gateway: GoogleAuthorizationGateway?,
): PrivateCalendarProviderDecision {
    if (!httpEnabled) return PrivateCalendarProviderDecision.Fake

    check(applicationId == PRIVATE_BETA_APPLICATION_ID && buildType == PRIVATE_BETA_BUILD_TYPE && privateUiEnabled) {
        "Google Calendar Live is restricted to the Private Beta product configuration."
    }
    check(oauthEnabled) { "Google Calendar Live requires the independent Private OAuth gate." }

    val authorization = checkNotNull(gateway) { "Google Calendar Live requires an active authorization gateway." }
    check(authorization.mode == GoogleAuthorizationMode.PLAY_SERVICES) {
        "Google Calendar Live requires the Play Services authorization gateway."
    }
    check(authorization.isEnabledInApp(GoogleFeature.CALENDAR)) {
        "Google Calendar authorization is disabled in this app session."
    }
    val sessionId = authorization.authorizationSessionId
    check(!sessionId.isNullOrBlank()) { "Google Calendar authorization session is not active." }

    val scopes = PrivatePrimaryCalendarScopeContract.scopes
    val status = authorization.status(scopes)
    check(status.failure == null && status.permissions.keys == scopes && status.permits(scopes)) {
        "Primary Calendar read-only authorization is not active."
    }
    check(authorization.authorizationSessionId == sessionId &&
        authorization.isEnabledInApp(GoogleFeature.CALENDAR)
    ) {
        "Google Calendar authorization session changed during provider selection."
    }
    return PrivateCalendarProviderDecision.Live(sessionId)
}

private const val PRIVATE_BETA_APPLICATION_ID = "app.serea.android.privatebeta"
private const val PRIVATE_BETA_BUILD_TYPE = "privateBeta"
