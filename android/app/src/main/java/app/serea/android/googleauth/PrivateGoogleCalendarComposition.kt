package app.serea.android.googleauth

import app.serea.android.BuildConfig
import app.serea.android.calendar.CalendarProviderKind
import app.serea.android.calendar.CalendarProviderSession
import app.serea.android.calendar.CalendarProviderSessions
import app.serea.android.calendar.FakeCalendarProvider
import okhttp3.HttpUrl
import okhttp3.OkHttpClient

/** Product builds remain Fake; isolated tests can explicitly select the primary Google path. */
internal class PrivateGoogleCalendarComposition(
    privateVariantEnabled: Boolean = BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED,
    googleHttpEnabledByOwner: Boolean = BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED,
    private val client: OkHttpClient = OkHttpClient(),
    private val baseUrl: HttpUrl? = null,
) : AutoCloseable {
    val opensGoogleProvider = privateVariantEnabled && googleHttpEnabledByOwner

    init {
        check(!opensGoogleProvider || BuildConfig.BUILD_TYPE == "isolated") {
            "Google Calendar HTTP test composition is restricted to the isolated variant."
        }
        check(!opensGoogleProvider || baseUrl?.let {
            it.host in LOOPBACK_TEST_HOSTS && it.scheme in setOf("http", "https") &&
                it.encodedPath == "/calendar/v3/" && it.encodedUsername.isEmpty() &&
                it.encodedPassword.isEmpty() && it.encodedQuery == null && it.encodedFragment == null
        } == true) {
            "Google Calendar test composition requires an explicit loopback MockWebServer URL."
        }
    }

    private val sessions = CalendarProviderSessions(
        fakeProvider = { FakeCalendarProvider.Fixtures.privatePrimaryProvider() },
        googleHttpEnabledByOwner = opensGoogleProvider,
    )
    private var current: CalendarProviderSession? = null

    fun openAuthorizedPrimary(gateway: GoogleAuthorizationGateway): CalendarProviderSession {
        val sessionId = if (opensGoogleProvider) {
            check(gateway.mode == GoogleAuthorizationMode.PLAY_SERVICES) {
                "Primary Google Calendar requires Play Services authorization."
            }
            check(!gateway.authorizationSessionId.isNullOrBlank() && gateway.isEnabledInApp(GoogleFeature.CALENDAR)) {
                "Primary Google Calendar authorization session is not active."
            }
            gateway.authorizationSessionId
        } else null
        val scopes = PrivatePrimaryCalendarScopeContract.scopes
        val status = gateway.status(scopes)
        check(status.failure == null && status.permits(scopes)) {
            "Primary Calendar read-only authorization is not active."
        }
        if (opensGoogleProvider) {
            check(!sessionId.isNullOrBlank() && gateway.authorizationSessionId == sessionId &&
                gateway.isEnabledInApp(GoogleFeature.CALENDAR)) {
                "Primary Google Calendar authorization session is no longer active."
            }
        }
        closeCurrent()
        val session = if (opensGoogleProvider) {
            sessions.openPrimaryGoogleReadOnly(gateway, client, baseUrl)
        } else {
            sessions.openPrimaryFake()
        }
        if (opensGoogleProvider &&
            (gateway.authorizationSessionId != sessionId || !gateway.isEnabledInApp(GoogleFeature.CALENDAR))
        ) {
            session.close()
            error("Primary Google Calendar authorization session changed while opening.")
        }
        if (!session.primaryCalendarOnly ||
            session.kind != if (opensGoogleProvider) CalendarProviderKind.GOOGLE_READ_ONLY else CalendarProviderKind.FAKE
        ) {
            session.close()
            error("Primary Calendar provider selection was rejected.")
        }
        current = session
        return session
    }

    fun closeCurrent() {
        current?.close()
        current = null
    }

    override fun close() = closeCurrent()

    private companion object {
        val LOOPBACK_TEST_HOSTS = setOf("127.0.0.1", "localhost", "::1")
    }
}
