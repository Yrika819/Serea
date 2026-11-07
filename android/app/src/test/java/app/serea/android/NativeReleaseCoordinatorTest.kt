package app.serea.android

import org.junit.Assert.*
import org.junit.Test

class NativeReleaseCoordinatorTest {
    @Test fun oneOwnerAtATimeAndReleaseRequiredBeforeNextAcquire() {
        val coordinator = NativeReleaseCoordinator()
        val first = Any()
        val second = Any()
        coordinator.acquire(first)
        assertEquals(NativeReleaseCoordinator.State.ACTIVE, coordinator.currentState())
        assertTrue(runCatching { coordinator.acquire(second) }.isFailure)
        coordinator.releasing(first)
        assertTrue(runCatching { coordinator.acquire(second) }.isFailure)
        coordinator.released(first)
        coordinator.acquire(second)
        assertEquals(NativeReleaseCoordinator.State.ACTIVE, coordinator.currentState())
    }

    @Test fun failedReleaseAndStaleOwnerCannotPermitOrChangeOwnership() {
        val coordinator = NativeReleaseCoordinator()
        val first = Any()
        val second = Any()
        coordinator.acquire(first)
        coordinator.releasing(first)
        coordinator.failed(first)
        assertEquals(NativeReleaseCoordinator.State.RELEASE_FAILED, coordinator.currentState())
        assertTrue(runCatching { coordinator.acquire(second) }.isFailure)
        assertTrue(runCatching { coordinator.released(first) }.isFailure)
        assertEquals(NativeReleaseCoordinator.State.RELEASE_FAILED, coordinator.currentState())
    }

    @Test fun nativeCapabilitiesAreIndependentFromResidentMemoryLimit() {
        val capabilities = NativeRuntimeCapabilities()
        capabilities.validate(LocalModelRuntimeLimits(contextTokens = 512), 100)
        assertTrue(runCatching {
            capabilities.validate(LocalModelRuntimeLimits(), 100)
        }.isFailure)
        assertTrue(runCatching {
            capabilities.validate(LocalModelRuntimeLimits(contextTokens = 512, workerThreads = 3), 100)
        }.isFailure)
        assertTrue(runCatching {
            capabilities.validate(LocalModelRuntimeLimits(contextTokens = 512, maxOutputTokens = 257), 100)
        }.isFailure)
        assertTrue(runCatching {
            capabilities.validate(LocalModelRuntimeLimits(contextTokens = 512), capabilities.maxModelFileBytes + 1)
        }.isFailure)
    }
}
