package app.serea.android

import android.os.Build
import android.os.Debug
import android.os.SystemClock
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.TimeUnit

/** Non-Required diagnostic: exercises the product ModelChatPort and native backend offline. */
@RunWith(AndroidJUnit4::class)
class LocalModelNativeRuntimeE2ETest {
    companion object {
        private const val MODEL_ID = "serea-local/SmolLM2-135M-Q4_K_M"
        private const val MODEL_SHA256 = "e3131339bf4e8065265593d4fd8f7bb7ff2d3abff1edb5618aa1197b89cad9f5"
        private const val MODEL_SIZE = 101_016_128L
        private const val MODEL_REVISION = "1076b85d135017971845b19b8d07df1479155988"
        private const val LLAMA_REVISION = "276378515ceebee5d14bcc8536603dd5c87491a2"
        private const val CONVERSATION_ID = "native-runtime-synthetic-conversation"
    }

    @Test
    fun pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend() = runBlocking {
        assertTrue("diagnostic build must opt into the native library", BuildConfig.LOCAL_NATIVE_RUNTIME_ENABLED)
        assertTrue(
            "runtime diagnostic must use a disposable Emulator",
            Build.FINGERPRINT.contains("generic", ignoreCase = true) ||
                Build.HARDWARE.contains("ranchu", ignoreCase = true) ||
                Build.HARDWARE.contains("goldfish", ignoreCase = true),
        )
        assertEquals("diagnostic runner must execute x86_64 JNI", "x86_64", Build.SUPPORTED_ABIS.firstOrNull())

        val appContext = InstrumentationRegistry.getInstrumentation().targetContext
        val modelFile = File(appContext.cacheDir, "g8-smollm2-135m-q4_k_m.gguf")
        assertTrue("model must be staged under app-private cache", modelFile.isFile)
        assertEquals(MODEL_SIZE, modelFile.length())
        assertEquals(MODEL_SHA256, sha256(modelFile))
        assertTrue(modelFile.canonicalPath.startsWith(appContext.cacheDir.canonicalPath + File.separator))

        val artifact = LocalModelArtifact(
            modelId = MODEL_ID,
            displayName = "SmolLM2 135M Q4_K_M (technical fixture)",
            modelRepository = "https://huggingface.co/ggml-org/SmolLM2-135M-GGUF",
            modelRevision = MODEL_REVISION,
            licenseId = "Apache-2.0",
            expectedSha256 = MODEL_SHA256,
            expectedSizeBytes = MODEL_SIZE,
            runtimeName = "llama.cpp",
            runtimeRevision = LLAMA_REVISION,
            supportedAbis = setOf("x86_64"),
        )
        // Keep the concrete diagnostic handle so the test can await the backend's asynchronous
        // native release after the product adapter performs its interface-level close().
        val backend = LlamaCppLocalModelInferenceBackend()
        val adapter = LocalModelChatAdapter(
            artifact = artifact,
            modelFile = modelFile,
            backendFactory = LocalModelInferenceBackendFactory { backend },
            deviceAbi = "x86_64",
            runtimeLimits = LocalModelRuntimeLimits(contextTokens = 512, maxOutputTokens = 32, workerThreads = 2),
        )
        val usageDao = NativeRuntimeUsageDao()
        val now = { SystemClock.elapsedRealtime() }
        val authorization = LocalModelAuthorizationRegistry(
            clock = now,
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding(CONVERSATION_ID, 0)
            },
        )
        val usageControlled = UsageControlledModelChatPort(
            delegate = adapter,
            usage = UsageRepository(usageDao),
            outputReservationTokens = 32,
            attributionResolver = LocalModelUsageAttributionResolver(artifact),
        )
        val egress = EgressCheckedModelChatPort(
            usageControlled,
            now,
            EgressTarget.LOCAL_MODEL,
            authorization,
            now,
        )

        val failedLoadBackend = LlamaCppLocalModelInferenceBackend()
        NativeJniFailureSeam().failNextLoadAfterModel()
        val injectedFailure = runCatching {
            failedLoadBackend.load(
                modelFile.canonicalPath,
                LocalModelRuntimeLimits(contextTokens = 512, maxOutputTokens = 32, workerThreads = 2),
            )
        }.exceptionOrNull()
        assertTrue("C++ failure must become a Java IllegalStateException", injectedFailure is IllegalStateException)
        failedLoadBackend.close()
        assertTrue(
            "failed load must release its lease and native model before retry",
            failedLoadBackend.awaitReleased(30, TimeUnit.SECONDS),
        )

        val firstRequest = request("Write one short greeting.", "native-e2e-turn-1")
        val pssBeforeKb = Debug.getPss()
        val firstStarted = SystemClock.elapsedRealtime()
        try {
            val first = egress.send(withConsent(firstRequest, authorization))
            val firstElapsedMs = SystemClock.elapsedRealtime() - firstStarted
            assertFalse("native model must return nonblank text", first.text.isBlank())
            assertEquals(MODEL_ID, usageDao.ledger("native-e2e-turn-1")?.modelId)
            assertEquals("local-device", usageDao.ledger("native-e2e-turn-1")?.providerId)
            assertEquals(UsageLedgerState.SETTLED.name, usageDao.reservationState("native-e2e-turn-1"))
            assertEquals(CostEstimationStatus.UNKNOWN.name, usageDao.ledger("native-e2e-turn-1")?.costStatus)
            assertNotNull("native prompt-token usage must be recorded", usageDao.ledger("native-e2e-turn-1")?.providerInputTokens)
            assertNotNull("native output-token usage must be recorded", usageDao.ledger("native-e2e-turn-1")?.providerOutputTokens)

            val context = listOf(
                contextMessage(MessageRole.USER, "Write one short greeting.", "native-e2e-user-1"),
                contextMessage(MessageRole.ASSISTANT, first.text, "native-e2e-assistant-1", DataOrigin.MODEL_OUTPUT),
            )
            val secondRequest = request("Continue with one short sentence.", "native-e2e-turn-2", context)
            val secondStarted = SystemClock.elapsedRealtime()
            val second = egress.send(withConsent(secondRequest, authorization))
            val secondElapsedMs = SystemClock.elapsedRealtime() - secondStarted
            assertFalse("second native turn must return nonblank text", second.text.isBlank())
            assertEquals(UsageLedgerState.SETTLED.name, usageDao.reservationState("native-e2e-turn-2"))
            assertTrue("each turn must reserve and attribute usage", usageDao.ledgerRows().size == 2)

            val pssAfterInferenceKb = Debug.getPss()
            Log.i(
                "SereaLocalNativeRuntime",
                "fixture=${MODEL_SHA256.take(12)} first_ms=$firstElapsedMs second_ms=$secondElapsedMs " +
                    "pss_before_kb=$pssBeforeKb pss_after_inference_kb=$pssAfterInferenceKb " +
                    "output_chars=${first.text.length + second.text.length}",
            )
        } finally {
            adapter.close()
            assertTrue("JNI model resources must release after close", backend.awaitReleased(30, TimeUnit.SECONDS))
            assertTrue("app-private fixture cleanup must succeed", modelFile.delete() || !modelFile.exists())
        }
        Log.i("SereaLocalNativeRuntime", "pss_after_release_kb=${Debug.getPss()}")
    }

    private fun withConsent(request: ModelChatRequest, registry: LocalModelAuthorizationRegistry): ModelChatRequest {
        val proof = registry.approve(request, MODEL_ID, conversationGeneration = 0)
        val contract = EgressContract(
            conversationId = request.conversationId,
            currentMessageId = request.sendId,
            origin = DataOrigin.USER_AUTHORED,
            classification = DataClassification.PUBLIC,
            destination = ChatEgressGate.LOCAL_MODEL_DESTINATION,
            plannedBody = request.message,
            context = request.context,
            preparation = PayloadPreparation.RAW,
            explicitSendIntentAtMillis = null,
            expiresAtMillis = null,
            selectedModelId = MODEL_ID,
            target = EgressTarget.LOCAL_MODEL,
            plannedBodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
            localModelConsent = proof,
        )
        return request.copy(selectedModelId = MODEL_ID, egressContract = contract)
    }

    private fun request(text: String, id: String, context: List<ContextMessage> = emptyList()) = ModelChatRequest(
        conversationId = CONVERSATION_ID,
        message = text,
        sendId = id,
        classification = DataClassification.PUBLIC,
        context = context,
        bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
    )

    private fun contextMessage(
        role: MessageRole,
        text: String,
        id: String,
        origin: DataOrigin = DataOrigin.USER_AUTHORED,
    ) = ContextMessage(
        role = role,
        text = text,
        classification = DataClassification.PUBLIC,
        sourceMessageId = id,
        origin = origin,
        conversationId = CONVERSATION_ID,
        sourceLineage = setOf(origin),
    )

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

    private class NativeRuntimeUsageDao : UsageDao {
        private val windows = mutableMapOf<String, UsageWindowRow>()
        private val reservations = mutableMapOf<String, UsageReservationRow>()
        private val ledgers = mutableMapOf<String, ModelUsageLedgerRow>()
        private var savedSettings: UsageSettingsRow? = null

        fun reservationState(id: String) = reservations[id]?.state
        fun ledgerRows() = ledgers.values.toList()
        override suspend fun window(key: String) = windows[key]
        override suspend fun putWindow(row: UsageWindowRow) { windows[row.windowKey] = row }
        override suspend fun reservation(id: String) = reservations[id]
        override suspend fun insertReservation(row: UsageReservationRow) { check(row.requestId !in reservations); reservations[row.requestId] = row }
        override suspend fun putReservation(row: UsageReservationRow) { reservations[row.requestId] = row }
        override suspend fun deleteInFlightReservation(id: String): Int {
            if (reservations[id]?.state != UsageLedgerState.IN_FLIGHT.name) return 0
            reservations.remove(id); ledgers.remove(id); return 1
        }
        override suspend fun inFlight() = reservations.values.filter { it.state == UsageLedgerState.IN_FLIGHT.name }
        override suspend fun markUnknown(id: String): Int {
            val row = reservations[id] ?: return 0
            if (row.state != UsageLedgerState.IN_FLIGHT.name) return 0
            reservations[id] = row.copy(state = UsageLedgerState.OUTCOME_UNKNOWN.name); return 1
        }
        override suspend fun settings() = savedSettings
        override suspend fun putSettings(row: UsageSettingsRow) { savedSettings = row }
        override suspend fun ledger(id: String) = ledgers[id]
        override suspend fun insertLedger(row: ModelUsageLedgerRow) { check(row.requestId !in ledgers); ledgers[row.requestId] = row }
        override suspend fun putLedger(row: ModelUsageLedgerRow) { ledgers[row.requestId] = row }
        override suspend fun allLedgerRows() = ledgerRows()
        override suspend fun markRequestUnknown(id: String) = markUnknown(id)
        override suspend fun totals(key: String) = UsageWindowTotals(0, 0, 0, 0)
        override suspend fun unresolvedCount() = reservations.values.count {
            it.state in setOf(UsageLedgerState.IN_FLIGHT.name, UsageLedgerState.OUTCOME_UNKNOWN.name, UsageLedgerState.USAGE_UNKNOWN.name)
        }.toLong()
    }
}

/** JNI failure injection exists only in the focused isolated native-runtime build. */
class NativeJniFailureSeam {
    external fun failNextLoadAfterModel()
}
