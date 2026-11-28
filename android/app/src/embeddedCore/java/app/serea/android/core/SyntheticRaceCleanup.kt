package app.serea.android.core

import java.io.File
import java.nio.file.Files
import java.nio.file.LinkOption
import java.util.concurrent.CompletableFuture
import java.util.concurrent.TimeUnit

/** Shared by the synthetic diagnostic and its host regression tests; never owns another lease. */
internal object SyntheticRaceCleanup {
    /** Remove only a test-generated SQLite database and its sidecars under the app's private cache. */
    fun removeOwnedSyntheticDatabase(database: File, cacheDirectory: File) {
        val expectedName = Regex("synthetic-race-[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}\\.db")
        require(expectedName.matches(database.name)) { "Database name is outside the synthetic test namespace" }
        require(Files.isDirectory(cacheDirectory.toPath(), LinkOption.NOFOLLOW_LINKS)) {
            "Cache cleanup root must be a real directory"
        }
        require(!Files.isSymbolicLink(cacheDirectory.toPath())) { "Cache cleanup root must not be a symlink" }
        val root = cacheDirectory.canonicalFile.toPath()
        require(database.parentFile?.canonicalFile?.toPath() == root) {
            "Synthetic database must be directly inside the app cache"
        }
        val artifacts = listOf(File("${database.path}-wal"), File("${database.path}-shm"), File("${database.path}-journal"), database)
        artifacts.forEach { artifact ->
            if (!Files.exists(artifact.toPath(), LinkOption.NOFOLLOW_LINKS)) return@forEach
            require(!Files.isSymbolicLink(artifact.toPath())) { "SQLite artifact must not be a symlink" }
            require(Files.isRegularFile(artifact.toPath(), LinkOption.NOFOLLOW_LINKS)) {
                "SQLite artifact must be a regular file"
            }
            require(artifact.canonicalFile.parentFile?.toPath() == root) {
                "SQLite artifact escaped the app cache"
            }
        }
        artifacts.forEach { Files.deleteIfExists(it.toPath()) }
    }

    fun run(
        adapter: EmbeddedCoreAdapter,
        leases: List<CompletableFuture<SyntheticCoreLease>>,
        close: CompletableFuture<Unit>?,
        timeout: Long,
        unit: TimeUnit,
        release: () -> Unit,
        body: () -> Unit,
    ) {
        var failure: Throwable? = null
        fun retain(error: Throwable) {
            if (failure == null) failure = error else if (failure !== error) failure.addSuppressed(error)
        }
        try { body() } catch (error: Throwable) { retain(error) }
        try { release() } catch (error: Throwable) { retain(error) }
        // Register each callback before waiting, so later leases are still cleaned after a timeout.
        val cleanups = leases.map { initialization ->
            if (initialization is SyntheticInitializationFuture) initialization.registerCleanup(adapter)
            else initialization.thenCompose { adapter.closeIfOwned(it) }
        }
        try { close?.get(timeout, unit) } catch (error: Throwable) { retain(error) }
        for (cleanup in cleanups) {
            try { cleanup.get(timeout, unit) } catch (error: Throwable) { retain(error) }
        }
        failure?.let { throw it }
    }
}
