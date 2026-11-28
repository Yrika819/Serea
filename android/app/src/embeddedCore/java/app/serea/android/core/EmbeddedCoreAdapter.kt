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

internal class SyntheticCoreLease internal constructor(val handle: ULong, internal val generation: Any)

/** Caller cancellation never cancels the worker's ownership completion. */
internal class SyntheticInitializationFuture(private val adapter: EmbeddedCoreAdapter) : CompletableFuture<SyntheticCoreLease>() {
    private val ownership = CompletableFuture<SyntheticCoreLease>()
    private var lastCleanup: CompletableFuture<Boolean>? = null

    internal fun completeOwnership(lease: SyntheticCoreLease) {
        ownership.complete(lease)
        super.complete(lease)
    }

    internal fun failOwnership(failure: Throwable) {
        ownership.completeExceptionally(failure)
        super.completeExceptionally(failure)
    }

    override fun cancel(mayInterruptIfRunning: Boolean): Boolean {
        val cancelled = super.cancel(false)
        if (cancelled) registerCleanup(adapter)
        return cancelled
    }

    override fun complete(value: SyntheticCoreLease): Boolean = throw UnsupportedOperationException("worker-owned completion")
    override fun completeExceptionally(ex: Throwable): Boolean = throw UnsupportedOperationException("worker-owned completion")
    override fun obtrudeValue(value: SyntheticCoreLease): Unit = throw UnsupportedOperationException("worker-owned completion")
    override fun obtrudeException(ex: Throwable): Unit = throw UnsupportedOperationException("worker-owned completion")
    override fun completeAsync(supplier: java.util.function.Supplier<out SyntheticCoreLease>): CompletableFuture<SyntheticCoreLease> =
        throw UnsupportedOperationException("worker-owned completion")
    override fun completeAsync(supplier: java.util.function.Supplier<out SyntheticCoreLease>, executor: java.util.concurrent.Executor): CompletableFuture<SyntheticCoreLease> =
        throw UnsupportedOperationException("worker-owned completion")
    override fun orTimeout(timeout: Long, unit: java.util.concurrent.TimeUnit): CompletableFuture<SyntheticCoreLease> =
        throw UnsupportedOperationException("worker-owned completion")
    override fun completeOnTimeout(value: SyntheticCoreLease, timeout: Long, unit: java.util.concurrent.TimeUnit): CompletableFuture<SyntheticCoreLease> =
        throw UnsupportedOperationException("worker-owned completion")

    @Synchronized
    internal fun registerCleanup(adapter: EmbeddedCoreAdapter): CompletableFuture<Boolean> {
        val result = lastCleanup ?: ownership.thenCompose { adapter.closeIfOwned(it) }.also { lastCleanup = it }
        return result.thenApply { it }
    }

    /** Retry is explicit, preserving a failed attempt until a caller chooses to retry. */
    @Synchronized
    internal fun retryCleanup(): CompletableFuture<Boolean> {
        val previous = checkNotNull(lastCleanup)
        check(previous.isDone && previous.isCompletedExceptionally)
        val result = ownership.thenCompose { adapter.closeIfOwned(it) }
        lastCleanup = result
        return result.thenApply { it }
    }

    /** A detached observer: cancelling it cannot cancel the pending cleanup. */
    @Synchronized
    internal fun observeCleanup(): CompletableFuture<Boolean>? = lastCleanup?.thenApply { it }
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
        private var generation: Any? = null
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

    private fun initializeCurrent(): SyntheticCoreLease {
        check(handle == null)
        val current = native.initialize(path())
        val currentGeneration = Any()
        handle = current
        owner = native
        generation = currentGeneration
        return SyntheticCoreLease(current, currentGeneration)
    }

    fun initialize(): CompletableFuture<ULong> = submit { initializeCurrent().handle }

    internal fun initializeForCleanup(): SyntheticInitializationFuture {
        val result = SyntheticInitializationFuture(this)
        worker.execute {
            try { result.completeOwnership(initializeCurrent()) }
            catch (_: Exception) { result.failOwnership(IllegalStateException("Synthetic Core unavailable")) }
            catch (_: LinkageError) { result.failOwnership(IllegalStateException("Synthetic Core unavailable")) }
        }
        return result
    }

    internal fun closeIfOwned(lease: SyntheticCoreLease): CompletableFuture<Boolean> = submit {
        if (generation !== lease.generation) false else {
            closeCurrent()
            true
        }
    }
    fun getStatus(): CompletableFuture<Boolean> = submit { checkNotNull(owner).status(checkNotNull(handle)) }
    fun schemaVersion(): CompletableFuture<UInt> = submit { checkNotNull(owner).schema(checkNotNull(handle)) }
    fun close(): CompletableFuture<Unit> = submit { closeCurrent() }

    private fun closeCurrent() {
        val current = checkNotNull(handle)
        val currentOwner = checkNotNull(owner)
        try {
            currentOwner.close(current)
            handle = null
            owner = null
            generation = null
        } catch (failure: BridgeException.StorageUnavailable) {
            // Rust removes the handle before shutdown; this error means it was consumed.
            handle = null
            owner = null
            generation = null
            throw failure
        }
        // Other errors are ambiguous: keep ownership, fail closed and allow retry.
    }
}
