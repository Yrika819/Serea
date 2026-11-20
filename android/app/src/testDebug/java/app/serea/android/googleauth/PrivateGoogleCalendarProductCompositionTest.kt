package app.serea.android.googleauth

import app.serea.android.BuildConfig
import app.serea.android.calendar.CalendarProviderKind
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PrivateGoogleCalendarProductCompositionTest {
    private class Gateway : GoogleAuthorizationGateway {
        override val mode = GoogleAuthorizationMode.MOCK_ONLY
        var statusCalls = 0
        override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
            statusCalls++
            error("Gate-off product composition must not consult authorization status.")
        }
        override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>) =
            TokenSupplyResult.Unavailable(AuthorizationFailure.NOT_CONFIGURED)
    }

    @Test
    fun debugProductBuildHasNoLiveOverrideAndGateOffShortCircuitsAuthorization() {
        assertEquals("debug", BuildConfig.BUILD_TYPE)
        assertFalse(BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED)
        val gateway = Gateway()

        PrivateGoogleCalendarComposition().use { composition ->
            val session = composition.openAuthorizedPrimary(gateway)
            assertEquals(CalendarProviderKind.FAKE, session.kind)
            assertTrue(session.primaryCalendarOnly)
        }
        assertEquals(0, gateway.statusCalls)
    }
}
