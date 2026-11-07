package app.serea.android

import java.io.File
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.util.concurrent.TimeUnit

/** JNI-only ownership checks; no model fixture is loaded by this contract test. */
@RunWith(AndroidJUnit4::class)
class NativeJniReleaseContractTest {
    @Test fun unloadedEngineIsReleasedAndForeignFreeCannotClaimSuccess() = runBlocking {
        val backend = LlamaCppLocalModelInferenceBackend()
        assertEquals(0, backend.nativeOwnershipForDiagnosticTest())
        assertFalse(backend.nativeFreeUnownedForDiagnosticTest())
        assertEquals(0, backend.nativeOwnershipForDiagnosticTest())

        backend.close()
        assertEquals(NativeReleaseResult.NotAcquired, backend.awaitReleaseResult(10, TimeUnit.SECONDS))
    }

    @Test fun invalidModelLoadRollsBackTheNativeEngineLease() = runBlocking {
        val modelFile = File.createTempFile("invalid-native-model", ".gguf")
        modelFile.writeBytes(byteArrayOf(0x53, 0x45, 0x52, 0x45, 0x41))
        val backend = LlamaCppLocalModelInferenceBackend()

        val load = runCatching {
            backend.load(modelFile.absolutePath, LocalModelRuntimeLimits(contextTokens = 512))
        }
        assertTrue("invalid non-GGUF file must be rejected", load.isFailure)
        assertEquals("failed JNI load must leave C++ ownership empty", 0, backend.nativeOwnershipForDiagnosticTest())
        assertEquals(NativeReleaseResult.Released, backend.awaitReleaseResult(10, TimeUnit.SECONDS))
        modelFile.delete()
    }
}
