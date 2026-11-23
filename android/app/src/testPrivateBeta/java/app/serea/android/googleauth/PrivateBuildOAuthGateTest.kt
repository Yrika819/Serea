package app.serea.android.googleauth

import app.serea.android.BuildConfig
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PrivateBuildOAuthGateTest {
    @Test fun httpGateIsPrivateOnlyAndIndependentFromOAuthGate() {
        assertEquals(
            System.getenv("SEREA_OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED") == "true",
            BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED,
        )
        assertEquals(
            System.getenv("SEREA_PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED") == "true",
            BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED,
        )
        assertEquals("privateBeta", BuildConfig.BUILD_TYPE)
        assertEquals("app.serea.android.privatebeta", BuildConfig.APPLICATION_ID)
        assertEquals(
            BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED &&
                BuildConfig.PRIVATE_GOOGLE_CALENDAR_HTTP_ENABLED,
            PrivateGoogleCalendarComposition().opensGoogleProvider,
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

        listOf(false, true).forEach { httpEnabled ->
            val gateway = Gateway()
            val selection = runCatching {
                selectPrivateCalendarProvider(
                    applicationId = BuildConfig.APPLICATION_ID,
                    buildType = BuildConfig.BUILD_TYPE,
                    privateUiEnabled = BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED,
                    httpEnabled = httpEnabled,
                    oauthEnabled = false,
                    gateway = gateway,
                )
            }
            if (httpEnabled) {
                val failure = selection.exceptionOrNull()
                assertTrue(failure is IllegalStateException)
                assertTrue(requireNotNull(failure).message!!.contains("independent Private OAuth gate"))
            } else {
                assertEquals(PrivateCalendarProviderDecision.Fake, selection.getOrThrow())
            }
            assertEquals("OAuth is disabled; no authorization status may be queried", 0, gateway.statusCalls)
            assertEquals("OAuth is disabled; no token may be acquired", 0, gateway.tokenCalls)
        }
    }
}
