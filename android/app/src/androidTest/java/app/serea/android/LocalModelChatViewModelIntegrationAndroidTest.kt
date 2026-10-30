package app.serea.android

import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import kotlinx.coroutines.awaitCancellation
import kotlinx.coroutines.delay
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.flow.first
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicInteger

/** Test-only ViewModel composition. It never wires Local Model Chat into product Chat. */
@RunWith(AndroidJUnit4::class)
class LocalModelChatViewModelIntegrationAndroidTest {
    @Test fun selectedAndConsentedLocalRequestUsesAdapterSettlesUsageAndRejectsProofReplay() = runBlocking {
        val fixture = fixture(blockUntilCancelled = false)
        try {
            assertFalse("product Chat route remains disabled", BuildConfig.LOCAL_MODEL_CHAT_ENABLED)
            fixture.intent.selectLocalModel()
            fixture.intent.confirmLocalConsent()
            fixture.viewModel.updateInput("answer using the local fake model")
            fixture.viewModel.sendMessage()

            val state = withTimeout(WAIT_TIMEOUT_MILLIS) {
                fixture.viewModel.state.first { it.conversation.messages.any { message -> message.role == MessageRole.ASSISTANT } }
            }
            val user = state.conversation.messages.first { it.role == MessageRole.USER }
            val assistant = state.conversation.messages.last { it.role == MessageRole.ASSISTANT }
            assertEquals("fake local answer", assistant.text)
            assertEquals(UsageLedgerState.SETTLED.name, fixture.usageDao.reservation(user.id)?.state)
            assertEquals(UsageLedgerState.SETTLED.name, fixture.usageDao.ledgers[user.id]?.usageStatus)
            assertEquals(TEST_MODEL_ID, fixture.usageDao.ledgers[user.id]?.modelId)
            assertEquals("local-device", fixture.usageDao.ledgers[user.id]?.providerId)
            assertEquals(1, fixture.backend.loadCalls.get())
            assertEquals(1, fixture.backend.generateCalls.get())
            assertEquals(0, fixture.demo.calls.get())

            val approved = checkNotNull(fixture.testPort.lastPreparedRequest)
            assertEquals(null, fixture.testPort.lastRawRequest?.selectedModelId)
            assertEquals(DataClassification.UNKNOWN, fixture.testPort.lastRawRequest?.classification)
            assertEquals(TEST_MODEL_ID, approved.selectedModelId)
            assertEquals(DataClassification.UNKNOWN, approved.classification)
            val replayFailure = runCatching { fixture.router.send(approved) }.exceptionOrNull()
            assertTrue(replayFailure is PreDispatchModelChatFailure)
            assertEquals("consent replay must not create another Usage reservation", 1, fixture.usageDao.reservations.size)
            assertEquals("consent replay must not reach the adapter", 1, fixture.backend.generateCalls.get())
            assertEquals("local route must not call Demo after success or replay denial", 0, fixture.demo.calls.get())
        } finally {
            fixture.close()
        }
    }

    @Test fun localSelectionWithoutConsentFailsBeforeUsageAndProviderDispatch() = runBlocking {
        val fixture = fixture(blockUntilCancelled = false)
        try {
            fixture.intent.selectLocalModel()
            fixture.viewModel.updateInput("consent is required")
            fixture.viewModel.sendMessage()

            val state = withTimeout(WAIT_TIMEOUT_MILLIS) {
                fixture.viewModel.state.first { it.errorMessage != null && !it.isLoading }
            }
            assertNotNull(state.errorMessage)
            assertTrue(fixture.usageDao.reservations.isEmpty())
            assertEquals(0, fixture.backend.loadCalls.get())
            assertEquals(0, fixture.backend.generateCalls.get())
            assertEquals(0, fixture.demo.calls.get())
        } finally {
            fixture.close()
        }
    }

    @Test fun userCancellationDuringLocalInferenceKeepsUnknownUsageWithoutFallback() = runBlocking {
        val fixture = fixture(blockUntilCancelled = true)
        try {
            fixture.intent.selectAndConfirmLocalModel()
            fixture.viewModel.updateInput("cancel during local inference")
            fixture.viewModel.sendMessage()
            assertTrue("fake provider should enter inference", fixture.backend.generationStarted.await(5, TimeUnit.SECONDS))
            val userId = fixture.viewModel.state.value.conversation.messages.first { it.role == MessageRole.USER }.id

            fixture.viewModel.cancelCurrent()
            awaitReservationState(fixture.usageDao, userId, UsageLedgerState.OUTCOME_UNKNOWN.name)
            assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, fixture.usageDao.ledgers[userId]?.usageStatus)
            assertTrue(fixture.backend.cancelCalls.get() > 0)
            assertEquals(1, fixture.backend.closeCalls.get())
            assertEquals(0, fixture.demo.calls.get())
        } finally {
            fixture.close()
        }
    }

    @Test fun clearingViewModelStoreCancelsLocalWorkAndFinalizesUnknownUsage() = runBlocking {
        val fixture = fixture(blockUntilCancelled = true)
        try {
            fixture.intent.selectAndConfirmLocalModel()
            fixture.viewModel.updateInput("clear ViewModel while local inference is active")
            fixture.viewModel.sendMessage()
            assertTrue("fake provider should enter inference", fixture.backend.generationStarted.await(5, TimeUnit.SECONDS))
            val userId = fixture.viewModel.state.value.conversation.messages.first { it.role == MessageRole.USER }.id

            fixture.store.clear()
            awaitReservationState(fixture.usageDao, userId, UsageLedgerState.OUTCOME_UNKNOWN.name)
            assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, fixture.usageDao.ledgers[userId]?.usageStatus)
            assertTrue(fixture.backend.cancelCalls.get() > 0)
            assertEquals(1, fixture.backend.closeCalls.get())
            assertEquals(0, fixture.demo.calls.get())
        } finally {
            fixture.close()
        }
    }

    private suspend fun awaitReservationState(dao: ViewModelIntegrationUsageDao, requestId: String, state: String) {
        withTimeout(WAIT_TIMEOUT_MILLIS) {
            while (dao.reservation(requestId)?.state != state || dao.ledgers[requestId]?.usageStatus != state) delay(10)
        }
    }

    private fun fixture(blockUntilCancelled: Boolean): Fixture {
        val context = InstrumentationRegistry.getInstrumentation().targetContext
        val modelFile = File.createTempFile("serea-viewmodel-local-fake-", ".gguf", context.cacheDir)
            .apply { writeBytes(byteArrayOf(6, 2, 6, 4, 3, 3, 8)) }
        val artifact = LocalModelArtifact(
            modelId = TEST_MODEL_ID,
            displayName = "ViewModel integration fake model",
            modelRepository = "https://huggingface.co/test/viewmodel-integration-fake",
            modelRevision = "1076b85d135017971845b19b8d07df1479155988",
            licenseId = "Apache-2.0",
            expectedSha256 = sha256(modelFile),
            expectedSizeBytes = modelFile.length(),
            runtimeName = "fake-runtime",
            runtimeRevision = "2763785",
            supportedAbis = setOf("arm64-v8a", "x86_64"),
        )
        val backend = ViewModelIntegrationFakeBackend(blockUntilCancelled)
        val adapter = LocalModelChatAdapter(
            artifact = artifact,
            modelFile = modelFile,
            backendFactory = LocalModelInferenceBackendFactory { backend },
            deviceAbi = android.os.Build.SUPPORTED_ABIS.first { it in artifact.supportedAbis },
        )
        val wallClock = 1_800_000_000_000L
        val dao = ViewModelIntegrationUsageDao()
        val usage = UsageRepository(dao) { wallClock }
        val intent = TestOnlyLocalModelIntent()
        lateinit var viewModel: ChatViewModel
        val authorizations = LocalModelAuthorizationRegistry(
            clock = { android.os.SystemClock.elapsedRealtime() },
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding(
                    viewModel.state.value.conversation.id,
                    viewModel.currentConversationSelectionGeneration(),
                )
            },
        )
        val demo = ViewModelIntegrationRecordingPort("demo answer")
        val localUsage = UsageControlledModelChatPort(
            delegate = adapter,
            usage = usage,
            outputReservationTokens = 32,
            attributionResolver = LocalModelUsageAttributionResolver(artifact),
        )
        val egress = EgressCheckedModelChatPort(
            delegate = localUsage,
            clock = { wallClock },
            expectedTarget = EgressTarget.LOCAL_MODEL,
            localModelAuthorizations = authorizations,
            localMonotonicClock = { android.os.SystemClock.elapsedRealtime() },
        )
        val router = LocalModelChatFeatureRouter(
            localModelId = artifact.modelId,
            demo = demo,
            local = egress,
            localModelChatEnabled = true,
        )
        val testPort = TestOnlyViewModelSelectionConsentPort(
            selectedIntent = intent,
            route = router,
            authorizations = authorizations,
            selectionGeneration = { viewModel.currentConversationSelectionGeneration() },
        )
        val store = ViewModelStore()
        viewModel = ViewModelProvider(store, ChatViewModel.factory(testPort, usage = usage))
            .get(ChatViewModel::class.java)
        return Fixture(modelFile, adapter, backend, dao, intent, demo, router, testPort, store, viewModel)
    }

    private fun sha256(file: File): String = MessageDigest.getInstance("SHA-256")
        .digest(file.readBytes())
        .joinToString("") { "%02x".format(it) }

    private class Fixture(
        private val modelFile: File,
        private val adapter: LocalModelChatAdapter,
        val backend: ViewModelIntegrationFakeBackend,
        val usageDao: ViewModelIntegrationUsageDao,
        val intent: TestOnlyLocalModelIntent,
        val demo: ViewModelIntegrationRecordingPort,
        val router: LocalModelChatFeatureRouter,
        val testPort: TestOnlyViewModelSelectionConsentPort,
        val store: ViewModelStore,
        val viewModel: ChatViewModel,
    ) {
        fun close() {
            store.clear()
            adapter.close()
            modelFile.delete()
        }
    }

    private class TestOnlyLocalModelIntent {
        @Volatile var selectedModelId: String? = null
            private set
        @Volatile private var localConsentConfirmed = false

        fun selectLocalModel() { selectedModelId = TEST_MODEL_ID }
        fun confirmLocalConsent() { check(selectedModelId == TEST_MODEL_ID); localConsentConfirmed = true }
        fun selectAndConfirmLocalModel() { selectLocalModel(); confirmLocalConsent() }
        fun consumeConsentFor(modelId: String?): Boolean = synchronized(this) {
            if (modelId == TEST_MODEL_ID && selectedModelId == modelId && localConsentConfirmed) {
                localConsentConfirmed = false
                true
            } else false
        }
    }

    private class TestOnlyViewModelSelectionConsentPort(
        private val selectedIntent: TestOnlyLocalModelIntent,
        private val route: LocalModelChatFeatureRouter,
        private val authorizations: LocalModelAuthorizationRegistry,
        private val selectionGeneration: () -> Long,
    ) : ModelChatPort {
        @Volatile var lastRawRequest: ModelChatRequest? = null
            private set
        @Volatile var lastPreparedRequest: ModelChatRequest? = null
            private set

        override suspend fun send(request: ModelChatRequest): ModelChatResponse {
            lastRawRequest = request
            val selected = selectedIntent.selectedModelId
            val localRequest = request.copy(selectedModelId = selected)
            val proof = if (selectedIntent.consumeConsentFor(selected)) {
                authorizations.approve(localRequest, TEST_MODEL_ID, conversationGeneration = selectionGeneration())
            } else null
            val contract = EgressContract(
                conversationId = localRequest.conversationId,
                currentMessageId = localRequest.sendId,
                origin = DataOrigin.USER_AUTHORED,
                classification = localRequest.classification,
                destination = ChatEgressGate.LOCAL_MODEL_DESTINATION,
                plannedBody = localRequest.message,
                context = localRequest.context,
                preparation = PayloadPreparation.RAW,
                explicitSendIntentAtMillis = null,
                expiresAtMillis = null,
                selectedModelId = selected,
                target = EgressTarget.LOCAL_MODEL,
                plannedBodySourceLineage = localRequest.bodySourceLineage,
                localModelConsent = proof,
            )
            val prepared = localRequest.copy(egressContract = contract)
            lastPreparedRequest = prepared
            return route.send(prepared)
        }
    }

    private class ViewModelIntegrationRecordingPort(private val text: String) : ModelChatPort {
        val calls = AtomicInteger()
        override suspend fun send(request: ModelChatRequest): ModelChatResponse {
            calls.incrementAndGet()
            return ModelChatResponse(text)
        }
    }

    private class ViewModelIntegrationFakeBackend(private val blockUntilCancelled: Boolean) : LocalModelInferenceBackend {
        val loadCalls = AtomicInteger()
        val generateCalls = AtomicInteger()
        val cancelCalls = AtomicInteger()
        val closeCalls = AtomicInteger()
        val generationStarted = CountDownLatch(1)

        override suspend fun load(modelPath: String, limits: LocalModelRuntimeLimits) {
            check(File(modelPath).isFile)
            loadCalls.incrementAndGet()
        }

        override suspend fun generate(prompt: String, maxOutputTokens: Int): LocalModelInferenceResult {
            generateCalls.incrementAndGet()
            generationStarted.countDown()
            if (blockUntilCancelled) awaitCancellation()
            return LocalModelInferenceResult("fake local answer", 6, 3)
        }

        override fun cancelActive() { cancelCalls.incrementAndGet() }
        override fun close() { closeCalls.incrementAndGet() }
    }

    private class ViewModelIntegrationUsageDao : UsageDao {
        val windows = java.util.concurrent.ConcurrentHashMap<String, UsageWindowRow>()
        val reservations = java.util.concurrent.ConcurrentHashMap<String, UsageReservationRow>()
        val ledgers = java.util.concurrent.ConcurrentHashMap<String, ModelUsageLedgerRow>()
        @Volatile private var savedSettings: UsageSettingsRow? = null

        override suspend fun window(key: String) = windows[key]
        override suspend fun putWindow(row: UsageWindowRow) { windows[row.windowKey] = row }
        override suspend fun reservation(id: String) = reservations[id]
        override suspend fun insertReservation(row: UsageReservationRow) { check(reservations.putIfAbsent(row.requestId, row) == null) }
        override suspend fun putReservation(row: UsageReservationRow) { reservations[row.requestId] = row }
        override suspend fun deleteInFlightReservation(id: String): Int =
            if (reservations[id]?.state == UsageLedgerState.IN_FLIGHT.name && reservations.remove(id) != null) 1 else 0
        override suspend fun inFlight() = reservations.values.filter { it.state == UsageLedgerState.IN_FLIGHT.name }
        override suspend fun markUnknown(id: String): Int {
            val old = reservations[id] ?: return 0
            if (old.state != UsageLedgerState.IN_FLIGHT.name) return 0
            reservations[id] = old.copy(state = UsageLedgerState.OUTCOME_UNKNOWN.name)
            return 1
        }
        override suspend fun settings() = savedSettings
        override suspend fun putSettings(row: UsageSettingsRow) { savedSettings = row }
        override suspend fun ledger(id: String) = ledgers[id]
        override suspend fun insertLedger(row: ModelUsageLedgerRow) { check(ledgers.putIfAbsent(row.requestId, row) == null) }
        override suspend fun putLedger(row: ModelUsageLedgerRow) { ledgers[row.requestId] = row }
        override suspend fun allLedgerRows() = ledgers.values.toList()
        override suspend fun markRequestUnknown(id: String) = markUnknown(id)
        override suspend fun totals(key: String) = UsageWindowTotals(0, 0, 0, 0)
        override suspend fun unresolvedCount() = reservations.values.count {
            it.state in setOf(UsageLedgerState.IN_FLIGHT.name, UsageLedgerState.OUTCOME_UNKNOWN.name, UsageLedgerState.USAGE_UNKNOWN.name)
        }.toLong()
    }

    private companion object {
        const val TEST_MODEL_ID = "serea-local/viewmodel-integration-fake"
        const val WAIT_TIMEOUT_MILLIS = 10_000L
    }
}
