package app.serea.android.googleauth

import app.serea.android.BuildConfig
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PrivateBuildOAuthGateTest {
    @Test fun oauthGateRequiresBothPrivateVariantAndCallsiteOptIn() {
        assertFalse(BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED)
        assertEquals(BuildConfig.BUILD_TYPE == "privateBeta", BuildConfig.PRIVATE_GOOGLE_AUTH_UI_ENABLED)
        assertEquals(BuildConfig.OWNER_PRIVATE_BETA_GOOGLE_OAUTH_ENABLED, ownerPrivateGoogleOAuthEnabled(true))
        assertFalse(ownerPrivateGoogleOAuthEnabled(requestedByLocalPrivateBuild = false))
        if (BuildConfig.BUILD_TYPE == "privateBeta") {
            assertEquals("app.serea.android.privatebeta", BuildConfig.APPLICATION_ID)
            assertTrue(BuildConfig.VERSION_CODE >= 2)
        }
    }
}
