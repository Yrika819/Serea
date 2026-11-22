package app.serea.android.googleauth

import app.serea.android.BuildConfig
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PrivateBuildOAuthGateTest {
    @Test fun httpGateIsPrivateOnlyAndIndependentFromOAuthGate() {
        assertFalse(BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED)
        assertEquals("privateBeta", BuildConfig.BUILD_TYPE)
        assertEquals("app.serea.android.privatebeta", BuildConfig.APPLICATION_ID)
        assertEquals(
            System.getenv("SEREA_PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED") == "true",
            BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED,
        )
        assertEquals(BuildConfig.BUILD_TYPE == "privateBeta", BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED)
        assertEquals(BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED, ownerPrivateGoogleOAuthEnabled(true))
        assertFalse(ownerPrivateGoogleOAuthEnabled(requestedByLocalPrivateBuild = false))
        assertTrue(BuildConfig.VERSION_CODE >= 2)
    }

    @Test fun httpGateWithoutOAuthNeverConsultsAuthorizationOrSelectsLive() {
        class Gateway : GoogleAuthorizationGateway {
            override val mode = GoogleAuthorizationMode.PLAY_SERVICES
            override val authorizationSessionId: String? = "synthetic-session"
            var statusCalls = 0
            var tokenCalls = 0
            override fun isEnabledInApp(feature: GoogleFeature) = true
            override fun status(scopes: Set<GoogleScope>): AuthorizationStatus {
                statusCalls++
                error("OAuth is disabled, so authorization must not be queried.")
            }
            override suspend fun acquire(feature: GoogleFeature, scopes: Set<GoogleScope>): TokenSupplyResult {
                tokenCalls++
                error("OAuth is disabled, so no token may be acquired.")
            }
        }

        val gateway = Gateway()
        PrivateGoogleCalendarComposition().use { composition ->
            if (BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED) {
                val failure = runCatching { composition.openAuthorizedPrimary(gateway) }.exceptionOrNull()
                assertTrue(failure is IllegalStateException)
                assertTrue(failure!!.message!!.contains("independent Private OAuth gate"))
            } else {
                assertEquals(
                    app.serea.android.calendar.CalendarProviderKind.FAKE,
                    composition.openAuthorizedPrimary(gateway).kind,
                )
            }
        }
        assertEquals(0, gateway.statusCalls)
        assertEquals(0, gateway.tokenCalls)
    }
}
