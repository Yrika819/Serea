package app.serea.android.gmail.transport

import app.serea.android.googleauth.AccessTokenLease

/** Supplies a short-lived token for the one Gmail read scope. */
fun interface GmailAccessTokenPort {
    suspend fun accessToken(scope: String): GmailAccessToken?
}

/** Token bytes stay inside the authorization lease and are never persisted or logged. */
class GmailAccessToken internal constructor(private val lease: AccessTokenLease) : AutoCloseable {
    internal fun <T> useToken(block: (String) -> T): T = lease.useToken(block)
    override fun close() = lease.close()
    override fun toString(): String = "GmailAccessToken(token=<redacted>)"
}
