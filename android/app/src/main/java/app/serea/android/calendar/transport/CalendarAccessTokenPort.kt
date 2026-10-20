package app.serea.android.calendar.transport

/** Supplies a short-lived bearer token from the host's authorization layer. */
fun interface CalendarAccessTokenPort {
    suspend fun accessToken(): CalendarAccessToken?
}

/** Never persist or log this value. A new value may be requested for each HTTP call. */
class CalendarAccessToken internal constructor(internal val value: String) {
    init { require(value.isNotBlank() && value.length <= 16_384) }
}
