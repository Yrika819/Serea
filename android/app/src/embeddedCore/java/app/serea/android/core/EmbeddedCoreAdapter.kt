package app.serea.android.core

import android.content.Context
import java.io.File
import java.util.concurrent.CompletableFuture
import java.util.concurrent.Executors
import uniffi.serea_android_bridge.close as closeCore
import uniffi.serea_android_bridge.getStatus as coreStatus
import uniffi.serea_android_bridge.initialize as initializeCore
import uniffi.serea_android_bridge.schemaVersion as coreSchemaVersion

/** Isolated synthetic diagnostic only. Nothing here is connected to product UI or Room. */
class EmbeddedCoreAdapter(context: Context) {
    private val appContext = context.applicationContext
    companion object {
        private val worker = Executors.newSingleThreadExecutor { r ->
            Thread(r, "serea-synthetic-core").apply { isDaemon = true }
        }
    }
    // Accessed only on worker; the Rust bridge also enforces one process-local owner.
    private var handle: ULong? = null

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
        val directory = File(appContext.noBackupFilesDir, "serea-core")
        check(directory.isDirectory || directory.mkdirs())
        initializeCore(File(directory, "synthetic-v1.db").absolutePath).also { handle = it }
    }

    fun getStatus(): CompletableFuture<Boolean> = submit { coreStatus(checkNotNull(handle)) }
    fun schemaVersion(): CompletableFuture<UInt> = submit { coreSchemaVersion(checkNotNull(handle)) }
    fun close(): CompletableFuture<Unit> = submit {
        val current = checkNotNull(handle)
        // Rust consumes the Core even when checkpointing reports a failure.
        handle = null
        closeCore(current)
    }
}
