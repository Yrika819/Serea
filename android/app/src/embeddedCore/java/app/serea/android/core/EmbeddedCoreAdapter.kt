package app.serea.android.core

import android.content.Context
import java.io.File
import java.util.concurrent.CompletableFuture
import java.util.concurrent.Executors
import uniffi.serea_android_bridge.BridgeException
import uniffi.serea_android_bridge.close as closeCore
import uniffi.serea_android_bridge.getStatus as coreStatus
import uniffi.serea_android_bridge.initialize as initializeCore
import uniffi.serea_android_bridge.schemaVersion as coreSchemaVersion

internal interface SyntheticCoreNative {
    fun initialize(path: String): ULong
    fun status(handle: ULong): Boolean
    fun schema(handle: ULong): UInt
    fun close(handle: ULong)
}

private object RealSyntheticCore : SyntheticCoreNative {
    override fun initialize(path: String) = initializeCore(path)
    override fun status(handle: ULong) = coreStatus(handle)
    override fun schema(handle: ULong) = coreSchemaVersion(handle)
    override fun close(handle: ULong) = closeCore(handle)
}

/** Isolated synthetic diagnostic only. Nothing here is connected to product UI or Room. */
class EmbeddedCoreAdapter internal constructor(
    private val path: () -> String,
    private val native: SyntheticCoreNative,
) {
    constructor(context: Context) : this(syntheticPath(context.applicationContext), RealSyntheticCore)

    companion object {
        private fun syntheticPath(appContext: Context): () -> String = {
            val directory = File(appContext.noBackupFilesDir, "serea-core")
            check(directory.isDirectory || directory.mkdirs())
            File(directory, "synthetic-v1.db").absolutePath
        }
        private val worker = Executors.newSingleThreadExecutor { r ->
            Thread(r, "serea-synthetic-core").apply { isDaemon = true }
        }
        // Process-owned and accessed only on the single worker. Futures do not own the Core.
        private var handle: ULong? = null
        private var owner: SyntheticCoreNative? = null
    }

    private fun <T> submit(block: () -> T): CompletableFuture<T> {
        val result = CompletableFuture<T>()
        worker.execute {
            try { result.complete(block()) }
            catch (_: Exception) { result.completeExceptionally(IllegalStateException("Synthetic Core unavailable")) }
            catch (_: LinkageError) { result.completeExceptionally(IllegalStateException("Synthetic Core unavailable")) }
        }
        return result
    }

    fun initialize(): CompletableFuture<ULong> = submit {
        check(handle == null)
        native.initialize(path()).also { handle = it; owner = native }
    }
    fun getStatus(): CompletableFuture<Boolean> = submit { checkNotNull(owner).status(checkNotNull(handle)) }
    fun schemaVersion(): CompletableFuture<UInt> = submit { checkNotNull(owner).schema(checkNotNull(handle)) }
    fun close(): CompletableFuture<Unit> = submit {
        val current = checkNotNull(handle)
        val currentOwner = checkNotNull(owner)
        try {
            currentOwner.close(current)
            handle = null
            owner = null
        } catch (failure: BridgeException.StorageUnavailable) {
            // Rust removes the handle before shutdown; this error means it was consumed.
            handle = null
            owner = null
            throw failure
        }
        // Other errors are ambiguous: keep ownership, fail closed and allow retry.
    }
}
