package app.serea.android.core

import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import androidx.test.platform.app.InstrumentationRegistry
import androidx.test.core.app.ActivityScenario
import androidx.lifecycle.Lifecycle
import app.serea.android.MainActivity
import java.util.concurrent.ExecutionException
import uniffi.serea_android_bridge.BridgeException
import uniffi.serea_android_bridge.getStatus as rawStatus
import uniffi.serea_android_bridge.initialize as rawInitialize
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.TimeUnit
import java.util.concurrent.CountDownLatch
import java.util.concurrent.CompletableFuture

@RunWith(AndroidJUnit4::class)
class EmbeddedCoreAdapterTest {
    @Test fun syntheticLifecycle() {
        val adapter = EmbeddedCoreAdapter(ApplicationProvider.getApplicationContext<Context>())
        val heartbeat = CountDownLatch(1)
        lateinit var initial: CompletableFuture<ULong>
        val start = SystemClock.elapsedRealtime()
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            initial = adapter.initialize()
            Handler(Looper.getMainLooper()).post { heartbeat.countDown() }
        }
        assertTrue("main looper heartbeat timed out", heartbeat.await(5, TimeUnit.SECONDS))
        assertTrue(initial.get(60, TimeUnit.SECONDS) > 0uL)
        Log.i("SereaEmbeddedCoreTest", "first_open_ms=${SystemClock.elapsedRealtime() - start}")
        assertTrue(adapter.getStatus().get(10, TimeUnit.SECONDS))
        assertTrue(adapter.schemaVersion().get(10, TimeUnit.SECONDS) > 0u)
        assertThrows(Exception::class.java) { adapter.initialize().get(10, TimeUnit.SECONDS) }
        adapter.close().get(10, TimeUnit.SECONDS)
        assertThrows(Exception::class.java) { adapter.close().get(10, TimeUnit.SECONDS) }
        assertThrows(Exception::class.java) { adapter.getStatus().get(10, TimeUnit.SECONDS) }
        val reopenStart = SystemClock.elapsedRealtime()
        adapter.initialize().get(60, TimeUnit.SECONDS)
        Log.i("SereaEmbeddedCoreTest", "reopen_ms=${SystemClock.elapsedRealtime() - reopenStart}")
        adapter.close().get(10, TimeUnit.SECONDS)
        assertThrows(BridgeException.InvalidHandle::class.java) { rawStatus(ULong.MAX_VALUE) }
        assertThrows(BridgeException.StorageUnavailable::class.java) { rawInitialize("/dev/null/synthetic.db") }
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            val error = assertThrows(ExecutionException::class.java) {
                adapter.getStatus().get(10, TimeUnit.SECONDS)
            }
            assertEquals("Synthetic Core unavailable", error.cause?.message)
            val uiHeartbeat = CountDownLatch(1)
            Handler(Looper.getMainLooper()).post { uiHeartbeat.countDown() }
            assertTrue("UI stopped responding after Core error", uiHeartbeat.await(5, TimeUnit.SECONDS))
            assertTrue(scenario.state.isAtLeast(Lifecycle.State.RESUMED))
        }
    }
}
