package app.serea.android

import android.Manifest
import android.content.pm.PackageManager
import android.os.Build
import android.os.Debug
import android.os.SystemClock
import android.util.Log
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.withTimeoutOrNull
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
    fun pinnedModelRunsThroughLocalModelEgressUsageAndNativeBackend(): Unit = runBlocking<Unit> {
        assertTrue("diagnostic build must opt into the native library", BuildConfig.LOCAL_NATIVE_RUNTIME_ENABLED)
        assertTrue(
            "runtime diagnostic must use a disposable Emulator",
            Build.FINGERPRINT.contains("generic", ignoreCase = true) ||
                Build.HARDWARE.contains("ranchu", ignoreCase = true) ||
                Build.HARDWARE.contains("goldfish", ignoreCase = true),
        )
        assertEquals("diagnostic runner must execute x86_64 JNI", "x86_64", Build.SUPPORTED_ABIS.firstOrNull())

        val appContext = InstrumentationRegistry.getInstrumentation().targetContext
        val testContext = InstrumentationRegistry.getInstrumentation().context
        assertEquals(
            "isolated native-runtime target must not hold Android network permission",
            PackageManager.PERMISSION_DENIED,
            appContext.checkSelfPermission(Manifest.permission.INTERNET),
        )
        assertEquals(
            "instrumentation APK must not hold Android network permission",
            PackageManager.PERMISSION_DENIED,
            testContext.checkSelfPermission(Manifest.permission.INTERNET),
        )
        Log.i("SereaLocalNativeRuntime", "stage=offline_permission_denied target_and_instrumentation=true")
        val modelFile = File(appContext.cacheDir, "g8-smollm2-135m-q4_k_m.gguf")
        assertTrue("model must be staged under app-private cache", modelFile.isFile)
        assertEquals(MODEL_SIZE, modelFile.length())
        assertEquals(MODEL_SHA256, sha256(modelFile))
        assertTrue(modelFile.canonicalPath.startsWith(appContext.cacheDir.canonicalPath + File.separator))
        Log.i(
            "SereaLocalNativeRuntime",
            "stage=fixture_hash_pass revision=$MODEL_REVISION size=$MODEL_SIZE sha256=$MODEL_SHA256",
        )

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
        val jniSeam = NativeJniFailureSeam()
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
        jniSeam.failNextLoadAfterModel()
        Log.i("SereaLocalNativeRuntime", "stage=failed_model_load_started timeout_ms=60000")
        val injectedFailure = try {
            runCatching {
                withTimeout(60_000) {
                    failedLoadBackend.load(
                        modelFile.canonicalPath,
                        LocalModelRuntimeLimits(contextTokens = 512, maxOutputTokens = 32, workerThreads = 2),
                    )
                }
            }.exceptionOrNull()
        } finally {
            failedLoadBackend.close()
            assertTrue(
                "failed or timed-out load must release its lease and native model",
                failedLoadBackend.awaitReleased(30, TimeUnit.SECONDS),
            )
        }
        Log.i("SereaLocalNativeRuntime", "stage=failed_model_load_completed cause=${injectedFailure?.javaClass?.simpleName}")
        assertTrue("C++ failure must become a Java IllegalStateException", injectedFailure is IllegalStateException)
        Log.i("SereaLocalNativeRuntime", "stage=failed_load_cleanup_pass")

        val firstRequest = request("Write one short greeting.", "native-e2e-turn-1")
        val pssBeforeKb = Debug.getPss()
        val firstStarted = SystemClock.elapsedRealtime()
        try {
            Log.i("SereaLocalNativeRuntime", "stage=first_native_turn_started")
            val first = egress.send(withConsent(firstRequest, authorization))
            val firstElapsedMs = SystemClock.elapsedRealtime() - firstStarted
            assertFalse("native model must return nonblank text", first.text.isBlank())
            assertEquals(MODEL_ID, usageDao.ledger("native-e2e-turn-1")?.modelId)
            assertEquals("local-device", usageDao.ledger("native-e2e-turn-1")?.providerId)
            assertEquals(UsageLedgerState.SETTLED.name, usageDao.reservationState("native-e2e-turn-1"))
            assertEquals(CostEstimationStatus.UNKNOWN.name, usageDao.ledger("native-e2e-turn-1")?.costStatus)
            assertNotNull("native prompt-token usage must be recorded", usageDao.ledger("native-e2e-turn-1")?.providerInputTokens)
            assertNotNull("native output-token usage must be recorded", usageDao.ledger("native-e2e-turn-1")?.providerOutputTokens)
            Log.i("SereaLocalNativeRuntime", "stage=first_native_turn_pass elapsed_ms=$firstElapsedMs")

            val context = listOf(
                contextMessage(MessageRole.USER, "Write one short greeting.", "native-e2e-user-1"),
                contextMessage(MessageRole.ASSISTANT, first.text, "native-e2e-assistant-1", DataOrigin.MODEL_OUTPUT),
            )
            val secondRequest = request("Continue with one short sentence.", "native-e2e-turn-2", context)
            val secondStarted = SystemClock.elapsedRealtime()
            Log.i("SereaLocalNativeRuntime", "stage=second_native_turn_started")
            val second = egress.send(withConsent(secondRequest, authorization))
            val secondElapsedMs = SystemClock.elapsedRealtime() - secondStarted
            assertFalse("second native turn must return nonblank text", second.text.isBlank())
            assertEquals(UsageLedgerState.SETTLED.name, usageDao.reservationState("native-e2e-turn-2"))
            assertTrue("each turn must reserve and attribute usage", usageDao.ledgerRows().size == 2)
            Log.i("SereaLocalNativeRuntime", "stage=second_native_turn_pass elapsed_ms=$secondElapsedMs")

            val pssAfterInferenceKb = Debug.getPss()
            Log.i(
                "SereaLocalNativeRuntime",
                "fixture=${MODEL_SHA256.take(12)} first_ms=$firstElapsedMs second_ms=$secondElapsedMs " +
                    "pss_before_kb=$pssBeforeKb pss_after_inference_kb=$pssAfterInferenceKb " +
                    "output_chars=${first.text.length + second.text.length}",
            )
        } finally {
            adapter.close()
            backend.close()
            assertTrue("JNI model resources must release after repeated close", backend.awaitReleased(30, TimeUnit.SECONDS))
            Log.i("SereaLocalNativeRuntime", "stage=backend_double_close_release_pass pss_kb=${Debug.getPss()}")
        }

        // Adapter close plus a direct backend close must be idempotent, and a fresh JNI owner
        // must be able to load the same verified fixture and generate again.
        val regeneratedBackend = LlamaCppLocalModelInferenceBackend()
        val regeneratedAdapter = LocalModelChatAdapter(
            artifact = artifact,
            modelFile = modelFile,
            backendFactory = LocalModelInferenceBackendFactory { regeneratedBackend },
            deviceAbi = "x86_64",
            runtimeLimits = LocalModelRuntimeLimits(contextTokens = 512, maxOutputTokens = 32, workerThreads = 2),
        )
        val regeneratedDao = NativeRuntimeUsageDao()
        val regeneratedAuthorization = LocalModelAuthorizationRegistry(
            clock = now,
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding(CONVERSATION_ID, 0)
            },
        )
        val regeneratedEgress = EgressCheckedModelChatPort(
            UsageControlledModelChatPort(
                regeneratedAdapter,
                UsageRepository(regeneratedDao),
                outputReservationTokens = 32,
                attributionResolver = LocalModelUsageAttributionResolver(artifact),
            ),
            now,
            EgressTarget.LOCAL_MODEL,
            regeneratedAuthorization,
            now,
        )
        try {
            val regeneratedRequest = request("Generate after a fresh native load.", "native-e2e-regenerated")
            Log.i("SereaLocalNativeRuntime", "stage=fresh_backend_generation_started")
            val regenerated = regeneratedEgress.send(withConsent(regeneratedRequest, regeneratedAuthorization))
            assertFalse("fresh JNI backend must generate after prior double-close", regenerated.text.isBlank())
            assertEquals(UsageLedgerState.SETTLED.name, regeneratedDao.reservationState("native-e2e-regenerated"))
            Log.i("SereaLocalNativeRuntime", "stage=double_close_fresh_load_regeneration_pass")
        } finally {
            regeneratedAdapter.close()
            regeneratedBackend.close()
            assertTrue("regenerated JNI backend must release", regeneratedBackend.awaitReleased(30, TimeUnit.SECONDS))
        }

        val cancellationBackend = LlamaCppLocalModelInferenceBackend()
        val cancellationAdapter = LocalModelChatAdapter(
            artifact = artifact,
            modelFile = modelFile,
            backendFactory = LocalModelInferenceBackendFactory { cancellationBackend },
            deviceAbi = "x86_64",
            runtimeLimits = LocalModelRuntimeLimits(contextTokens = 512, maxOutputTokens = 256, workerThreads = 2),
        )
        val cancellationDao = NativeRuntimeUsageDao()
        val cancellationAuthorization = LocalModelAuthorizationRegistry(
            clock = now,
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding(CONVERSATION_ID, 0)
            },
        )
        val cancellationEgress = EgressCheckedModelChatPort(
            UsageControlledModelChatPort(
                cancellationAdapter,
                UsageRepository(cancellationDao),
                outputReservationTokens = 256,
                attributionResolver = LocalModelUsageAttributionResolver(artifact),
            ),
            now,
            EgressTarget.LOCAL_MODEL,
            cancellationAuthorization,
            now,
        )
        val cancellationRequest = request(
            "Write a long sequence of words and continue for many tokens.",
            "native-e2e-cancel-active",
            timeoutMillis = 60_000,
        )
        val cancelCountBefore = jniSeam.nativeCancelCount()
        val generationSequenceBefore = jniSeam.generationSequence()
        val cancellationJob = async(Dispatchers.Default) {
            cancellationEgress.send(withConsent(cancellationRequest, cancellationAuthorization))
        }
        try {
            Log.i("SereaLocalNativeRuntime", "stage=wait_for_active_native_generation")
            val cancellationTarget = withTimeoutOrNull(45_000) {
                while (true) {
                    val sequence = jniSeam.generationSequence()
                    val progress = jniSeam.generationProgress()
                    val generationId = jniSeam.generationId()
                    val outcome = jniSeam.generationOutcome()
                    if (sequence > generationSequenceBefore && progress == -2) {
                        Log.e(
                            "SereaLocalNativeRuntime",
                            "stage=cancel_progress_terminal_before_cancel sequence=$sequence " +
                                "generation_id=$generationId outcome=$outcome",
                        )
                        throw IllegalStateException("Native generation completed before cancellation could be issued.")
                    }
                    if (sequence > generationSequenceBefore && progress > 0 &&
                        generationId > 0L && outcome == NativeJniFailureSeam.GENERATION_OUTCOME_ACTIVE
                    ) {
                        return@withTimeoutOrNull Triple(sequence, generationId, progress)
                    }
                    delay(5)
                }
                @Suppress("UNREACHABLE_CODE")
                Triple(0, 0L, 0)
            }
            val activeTarget = cancellationTarget ?: throw AssertionError(
                "timed out waiting for active native generation; " +
                    "sequence=${jniSeam.generationSequence()} progress=${jniSeam.generationProgress()} " +
                    "generation_id=${jniSeam.generationId()} outcome=${jniSeam.generationOutcome()}",
            )
            val (generationSequenceAtCancel, generationIdAtCancel, generatedTokensBeforeCancel) = activeTarget
            val cancellationStarted = SystemClock.elapsedRealtime()
            cancellationJob.cancel()
            val cancelJoinCompleted = withTimeoutOrNull(15_000) {
                cancellationJob.join()
                true
            } == true
            assertTrue("cancellation coroutine did not join within 15 seconds", cancelJoinCompleted)
            assertTrue("cancelled request must not return normally", cancellationJob.isCancelled)
            val nativeCancellationTerminal = withTimeoutOrNull(15_000) {
                while (true) {
                    val sequence = jniSeam.generationSequence()
                    val generationId = jniSeam.generationId()
                    val outcome = jniSeam.generationOutcome()
                    val progress = jniSeam.generationProgress()
                    if (sequence == generationSequenceAtCancel && generationId == generationIdAtCancel &&
                        outcome == NativeJniFailureSeam.GENERATION_OUTCOME_CANCELLED && progress == -2
                    ) {
                        return@withTimeoutOrNull true
                    }
                    delay(5)
                }
                @Suppress("UNREACHABLE_CODE")
                false
            } == true
            assertTrue(
                "timed out waiting for matching native cancellation terminal state; " +
                    "expected_sequence=$generationSequenceAtCancel expected_generation_id=$generationIdAtCancel " +
                    "sequence=${jniSeam.generationSequence()} generation_id=${jniSeam.generationId()} " +
                    "outcome=${jniSeam.generationOutcome()} progress=${jniSeam.generationProgress()} " +
                    "cancel_observed_generation_id=${jniSeam.cancelObservedGenerationId()}",
                nativeCancellationTerminal,
            )
            assertEquals("JNI must retain the requested generation identity", generationIdAtCancel, jniSeam.generationId())
            assertEquals("native generation sequence must still identify the cancelled request", generationSequenceAtCancel, jniSeam.generationSequence())
            assertEquals(
                "the matching native generation must observe cancellation before returning",
                generationIdAtCancel,
                jniSeam.cancelObservedGenerationId(),
            )
            assertEquals(
                "normal completion must not satisfy the cancellation test",
                NativeJniFailureSeam.GENERATION_OUTCOME_CANCELLED,
                jniSeam.generationOutcome(),
            )
            cancellationAdapter.close()
            assertTrue(
                "JNI generation must unwind and release within 15 seconds of cancellation",
                cancellationBackend.awaitReleased(15, TimeUnit.SECONDS),
            )
            val cancellationLatencyMs = SystemClock.elapsedRealtime() - cancellationStarted
            assertTrue("JNI cancel must reach an active native generation", jniSeam.nativeCancelCount() > cancelCountBefore)
            assertEquals(
                "nativeFree must observe the same generation's cancellation result before release",
                generationIdAtCancel,
                jniSeam.cancelledReleaseGenerationId(),
            )
            assertEquals(
                "nativeFree must observe the cancellation result for the same generation sequence",
                generationSequenceAtCancel,
                jniSeam.cancelledReleaseSequence(),
            )
            assertTrue("native cancellation latency exceeded 15 seconds", cancellationLatencyMs <= 15_000)
            assertEquals(
                "post-dispatch cancellation must keep usage outcome unknown",
                UsageLedgerState.OUTCOME_UNKNOWN.name,
                cancellationDao.reservationState("native-e2e-cancel-active"),
            )
            assertEquals("native generation progress must be terminal after release", -2, jniSeam.generationProgress())
            Log.i(
                "SereaLocalNativeRuntime",
                "stage=jni_cancellation_pass generation_sequence=$generationSequenceAtCancel " +
                    "generation_id=$generationIdAtCancel tokens_before_cancel=$generatedTokensBeforeCancel " +
                    "cancel_to_release_ms=$cancellationLatencyMs",
            )
        } finally {
            withTimeoutOrNull(15_000) { cancellationJob.cancelAndJoin() }
            cancellationAdapter.close()
            cancellationBackend.close()
            assertTrue("cancelled JNI backend must release", cancellationBackend.awaitReleased(30, TimeUnit.SECONDS))
        }

        assertTrue("app-private fixture cleanup must succeed", modelFile.delete() || !modelFile.exists())
        Log.i("SereaLocalNativeRuntime", "stage=all_native_checks_pass pss_after_release_kb=${Debug.getPss()}")
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

    private fun request(
        text: String,
        id: String,
        context: List<ContextMessage> = emptyList(),
        timeoutMillis: Long = 60_000,
    ) = ModelChatRequest(
        conversationId = CONVERSATION_ID,
        message = text,
        sendId = id,
        classification = DataClassification.PUBLIC,
        context = context,
        bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
        timeoutMillis = timeoutMillis,
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
    companion object {
        const val GENERATION_OUTCOME_ACTIVE = 1
        const val GENERATION_OUTCOME_COMPLETED = 2
        const val GENERATION_OUTCOME_CANCELLED = 3
        const val GENERATION_OUTCOME_FAILED = 4
    }

    external fun failNextLoadAfterModel()
    external fun generationProgress(): Int
    external fun generationSequence(): Int
    external fun nativeCancelCount(): Int
    external fun generationId(): Long
    external fun generationOutcome(): Int
    external fun cancelObservedGenerationId(): Long
    external fun cancelledReleaseGenerationId(): Long
    external fun cancelledReleaseSequence(): Int
}
