package app.serea.android.core

import java.io.File
import java.nio.file.Files
import java.util.UUID
import java.util.concurrent.CompletableFuture
import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import org.junit.Assert.*
import org.junit.Test

class SyntheticRaceCleanupTest {
    private class Fake : SyntheticCoreNative {
        var current: ULong? = null
        var closes = 0
        var failNextClose = false
        var initializeStarted: CountDownLatch? = null
        var initializeProceed: CountDownLatch? = null
        override fun initialize(path: String): ULong {
            initializeStarted?.countDown()
            check(initializeProceed?.await(5, TimeUnit.SECONDS) != false)
            check(current == null)
            current = 17uL
            return 17uL
        }
        override fun status(handle: ULong) = current == handle
        override fun schema(handle: ULong): UInt = 3u
        override fun close(handle: ULong) {
            check(current == handle)
            if (failNextClose) { failNextClose = false; throw IllegalStateException("synthetic close failure") }
            closes++; current = null
        }
    }

    // Contract: run body, release, register conditional cleanup for each lease, then await
    // the optional close and cleanup results; preserve primary and observed failure order.
    private fun run(adapter: EmbeddedCoreAdapter, leases: List<CompletableFuture<SyntheticCoreLease>> = emptyList(),
        close: CompletableFuture<Unit>? = null, release: () -> Unit = {}, body: () -> Unit = {}) =
        SyntheticRaceCleanup.run(adapter, leases, close, 5, TimeUnit.SECONDS, release, body)

    @Test fun externalCompletionCannotFabricateInitializationFailure() {
        val fake = Fake().apply { initializeStarted = CountDownLatch(1); initializeProceed = CountDownLatch(1) }
        val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val initialization = adapter.initializeForCleanup()
        try {
            assertTrue(fake.initializeStarted!!.await(5, TimeUnit.SECONDS))
            assertThrows(UnsupportedOperationException::class.java) {
                initialization.completeExceptionally(IllegalStateException("fabricated"))
            }
            fake.initializeProceed!!.countDown()
            val lease = initialization.get(5, TimeUnit.SECONDS)
            assertTrue(adapter.closeIfOwned(lease).get(5, TimeUnit.SECONDS))
            assertNull(fake.current)
        } finally {
            fake.initializeProceed!!.countDown()
            try { if (adapter.getStatus().get(5, TimeUnit.SECONDS)) adapter.close().get(5, TimeUnit.SECONDS) }
            catch (_: ExecutionException) { /* no owner remains */ }
        }
    }

    @Test fun asynchronousExternalCompletionIsRejected() {
        val fake = Fake().apply { initializeStarted = CountDownLatch(1); initializeProceed = CountDownLatch(1) }
        val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val initialization = adapter.initializeForCleanup()
        try {
            assertTrue(fake.initializeStarted!!.await(5, TimeUnit.SECONDS))
            assertThrows(UnsupportedOperationException::class.java) {
                initialization.completeAsync { SyntheticCoreLease(999uL, Any()) }
            }
            assertThrows(UnsupportedOperationException::class.java) { initialization.orTimeout(1, TimeUnit.DAYS) }
            fake.initializeProceed!!.countDown()
            assertTrue(adapter.closeIfOwned(initialization.get(5, TimeUnit.SECONDS)).get(5, TimeUnit.SECONDS))
        } finally {
            fake.initializeProceed!!.countDown()
            try { if (adapter.getStatus().get(5, TimeUnit.SECONDS)) adapter.close().get(5, TimeUnit.SECONDS) }
            catch (_: ExecutionException) { /* no owner remains */ }
        }
    }

    @Test fun duplicateCleanupRegistrationDoesNotHideFirstFailure() {
        val fake = Fake().apply { initializeStarted = CountDownLatch(1); initializeProceed = CountDownLatch(1) }
        val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val initialization = adapter.initializeForCleanup()
        try {
            assertTrue(fake.initializeStarted!!.await(5, TimeUnit.SECONDS))
            fake.failNextClose = true
            val first = initialization.registerCleanup(adapter)
            val duplicate = initialization.registerCleanup(adapter)
            fake.initializeProceed!!.countDown()
            assertThrows(ExecutionException::class.java) { first.get(5, TimeUnit.SECONDS) }
            assertThrows(ExecutionException::class.java) { duplicate.get(5, TimeUnit.SECONDS) }
            assertThrows(ExecutionException::class.java) { initialization.observeCleanup()!!.get(5, TimeUnit.SECONDS) }
            assertTrue(adapter.getStatus().get(5, TimeUnit.SECONDS))
        } finally {
            fake.initializeProceed!!.countDown()
            try { if (adapter.getStatus().get(5, TimeUnit.SECONDS)) adapter.close().get(5, TimeUnit.SECONDS) }
            catch (_: ExecutionException) { /* no owner remains */ }
        }
    }

    @Test fun cancelWithoutCallerCleanupStillSchedulesOwnedClose() {
        val fake = Fake().apply { initializeStarted = CountDownLatch(1); initializeProceed = CountDownLatch(1) }
        val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val initialization = adapter.initializeForCleanup()
        try {
            assertTrue(fake.initializeStarted!!.await(5, TimeUnit.SECONDS))
            assertTrue(initialization.cancel(false))
            val cleanup = initialization.observeCleanup()
            assertNotNull("cancellation must register cleanup even if caller exits", cleanup)
            fake.initializeProceed!!.countDown()
            assertTrue(cleanup!!.get(5, TimeUnit.SECONDS))
            assertNull(fake.current)
            assertEquals(1, fake.closes)
        } finally {
            fake.initializeProceed!!.countDown()
            try { if (adapter.getStatus().get(5, TimeUnit.SECONDS)) adapter.close().get(5, TimeUnit.SECONDS) }
            catch (_: ExecutionException) { /* no owner remains */ }
        }
    }

    @Test fun cancelledInitializationRetainsCleanupResponsibility() {
        val fake = Fake().apply {
            initializeStarted = CountDownLatch(1)
            initializeProceed = CountDownLatch(1)
        }
        val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val initialization = adapter.initializeForCleanup()
        try {
            assertTrue(fake.initializeStarted!!.await(5, TimeUnit.SECONDS))
            assertTrue(initialization.cancel(false))
            fake.initializeProceed!!.countDown()
            val failure = runCatching { run(adapter, listOf(initialization)) }.exceptionOrNull()
            assertNull("cancelled caller future must not discard cleanup responsibility", failure)
            assertNull(fake.current)
            assertEquals(1, fake.closes)
        } finally {
            fake.initializeProceed!!.countDown()
            if (fake.current != null) adapter.close().get(5, TimeUnit.SECONDS)
        }
    }

    @Test fun cancellationBeforeWorkerEntryStillCleansTheLaterInitialization() {
        val fake = Fake().apply { initializeStarted = CountDownLatch(1); initializeProceed = CountDownLatch(1) }
        val first = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val second = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val blocker = first.initialize()
        try {
            assertTrue(fake.initializeStarted!!.await(5, TimeUnit.SECONDS))
            val firstClose = first.close()
            val queued = second.initializeForCleanup()
            assertTrue(queued.cancel(false))
            fake.initializeProceed!!.countDown()
            blocker.get(5, TimeUnit.SECONDS)
            firstClose.get(5, TimeUnit.SECONDS)
            run(second, listOf(queued))
            assertNull(fake.current)
            assertEquals(2, fake.closes)
            assertTrue(queued.isCancelled)
        } finally {
            fake.initializeProceed!!.countDown()
            runCatching { if (fake.current != null) second.close().get(5, TimeUnit.SECONDS) }
        }
    }

    @Test fun cancellationRacingInitializationCompletionNeverLosesCleanup() {
        repeat(16) {
            val fake = Fake().apply { initializeStarted = CountDownLatch(1); initializeProceed = CountDownLatch(1) }
            val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
            val initialization = adapter.initializeForCleanup()
            val start = CountDownLatch(1)
            val ready = CountDownLatch(1)
            val cancelling = Thread {
                ready.countDown()
                check(start.await(5, TimeUnit.SECONDS))
                initialization.cancel(false)
            }
            try {
                assertTrue(fake.initializeStarted!!.await(5, TimeUnit.SECONDS))
                cancelling.start()
                assertTrue(ready.await(5, TimeUnit.SECONDS))
                start.countDown()
                fake.initializeProceed!!.countDown()
                cancelling.join(5000)
                assertFalse("cancel thread failed to terminate", cancelling.isAlive)
                run(adapter, listOf(initialization))
                assertNull(fake.current)
                assertEquals(1, fake.closes)
            } finally {
                start.countDown()
                fake.initializeProceed!!.countDown()
                cancelling.join(5000)
                if (fake.current != null) adapter.close().get(5, TimeUnit.SECONDS)
            }
        }
    }

    @Test fun successfulCompletionBeforeCancelPreservesTheLease() {
        val fake = Fake(); val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val initialization = adapter.initializeForCleanup()
        try {
            initialization.get(5, TimeUnit.SECONDS)
            assertFalse(initialization.cancel(false))
            run(adapter, listOf(initialization))
            assertNull(fake.current); assertEquals(1, fake.closes)
        } finally { if (fake.current != null) adapter.close().get(5, TimeUnit.SECONDS) }
    }

    @Test fun cancelledLeaseCannotCloseReplacementWithReusedHandle() {
        val fake = Fake().apply { initializeStarted = CountDownLatch(1); initializeProceed = CountDownLatch(1) }
        val first = EmbeddedCoreAdapter({ "synthetic" }, fake); val second = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val cancelled = first.initializeForCleanup()
        try {
            assertTrue(fake.initializeStarted!!.await(5, TimeUnit.SECONDS)); assertTrue(cancelled.cancel(false))
            fake.initializeProceed!!.countDown()
            assertTrue(cancelled.observeCleanup()!!.get(5, TimeUnit.SECONDS))
            val replacement = second.initializeForCleanup().get(5, TimeUnit.SECONDS)
            try {
                assertEquals(17uL, replacement.handle)
                run(first, listOf(cancelled))
                assertTrue(second.getStatus().get(5, TimeUnit.SECONDS)); assertEquals(1, fake.closes)
            } finally { second.closeIfOwned(replacement).get(5, TimeUnit.SECONDS) }
        } finally {
            fake.initializeProceed!!.countDown()
            runCatching { if (fake.current != null) second.close().get(5, TimeUnit.SECONDS) }
        }
    }

    @Test fun failedCancelledLeaseCleanupIsObservableAndRetryable() {
        val fake = Fake().apply { initializeStarted = CountDownLatch(1); initializeProceed = CountDownLatch(1) }
        val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake); val initialization = adapter.initializeForCleanup()
        try {
            assertTrue(fake.initializeStarted!!.await(5, TimeUnit.SECONDS)); assertTrue(initialization.cancel(false))
            fake.failNextClose = true
            fake.initializeProceed!!.countDown()
            assertThrows(ExecutionException::class.java) { run(adapter, listOf(initialization)) }
            assertThrows(ExecutionException::class.java) { initialization.observeCleanup()!!.get(5, TimeUnit.SECONDS) }
            assertTrue(adapter.getStatus().get(5, TimeUnit.SECONDS))
            assertTrue(initialization.retryCleanup().get(5, TimeUnit.SECONDS))
            assertTrue(initialization.observeCleanup()!!.get(5, TimeUnit.SECONDS))
            assertNull(fake.current); assertEquals(1, fake.closes)
        } finally {
            fake.initializeProceed!!.countDown()
            runCatching { if (fake.current != null) adapter.close().get(5, TimeUnit.SECONDS) }
        }
    }

    @Test fun cancelledFutureTimeoutStillCleansLateCompletionAndObserverIsDetached() {
        val fake = Fake().apply { initializeStarted = CountDownLatch(1); initializeProceed = CountDownLatch(1) }
        val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake); val initialization = adapter.initializeForCleanup()
        try {
            assertTrue(fake.initializeStarted!!.await(5, TimeUnit.SECONDS)); assertTrue(initialization.cancel(false))
            assertThrows(java.util.concurrent.TimeoutException::class.java) {
                SyntheticRaceCleanup.run(adapter, listOf(initialization), null, 30, TimeUnit.MILLISECONDS, {}, {})
            }
            assertTrue(initialization.observeCleanup()!!.cancel(false))
            fake.initializeProceed!!.countDown()
            assertTrue(initialization.observeCleanup()!!.get(5, TimeUnit.SECONDS))
            assertNull(fake.current); assertEquals(1, fake.closes)
        } finally {
            fake.initializeProceed!!.countDown()
            runCatching { if (fake.current != null) adapter.close().get(5, TimeUnit.SECONDS) }
        }
    }

    @Test fun bodyFailureIsPreservedAndLeaseIsClosed() {
        val fake = Fake(); val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val lease = adapter.initializeForCleanup(); lease.get(5, TimeUnit.SECONDS)
        val body = IllegalStateException("body")
        val thrown = assertThrows(IllegalStateException::class.java) { run(adapter, listOf(lease)) { throw body } }
        assertSame(body, thrown); assertEquals(0, thrown.suppressed.size); assertNull(fake.current)
    }

    @Test fun cleanupFailureIsThrownWhenBodySucceeds() {
        val fake = Fake(); val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val failure = IllegalStateException("release")
        val thrown = assertThrows(IllegalStateException::class.java) { run(adapter, release = { throw failure }) }
        assertSame(failure, thrown)
    }

    @Test fun bodyRemainsPrimaryWhenCleanupFails() {
        val adapter = EmbeddedCoreAdapter({ "synthetic" }, Fake())
        val body = IllegalArgumentException("body"); val cleanup = IllegalStateException("release")
        val thrown = assertThrows(IllegalArgumentException::class.java) {
            run(adapter, release = { throw cleanup }) { throw body }
        }
        assertSame(body, thrown); assertArrayEquals(arrayOf(cleanup), thrown.suppressed)
    }

    @Test fun multipleCleanupFailuresAreRetainedAndLaterLeaseStillCloses() {
        val fake = Fake(); val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val lease = adapter.initializeForCleanup(); lease.get(5, TimeUnit.SECONDS)
        val release = IllegalStateException("release")
        val failedClose = CompletableFuture<Unit>().apply { completeExceptionally(IllegalStateException("close")) }
        val failedLease = CompletableFuture<SyntheticCoreLease>().apply { completeExceptionally(IllegalStateException("init")) }
        val thrown = assertThrows(IllegalStateException::class.java) {
            run(adapter, listOf(failedLease, lease), failedClose, { throw release })
        }
        assertSame(release, thrown); assertEquals(2, thrown.suppressed.size)
        assertTrue(thrown.suppressed[0] is ExecutionException); assertTrue(thrown.suppressed[1] is ExecutionException)
        assertEquals("close", thrown.suppressed[0].cause?.message)
        assertEquals("init", thrown.suppressed[1].cause?.message)
        assertNull(fake.current); assertEquals(1, fake.closes)
    }

    @Test fun timedOutCleanupStillClosesLeaseThatArrivesLater() {
        val fake = Fake(); val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val lease = adapter.initializeForCleanup().get(5, TimeUnit.SECONDS)
        val late = CompletableFuture<SyntheticCoreLease>()
        try {
            assertThrows(java.util.concurrent.TimeoutException::class.java) {
                SyntheticRaceCleanup.run(adapter, listOf(late), null, 30, TimeUnit.MILLISECONDS, {}, {})
            }
            late.complete(lease)
            assertThrows(ExecutionException::class.java) { adapter.getStatus().get(5, TimeUnit.SECONDS) }
            assertEquals(1, fake.closes)
        } finally {
            late.complete(lease)
            adapter.closeIfOwned(lease).get(5, TimeUnit.SECONDS)
        }
    }

    @Test fun replacementOwnerAcquiredDuringReleaseSurvivesStaleCleanup() {
        val fake = Fake(); val first = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val second = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val old = first.initializeForCleanup(); val oldLease = old.get(5, TimeUnit.SECONDS)
        var replacement: SyntheticCoreLease? = null
        try {
            run(first, listOf(old), release = {
                first.closeIfOwned(oldLease).get(5, TimeUnit.SECONDS)
                replacement = second.initializeForCleanup().get(5, TimeUnit.SECONDS)
            })
            assertNotNull(replacement)
            assertTrue(second.getStatus().get(5, TimeUnit.SECONDS))
            assertEquals(1, fake.closes)
        } finally {
            replacement?.let { second.closeIfOwned(it).get(5, TimeUnit.SECONDS) }
                ?: first.closeIfOwned(oldLease).get(5, TimeUnit.SECONDS)
        }
    }

    @Test fun removesOnlyTheOwnedSyntheticDatabaseAndSqliteSidecars() {
        val root=Files.createTempDirectory("serea-race-cleanup-").toFile()
        try {
            val cache=File(root,"cache").apply { check(mkdir()) }
            val database=File(cache,"synthetic-race-${UUID.randomUUID()}.db")
            val owned=listOf(database,File("${database.path}-wal"),File("${database.path}-shm"),File("${database.path}-journal"))
            owned.forEach { check(it.createNewFile()) }
            val unrelated=File(cache,"unrelated.db").apply { check(createNewFile()) }

            SyntheticRaceCleanup.removeOwnedSyntheticDatabase(database,cache)

            owned.forEach { assertFalse("owned SQLite artifact remains: ${it.name}",it.exists()) }
            assertTrue("cleanup touched an unrelated database",unrelated.exists())
        } finally { root.deleteRecursively() }
    }

    @Test fun refusesToDeleteSyntheticDatabaseOutsideItsPrivateCacheDirectory() {
        val root=Files.createTempDirectory("serea-race-cleanup-boundary-").toFile()
        try {
            val cache=File(root,"cache").apply { check(mkdir()) }
            val outside=File(root,"synthetic-race-${UUID.randomUUID()}.db").apply { check(createNewFile()) }
            assertThrows(IllegalArgumentException::class.java) {
                SyntheticRaceCleanup.removeOwnedSyntheticDatabase(outside,cache)
            }
            assertTrue(outside.exists())
        } finally { root.deleteRecursively() }
    }

    @Test fun staleLeaseDoesNotCloseReplacementOwnerWithReusedHandle() {
        val fake = Fake(); val first = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val second = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val old = first.initializeForCleanup(); val oldLease = old.get(5, TimeUnit.SECONDS)
        first.closeIfOwned(oldLease).get(5, TimeUnit.SECONDS)
        val replacement = second.initializeForCleanup().get(5, TimeUnit.SECONDS)
        try {
            assertEquals(oldLease.handle, replacement.handle)
            run(first, listOf(old))
            assertTrue(second.getStatus().get(5, TimeUnit.SECONDS)); assertEquals(1, fake.closes)
        } finally { second.closeIfOwned(replacement).get(5, TimeUnit.SECONDS) }
    }
}
