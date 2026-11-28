package app.serea.android.core

import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import java.util.concurrent.TimeoutException
import org.junit.Assert.*
import org.junit.Test

class EmbeddedCoreOwnerTest {
    private class Fake : SyntheticCoreNative {
        @Volatile var current: ULong? = null
        var started: CountDownLatch? = null
        var proceed: CountDownLatch? = null
        var closeStarted: CountDownLatch? = null
        var closeProceed: CountDownLatch? = null
        var closeCount = 0
        var failNextClose = false
        override fun initialize(path: String): ULong {
            started?.countDown()
            check(proceed?.await(5, TimeUnit.SECONDS) != false)
            check(current == null)
            return 17uL.also { current = it }
        }
        override fun status(handle: ULong): Boolean = current == handle
        override fun schema(handle: ULong): UInt { check(current == handle); return 3u }
        override fun close(handle: ULong) {
            closeStarted?.countDown()
            check(closeProceed?.await(5, TimeUnit.SECONDS) != false)
            check(current == handle)
            if (failNextClose) {
                failNextClose = false
                throw IllegalStateException("synthetic close failure")
            }
            closeCount++
            current = null
        }
    }

    @Test fun timedOutInitializationCanCleanUpItsLateLease() {
        val fake = Fake().apply {
            started = CountDownLatch(1)
            proceed = CountDownLatch(1)
        }
        val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val initialization = adapter.initializeForCleanup()
        try {
            assertTrue(fake.started!!.await(5, TimeUnit.SECONDS))
            assertThrows(TimeoutException::class.java) { initialization.get(50, TimeUnit.MILLISECONDS) }
            fake.proceed!!.countDown()
            val lease = initialization.get(5, TimeUnit.SECONDS)
            assertTrue(adapter.getStatus().get(5, TimeUnit.SECONDS))
            assertTrue(adapter.closeIfOwned(lease).get(5, TimeUnit.SECONDS))
            assertEquals(1, fake.closeCount)
            assertNull(fake.current)
        } finally {
            fake.proceed!!.countDown()
            val lease = initialization.get(5, TimeUnit.SECONDS)
            adapter.closeIfOwned(lease).get(5, TimeUnit.SECONDS)
        }
    }

    @Test fun queuedInitializationCannotAcquireOwnerUntilFirstLeaseIsClosed() {
        val fake = Fake().apply {
            started = CountDownLatch(1)
            proceed = CountDownLatch(1)
        }
        val first = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val second = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val initial = first.initializeForCleanup()
        try {
            assertTrue(fake.started!!.await(5, TimeUnit.SECONDS))
            val competing = second.initializeForCleanup()
            fake.proceed!!.countDown()
            val lease = initial.get(5, TimeUnit.SECONDS)
            assertThrows(ExecutionException::class.java) { competing.get(5, TimeUnit.SECONDS) }
            assertTrue(second.getStatus().get(5, TimeUnit.SECONDS))
            assertEquals(0, fake.closeCount)
            assertTrue(first.closeIfOwned(lease).get(5, TimeUnit.SECONDS))
            val replacement = second.initializeForCleanup().get(5, TimeUnit.SECONDS)
            try {
                assertEquals(lease.handle, replacement.handle)
                assertFalse(first.closeIfOwned(lease).get(5, TimeUnit.SECONDS))
                assertTrue(second.getStatus().get(5, TimeUnit.SECONDS))
            } finally {
                second.closeIfOwned(replacement).get(5, TimeUnit.SECONDS)
            }
        } finally {
            fake.proceed!!.countDown()
            val lease = initial.get(5, TimeUnit.SECONDS)
            first.closeIfOwned(lease).get(5, TimeUnit.SECONDS)
        }
    }

    @Test fun staleInitializationCleanupMustNotCloseReinitializedOwnerEvenWhenHandleIsReused() {
        val fake = Fake()
        val first = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val second = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val initialization = first.initializeForCleanup()
        val firstLease = initialization.get(5, TimeUnit.SECONDS)
        first.close().get(5, TimeUnit.SECONDS)
        val secondHandle = second.initialize().get(5, TimeUnit.SECONDS)
        try {
            assertEquals(firstLease.handle, secondHandle) // Native handle ABA.
            assertFalse(initialization.thenCompose { first.closeIfOwned(it) }.get(5, TimeUnit.SECONDS))
            assertEquals(1, fake.closeCount)
            assertTrue(second.getStatus().get(5, TimeUnit.SECONDS))
        } finally {
            if (fake.current != null) second.close().get(5, TimeUnit.SECONDS)
        }
    }

    @Test fun failedConditionalCloseRetainsLeaseForRetry() {
        val fake = Fake()
        val adapter = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val lease = adapter.initializeForCleanup().get(5, TimeUnit.SECONDS)
        try {
            fake.failNextClose = true
            assertThrows(ExecutionException::class.java) { adapter.closeIfOwned(lease).get(5, TimeUnit.SECONDS) }
            assertTrue(adapter.getStatus().get(5, TimeUnit.SECONDS))
            assertTrue(adapter.closeIfOwned(lease).get(5, TimeUnit.SECONDS))
            assertNull(fake.current)
        } finally {
            if (fake.current != null) adapter.closeIfOwned(lease).get(5, TimeUnit.SECONDS)
        }
    }

    @Test fun cancelledCloseStillReleasesOwnershipBeforeReinitialize() {
        val fake = Fake()
        fake.closeStarted = CountDownLatch(1)
        fake.closeProceed = CountDownLatch(1)
        val first = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val second = EmbeddedCoreAdapter({ "synthetic" }, fake)
        first.initialize().get(5, TimeUnit.SECONDS)
        val close = first.close()
        assertTrue(fake.closeStarted!!.await(5, TimeUnit.SECONDS))
        assertTrue(close.cancel(false))
        val next = second.initialize()
        fake.closeProceed!!.countDown()
        assertEquals(17uL, next.get(5, TimeUnit.SECONDS))
        second.close().get(5, TimeUnit.SECONDS)
        assertNull(fake.current)
    }

    @Test fun cancelledInitializeStillHasProcessOwner() {
        val fake = Fake()
        fake.started = CountDownLatch(1)
        fake.proceed = CountDownLatch(1)
        val first = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val second = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val future = first.initialize()
        assertTrue(fake.started!!.await(5, TimeUnit.SECONDS))
        assertTrue(future.cancel(false))
        fake.proceed!!.countDown()
        assertTrue(second.getStatus().get(5, TimeUnit.SECONDS))
        second.close().get(5, TimeUnit.SECONDS)
        assertNull(fake.current)
    }

    @Test fun anotherAdapterCanCloseProcessOwnedCore() {
        val fake = Fake()
        val first = EmbeddedCoreAdapter({ "synthetic" }, fake)
        val second = EmbeddedCoreAdapter({ "synthetic" }, fake)
        first.initialize().get(5, TimeUnit.SECONDS)
        try {
            assertThrows(ExecutionException::class.java) { second.initialize().get(5, TimeUnit.SECONDS) }
            assertTrue(second.getStatus().get(5, TimeUnit.SECONDS))
            assertEquals(3u, second.schemaVersion().get(5, TimeUnit.SECONDS))
            second.close().get(5, TimeUnit.SECONDS)
            assertNull(fake.current)
        } finally {
            if (fake.current != null) first.close().get(5, TimeUnit.SECONDS)
        }
    }
}
