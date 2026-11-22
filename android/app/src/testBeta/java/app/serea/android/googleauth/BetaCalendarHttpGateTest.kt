package app.serea.android.googleauth

import app.serea.android.BuildConfig
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Test

class BetaCalendarHttpGateTest {
    @Test
    fun publicBetaCannotEnablePrivateGoogleCalendarHttp() {
        assertEquals("beta", BuildConfig.BUILD_TYPE)
        assertFalse(BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED)
        assertFalse(BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED)
        assertFalse(BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED)
    }
}
