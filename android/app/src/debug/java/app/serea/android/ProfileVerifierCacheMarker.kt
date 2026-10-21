package app.serea.android

import java.io.File
import java.io.IOException
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.nio.file.Files
import java.nio.file.LinkOption

/** Strict structural validation for the AndroidX ProfileVerifier 1.4.0 private cache file. */
internal object ProfileVerifierCacheMarker {
    const val FILE_NAME = "profileInstalled"
    private const val SCHEMA = 1
    private const val BYTE_COUNT = 24

    // ProfileVerifier 1.4.0 may persist these results. Its other error results return before
    // writing the cache (or are unsupported on API 35).
    private val persistedResultCodes = setOf(0, 1, 2, 3, 5 shl 16)

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
                marker.canonicalFile.parentFile != filesDirectory.canonicalFile ||
                Files.size(markerPath) != BYTE_COUNT.toLong()
            ) return false

            isValidBytes(Files.readAllBytes(markerPath), packageLastUpdateTime)
        } catch (_: IOException) {
            false
        } catch (_: SecurityException) {
            false
        }
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
