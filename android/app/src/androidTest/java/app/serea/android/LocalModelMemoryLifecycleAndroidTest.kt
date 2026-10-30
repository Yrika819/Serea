package app.serea.android

import android.app.Application
import android.os.ParcelFileDescriptor
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel

/** Focused real Application callback delivery test; excluded from the formal Required JUnit set. */
@RunWith(AndroidJUnit4::class)
class LocalModelMemoryLifecycleAndroidTest {
    @Test fun registeredApplicationMemoryTrimEvictsTheFakeBackendAfterAuthorizedUse() = runBlocking {
        val instrumentation = InstrumentationRegistry.getInstrumentation()
        val context = instrumentation.targetContext
        val application = context.applicationContext as Application
        assertEquals("callback registration must target the app under test", context.packageName, application.packageName)
        val modelFile = File.createTempFile("serea-memory-lifecycle-", ".gguf", context.cacheDir)
            .apply { writeBytes(byteArrayOf(1, 4, 1, 4, 2, 1)) }
        val modelId = "serea-local/component-callback-test"
        val backend = ApplicationTrimFakeBackend()
        val artifact = LocalModelArtifact(
            modelId = modelId,
            displayName = "Lifecycle callback fake model",
            modelRepository = "https://huggingface.co/test/lifecycle-fake",
            modelRevision = "1076b85d135017971845b19b8d07df1479155988",
            licenseId = "Apache-2.0",
            expectedSha256 = sha256(modelFile),
            expectedSizeBytes = modelFile.length(),
            runtimeName = "fake-runtime",
            runtimeRevision = "2763785",
            supportedAbis = setOf("arm64-v8a", "x86_64"),
        )
        val deviceAbi = android.os.Build.SUPPORTED_ABIS.first { it in artifact.supportedAbis }
        val adapter = LocalModelChatAdapter(
            artifact = artifact,
            modelFile = modelFile,
            backendFactory = LocalModelInferenceBackendFactory { backend },
            deviceAbi = deviceAbi,
        )
        val now = android.os.SystemClock.elapsedRealtime()
        val registry = LocalModelAuthorizationRegistry(
            clock = { now },
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding("trim-callback-conversation", 0L)
            },
        )
        val request = ModelChatRequest(
            conversationId = "trim-callback-conversation",
            message = "fake backend lifecycle request",
            sendId = "send-trim-callback",
            classification = DataClassification.PUBLIC,
            selectedModelId = modelId,
            bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
        )
        val proof = registry.approve(request, modelId)
        val contract = EgressContract(
            conversationId = request.conversationId,
            currentMessageId = request.sendId,
            origin = DataOrigin.USER_AUTHORED,
            classification = request.classification,
            destination = ChatEgressGate.LOCAL_MODEL_DESTINATION,
            plannedBody = request.message,
            context = request.context,
            preparation = PayloadPreparation.RAW,
            explicitSendIntentAtMillis = null,
            expiresAtMillis = null,
            selectedModelId = modelId,
            target = EgressTarget.LOCAL_MODEL,
            plannedBodySourceLineage = request.bodySourceLineage,
            localModelConsent = proof,
        )
        val authorizedAdapter = EgressCheckedModelChatPort(
            delegate = adapter,
            clock = { now },
            expectedTarget = EgressTarget.LOCAL_MODEL,
            localModelAuthorizations = registry,
            localMonotonicClock = { now },
        )
        val lifecycleScope = CoroutineScope(SupervisorJob() + Dispatchers.Default)
        val callback = LocalModelMemoryLifecycle(adapter, lifecycleScope, Dispatchers.Default)
        try {
            val response = authorizedAdapter.send(request.copy(egressContract = contract))
            assertEquals("fake local answer", response.text)
            assertEquals(1, backend.loadCalls.get())
            assertEquals(1, backend.generateCalls.get())

            instrumentation.runOnMainSync { application.registerComponentCallbacks(callback) }
            val targetProcess = application.applicationInfo.processName ?: context.packageName
            val command = "am send-trim-memory $targetProcess RUNNING_LOW"
            val shellOutput = instrumentation.uiAutomation.executeShellCommand(command)
            val output = ParcelFileDescriptor.AutoCloseInputStream(shellOutput).bufferedReader().use { it.readText() }

            assertFalse("ActivityManager rejected trim request for target process $targetProcess; output=$output",
                output.contains("Error", ignoreCase = true))
            assertTrue("Android should deliver RUNNING_LOW to the registered callback within 10s; output=$output",
                backend.closed.await(10, TimeUnit.SECONDS))
            assertEquals("trim closes the loaded backend once", 1, backend.closeCalls.get())
            assertFalse("memory trim occurs after generation has unwound", backend.closedDuringGeneration.get())
        } finally {
            instrumentation.runOnMainSync { application.unregisterComponentCallbacks(callback) }
            lifecycleScope.cancel()
            adapter.close()
            modelFile.delete()
        }
    }

    private fun sha256(file: File): String = MessageDigest.getInstance("SHA-256")
        .digest(file.readBytes())
        .joinToString("") { "%02x".format(it) }

    private class ApplicationTrimFakeBackend : LocalModelInferenceBackend {
        val loadCalls = AtomicInteger()
        val generateCalls = AtomicInteger()
        val closeCalls = AtomicInteger()
        val closedDuringGeneration = AtomicBoolean(false)
        val closed = CountDownLatch(1)
        private val generating = AtomicBoolean(false)

        override suspend fun load(modelPath: String, limits: LocalModelRuntimeLimits) {
            check(File(modelPath).isFile)
            loadCalls.incrementAndGet()
        }

        override suspend fun generate(prompt: String, maxOutputTokens: Int): LocalModelInferenceResult {
            generating.set(true)
            return try {
                generateCalls.incrementAndGet()
                LocalModelInferenceResult("fake local answer", 5, 3)
            } finally {
                generating.set(false)
            }
        }

        override fun cancelActive() = Unit

        override fun close() {
            closedDuringGeneration.set(generating.get())
            closeCalls.incrementAndGet()
            closed.countDown()
        }
    }
}
