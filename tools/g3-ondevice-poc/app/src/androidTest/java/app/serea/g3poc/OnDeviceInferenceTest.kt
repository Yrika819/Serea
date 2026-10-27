package app.serea.g3poc

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import org.junit.Assert.assertFalse
import org.junit.Assert.assertEquals
import org.junit.Assert.fail
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import android.os.Build
import android.os.Debug
import android.os.SystemClock
import android.util.Log
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.TimeUnit

@RunWith(AndroidJUnit4::class)
class OnDeviceInferenceTest {
    companion object {
        private const val PINNED_MODEL_SHA256 =
            "e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5"
        private const val PINNED_MODEL_SIZE_BYTES = 101_016_128L
    }

    @Test
    fun pinnedModelLoadsGeneratesCancelsReleasesAndRejectsCorruptFile() = runBlocking {
        val arguments = InstrumentationRegistry.getArguments()
        val modelPath = arguments.getString("modelPath")
            ?: error("modelPath instrumentation argument is required; inference is not skipped")
        val expectedHash = arguments.getString("modelSha256")
            ?: error("modelSha256 instrumentation argument is required; inference is not skipped")
        assertTrue("instrumentation must use the fixed SmolLM2 Q4_K_M SHA-256", expectedHash == PINNED_MODEL_SHA256)
        val model = File(modelPath)
        assertTrue("pinned model must be staged into app-private storage", model.isFile)
        assertTrue("model byte size must match the fixed fixture", model.length() == PINNED_MODEL_SIZE_BYTES)
        assertTrue("model artifact hash must match fixed fixture", sha256(model) == expectedHash)
        val targetContext = InstrumentationRegistry.getInstrumentation().targetContext
        val modelPathCanonical = model.canonicalFile.toPath()
        val privateRoots = listOf(targetContext.filesDir, targetContext.cacheDir)
            .map { it.canonicalFile.toPath() }
        assertTrue(
            "model file must remain inside target app-private files/cache",
            privateRoots.any { modelPathCanonical.startsWith(it) },
        )
        assertTrue(
            "this focused run is restricted to a disposable Android Emulator",
            Build.FINGERPRINT.contains("generic", ignoreCase = true) ||
                Build.HARDWARE.contains("ranchu", ignoreCase = true) ||
                Build.HARDWARE.contains("goldfish", ignoreCase = true),
        )
        assertTrue("the planned runner must exercise x86_64 emulator JNI", Build.SUPPORTED_ABIS.firstOrNull() == "x86_64")

        val nativeEngine = NativeLlamaEngine()
        val port = GgufModelChatPort(model.absolutePath, expectedHash, nativeEngine)
        val pssBeforeLoadKb = Debug.getPss()
        val loadStarted = SystemClock.elapsedRealtime()
        var loadElapsedMs = 0L
        var firstElapsedMs = 0L
        var secondElapsedMs = 0L
        var cancellationElapsedMs = 0L
        var recoveryElapsedMs = 0L
        var pssAfterLoadKb = 0L
        var pssAfterGenerationKb = 0L
        var pssAfterCloseKb = 0L
        try {
            phase("model_load_started")
            port.load()
            loadElapsedMs = SystemClock.elapsedRealtime() - loadStarted
            pssAfterLoadKb = Debug.getPss()
            phase("model_load_completed elapsed_ms=$loadElapsedMs")

            val secondaryEngine = NativeLlamaEngine()
            val secondaryPort = GgufModelChatPort(model.absolutePath, expectedHash, secondaryEngine)
            try {
                try {
                    secondaryPort.load()
                    fail("a second engine must be rejected while the first owns the native model lease")
                } catch (_: IllegalStateException) {
                    // The process-wide native resource lease is owned by nativeEngine.
                }
                assertTrue("the first engine must retain its loaded model", nativeEngine.hasNativeModelForTest())

                // Both engine instances start request numbering at 1. The JNI owner ID must
                // prevent the second instance's cancellation/free calls from touching the first.
                nativeEngine.armStartGateForTest()
                // The test thread waits on a blocking latch below. Dispatch this coroutine
                // away from runBlocking's event loop so it can submit the native request.
                val firstRun = async(Dispatchers.IO) {
                    port.send(ModelChatRequest("synthetic", "Lease isolation check.", maxOutputTokens = 8))
                }
                phase("foreign_engine_generation_submitted")
                var gateReached = false
                var foreignFree: kotlinx.coroutines.Deferred<Boolean>? = null
                try {
                    gateReached = nativeEngine.awaitStartGateForTest(15, TimeUnit.SECONDS)
                    phase("foreign_engine_generation_gate_reached=$gateReached starts=${nativeEngine.nativeGenerationStartsForTest()}")
                    assertTrue("the first engine must enter its native generation gate", gateReached)
                    gateReached = true
                    val activeRequestId = nativeEngine.activeGenerationIdForTest()
                    assertEquals("first request ID provides a deterministic namespace collision", 1L, activeRequestId)
                    foreignFree = async(Dispatchers.IO) { secondaryEngine.attemptNativeFreeForTest() }
                    assertFalse(
                        "same-numbered cancellation from a non-owner must be ignored",
                        secondaryEngine.attemptForeignCancellationForTest(checkNotNull(activeRequestId)),
                    )
                } finally {
                    nativeEngine.releaseStartGateForTest()
                    if (!gateReached) firstRun.cancel()
                }
                assertFalse("foreign free must not release the first engine", checkNotNull(foreignFree).await())
                assertFalse("first generation must survive foreign cancellation/free", firstRun.await().text.isBlank())
                assertTrue("first native model must remain loaded after foreign free", nativeEngine.hasNativeModelForTest())
            } finally {
                secondaryPort.close()
            }

            val firstStarted = SystemClock.elapsedRealtime()
            val first = port.send(
                ModelChatRequest("synthetic", "Write one short greeting.", maxOutputTokens = 12),
            )
            firstElapsedMs = SystemClock.elapsedRealtime() - firstStarted
            assertFalse("single-turn generation must produce text", first.text.isBlank())

            val secondStarted = SystemClock.elapsedRealtime()
            val second = port.send(
                ModelChatRequest(
                    "synthetic",
                    "Continue with one short sentence.",
                    context = listOf(
                        ContextMessage("user", "Write one short greeting."),
                        ContextMessage("assistant", first.text),
                    ),
                    maxOutputTokens = 12,
                ),
            )
            secondElapsedMs = SystemClock.elapsedRealtime() - secondStarted
            assertFalse("multi-turn generation must produce text", second.text.isBlank())

            val startsBeforeQueuedCancellation = nativeEngine.nativeGenerationStartsForTest()
            nativeEngine.armQueuedCancellationTest()
            nativeEngine.blockExecutorForTest()
            var cancelQueuedRun: (() -> Unit)? = null
            try {
                assertTrue(
                    "test executor blocker must start before queuing the cancelled request",
                    nativeEngine.awaitExecutorBlockedForTest(15, TimeUnit.SECONDS),
                )
                val queuedRun = async(Dispatchers.IO) {
                    port.send(ModelChatRequest("synthetic", "This request is cancelled before JNI entry.", maxOutputTokens = 8))
                }
                cancelQueuedRun = { queuedRun.cancel() }
                assertTrue(
                    "cancelled request must be queued behind the deterministic executor blocker",
                    nativeEngine.awaitGenerationQueuedForTest(15, TimeUnit.SECONDS),
                )
                queuedRun.cancel()
                try {
                    queuedRun.await()
                    fail("queued request must remain cancelled")
                } catch (_: CancellationException) {
                    // Expected: native work has not entered and must not start later.
                }
            } finally {
                cancelQueuedRun?.invoke()
                nativeEngine.releaseExecutorBlockForTest()
            }
            assertTrue(
                "executor must observe and skip the pre-cancelled queued request",
                nativeEngine.awaitQueuedGenerationSkippedForTest(15, TimeUnit.SECONDS),
            )
            assertEquals(
                "cancellation before JNI entry must not start native generation",
                startsBeforeQueuedCancellation,
                nativeEngine.nativeGenerationStartsForTest(),
            )

            nativeEngine.armStartGateForTest()
            val longRun = async(Dispatchers.IO) {
                port.send(ModelChatRequest("synthetic", "Continue this sequence: 1, 2, 3,", maxOutputTokens = 256))
            }
            phase("cancellation_generation_submitted")
            var cancellationStarted = 0L
            try {
                val cancellationGateReached = nativeEngine.awaitStartGateForTest(15, TimeUnit.SECONDS)
                phase("cancellation_generation_gate_reached=$cancellationGateReached starts=${nativeEngine.nativeGenerationStartsForTest()}")
                assertTrue("native generation must reach the deterministic cancellation gate", cancellationGateReached)
                cancellationStarted = SystemClock.elapsedRealtime()
            } finally {
                // Always cancel and release the native test barrier, even when the start assertion fails.
                longRun.cancel()
                nativeEngine.releaseStartGateForTest()
                nativeEngine.awaitFinishedForTest(5, TimeUnit.SECONDS)
            }
            assertTrue(
                "native generation must return after cancellation",
                nativeEngine.awaitFinishedForTest(15, TimeUnit.SECONDS),
            )
            cancellationElapsedMs = SystemClock.elapsedRealtime() - cancellationStarted
            try {
                longRun.await()
                error("cancelling the coroutine must stop native inference")
            } catch (_: CancellationException) {
                // Expected cancellation path: JNI abort callback observes cancellation.
            }

            val recoveryStarted = SystemClock.elapsedRealtime()
            val afterCancel = port.send(
                ModelChatRequest("synthetic", "Reply with a short word.", maxOutputTokens = 8),
            )
            recoveryElapsedMs = SystemClock.elapsedRealtime() - recoveryStarted
            assertFalse("model must recover after cancellation", afterCancel.text.isBlank())
            pssAfterGenerationKb = Debug.getPss()
        } finally {
            port.close()
            pssAfterCloseKb = Debug.getPss()
        }
        try {
            port.send(ModelChatRequest("synthetic", "This must be rejected after close.", maxOutputTokens = 4))
            fail("closed ModelChatPort must reject another request")
        } catch (_: IllegalStateException) {
            // Expected: close released the engine and permanently closed the adapter.
        }
        Log.i(
            "G3_ONDEVICE_METRICS",
            "scope=x86_64_emulator api=${Build.VERSION.SDK_INT} " +
                "load_elapsed_ms=$loadElapsedMs first_turn_elapsed_ms=$firstElapsedMs " +
                "multi_turn_elapsed_ms=$secondElapsedMs cancel_to_native_return_ms=$cancellationElapsedMs " +
                "recovery_elapsed_ms=$recoveryElapsedMs pss_before_load_kb=$pssBeforeLoadKb " +
                "pss_after_load_kb=$pssAfterLoadKb pss_after_generation_kb=$pssAfterGenerationKb " +
                "pss_after_close_kb=$pssAfterCloseKb model_bytes=${model.length()} " +
                "abi=${Build.SUPPORTED_ABIS.first()}",
        )

        val reopenedEngine = NativeLlamaEngine()
        val reopenedPort = GgufModelChatPort(model.absolutePath, expectedHash, reopenedEngine)
        try {
            reopenedPort.load()
            assertFalse(
                "a new engine can load and generate after the prior owner has closed",
                reopenedPort.send(ModelChatRequest("synthetic", "Reopened engine check.", maxOutputTokens = 8)).text.isBlank(),
            )
        } finally {
            reopenedPort.close()
        }
        assertFalse("new engine close must release its own native state", reopenedEngine.hasNativeModelForTest())

        val racingEngine = NativeLlamaEngine()
        racingEngine.armLoadCloseRaceTest()
        racingEngine.blockExecutorForTest()
        var racingLoad: kotlinx.coroutines.Deferred<Unit>? = null
        var racingClose: kotlinx.coroutines.Deferred<Unit>? = null
        try {
            assertTrue(
                "race test executor blocker must be active before queuing model load",
                racingEngine.awaitExecutorBlockedForTest(15, TimeUnit.SECONDS),
            )
            racingLoad = async(Dispatchers.IO) { racingEngine.load(model.absolutePath) }
            assertTrue("load request must be queued behind the executor blocker", racingEngine.awaitLoadQueuedForTest(15, TimeUnit.SECONDS))
            racingClose = async(Dispatchers.IO) { racingEngine.close() }
            assertTrue("close must mark the engine closed before queue release", racingEngine.awaitCloseStartedForTest(15, TimeUnit.SECONDS))
        } finally {
            racingEngine.releaseExecutorBlockForTest()
            if (racingClose == null) racingClose = async(Dispatchers.IO) { racingEngine.close() }
        }
        var loadRejectedByClose = false
        try {
            withTimeout(15_000) { checkNotNull(racingLoad).await() }
        } catch (_: IllegalStateException) {
            loadRejectedByClose = true
        }
        withTimeout(15_000) { checkNotNull(racingClose).await() }
        assertTrue("queued load must be skipped after close wins admission", racingEngine.awaitLoadSkippedForTest(5, TimeUnit.SECONDS))
        assertTrue("load-vs-close race must reject or cancel load", loadRejectedByClose)
        assertFalse("closed engine must not publish native model state", racingEngine.hasNativeModelForTest())
        assertEquals("no lease should have been acquired by skipped load", false, racingEngine.nativeFreeResultForTest())

        val corrupt = File(targetContext.cacheDir, "corrupt-model.gguf")
        corrupt.writeBytes(byteArrayOf(0x00, 0x01, 0x02, 0x03, 0x04))
        val corruptHash = "08bb5e5d6eaac1049ede0893d30ed022b1a4d9b5b48db414871f51c9cb35283d"
        assertTrue("corrupt fixture bytes must match their pinned digest", sha256(corrupt) == corruptHash)
        val corruptPort = GgufModelChatPort(corrupt.absolutePath, corruptHash)
        try {
            try {
                corruptPort.load()
                fail("invalid GGUF bytes must be rejected by the native loader")
            } catch (_: IllegalStateException) {
                // The exact-size corrupt fixture passes the hash check, then llama.cpp rejects it.
            }
        } finally {
            corruptPort.close()
            corrupt.delete()
        }
    }

    private fun sha256(file: File): String {
        val digest = MessageDigest.getInstance("SHA-256")
        file.inputStream().buffered().use { input ->
            val buffer = ByteArray(64 * 1024)
            while (true) {
                val count = input.read(buffer)
                if (count < 0) break
                digest.update(buffer, 0, count)
            }
        }
        return digest.digest().joinToString("") { "%02x".format(it) }
    }

    private fun phase(detail: String) {
        Log.i("G3_TEST_PHASE", "elapsed_realtime_ms=${SystemClock.elapsedRealtime()} $detail")
    }
}
