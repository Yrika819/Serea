package app.serea.android.core

import android.content.Context
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import java.util.concurrent.CompletableFuture
import java.util.UUID
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.serea_android_bridge.close as nativeClose
import uniffi.serea_android_bridge.getStatus as nativeStatus
import uniffi.serea_android_bridge.initialize as nativeInitialize
import uniffi.serea_android_bridge.schemaVersion as nativeSchema

/** Gates are outside UniFFI: these tests do not claim overlap inside native code. */
@RunWith(AndroidJUnit4::class)
class EmbeddedCoreNativeRaceTest {
    // The adapter owns a process-wide handle. Only close after this initialization succeeds;
    // on timeout the callback queues cleanup even if initialization completes later.
    private fun initialize(adapter: EmbeddedCoreAdapter): CompletableFuture<SyntheticCoreLease> =
        adapter.initializeForCleanup()

    private fun finish(
        native: GatedNative,
        closeQueued: Boolean,
        adapter: EmbeddedCoreAdapter,
        initializations: List<CompletableFuture<SyntheticCoreLease>>,
        database: File,
        cacheDirectory: File,
        primary: Throwable?,
    ) {
        var cleanupFailure: Throwable? = null
        try {
            SyntheticRaceCleanup.run(
                adapter, initializations, if (closeQueued) native.closeResult else null,
                60, TimeUnit.SECONDS, { native.releaseStatus.countDown() }, {},
            )
        } catch (failure: Throwable) {
            cleanupFailure = failure
        }
        if (cleanupFailure == null) {
            try { SyntheticRaceCleanup.removeOwnedSyntheticDatabase(database, cacheDirectory) }
            catch (failure: Throwable) { cleanupFailure = failure }
        }
        if (primary != null) {
            cleanupFailure?.let(primary::addSuppressed)
            throw primary
        }
        cleanupFailure?.let { throw it }
    }

    private class GatedNative : SyntheticCoreNative {
        val statusReturned = CountDownLatch(1)
        val releaseStatus = CountDownLatch(1)
        val closeResult = CompletableFuture<Unit>()
        override fun initialize(path: String) = nativeInitialize(path)
        override fun status(handle: ULong): Boolean {
            val result = nativeStatus(handle)
            statusReturned.countDown()
            check(releaseStatus.await(10, TimeUnit.SECONDS))
            return result
        }
        override fun schema(handle: ULong) = nativeSchema(handle)
        override fun close(handle: ULong) {
            try {
                nativeClose(handle)
                closeResult.complete(Unit)
            } catch (failure: Throwable) {
                closeResult.completeExceptionally(failure)
                throw failure
            }
        }
    }

    @Test fun cancelledQueuedCloseStillClosesAndAllowsReopen(): Unit = synchronized(EmbeddedCoreTestLock.monitor) {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val native = GatedNative()
        val path = File(context.cacheDir, "synthetic-race-${UUID.randomUUID()}.db").absolutePath
        val adapter = EmbeddedCoreAdapter({ path }, native)
        val initializations = mutableListOf<CompletableFuture<SyntheticCoreLease>>()
        var closeQueued = false
        var primary: Throwable? = null
        try {
            val initialization = initialize(adapter).also { initializations.add(it) }
            val lease = initialization.get(60, TimeUnit.SECONDS)
            val status = adapter.getStatus()
            assertTrue(native.statusReturned.await(10, TimeUnit.SECONDS))
            val close = adapter.closeIfOwned(lease)
            // Retain the lease: an ambiguous close failure leaves ownership for cleanup to retry.
            closeQueued = true
            assertTrue(close.cancel(false))
            val afterClose = adapter.schemaVersion()
            native.releaseStatus.countDown()
            assertTrue(status.get(10, TimeUnit.SECONDS))
            assertTrue(close.isCancelled)
            assertThrows(ExecutionException::class.java) { afterClose.get(10, TimeUnit.SECONDS) }
            // The queued close ran despite cancellation of its Future.
            initialize(adapter).also { initializations.add(it) }.get(60, TimeUnit.SECONDS)
        } catch (error: Throwable) {
            primary = error
            throw error
        } finally {
            finish(native, closeQueued, adapter, initializations, File(path), context.cacheDir, primary)
        }
    }

    @Test fun cancelledRunningStatusStillFinishesBeforeQueuedClose(): Unit = synchronized(EmbeddedCoreTestLock.monitor) {
        val context = ApplicationProvider.getApplicationContext<Context>()
        val native = GatedNative()
        val path = File(context.cacheDir, "synthetic-race-${UUID.randomUUID()}.db").absolutePath
        val adapter = EmbeddedCoreAdapter({ path }, native)
        val initializations = mutableListOf<CompletableFuture<SyntheticCoreLease>>()
        var closeQueued = false
        var primary: Throwable? = null
        try {
            val initialization = initialize(adapter).also { initializations.add(it) }
            val lease = initialization.get(60, TimeUnit.SECONDS)
            val status = adapter.getStatus()
            assertTrue(native.statusReturned.await(10, TimeUnit.SECONDS))
            assertTrue(status.cancel(true))
            val close = adapter.closeIfOwned(lease)
            // Retain the lease: an ambiguous close failure leaves ownership for cleanup to retry.
            closeQueued = true
            val afterClose = adapter.getStatus()
            native.releaseStatus.countDown()
            close.get(60, TimeUnit.SECONDS)
            assertTrue(status.isCancelled)
            assertThrows(ExecutionException::class.java) { afterClose.get(10, TimeUnit.SECONDS) }
            initialize(adapter).also { initializations.add(it) }.get(60, TimeUnit.SECONDS)
        } catch (error: Throwable) {
            primary = error
            throw error
        } finally {
            finish(native, closeQueued, adapter, initializations, File(path), context.cacheDir, primary)
        }
    }
}
