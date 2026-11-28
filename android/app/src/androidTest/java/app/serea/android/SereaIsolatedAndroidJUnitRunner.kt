package app.serea.android

import android.os.Build
import android.os.Bundle
import android.content.pm.PackageManager
import android.util.Log
import androidx.test.runner.AndroidJUnitRunner
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File

/**
 * Checks campaign arguments before starting tests. Campaign manifests remove declared app/test
 * components, but this is not a pre-process boundary: framework or Application initialization may
 * already have run. Persistent-AVD campaigns also require host package, signing, device and ledger
 * preflight before installation.
 */
class SereaIsolatedAndroidJUnitRunner : AndroidJUnitRunner() {
    override fun onCreate(arguments: Bundle) {
        // Build-only checks do not touch app storage; repeat at the final gate.
        checkSupportedPlatform()
        // Exploratory IME and Fake authorization UI tests run through focused campaigns. Keep the
        // manifest-listed identities outside the formal 105-test Required set, while retaining
        // the R4 required IME assertion embedded in SereaAppShellTest. An explicit class filter
        // opts into those focused cases.
        val explicitClassFilter = arguments.getString("class").orEmpty()
        if (arguments.containsKey(S1B_CAMPAIGN_ARGUMENT)) {
            val proposedId = arguments.getString(S1B_CAMPAIGN_ARGUMENT).orEmpty()
            check(Regex("""s1b[a-f0-9]{10}""").matches(proposedId)) {
                "Invalid persistent-AVD campaign identifier."
            }
            check(explicitClassFilter in S1B_ALLOWED_TEST_METHODS) {
                "Only a specifically approved test method may run on a persistent AVD."
            }
            val proposedToken = arguments.getString(CAMPAIGN_TOKEN_ARGUMENT).orEmpty()
            check(Regex("""[0-9a-f]{32}""").matches(proposedToken)) {
                "Invalid persistent-AVD campaign token."
            }
        }
        if (explicitClassFilter.isBlank() && arguments.getString("sereaIncludeDiagnostics") != "true") {
            val diagnosticClasses = context.assets
                .open("serea-non-required-test-identities.txt")
                .bufferedReader()
                .useLines { lines ->
                    lines.map(String::trim)
                        .filter { it.isNotEmpty() && !it.startsWith("#") }
                        .map { identity -> identity.substringBefore('#') }
                        .distinct()
                        .toList()
                }
            check(diagnosticClasses.isNotEmpty()) {
                "Non-required test identity manifest must explicitly list exploratory tests."
            }
            arguments.putString(
                "notClass",
                diagnosticClasses.joinToString(","),
            )
        }
        super.onCreate(arguments)
    }

    override fun onStart() {
        val target = targetContext
        val dataDir = File(target.applicationInfo.dataDir)
        checkSupportedPlatform()
        val preferences = File(dataDir, "shared_prefs")
        val preferencesEmpty = isAbsentOrEmptyDirectory(preferences)
        val filesDirectory = target.filesDir
        val filesDirectoryEntries = filesDirectory.listFiles()?.map { it.name }?.toSet()
        val filesEmpty = isAbsentOrEmptyDirectory(filesDirectory)
        // AndroidX ProfileVerifier 1.4.0 persists this schema'd cache on supported API levels.
        // API 26 is unsupported by ProfileVerifier and therefore must still have an empty filesDir.
        val packageLastUpdateTime = runCatching {
            @Suppress("DEPRECATION")
            target.packageManager.getPackageInfo(target.packageName, 0).lastUpdateTime
        }.getOrDefault(-1L)
        val profileVerifierApiSupported = Build.VERSION.SDK_INT >= 28 && Build.VERSION.SDK_INT != 30
        val onlyKnownPlatformProfileMarker = profileVerifierApiSupported &&
            ProfileVerifierCacheMarker.isOnlyValidMarker(filesDirectory, packageLastUpdateTime)
        val profileMarkerDetails = if (profileVerifierApiSupported && !filesEmpty) {
            ProfileVerifierCacheMarker.rejectionDetails(filesDirectory, packageLastUpdateTime)
        } else {
            if (profileVerifierApiSupported) "marker=absent" else "api=unsupported"
        }
        Log.i(
            PROFILE_LOG_TAG,
            "PROFILE_MARKER_CHECK api=${Build.VERSION.SDK_INT} present=${!filesEmpty} " +
                "valid=$onlyKnownPlatformProfileMarker details=$profileMarkerDetails",
        )
        val filesSafe = filesEmpty || onlyKnownPlatformProfileMarker
        val databaseDirectory = File(dataDir, "databases")
        val databasesEmpty = target.databaseList().isEmpty() &&
            isAbsentOrEmptyDirectory(databaseDirectory) &&
            !target.getDatabasePath("serea-history.db").exists()
        val marker = File(target.noBackupFilesDir, INSTRUMENTATION_MARKER)
        val claimedMarker = File(target.noBackupFilesDir, CLAIMED_INSTRUMENTATION_MARKER)
        val campaignToken = InstrumentationRegistry.getArguments().getString(CAMPAIGN_TOKEN_ARGUMENT)
        val noBackupFiles = target.noBackupFilesDir.listFiles()?.map { it.name }?.toSet()
        val campaignMarkerMatches = campaignToken != null && marker.isFile && marker.readText() == campaignToken &&
            noBackupFiles == setOf(INSTRUMENTATION_MARKER)

        // The app itself attests to a unique opt-in package. The original CI package
        // remains accepted exactly as before; fresh-data and marker guards stay intact.
        val campaignName = target.packageName.removePrefix("$ISOLATED_PACKAGE.")
        val appMetadata = target.packageManager.getApplicationInfo(
            target.packageName, PackageManager.GET_META_DATA,
        ).metaData
        val attestedPackage = appMetadata?.getString(S1B_CAMPAIGN_METADATA)
        val s1bDiagnostic = target.packageName != ISOLATED_PACKAGE &&
            Regex("""app\.serea\.android\.isolated\.s1b[a-f0-9]{10}""").matches(target.packageName) &&
            attestedPackage == target.packageName &&
            InstrumentationRegistry.getArguments().getString(S1B_CAMPAIGN_ARGUMENT) == campaignName &&
            context.packageName == "${target.packageName}.test"
        check(target.packageName == ISOLATED_PACKAGE || s1bDiagnostic) {
            "Refusing instrumentation against unapproved target ${target.packageName}."
        }
        if (s1bDiagnostic) {
            // A persistent AVD only permits an independently demonstrated,
            // exact JUnit method. Class-wide filters or unreviewed race tests
            // are not permitted on preserved emulator data.
            val selected = InstrumentationRegistry.getArguments().getString("class").orEmpty()
            check(selected in S1B_ALLOWED_TEST_METHODS) {
                "Campaign namespaces require a specifically approved embedded-Core test method."
            }
        }
        check(preferencesEmpty && filesSafe && databasesEmpty && campaignMarkerMatches && !claimedMarker.exists()) {
            "Refusing instrumentation without a fresh isolated namespace and matching CI campaign marker " +
                "(preferencesEmpty=$preferencesEmpty, filesEmpty=$filesEmpty, filesDirEntries=${filesDirectoryEntries?.sorted()}, " +
                "knownProfileInstallerMarker=$onlyKnownPlatformProfileMarker, databasesEmpty=$databasesEmpty, " +
                "knownProfileInstallerMarkerDetails=$profileMarkerDetails, " +
                "campaignTokenPresent=${campaignToken != null}, markerMatches=$campaignMarkerMatches, " +
                "noBackupEntries=${noBackupFiles?.sorted()}, claimedMarkerExists=${claimedMarker.exists()})."
        }
        check(marker.renameTo(claimedMarker)) {
            "Refusing instrumentation because another run already claimed the campaign marker."
        }
        super.onStart()
    }

    private fun checkSupportedPlatform() {
        val hardware = Build.HARDWARE.lowercase()
        val fingerprint = Build.FINGERPRINT.lowercase()
        // The API 35 default image has a distinct emu64x fingerprint from API 26.
        val emulatorFingerprint = fingerprint.contains("generic") ||
            fingerprint.contains("sdk_gphone") ||
            fingerprint.contains("/sdk_phone64_x86_64/emu64x:")
        val emulator = hardware in setOf("ranchu", "goldfish") && emulatorFingerprint
        val apiSupported = Build.VERSION.SDK_INT == 26 || Build.VERSION.SDK_INT == 35
        check(apiSupported) { "Refusing instrumentation on unsupported API ${Build.VERSION.SDK_INT}." }
        check(emulator) {
            "Refusing instrumentation on an unverified or physical Android device " +
                "(hardware=$hardware, fingerprint=$fingerprint)."
        }
    }

    private fun isAbsentOrEmptyDirectory(directory: File): Boolean {
        if (!directory.exists()) {
            val parent = directory.parentFile ?: return false
            if (!parent.isDirectory) return false
            val siblings = parent.listFiles() ?: return false
            return siblings.none { it.name == directory.name }
        }
        return directory.isDirectory && directory.listFiles()?.isEmpty() == true
    }

    private companion object {
        const val PROFILE_LOG_TAG = "SereaIsolatedRunner"
        const val ISOLATED_PACKAGE = "app.serea.android.isolated"
        const val S1B_CAMPAIGN_METADATA = "app.serea.android.S1B_CAMPAIGN_PACKAGE"
        const val S1B_CAMPAIGN_ARGUMENT = "sereaS1bCampaign"
        val S1B_ALLOWED_TEST_METHODS = setOf(
            "app.serea.android.core.EmbeddedCoreAdapterTest#processOwnerSurvivesAdapterReplacement",
            "app.serea.android.EmbeddedCoreDiagnosticChatUiTest#diagnosticChatUsesCoreMockAcrossRecreationAndCleansUp",
            "app.serea.android.EmbeddedCoreCloudChatIntegrationTest#fixedCloudProbeCrossesRealCoreBeforeLoopbackRequestAndSettlesUsage",
        )
        const val CAMPAIGN_TOKEN_ARGUMENT = "sereaCampaignToken"
        const val INSTRUMENTATION_MARKER = "serea-instrumentation-token"
        const val CLAIMED_INSTRUMENTATION_MARKER = "serea-instrumentation-claimed"
    }
}
