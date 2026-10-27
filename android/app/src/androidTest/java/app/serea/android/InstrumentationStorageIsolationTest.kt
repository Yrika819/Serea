package app.serea.android

import android.os.Build
import android.util.Log
import androidx.profileinstaller.ProfileVerifier
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import java.io.File
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/** Guard the Android instrumentation variant's package-private data boundary. */
@RunWith(AndroidJUnit4::class)
class InstrumentationStorageIsolationTest {
    @Test fun instrumentationUsesAnAppPrivateNamespaceSeparateFromDebugDemo() {
        val target = InstrumentationRegistry.getInstrumentation().targetContext
        val database = target.getDatabasePath("serea-history.db").canonicalFile
        val preferences = File(
            target.applicationInfo.dataDir,
            "shared_prefs/model_selection.xml",
        ).canonicalFile
        val campaignMarker = File(target.noBackupFilesDir, "serea-instrumentation-claimed")
        val demoDataDirectory = target.applicationInfo.dataDir.replace(
            "app.serea.android.isolated",
            "app.serea.android.demo",
        )

        assertEquals("app.serea.android.isolated", target.packageName)
        assertNotEquals(demoDataDirectory, target.applicationInfo.dataDir)
        assertTrue(database.path.contains("app.serea.android.isolated"))
        assertFalse(database.path.contains("app.serea.android.demo"))
        assertEquals(File(target.applicationInfo.dataDir, "databases/serea-history.db").canonicalPath, database.path)
        assertTrue(preferences.path.contains("app.serea.android.isolated"))
        assertFalse(preferences.path.contains("app.serea.android.demo"))
        assertTrue("tests require an atomically claimed one-run campaign marker", campaignMarker.isFile)

        // Instrumentation test methods run off the main thread; this worker-thread API executes
        // ProfileVerifier directly and returns its own result rather than inferring execution
        // from an empty-before-run cache check.
        val compilationStatus = ProfileVerifier.writeProfileVerification(target)
        val resultCode = compilationStatus.profileInstallResultCode
        if (Build.VERSION.SDK_INT == 26) {
            assertEquals(
                "API 26 must report ProfileVerifier's explicit unsupported result",
                ProfileVerifier.CompilationStatus.RESULT_CODE_ERROR_UNSUPPORTED_API_VERSION,
                resultCode,
            )
            val profileVerifierMarker = File(target.filesDir, ProfileVerifierCacheMarker.FILE_NAME)
            assertFalse(
                "unsupported API 26 must not persist a ProfileVerifier marker; " +
                    "filesDirEntries=${target.filesDir.listFiles()?.map { it.name }?.sorted()}",
                profileVerifierMarker.exists(),
            )
            Log.i(
                PROFILE_VERIFIER_EVIDENCE_TAG,
                "api=26 status=UNSUPPORTED resultCode=$resultCode embedded=false marker=absent",
            )
        } else {
            @Suppress("DEPRECATION")
            val packageUpdateTime = target.packageManager.getPackageInfo(target.packageName, 0).lastUpdateTime
            val markerDetails = ProfileVerifierCacheMarker.rejectionDetails(target.filesDir, packageUpdateTime)
            assertTrue(
                "API 35 ProfileVerifier must run and report an installed profile, resultCode=$resultCode " +
                    "(1=compiled, 2=installed and queued for compilation, 3=compiled with a non-matching profile) " +
                    "embedded=${compilationStatus.appApkHasEmbeddedProfile()} marker=$markerDetails",
                resultCode in setOf(
                    ProfileVerifier.CompilationStatus.RESULT_CODE_COMPILED_WITH_PROFILE,
                    ProfileVerifier.CompilationStatus.RESULT_CODE_PROFILE_ENQUEUED_FOR_COMPILATION,
                    ProfileVerifier.CompilationStatus.RESULT_CODE_COMPILED_WITH_PROFILE_NON_MATCHING,
                ),
            )
            assertTrue(
                "API 35 APK must embed the profile, resultCode=$resultCode",
                compilationStatus.appApkHasEmbeddedProfile(),
            )
            assertTrue(
                "API 35 ProfileVerifier cache must prove an installed profile, " +
                    "resultCode=$resultCode marker=$markerDetails",
                ProfileVerifierCacheMarker.isValidMarker(target.filesDir, packageUpdateTime),
            )
            Log.i(
                PROFILE_VERIFIER_EVIDENCE_TAG,
                "api=${Build.VERSION.SDK_INT} status=COMPLETED resultCode=$resultCode " +
                    "compiled=${compilationStatus.isCompiledWithProfile} " +
                    "enqueued=${compilationStatus.hasProfileEnqueuedForCompilation()} " +
                    "embedded=${compilationStatus.appApkHasEmbeddedProfile()} marker=valid",
            )
        }
    }

    private companion object {
        const val PROFILE_VERIFIER_EVIDENCE_TAG = "SereaProfileVerifier"
    }
}
