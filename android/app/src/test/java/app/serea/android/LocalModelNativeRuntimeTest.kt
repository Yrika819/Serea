package app.serea.android

import org.junit.Assert.assertFalse
import org.junit.Assert.assertThrows
import org.junit.Test

class LocalModelNativeRuntimeTest {
    @Test
    fun defaultProductBuildFailsClosedBeforeLoadingNativeLibrary() {
        assertFalse(BuildConfig.LOCAL_NATIVE_RUNTIME_ENABLED)
        assertThrows(PreDispatchModelChatFailure::class.java) {
            LlamaCppLocalModelInferenceBackendFactory().create()
        }
    }
}
