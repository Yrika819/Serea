package app.serea.android

import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder

class ProfileVerifierCacheMarkerTest {
    @get:Rule val temporary = TemporaryFolder()

    @Test fun acceptsOnlyCanonicalPersistedProfileVerifierResults() {
        for (resultCode in listOf(0, 1, 2, 3, 5 shl 16)) {
            assertTrue(
                "Expected ProfileVerifier result $resultCode to be a persisted schema 1 value",
                ProfileVerifierCacheMarker.isValidBytes(cacheBytes(resultCode = resultCode), PACKAGE_UPDATE_TIME),
            )
        }
    }

    @Test fun rejectsUnknownSchemaOrResultAndStalePackageUpdateTime() {
        assertFalse(ProfileVerifierCacheMarker.isValidBytes(cacheBytes(schema = 2), PACKAGE_UPDATE_TIME))
        assertFalse(ProfileVerifierCacheMarker.isValidBytes(cacheBytes(resultCode = 7), PACKAGE_UPDATE_TIME))
        assertFalse(
            ProfileVerifierCacheMarker.isValidBytes(cacheBytes(packageUpdateTime = PACKAGE_UPDATE_TIME - 1), PACKAGE_UPDATE_TIME),
        )
        assertFalse(ProfileVerifierCacheMarker.isValidBytes(cacheBytes(), 0L))
    }

    @Test fun rejectsNegativeProfileSizeTruncationAndTrailingData() {
        assertFalse(
            ProfileVerifierCacheMarker.isValidBytes(cacheBytes(installedProfileSize = -1L), PACKAGE_UPDATE_TIME),
        )
        val canonical = cacheBytes()
        assertFalse(ProfileVerifierCacheMarker.isValidBytes(canonical.copyOf(canonical.size - 1), PACKAGE_UPDATE_TIME))
        assertFalse(ProfileVerifierCacheMarker.isValidBytes(canonical + byteArrayOf(0), PACKAGE_UPDATE_TIME))
    }

    @Test fun acceptsOnlyOneRegularDirectChildWithTheExactPlatformName() {
        val filesDirectory = temporary.newFolder("files")
        assertFalse(ProfileVerifierCacheMarker.isOnlyValidMarker(filesDirectory, PACKAGE_UPDATE_TIME))

        File(filesDirectory, ProfileVerifierCacheMarker.FILE_NAME).writeBytes(cacheBytes())
        assertTrue(ProfileVerifierCacheMarker.isOnlyValidMarker(filesDirectory, PACKAGE_UPDATE_TIME))

        File(filesDirectory, "extra").writeText("unexpected")
        assertFalse(ProfileVerifierCacheMarker.isOnlyValidMarker(filesDirectory, PACKAGE_UPDATE_TIME))
    }

    @Test fun rejectsWrongNameAndSymbolicLink() {
        val filesDirectory = temporary.newFolder("files")
        File(filesDirectory, "profileinstaller_profileWrittenFor_lastUpdateTime.dat").writeBytes(cacheBytes())
        assertFalse(ProfileVerifierCacheMarker.isOnlyValidMarker(filesDirectory, PACKAGE_UPDATE_TIME))

        val marker = File(filesDirectory, ProfileVerifierCacheMarker.FILE_NAME)
        marker.delete()
        val outside = temporary.newFile("outside")
        outside.writeBytes(cacheBytes())
        Files.createSymbolicLink(marker.toPath(), outside.toPath())
        assertFalse(ProfileVerifierCacheMarker.isOnlyValidMarker(filesDirectory, PACKAGE_UPDATE_TIME))
    }

    private fun cacheBytes(
        schema: Int = 1,
        resultCode: Int = 0,
        packageUpdateTime: Long = PACKAGE_UPDATE_TIME,
        installedProfileSize: Long = 128L,
    ): ByteArray = ByteBuffer.allocate(24)
        .order(ByteOrder.BIG_ENDIAN)
        .putInt(schema)
        .putInt(resultCode)
        .putLong(packageUpdateTime)
        .putLong(installedProfileSize)
        .array()

    private companion object {
        const val PACKAGE_UPDATE_TIME = 1_797_000_000_000L
    }
}
