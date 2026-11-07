package app.serea.android

import android.content.Context
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.ViewModelStore
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlinx.coroutines.yield
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.security.MessageDigest
import java.util.concurrent.atomic.AtomicBoolean
import java.util.concurrent.atomic.AtomicInteger

@RunWith(AndroidJUnit4::class)
class LocalModelChatViewModelIntegrationAndroidTest {
    private val modelId = "serea-local/g10-a-fake-fixture"
    private val testNow = 1_800_000_000_000L

    @Test fun explicitlySelectedFakeRouteSendsOnceAndSettlesUsage() = runBlocking {
        val fixture = fixtureFile()
        val database = Room.inMemoryDatabaseBuilder(
            ApplicationProvider.getApplicationContext<Context>(), SereaHistoryDatabase::class.java,
        ).build()
        val backend = InstrumentationFakeLocalBackend()
        val artifact = artifact(fixture)
        val adapter = adapter(fixture, backend, artifact)
        val viewModelStore = ViewModelStore()
        try {
            val composition = composition(adapter, artifact, database)
            val port = composition.router.resolve(
                LocalModelChatFeatureRouter.Selection.LocalModel(modelId),
            )
            val chat = ViewModelProvider(viewModelStore, ChatViewModel.factory(port))
                .get(ChatViewModel::class.java)
            composition.attach(chat)
            composition.grantExplicitTestConsent()
            chat.updateInput("G10-A deterministic integration prompt")
            chat.sendMessage()
            val completed = withTimeout(15_000) {
                chat.state.first { state ->
                    !state.isLoading && state.conversation.messages.any {
                        it.role == MessageRole.ASSISTANT && it.text == "g10-a fake response"
                    }
                }
            }
            assertEquals(2, completed.conversation.messages.size)
            assertEquals(1, backend.loadCalls.get())
            assertEquals(1, backend.generateCalls.get())
            assertEquals(1, backend.fixturePaths.size)
            assertEquals(fixture.canonicalPath, backend.fixturePaths.single())
            val sendId = completed.conversation.messages.first { it.role == MessageRole.USER }.id
            assertEquals(UsageLedgerState.SETTLED.name, database.usageDao().reservation(sendId)?.state)
            assertEquals(1, database.usageDao().allLedgerRows().count { it.requestId == sendId })
        } finally {
            viewModelStore.clear()
            adapter.close()
            database.close()
            fixture.delete()
        }
        assertEquals(1, backend.closeCalls.get())
        assertFalse(backend.nativeLoaded)
        assertFalse(backend.networkAttempted)
    }

    @Test fun missingConsentFailsBeforeUsageAndBackendDispatch() = runBlocking {
        val fixture = fixtureFile()
        val database = Room.inMemoryDatabaseBuilder(
            ApplicationProvider.getApplicationContext<Context>(), SereaHistoryDatabase::class.java,
        ).build()
        val backend = InstrumentationFakeLocalBackend()
        val artifact = artifact(fixture)
        val adapter = adapter(fixture, backend, artifact)
        val store = ViewModelStore()
        try {
            val composition = composition(adapter, artifact, database)
            val port = composition.router.resolve(LocalModelChatFeatureRouter.Selection.LocalModel(modelId))
            val chat = ViewModelProvider(store, ChatViewModel.factory(port)).get(ChatViewModel::class.java)
            composition.attach(chat)
            chat.updateInput("no consent")
            chat.sendMessage()
            val failed = withTimeout(15_000) {
                chat.state.first { !it.isLoading && it.errorMessage != null }
            }
            assertTrue(failed.conversation.messages.none { it.role == MessageRole.ASSISTANT })
            assertEquals(0, database.usageDao().inFlight().size)
            assertEquals(0, database.usageDao().allLedgerRows().size)
            assertEquals(0, backend.loadCalls.get())
            assertEquals(0, backend.generateCalls.get())
        } finally {
            store.clear()
            adapter.close()
            database.close()
            fixture.delete()
        }
    }

    @Test fun cancellationDuringFakeGenerationDoesNotPersistAssistantReply() = runBlocking {
        val fixture = fixtureFile()
        val database = Room.inMemoryDatabaseBuilder(
            ApplicationProvider.getApplicationContext<Context>(), SereaHistoryDatabase::class.java,
        ).build()
        val backend = InstrumentationFakeLocalBackend(blockGeneration = true)
        val artifact = artifact(fixture)
        val adapter = adapter(fixture, backend, artifact)
        val store = ViewModelStore()
        try {
            val composition = composition(adapter, artifact, database)
            val port = composition.router.resolve(LocalModelChatFeatureRouter.Selection.LocalModel(modelId))
            val chat = ViewModelProvider(store, ChatViewModel.factory(port)).get(ChatViewModel::class.java)
            composition.attach(chat)
            composition.grantExplicitTestConsent()
            chat.updateInput("cancel fake generation")
            chat.sendMessage()
            backend.generationStarted.await()
            chat.cancelCurrent()
            backend.generationFinished.await()
            withTimeout(15_000) { backend.cancellationObserved.await() }
            val state = chat.state.value
            assertTrue(state.conversation.messages.none { it.role == MessageRole.ASSISTANT })
            assertTrue(backend.cancelCalls.get() > 0)
            withTimeout(15_000) {
                while (database.usageDao().allLedgerRows().singleOrNull()?.usageStatus != UsageLedgerState.OUTCOME_UNKNOWN.name) {
                    yield()
                }
            }
            assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name,
                database.usageDao().allLedgerRows().single().usageStatus)
        } finally {
            store.clear()
            adapter.close()
            database.close()
            fixture.delete()
        }
        assertEquals(1, backend.closeCalls.get())
    }

    private class TestComposition(
        val router: LocalModelChatFeatureRouter,
        private val approval: AtomicBoolean,
    ) {
        private var currentChat: ChatViewModel? = null
        fun attach(chat: ChatViewModel) { currentChat = chat }
        fun grantExplicitTestConsent() { approval.set(true) }
        fun currentBinding(): LocalModelConversationBinding {
            val chat = checkNotNull(currentChat)
            return LocalModelConversationBinding(
                chat.state.value.conversation.id, chat.currentConversationSelectionGeneration(),
            )
        }
        fun currentChat(): ChatViewModel = checkNotNull(currentChat)
    }

    private fun composition(
        adapter: LocalModelChatAdapter,
        artifact: LocalModelArtifact,
        database: SereaHistoryDatabase,
    ): TestComposition {
        val approval = AtomicBoolean(false)
        lateinit var harness: TestComposition
        val authorizations = LocalModelAuthorizationRegistry(
            clock = { testNow },
            currentConversation = LocalModelCurrentConversationPort { harness.currentBinding() },
        )
        val usage = UsageControlledModelChatPort(
            adapter, UsageRepository(database.usageDao()) { testNow },
            attributionResolver = LocalModelUsageAttributionResolver(artifact),
        )
        val egress = EgressCheckedModelChatPort(
            usage, { testNow }, EgressTarget.LOCAL_MODEL, authorizations, { testNow },
        )
        val descriptor = ModelDescriptor(
            id = modelId, providerId = "local-device", displayName = "G10-A Fake Fixture",
            contextWindow = Knowledge.Known(2_048), maxOutput = Knowledge.Known(64),
            capabilities = ModelCapabilities(input = setOf("text/plain")),
            pricing = ModelPricing(Knowledge.UNKNOWN, Knowledge.UNKNOWN),
            availability = ModelAvailability.AVAILABLE, authorized = true,
            lastVerifiedAt = java.time.Instant.ofEpochMilli(testNow).toString(), source = "generated inert test fixture",
            verificationStatus = ModelVerificationStatus.TEST_FIXTURE,
        )
        val catalog = FakeModelCatalog(listOf(descriptor), clock = { java.time.Instant.ofEpochMilli(testNow) })
        val routed = RoutedModelChatPort(
            delegate = egress,
            router = AdaptiveModelRouter(catalog),
            contractFactory = { request, selected ->
                if (!approval.compareAndSet(true, false)) {
                    throw PreDispatchModelChatFailure("Explicit test consent was not granted.")
                }
                val proof = authorizations.approve(
                    request, selected.id,
                    conversationGeneration = harness.currentChat().currentConversationSelectionGeneration(),
                )
                EgressContract(
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
                    selectedModelId = selected.id,
                    target = EgressTarget.LOCAL_MODEL,
                    plannedBodySourceLineage = request.bodySourceLineage,
                    localModelConsent = proof,
                )
            },
            policy = {
                ModelRoutingPolicy(
                    mode = ModelSelectionMode.MANUAL,
                    manualModelId = modelId,
                    requestedOutputTokens = 32,
                    allowedProviderIds = setOf("local-device"),
                )
            },
        )
        val router = LocalModelChatFeatureRouter(
            demoPort = MockModelChatPort(0), localModelPort = routed,
            expectedLocalModelId = modelId, enabledForExplicitTestSelection = true,
        )
        return TestComposition(router, approval).also { harness = it }
    }

    private fun fixtureFile(): File {
        val context = ApplicationProvider.getApplicationContext<Context>()
        return File.createTempFile("g10-a-inert-", ".fixture", context.cacheDir).apply {
            writeBytes(byteArrayOf(0x47, 0x31, 0x30, 0x2d, 0x41))
        }
    }

    private fun artifact(file: File): LocalModelArtifact {
        val bytes = file.readBytes()
        val sha = MessageDigest.getInstance("SHA-256").digest(bytes).joinToString("") { "%02x".format(it) }
        return LocalModelArtifact(
            modelId = modelId, displayName = "G10-A generated inert fixture",
            modelRepository = "https://example.invalid/g10-a-fixture",
            modelRevision = "0123456789abcdef0123456789abcdef01234567", licenseId = "Test-Only",
            expectedSha256 = sha, expectedSizeBytes = bytes.size.toLong(), runtimeName = "fake-runtime",
            runtimeRevision = "0123456", supportedAbis = setOf("arm64-v8a", "x86_64"),
        )
    }

    private fun adapter(
        file: File,
        backend: InstrumentationFakeLocalBackend,
        artifact: LocalModelArtifact,
    ) = LocalModelChatAdapter(
        artifact, file, LocalModelInferenceBackendFactory { backend },
        // The injected backend is ABI agnostic and never loads native code. The adapter still
        // verifies its declared ABI boundary; real device ABI compatibility is a later G10 gate.
        deviceAbi = "x86_64",
    )

}

private class InstrumentationFakeLocalBackend(
    private val blockGeneration: Boolean = false,
) : LocalModelInferenceBackend {
    val loadCalls = AtomicInteger()
    val generateCalls = AtomicInteger()
    val cancelCalls = AtomicInteger()
    val closeCalls = AtomicInteger()
    val generationStarted = CompletableDeferred<Unit>()
    val generationFinished = CompletableDeferred<Unit>()
    val cancellationObserved = CompletableDeferred<Unit>()
    val fixturePaths = mutableListOf<String>()
    @Volatile var nativeLoaded = false
    @Volatile var networkAttempted = false
    private val releaseGeneration = CompletableDeferred<Unit>()

    override suspend fun load(modelPath: String, limits: LocalModelRuntimeLimits) {
        loadCalls.incrementAndGet()
        fixturePaths += modelPath
        check(File(modelPath).canonicalFile.isFile)
        nativeLoaded = false
    }

    override suspend fun generate(prompt: String, maxOutputTokens: Int): LocalModelInferenceResult {
        generateCalls.incrementAndGet()
        generationStarted.complete(Unit)
        try {
            if (blockGeneration) releaseGeneration.await()
            return LocalModelInferenceResult("g10-a fake response", 3, 4)
        } finally {
            generationFinished.complete(Unit)
        }
    }

    override fun cancelActive() {
        cancelCalls.incrementAndGet()
        releaseGeneration.cancel(CancellationException("test cancellation"))
        cancellationObserved.complete(Unit)
    }

    override fun close() { closeCalls.incrementAndGet() }
}
