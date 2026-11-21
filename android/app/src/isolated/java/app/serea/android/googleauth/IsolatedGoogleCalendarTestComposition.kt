package app.serea.android.googleauth

import app.serea.android.BuildConfig
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.calendar.CalendarProviderSession
import app.serea.android.calendar.CalendarProviderSessions
import app.serea.android.calendar.FakeCalendarProvider
import okhttp3.HttpUrl
import okhttp3.OkHttpClient

/** Synthetic, loopback-only composition compiled exclusively into the isolated build type. */
internal class IsolatedGoogleCalendarTestComposition(
    private val client: OkHttpClient = OkHttpClient(),
    private val baseUrl: HttpUrl? = null,
) : PrivateGoogleCalendarProviderComposition {
    override val opensGoogleProvider: Boolean = true

    private val sessions = CalendarProviderSessions(
        fakeProvider = { FakeCalendarProvider.Fixtures.privatePrimaryProvider() },
        googleHttpEnabledByOwner = true,
    )
    private var current: CalendarProviderSession? = null

    init {
        check(BuildConfig.BUILD_TYPE == "isolated") {
            "Synthetic Google Calendar composition is restricted to the isolated build type."
        }
        check(baseUrl?.let {
            it.host in LOOPBACK_TEST_HOSTS && it.scheme in setOf("http", "https") &&
                it.encodedPath == "/calendar/v3/" && it.encodedUsername.isEmpty() &&
                it.encodedPassword.isEmpty() && it.encodedQuery == null && it.encodedFragment == null
        } == true) {
            "Isolated Google Calendar tests require an explicit loopback MockWebServer URL."
        }
    }

    override fun openAuthorizedPrimary(gateway: GoogleAuthorizationGateway): CalendarProviderSession {
        closeCurrent()
        check(gateway.mode == GoogleAuthorizationMode.PLAY_SERVICES) {
            "Isolated Calendar test gateway is not eligible."
        }
        val sessionId = gateway.authorizationSessionId
        check(!sessionId.isNullOrBlank() && gateway.isEnabledInApp(GoogleFeature.CALENDAR)) {
            "Isolated Calendar test authorization session is not active."
        }
        val scopes = PrivatePrimaryCalendarScopeContract.scopes
        val status = gateway.status(scopes)
        check(status.failure == null && status.permissions.keys == scopes && status.permits(scopes)) {
            "Isolated Calendar test requires an exact read-only grant."
        }
        check(gateway.authorizationSessionId == sessionId && gateway.isEnabledInApp(GoogleFeature.CALENDAR)) {
            "Isolated Calendar test session changed during provider selection."
        }
        val session = openGoogleSessionBoundToAuthorization(gateway, sessionId) {
            sessions.openPrimaryGoogleReadOnly(gateway, client, checkNotNull(baseUrl))
        }
        check(session.primaryCalendarOnly && session.kind == CalendarProviderKind.GOOGLE_READ_ONLY) {
            session.close()
            error("Isolated Calendar test provider selection was rejected.")
        }
        current = session
        return session
    }

    override fun closeCurrent() {
        current?.close()
        current = null
    }

    override fun close() = closeCurrent()

    private companion object {
        val LOOPBACK_TEST_HOSTS = setOf("127.0.0.1", "localhost", "::1")
    }
}
