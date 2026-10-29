package app.serea.android.calendar

import okhttp3.OkHttpClient
import okhttp3.Interceptor
import okhttp3.Dns
import okhttp3.mockwebserver.MockWebServer
import okhttp3.tls.HandshakeCertificates
import okhttp3.tls.HeldCertificate
import java.net.InetAddress
import java.net.UnknownHostException

/** Test-only trust anchor for the HTTPS loopback origin; no certificate is bundled in the app. */
internal object CalendarMockWebServerTls {
    private const val GOOGLE_API_HOST = "www.googleapis.com"
    private const val GOOGLE_API_PORT = 443
    private const val CALENDAR_API_PATH = "/calendar/v3/"
    private val READ_ONLY_QUERY_NAMES = setOf(
        "maxResults", "pageToken", "timeMin", "timeMax", "singleEvents", "showDeleted", "orderBy", "fields",
    )

    private val certificate = HeldCertificate.Builder()
        .addSubjectAlternativeName("localhost")
        .build()
    private val serverCertificates = HandshakeCertificates.Builder()
        .heldCertificate(certificate)
        .build()
    private val clientCertificates = HandshakeCertificates.Builder()
        .addTrustedCertificate(certificate.certificate)
        .build()

    fun client(server: MockWebServer): OkHttpClient = OkHttpClient.Builder()
            .followRedirects(false)
            .followSslRedirects(false)
            .sslSocketFactory(clientCertificates.sslSocketFactory(), clientCertificates.trustManager)
            .dns(object : Dns {
                override fun lookup(hostname: String): List<InetAddress> {
                    if (hostname != "localhost") {
                        throw UnknownHostException("Calendar fixture DNS refuses $hostname")
                    }
                    return Dns.SYSTEM.lookup(hostname)
                }
            })
            .addInterceptor(Interceptor { chain ->
                val request = chain.request()
                val origin = request.url
                val segments = origin.pathSegments
                val exactCalendarListPath =
                    segments.size in 5..6 && segments[0] == "calendar" && segments[1] == "v3" &&
                        segments[2] == "users" && segments[3] == "me" && segments[4] == "calendarList"
                val exactCalendarEventsPath =
                    segments.size in 5..6 && segments[0] == "calendar" && segments[1] == "v3" &&
                        segments[2] == "calendars" && segments[4] == "events"
                check(
                    request.method == "GET" && origin.scheme == "https" && origin.host == GOOGLE_API_HOST &&
                        origin.port == GOOGLE_API_PORT && origin.encodedPath.startsWith(CALENDAR_API_PATH) &&
                        (exactCalendarListPath || exactCalendarEventsPath) &&
                        segments.none { it.isBlank() || it == "." || it == ".." } &&
                        !origin.encodedPath.contains("%2f", ignoreCase = true) &&
                        !origin.encodedPath.contains("%5c", ignoreCase = true) &&
                        origin.queryParameterNames.all { it in READ_ONLY_QUERY_NAMES } &&
                        origin.username.isEmpty() && origin.password.isEmpty() && origin.fragment == null
                ) { "Calendar fixture client refused a request outside the production Calendar origin." }
                val localUrl = origin.newBuilder().host("localhost").port(server.port).build()
                chain.proceed(
                    request.newBuilder()
                        .url(localUrl)
                        .header("Host", GOOGLE_API_HOST)
                        .build(),
                )
            })
            .build()

    fun start(server: MockWebServer) {
        server.useHttps(serverCertificates.sslSocketFactory(), tunnelProxy = false)
        server.start()
    }
}
