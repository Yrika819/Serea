package app.serea.android

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
    }
}
