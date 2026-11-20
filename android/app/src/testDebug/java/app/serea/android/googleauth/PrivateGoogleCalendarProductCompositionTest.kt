package app.serea.android.googleauth

import app.serea.android.BuildConfig
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class PrivateGoogleCalendarProductCompositionTest {
    @Test
    fun debugProductCannotConstructTestOnlyLiveComposition() {
        assertFalse(BuildConfig.BUILD_TYPE == "isolated")

        val liveComposition = runCatching {
            PrivateGoogleCalendarComposition(
                privateVariantEnabled = true,
                googleHttpEnabledByOwner = true,
            )
        }

        assertTrue(
            "Live HTTP test overrides must be rejected outside the isolated test variant",
            liveComposition.isFailure,
        )
        assertFalse(PrivateGoogleCalendarComposition().opensGoogleProvider)
    }
}
