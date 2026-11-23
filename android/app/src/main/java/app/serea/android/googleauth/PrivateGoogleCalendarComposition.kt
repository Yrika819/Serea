package app.serea.android.googleauth

import app.serea.android.BuildConfig
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.calendar.CalendarProviderSession
import app.serea.android.calendar.CalendarProviderSessions
import app.serea.android.calendar.FakeCalendarProvider

internal interface PrivateGoogleCalendarProviderComposition : AutoCloseable {
    /** Live build eligibility only; does not imply authorization, a session, or any Calendar read. */
    val opensGoogleProvider: Boolean
    fun openAuthorizedPrimary(gateway: GoogleAuthorizationGateway): CalendarProviderSession
    fun closeCurrent()
}

/** Product composition has no URL, client, gateway, or gate override injection points. */
internal class PrivateGoogleCalendarComposition : PrivateGoogleCalendarProviderComposition {
    override val opensGoogleProvider: Boolean =
        BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED &&
        BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED &&
        BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED &&
        BuildConfig.APPLICATION_ID == "app.serea.android.privatebeta" &&
        BuildConfig.BUILD_TYPE == "privateBeta"

    private val sessions = CalendarProviderSessions(
        fakeProvider = { FakeCalendarProvider.Fixtures.privatePrimaryProvider() },
        googleHttpEnabledByOwner = BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED,
    )
    private var current: CalendarProviderSession? = null

    override fun openAuthorizedPrimary(gateway: GoogleAuthorizationGateway): CalendarProviderSession {
        closeCurrent()
        val decision = selectPrivateCalendarProvider(
            applicationId = BuildConfig.APPLICATION_ID,
            buildType = BuildConfig.BUILD_TYPE,
            privateUiEnabled = BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED,
            httpEnabled = BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED,
            oauthEnabled = BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED,
            gateway = gateway,
        )
        val session = when (decision) {
            PrivateCalendarProviderDecision.Fake -> sessions.openPrimaryFake()
            is PrivateCalendarProviderDecision.Live -> openGoogleSessionBoundToAuthorization(
                gateway = gateway,
                expectedSessionId = decision.sessionId,
            ) { sessions.openPrimaryGoogleReadOnly(gateway) }
        }
        val expectedKind = when (decision) {
            PrivateCalendarProviderDecision.Fake -> CalendarProviderKind.FAKE
            is PrivateCalendarProviderDecision.Live -> CalendarProviderKind.GOOGLE_READ_ONLY
        }
        if (!session.primaryCalendarOnly || session.kind != expectedKind) {
            session.close()
            error("Primary Calendar provider selection was rejected.")
        }
        current = session
        return session
    }

    override fun closeCurrent() {
        current?.close()
        current = null
    }

    override fun close() = closeCurrent()
}

internal fun openGoogleSessionBoundToAuthorization(
    gateway: GoogleAuthorizationGateway,
    expectedSessionId: String,
    openSession: () -> CalendarProviderSession,
): CalendarProviderSession {
    check(gateway.authorizationSessionId == expectedSessionId &&
        gateway.isEnabledInApp(GoogleFeature.CALENDAR)
    ) {
        "Google Calendar authorization session changed before provider construction."
    }
    val session = openSession()
    if (gateway.authorizationSessionId != expectedSessionId ||
        !gateway.isEnabledInApp(GoogleFeature.CALENDAR)
    ) {
        session.close()
        error("Google Calendar authorization session changed during provider construction.")
    }
    return session
}
