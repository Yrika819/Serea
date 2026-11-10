package app.serea.android.googleauth

import android.content.ComponentName
import android.content.pm.PackageManager
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.fail
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class PrivateGooglePublicVariantIsolationTest {
    @Test
    fun publicDebugManifestDoesNotRegisterPrivateConsentOrAccountRelays() {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        listOf(
            "app.serea.android.googleauth.PrivateGoogleConsentRelayActivity",
            "app.serea.android.googleauth.PrivateGoogleAccountPickerActivity",
        ).forEach { className ->
            try {
                @Suppress("DEPRECATION")
                context.packageManager.getActivityInfo(ComponentName(context, className), 0)
                fail("Public Debug manifest registered private activity: $className")
            } catch (_: PackageManager.NameNotFoundException) {
                // Expected: private relay activities are declared only in the privateBeta manifest.
            }
        }
    }
}
