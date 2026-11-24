package app.serea.android.core

import java.util.concurrent.CountDownLatch
import java.util.concurrent.ExecutionException
import java.util.concurrent.TimeUnit
import org.junit.Assert.*
import org.junit.Test

class EmbeddedCoreOwnerTest {
    private class Fake : SyntheticCoreNative {
        @Volatile var current: ULong? = null
        var started: CountDownLatch? = null
        var proceed: CountDownLatch? = null
        var closeStarted: CountDownLatch? = null
        var closeProceed: CountDownLatch? = null
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
            current = null
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
