package app.serea.android

import java.io.File
import java.io.IOException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.nio.file.LinkOption
import java.nio.file.StandardOpenOption

/** Strict structural validation for the AndroidX ProfileVerifier 1.4.0 private cache file. */
internal object ProfileVerifierCacheMarker {
    const val FILE_NAME = "profileInstalled"
    private const val SCHEMA = 1
    private const val BYTE_COUNT = 24

    // A cache entry alone does not prove that a profile was installed: result 0 means no profile,
    // and 5 << 16 is the embedded-profile failure. Only successful installation outcomes qualify.
    private val persistedResultCodes = setOf(1, 2, 3)

    fun isOnlyValidMarker(filesDirectory: File, packageLastUpdateTime: Long): Boolean {
        return try {
            if (packageLastUpdateTime <= 0L) return false
            val directoryPath = filesDirectory.toPath()
            if (Files.isSymbolicLink(directoryPath) ||
                !Files.isDirectory(directoryPath, LinkOption.NOFOLLOW_LINKS)
            ) return false

            val entries = filesDirectory.listFiles() ?: return false
            if (entries.size != 1 || entries.single().name != FILE_NAME) return false

            val marker = entries.single()
            val markerPath = marker.toPath()
            if (Files.isSymbolicLink(markerPath) ||
                !Files.isRegularFile(markerPath, LinkOption.NOFOLLOW_LINKS) ||
                marker.canonicalFile.parentFile != filesDirectory.canonicalFile
            ) return false

            val bytes = readMarkerWithoutFollowingLinks(markerPath) ?: return false
            isValidBytes(bytes, packageLastUpdateTime)
        } catch (_: IOException) {
            false
        } catch (_: SecurityException) {
            false
        }
    }

    fun rejectionDetails(filesDirectory: File, packageLastUpdateTime: Long): String {
        return try {
            if (packageLastUpdateTime <= 0L) return "packageUpdateTime=invalid"
            val directoryPath = filesDirectory.toPath()
            if (Files.isSymbolicLink(directoryPath)) return "filesDir=symlink"
            if (!Files.isDirectory(directoryPath, LinkOption.NOFOLLOW_LINKS)) return "filesDir=not-directory"
            val entries = filesDirectory.listFiles() ?: return "filesDir=list-failed"
            if (entries.size != 1) return "entryCount=${entries.size}"
            val marker = entries.single()
            if (marker.name != FILE_NAME) return "entryName=unexpected"
            val markerPath = marker.toPath()
            if (Files.isSymbolicLink(markerPath)) return "marker=symlink"
            if (!Files.isRegularFile(markerPath, LinkOption.NOFOLLOW_LINKS)) return "marker=not-regular"
            if (marker.canonicalFile.parentFile != filesDirectory.canonicalFile) return "marker=not-direct-child"
            val bytes = readMarkerWithoutFollowingLinks(markerPath) ?: return "markerLengthOrRead=invalid"
            val cache = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)
            val schema = cache.int
            val resultCode = cache.int
            val storedPackageUpdateTime = cache.long
            val installedCurrentProfileSize = cache.long
            "schema=$schema resultCode=$resultCode packageUpdateTimeMatches=${storedPackageUpdateTime == packageLastUpdateTime} " +
                "profileSizeNonnegative=${installedCurrentProfileSize >= 0L}"
        } catch (_: IOException) {
            "inspection=io-failed"
        } catch (_: SecurityException) {
            "inspection=access-denied"
        } catch (_: RuntimeException) {
            "inspection=malformed"
        }
    }

    private fun readMarkerWithoutFollowingLinks(path: java.nio.file.Path): ByteArray? {
        var bytes: ByteArray? = null
        Files.newByteChannel(path, setOf(StandardOpenOption.READ, LinkOption.NOFOLLOW_LINKS)).use { channel ->
            if (channel.size() != BYTE_COUNT.toLong()) return null
            val buffer = ByteBuffer.allocate(BYTE_COUNT)
            while (buffer.hasRemaining()) {
                if (channel.read(buffer) < 0) return null
            }
            if (channel.read(ByteBuffer.allocate(1)) != -1) return null
            bytes = buffer.array()
        }
        return bytes
    }

    internal fun isValidBytes(bytes: ByteArray, packageLastUpdateTime: Long): Boolean {
        if (bytes.size != BYTE_COUNT || packageLastUpdateTime <= 0L) return false
        return try {
            val cache = ByteBuffer.wrap(bytes).order(ByteOrder.BIG_ENDIAN)
            val schema = cache.int
            val resultCode = cache.int
            val storedPackageUpdateTime = cache.long
            val installedCurrentProfileSize = cache.long
            !cache.hasRemaining() &&
                schema == SCHEMA &&
                resultCode in persistedResultCodes &&
                storedPackageUpdateTime == packageLastUpdateTime &&
                installedCurrentProfileSize >= 0L
        } catch (_: RuntimeException) {
            false
        }
    }
}
