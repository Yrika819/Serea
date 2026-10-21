package app.serea.android

import android.os.Build
import androidx.test.runner.AndroidJUnitRunner
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File

/**
 * Stops instrumentation before any test Activity can start unless the target is a clean,
 * supported disposable emulator app namespace. This makes IDE/direct Gradle entry points safe too.
 */
class SereaIsolatedAndroidJUnitRunner : AndroidJUnitRunner() {
    override fun onStart() {
        val target = targetContext
        val dataDir = File(target.applicationInfo.dataDir)
        val hardware = Build.HARDWARE.lowercase()
        val fingerprint = Build.FINGERPRINT.lowercase()
        val emulator = hardware in setOf("ranchu", "goldfish") &&
            (fingerprint.contains("generic") || fingerprint.contains("sdk_gphone"))
        val apiSupported = Build.VERSION.SDK_INT == 26 || Build.VERSION.SDK_INT == 35
        val preferences = File(dataDir, "shared_prefs")
        val preferencesEmpty = !preferences.exists() || preferences.listFiles().isNullOrEmpty()
        val filesEmpty = target.filesDir.listFiles().isNullOrEmpty()
        val databasesEmpty = target.databaseList().isEmpty()
        val marker = File(target.noBackupFilesDir, INSTRUMENTATION_MARKER)
        val claimedMarker = File(target.noBackupFilesDir, CLAIMED_INSTRUMENTATION_MARKER)
        val campaignToken = InstrumentationRegistry.getArguments().getString(CAMPAIGN_TOKEN_ARGUMENT)
        val noBackupFiles = target.noBackupFilesDir.listFiles()?.map { it.name }?.toSet().orEmpty()
        val campaignMarkerMatches = campaignToken != null && marker.isFile && marker.readText() == campaignToken &&
            noBackupFiles == setOf(INSTRUMENTATION_MARKER)

        check(target.packageName == ISOLATED_PACKAGE) {
            "Refusing instrumentation against ${target.packageName}; use the isolated test application."
        }
        check(apiSupported) { "Refusing instrumentation on unsupported API ${Build.VERSION.SDK_INT}." }
        check(emulator) { "Refusing instrumentation on an unverified or physical Android device." }
        check(preferencesEmpty && filesEmpty && databasesEmpty && campaignMarkerMatches && !claimedMarker.exists()) {
            "Refusing instrumentation without a fresh isolated namespace and matching CI campaign marker."
        }
        check(marker.renameTo(claimedMarker)) {
            "Refusing instrumentation because another run already claimed the campaign marker."
        }
        super.onStart()
    }

    private companion object {
        const val ISOLATED_PACKAGE = "app.serea.android.isolated"
        const val CAMPAIGN_TOKEN_ARGUMENT = "sereaCampaignToken"
        const val INSTRUMENTATION_MARKER = "serea-instrumentation-token"
        const val CLAIMED_INSTRUMENTATION_MARKER = "serea-instrumentation-claimed"
    }
}
