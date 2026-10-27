package app.serea.android.calendar.transport

import app.serea.android.googleauth.AccessTokenLease

/** Supplies a short-lived bearer token from the host's authorization layer. */
fun interface CalendarAccessTokenPort {
    /** The endpoint's exact read-only scope; token providers must not broaden it. */
    suspend fun accessToken(scope: String): CalendarAccessToken?
}

/** Never persist or log this value. A new value may be requested for each HTTP call. */
class CalendarAccessToken internal constructor(private val lease: AccessTokenLease) : AutoCloseable {
    internal fun <T> useToken(block: (String) -> T): T = lease.useToken(block)
    internal fun ensureValid() = lease.ensureValid()
    override fun close() = lease.close()
    override fun toString(): String = "CalendarAccessToken(token=<redacted>)"
}
