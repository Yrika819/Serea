package app.serea.android

import java.io.File
import java.util.ArrayDeque
import java.util.concurrent.AbstractExecutorService
import java.util.concurrent.CountDownLatch
import java.util.concurrent.RejectedExecutionException
import java.util.concurrent.TimeUnit
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.launch
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.TestScope
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LocalModelNativeRuntimeLifecycleTest {
    @Test fun loadGenerateAndCloseUseTheBridgeAndConfirmBothReleaseLayers() = runTest {
        val calls = FakeNativeCalls()
        val coordinator = NativeReleaseCoordinator()
        val executor = ManualExecutor()
        val backend = backend(calls, coordinator, executor)

        val loading = async { backend.load(modelFile().path, nativeLimits()) }
        runCurrent()
        assertFalse(loading.isCompleted)
        executor.runNext()
        runCurrent()
        loading.await()
        assertEquals(NativeReleaseCoordinator.State.ACTIVE, coordinator.currentState())
        assertEquals(1, calls.loadCount)

        val generated = async { backend.generate("hello", 8) }
        runCurrent()
        executor.runNext()
        runCurrent()
        assertEquals("ok", generated.await().text)
        assertEquals(1, calls.generateCount)

        backend.close()
        runCurrent()
        assertEquals(NativeReleaseCoordinator.State.RELEASING, coordinator.currentState())
        executor.runNext()
        runCurrent()
        assertTrue(backend.awaitReleased(1, TimeUnit.SECONDS))
        assertEquals(NativeReleaseResult.Released, backend.awaitReleaseResult(1, TimeUnit.SECONDS))
        assertEquals(NativeReleaseCoordinator.State.RELEASED, coordinator.currentState())
        assertEquals(1, calls.freeCount)
    }

    @Test fun cancellationAfterExecutorEnqueueDoesNotEnterNativeLoad() = runTest {
        val calls = FakeNativeCalls()
        val coordinator = NativeReleaseCoordinator()
        val executor = ManualExecutor()
        val backend = backend(calls, coordinator, executor)

        val loading = launch { backend.load(modelFile().path, nativeLimits()) }
        runCurrent()
        loading.cancelAndJoin()
        executor.runNext()
        runCurrent()

        assertEquals(0, calls.loadCount)
        assertEquals(NativeReleaseCoordinator.State.RELEASED, coordinator.currentState())
        assertTrue(backend.awaitReleased(1, TimeUnit.SECONDS))
        assertEquals(NativeReleaseResult.NotAcquired, backend.awaitReleaseResult(1, TimeUnit.SECONDS))
    }

    @Test fun closeWinsQueuedLoadRaceWithoutCallingJniAndIsIdempotent() = runTest {
        val calls = FakeNativeCalls()
        val coordinator = NativeReleaseCoordinator()
        val executor = ManualExecutor()
        val backend = backend(calls, coordinator, executor)

        val loading = async { backend.load(modelFile().path, nativeLimits()) }
        runCurrent()
        backend.close()
        backend.close()
        runCurrent()
        executor.runNext() // queued load observes close and never enters JNI
        runCurrent()
        executor.runNext() // close owner confirms the unloaded lease
        runCurrent()

        assertTrue(loading.isCompleted)
        assertEquals(0, calls.loadCount)
        assertEquals(0, calls.freeCount)
        assertTrue(backend.awaitReleased(1, TimeUnit.SECONDS))
        assertEquals(NativeReleaseResult.NotAcquired, backend.awaitReleaseResult(1, TimeUnit.SECONDS))
        assertEquals(NativeReleaseCoordinator.State.RELEASED, coordinator.currentState())
    }

    @Test fun nativeReleaseFailureAndCoordinatorRejectionBothFailClosed() = runTest {
        val calls = FakeNativeCalls().apply { freeSucceeds = false }
        val coordinator = NativeReleaseCoordinator()
        val executor = ManualExecutor()
        val backend = backend(calls, coordinator, executor)
        load(backend, executor)

        backend.close()
        runCurrent()
        executor.runNext()
        runCurrent()
        assertFalse(backend.awaitReleased(1, TimeUnit.SECONDS))
        assertTrue(backend.awaitReleaseResult(1, TimeUnit.SECONDS) is NativeReleaseResult.Failed)
        assertEquals(NativeReleaseCoordinator.State.RELEASE_FAILED, coordinator.currentState())
        assertTrue(runCatching { coordinator.acquire(Any()) }.isFailure)

        val calls2 = FakeNativeCalls()
        val rejectingCoordinator = RejectReleaseCoordinator()
        val executor2 = ManualExecutor()
        val backend2 = backend(calls2, rejectingCoordinator, executor2)
        load(backend2, executor2)
        backend2.close()
        runCurrent()
        executor2.runNext()
        runCurrent()
        assertFalse(backend2.awaitReleased(1, TimeUnit.SECONDS))
        assertTrue(backend2.awaitReleaseResult(1, TimeUnit.SECONDS) is NativeReleaseResult.Failed)
        assertEquals(NativeReleaseCoordinator.State.RELEASE_FAILED, rejectingCoordinator.currentState())

        val calls3 = FakeNativeCalls().apply { freeLeavesOwner = true }
        val coordinator3 = NativeReleaseCoordinator()
        val executor3 = ManualExecutor()
        val backend3 = backend(calls3, coordinator3, executor3)
        load(backend3, executor3)
        backend3.close()
        runCurrent()
        executor3.runNext()
        runCurrent()
        assertTrue(backend3.awaitReleaseResult(1, TimeUnit.SECONDS) is NativeReleaseResult.Failed)
        assertEquals(NativeReleaseCoordinator.State.RELEASE_FAILED, coordinator3.currentState())
    }

    @Test fun processCoordinatorRejectsOtherBackendAndAllowsNewOwnerOnlyAfterRelease() = runTest {
        val coordinator = NativeReleaseCoordinator()
        val firstExecutor = ManualExecutor()
        val first = backend(FakeNativeCalls(), coordinator, firstExecutor)
        load(first, firstExecutor)

        val secondExecutor = ManualExecutor()
        val second = backend(FakeNativeCalls(), coordinator, secondExecutor)
        val failedLoad = async { runCatching { second.load(modelFile().path, nativeLimits()) } }
        runCurrent()
        assertTrue(failedLoad.await().isFailure)
        assertEquals(0, secondExecutor.queuedCount)

        first.close()
        runCurrent()
        firstExecutor.runNext()
        runCurrent()
        assertTrue(first.awaitReleased(1, TimeUnit.SECONDS))

        val nextExecutor = ManualExecutor()
        val next = backend(FakeNativeCalls(), coordinator, nextExecutor)
        load(next, nextExecutor)
        assertEquals(NativeReleaseCoordinator.State.ACTIVE, coordinator.currentState())
    }

    @Test fun executorRejectionBeforeJniReleasesLeaseButFailsLoad() = runTest {
        val calls = FakeNativeCalls()
        val coordinator = NativeReleaseCoordinator()
        val executor = ManualExecutor().apply { rejectTasks = true }
        val backend = backend(calls, coordinator, executor)

        val result = async { runCatching { backend.load(modelFile().path, nativeLimits()) } }
        runCurrent()
        assertTrue(result.await().isFailure)
        assertEquals(0, calls.loadCount)
        assertEquals(NativeReleaseCoordinator.State.RELEASED, coordinator.currentState())
        assertTrue(backend.awaitReleased(1, TimeUnit.SECONDS))

        val loadedCalls = FakeNativeCalls()
        val loadedCoordinator = NativeReleaseCoordinator()
        val releaseExecutor = ManualExecutor()
        val loadedBackend = backend(loadedCalls, loadedCoordinator, releaseExecutor)
        load(loadedBackend, releaseExecutor)
        releaseExecutor.rejectTasks = true
        loadedBackend.close()
        assertEquals(NativeReleaseCoordinator.State.RELEASE_FAILED, loadedCoordinator.currentState())
        assertTrue(loadedBackend.awaitReleaseResult(1, TimeUnit.SECONDS) is NativeReleaseResult.Failed)
        assertEquals(0, loadedCalls.freeCount)
    }

    @Test fun releaseWaitTimeoutDoesNotPermitAnotherBackendAndLateSuccessIsObservable() = runTest {
        val coordinator = NativeReleaseCoordinator()
        val executor = ManualExecutor()
        val backend = backend(FakeNativeCalls(), coordinator, executor)
        load(backend, executor)

        backend.close()
        runCurrent()
        assertEquals(NativeReleaseCoordinator.State.RELEASING, coordinator.currentState())
        assertEquals(NativeReleaseResult.TimedOut, backend.awaitReleaseResult(1, TimeUnit.MILLISECONDS))
        assertTrue(runCatching { coordinator.acquire(Any()) }.isFailure)

        executor.runNext()
        runCurrent()
        assertTrue(backend.awaitReleased(1, TimeUnit.SECONDS))
        assertEquals(NativeReleaseResult.Released, backend.awaitReleaseResult(1, TimeUnit.SECONDS))
        assertEquals(NativeReleaseCoordinator.State.RELEASED, coordinator.currentState())
    }

    @Test fun nativeLoadFailureWithConfirmedRollbackReleasesLeaseButUnknownOwnerFailsClosed() = runTest {
        val rollbackCalls = FakeNativeCalls().apply { loadSucceeds = false }
        val rollbackCoordinator = NativeReleaseCoordinator()
        val rollbackExecutor = ManualExecutor()
        val rolledBack = backend(rollbackCalls, rollbackCoordinator, rollbackExecutor)
        val failedLoad = async { runCatching { rolledBack.load(modelFile().path, nativeLimits()) } }
        runCurrent()
        rollbackExecutor.runNext()
        runCurrent()
        assertTrue(failedLoad.await().isFailure)
        assertEquals(NativeReleaseCoordinator.State.RELEASED, rollbackCoordinator.currentState())
        assertEquals(NativeReleaseResult.Released, rolledBack.awaitReleaseResult(1, TimeUnit.SECONDS))

        val unknownCalls = FakeNativeCalls().apply { ownershipUnknownAfterLoad = true }
        val unknownCoordinator = NativeReleaseCoordinator()
        val unknownExecutor = ManualExecutor()
        val unknown = backend(unknownCalls, unknownCoordinator, unknownExecutor)
        val unknownLoad = async { runCatching { unknown.load(modelFile().path, nativeLimits()) } }
        runCurrent()
        unknownExecutor.runNext()
        runCurrent()
        assertTrue(unknownLoad.await().isFailure)
        assertEquals(NativeReleaseCoordinator.State.RELEASE_FAILED, unknownCoordinator.currentState())
        assertTrue(unknown.awaitReleaseResult(1, TimeUnit.SECONDS) is NativeReleaseResult.Failed)
    }

    @Test fun generationCancellationIsBoundToItsGenerationAndCompletionDoesNotRetargetIt() = runTest {
        val calls = FakeNativeCalls().apply { blockGenerate = true }
        val coordinator = NativeReleaseCoordinator()
        val executor = ManualExecutor()
        val backend = backend(calls, coordinator, executor)
        load(backend, executor)

        val generated = launch { backend.generate("hello", 4) }
        runCurrent()
        val worker = Thread { executor.runNext() }.apply { start() }
        assertTrue(calls.generateStarted.await(3, TimeUnit.SECONDS))
        generated.cancelAndJoin()
        worker.join(1_000)
        assertFalse(worker.isAlive)
        assertEquals(calls.lastGenerationId, calls.cancelledGenerationId)
        assertEquals(1, calls.cancelCount)

        calls.blockGenerate = false
        val complete = async { backend.generate("hello", 4) }
        runCurrent()
        executor.runNext()
        runCurrent()
        assertEquals("ok", complete.await().text)
        complete.cancel()
        runCurrent()
        assertEquals(1, calls.cancelCount) // late cancellation cannot target the completed request
    }

    @Test fun closeDuringGenerateCancelsThatGenerationBeforeFreeRuns() = runTest {
        val calls = FakeNativeCalls().apply {
            blockGenerate = true
            throwOnCancelledGenerate = true
        }
        val coordinator = NativeReleaseCoordinator()
        val executor = ManualExecutor()
        val backend = backend(calls, coordinator, executor)
        load(backend, executor)

        val generated = launch { backend.generate("hello", 4) }
        runCurrent()
        val worker = Thread { executor.runNext() }.apply { start() }
        assertTrue(calls.generateStarted.await(3, TimeUnit.SECONDS))
        backend.close()
        worker.join(1_000)
        assertFalse(worker.isAlive)
        assertEquals(calls.lastGenerationId, calls.cancelledGenerationId)
        assertEquals(NativeReleaseCoordinator.State.RELEASING, coordinator.currentState())

        executor.runNext()
        runCurrent()
        generated.join()
        assertTrue(backend.awaitReleased(1, TimeUnit.SECONDS))
        assertEquals(NativeReleaseCoordinator.State.RELEASED, coordinator.currentState())
        assertEquals(1, calls.freeCount)
    }

    private fun backend(
        calls: NativeRuntimeCalls,
        coordinator: NativeReleaseCoordinator,
        executor: ManualExecutor,
    ): LlamaCppLocalModelInferenceBackend {
        val constructor = LlamaCppLocalModelInferenceBackend::class.java.declaredConstructors
            .single { it.parameterCount == 4 }
            .apply { isAccessible = true }
        return constructor.newInstance(null, calls, coordinator, executor) as LlamaCppLocalModelInferenceBackend
    }

    private suspend fun TestScope.load(backend: LlamaCppLocalModelInferenceBackend, executor: ManualExecutor) {
        val loading = async {
            backend.load(modelFile().path, nativeLimits())
        }
        runCurrent()
        executor.runNext()
        runCurrent()
        loading.await()
    }

    private fun modelFile(): File = File.createTempFile("serea-native-contract", ".gguf").apply {
        writeBytes(byteArrayOf(1, 2, 3))
        deleteOnExit()
    }

    private fun nativeLimits() = LocalModelRuntimeLimits(contextTokens = 512)

    private class ManualExecutor : AbstractExecutorService() {
        private val queue = ArrayDeque<Runnable>()
        private var stopped = false
        var rejectTasks = false
        val queuedCount get() = synchronized(queue) { queue.size }

        override fun execute(command: Runnable) {
            if (rejectTasks || stopped) throw RejectedExecutionException("test executor rejected task")
            synchronized(queue) { queue.addLast(command) }
        }

        fun runNext() {
            val task = synchronized(queue) { queue.pollFirst() }
                ?: error("No queued executor task")
            task.run()
        }

        override fun shutdown() { stopped = true }
        override fun shutdownNow(): MutableList<Runnable> {
            stopped = true
            return synchronized(queue) { queue.toMutableList().also { queue.clear() } }
        }
        override fun isShutdown() = stopped
        override fun isTerminated() = stopped && queuedCount == 0
        override fun awaitTermination(timeout: Long, unit: TimeUnit) = isTerminated
    }

    private class FakeNativeCalls : NativeRuntimeCalls {
        private var owner = 0L
        var loadCount = 0
        var generateCount = 0
        var freeCount = 0
        var cancelCount = 0
        var lastGenerationId = 0L
        var cancelledGenerationId = 0L
        var freeSucceeds = true
        var freeLeavesOwner = false
        var loadSucceeds = true
        var ownershipUnknownAfterLoad = false
        var throwOnCancelledGenerate = false
        @Volatile private var cancellationRequested = false
        var blockGenerate = false
        val generateStarted = CountDownLatch(1)
        private val releaseGeneration = CountDownLatch(1)

        @Synchronized override fun load(engineId: Long, path: String, contextTokens: Int, workerThreads: Int): Boolean {
            loadCount++
            if (owner != 0L) throw IllegalStateException("foreign test owner")
            owner = engineId
            if (!loadSucceeds) owner = 0L // C++ load rollback contract
            return loadSucceeds
        }

        @Synchronized override fun ownership(engineId: Long): Int = when {
            ownershipUnknownAfterLoad -> -1
            owner == 0L -> 0
            owner == engineId -> 1
            else -> -1
        }

        override fun generate(engineId: Long, prompt: String, maxOutputTokens: Int, generationId: Long): String {
            generateCount++
            lastGenerationId = generationId
            if (blockGenerate) {
                generateStarted.countDown()
                check(releaseGeneration.await(3, TimeUnit.SECONDS))
            }
            if (throwOnCancelledGenerate && cancellationRequested) {
                throw kotlinx.coroutines.CancellationException("test generation cancelled")
            }
            return "ok"
        }

        override fun cancel(engineId: Long, generationId: Long): Boolean {
            cancelCount++
            cancelledGenerationId = generationId
            cancellationRequested = true
            releaseGeneration.countDown()
            return true
        }

        override fun forgetCancellation(engineId: Long, generationId: Long) = Unit
        override fun usageCounts(engineId: Long, generationId: Long) = intArrayOf(1, 1)
        override fun forgetUsage(engineId: Long, generationId: Long) = Unit

        @Synchronized override fun free(engineId: Long): Boolean {
            freeCount++
            if (!freeSucceeds) return false
            if (owner != engineId) return false
            if (!freeLeavesOwner) owner = 0L
            return true
        }
    }

    private class RejectReleaseCoordinator : NativeReleaseCoordinator() {
        override fun released(token: Any): Boolean = false
    }
}
