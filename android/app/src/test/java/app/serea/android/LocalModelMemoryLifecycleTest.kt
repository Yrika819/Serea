package app.serea.android

import android.content.ComponentCallbacks2
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.async
import kotlinx.coroutines.cancel
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

class LocalModelMemoryLifecycleTest {
    @Test fun applicationCompositionRegistersOneLifecycleWithItsOwnedAdapterAndClosesIt() = runTest {
        val owner = FakeCallbacksOwner()
        val processOwner = FakeProcessVisibilityOwner()
        val events = mutableListOf<LocalModelMemoryLifecycle.Event>()
        val directory = createTempDir(prefix = "serea-app-lifecycle")
        val composition = LocalModelRuntimeApplicationComposition.create(
            modelDirectory = directory,
            callbacks = owner,
            processVisibility = processOwner,
            scope = this,
            dispatcher = StandardTestDispatcher(testScheduler),
            eventObserver = { events += it },
        )
        try {
            assertTrue("lifecycle and runtime owner share one adapter object", composition.lifecycle.adapter === composition.adapter)
            composition.register()
            composition.register()
            assertEquals("Application owner registers the callback once", 1, owner.registered.size)
            assertEquals("process owner registers one visibility observer", 1, processOwner.registered.size)
            assertTrue("registered callback is the lifecycle instance", owner.registered.single() === composition.lifecycle)

            // Invoke the registered callback API directly. This checks registration and dispatch
            // identity only; it does not claim that Android itself delivered an OS notification.
            owner.registered.single().onTrimMemory(ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN)
            assertEquals(ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN, events.single().level)

            assertEquals("UI-hidden callback alone does not evict local consent", 0L,
                composition.authorizations.lifecycle.currentGeneration())

            processOwner.registered.single().onProcessVisibilityChanged(false)
            advanceUntilIdle()
            assertEquals("process background reaches this lifecycle and evicts local consent", 1L,
                composition.authorizations.lifecycle.currentGeneration())
            processOwner.registered.single().onProcessVisibilityChanged(true)
            advanceUntilIdle()
            assertEquals("foreground resumes without a second eviction", 1L,
                composition.authorizations.lifecycle.currentGeneration())
        } finally {
            composition.close()
            assertTrue("close unregisters Application callback", owner.registered.isEmpty())
            assertTrue("close unregisters process visibility observer", processOwner.registered.isEmpty())
            assertEquals(1, owner.unregistered.size)
            directory.deleteRecursively()
        }
    }

    @Test fun processVisibilityRegistrationFailureCleansCallbackAndClosesAdapter() = runTest {
        val owner = FakeCallbacksOwner()
        val processOwner = FakeProcessVisibilityOwner(failAfterRegister = true)
        val directory = createTempDir(prefix = "serea-app-lifecycle-register-failure")
        val composition = LocalModelRuntimeApplicationComposition.create(
            modelDirectory = directory,
            callbacks = owner,
            processVisibility = processOwner,
            scope = this,
            dispatcher = StandardTestDispatcher(testScheduler),
        )
        try {
            val failure = runCatching { composition.register() }.exceptionOrNull()
            assertEquals("process visibility registration failure", failure?.message)
            assertTrue("Application callback is cleaned after process observer failure", owner.registered.isEmpty())
            assertTrue("partially registered process observer is cleaned", processOwner.registered.isEmpty())
            val sendFailure = runCatching { composition.adapter.send(ModelChatRequest("c", "m")) }.exceptionOrNull()
            assertTrue("adapter is closed after failed registration", sendFailure is PreDispatchModelChatFailure)
            assertEquals("Local model is closed.", sendFailure?.message)
        } finally {
            runCatching { composition.close() }
            directory.deleteRecursively()
        }
    }

    @Test fun componentCallbackRegistrationFailureStillClosesAdapterAndKeepsOriginalCause() = runTest {
        val owner = FakeCallbacksOwner(failAfterRegister = true)
        val processOwner = FakeProcessVisibilityOwner()
        val directory = createTempDir(prefix = "serea-app-callback-register-failure")
        val composition = LocalModelRuntimeApplicationComposition.create(
            modelDirectory = directory,
            callbacks = owner,
            processVisibility = processOwner,
            scope = this,
            dispatcher = StandardTestDispatcher(testScheduler),
        )
        try {
            val failure = runCatching { composition.register() }.exceptionOrNull()
            assertEquals("component callback registration failure", failure?.message)
            assertTrue("partially registered Application callback is cleaned", owner.registered.isEmpty())
            assertTrue("process observer was never registered", processOwner.registered.isEmpty())
            val sendFailure = runCatching { composition.adapter.send(ModelChatRequest("c", "m")) }.exceptionOrNull()
            assertEquals("Local model is closed.", sendFailure?.message)
        } finally {
            runCatching { composition.close() }
            directory.deleteRecursively()
        }
    }

    @Test fun closeAttemptsEveryCleanupAndPreservesFirstUnregisterFailure() = runTest {
        val owner = FakeCallbacksOwner(failOnUnregister = true)
        val processOwner = FakeProcessVisibilityOwner(failOnUnregister = true)
        val directory = createTempDir(prefix = "serea-app-lifecycle-close-failure")
        val composition = LocalModelRuntimeApplicationComposition.create(
            modelDirectory = directory,
            callbacks = owner,
            processVisibility = processOwner,
            scope = this,
            dispatcher = StandardTestDispatcher(testScheduler),
        )
        try {
            composition.register()
            val failure = runCatching { composition.close() }.exceptionOrNull()
            assertEquals("process visibility unregister failure", failure?.message)
            assertEquals("process observer cleanup was attempted", 1, processOwner.unregisterCalls)
            assertEquals("Application callback cleanup was still attempted", 1, owner.unregisterCalls)
            val sendFailure = runCatching { composition.adapter.send(ModelChatRequest("c", "m")) }.exceptionOrNull()
            assertEquals("Local model is closed.", sendFailure?.message)
        } finally {
            runCatching { composition.close() }
            directory.deleteRecursively()
        }
    }

    @Test fun staleCoalescerCompletionCannotClearNewerEvictionTicket() {
        val coalescer = LocalModelEvictionCoalescer()
        val first = coalescer.begin()!!
        coalescer.complete(first)
        val second = coalescer.begin()!!
        assertFalse(first == second)
        coalescer.complete(first)
        assertEquals("stale completion cannot free the newer ticket", null, coalescer.begin())
        coalescer.complete(second)
        assertTrue(coalescer.begin() != null)
    }

    @Test fun localEvictionConsentGenerationDoesNotAffectCloudEgress() = runTest {
        val now = 1_800_000_000_000L
        val registry = LocalModelAuthorizationRegistry(clock = { now })
        registry.invalidateForEviction()
        val cloudRequest = ModelChatRequest(
            conversationId = "cloud-conversation", message = "cloud remains independently authorized",
            sendId = "cloud-after-local-eviction", classification = DataClassification.PUBLIC,
            bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
            egressContract = EgressContract(
                conversationId = "cloud-conversation", currentMessageId = "cloud-after-local-eviction",
                origin = DataOrigin.USER_AUTHORED, classification = DataClassification.PUBLIC,
                destination = "https://ollama.com/api/chat", plannedBody = "cloud remains independently authorized",
                context = emptyList(), preparation = PayloadPreparation.RAW,
                explicitSendIntentAtMillis = now, expiresAtMillis = now + 60_000,
                target = EgressTarget.CLOUD, plannedBodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
            ),
        )
        var dispatches = 0
        val delegate = object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                dispatches++
                return ModelChatResponse("cloud response")
            }
        }
        val cloud = EgressCheckedModelChatPort(delegate, { now }, EgressTarget.CLOUD)
        assertEquals("cloud response", cloud.send(cloudRequest).text)
        assertEquals(1, dispatches)
    }

    @Test fun evictionRacingSuspendedConsentConsumptionRejectsTheProof() = runTest {
        val validationStarted = CompletableDeferred<Unit>()
        val continueValidation = CompletableDeferred<Unit>()
        var bindingReads = 0
        val registry = LocalModelAuthorizationRegistry(
            clock = { 1_800_000_000_000L },
            currentConversation = LocalModelCurrentConversationPort {
                if (++bindingReads == 1) {
                    validationStarted.complete(Unit)
                    continueValidation.await()
                }
                LocalModelConversationBinding("conversation-1", 0)
            },
        )
        val request = request("consent-eviction-race")
        val proof = registry.approve(request, "serea-local/test")
        val consume = async { registry.consumeOnce(proof, contract(proof, "consent-eviction-race")) }
        validationStarted.await()
        registry.invalidateForEviction()
        continueValidation.complete(Unit)
        assertFalse("eviction between consume checks invalidates this proof", consume.await())
    }

    @Test fun osAndSyntheticSignalsAreDistinctUiHiddenIsNotEvictionAndBackgroundInvalidatesProofs() = runTest {
        val file = File.createTempFile("serea-lifecycle", ".gguf").apply { writeBytes(byteArrayOf(1, 2, 3)) }
        val owner = FakeCallbacksOwner()
        val now = 1_800_000_000_000L
        val registry = LocalModelAuthorizationRegistry(
            clock = { now },
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding("conversation-1", 0)
            },
        )
        val artifact = LocalModelArtifact(
            modelId = "serea-local/test", displayName = "test",
            modelRepository = "https://example.test/model", modelRevision = "a".repeat(40),
            licenseId = "Apache-2.0", expectedSha256 = "0".repeat(64), expectedSizeBytes = 3,
            runtimeName = "llama.cpp", runtimeRevision = "abcdef0123456789", supportedAbis = setOf("x86_64"),
        )
        val adapter = LocalModelChatAdapter(
            artifact, file, LocalModelInferenceBackendFactory { error("unloaded adapter must not create a backend") },
            "x86_64",
        )
        val scope = CoroutineScope(SupervisorJob() + StandardTestDispatcher(testScheduler))
        val events = mutableListOf<LocalModelMemoryLifecycle.Event>()
        val lifecycle = LocalModelMemoryLifecycle(
            owner, adapter, registry, scope, StandardTestDispatcher(testScheduler), { events += it },
        )
        try {
            lifecycle.register()
            assertEquals(1, owner.registered.size)
            val hidden = proof(registry, "hidden")
            owner.registered.single().onTrimMemory(ComponentCallbacks2.TRIM_MEMORY_UI_HIDDEN)
            assertTrue(registry.consumeOnce(hidden, contract(hidden, "hidden")))
            assertEquals("UI-hidden alone does not evict", 0L, registry.lifecycle.currentGeneration())

            val continued = proof(registry, "continued-background")
            lifecycle.setUserAllowedBackgroundWork(true)
            owner.registered.single().onTrimMemory(ComponentCallbacks2.TRIM_MEMORY_BACKGROUND)
            assertTrue("explicitly allowed background work keeps its current proof", registry.consumeOnce(
                continued, contract(continued, "continued-background"),
            ))
            assertEquals(0L, registry.lifecycle.currentGeneration())
            lifecycle.setUserAllowedBackgroundWork(false)
            lifecycle.setForeground(true)

            val stale = proof(registry, "stale")
            lifecycle.setForeground(false)
            advanceUntilIdle()
            assertEquals("background transition requests eviction without a trim callback", 1L, registry.lifecycle.currentGeneration())
            assertFalse(registry.consumeOnce(stale, contract(stale, "stale")))

            lifecycle.dispatchSyntheticTrimForTest(ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW)
            lifecycle.dispatchSyntheticTrimForTest(ComponentCallbacks2.TRIM_MEMORY_BACKGROUND)
            lifecycle.dispatchSyntheticTrimForTest(ComponentCallbacks2.TRIM_MEMORY_BACKGROUND)
            advanceUntilIdle()
            assertEquals("duplicate callback events add only one eviction generation", 2L, registry.lifecycle.currentGeneration())
            assertFalse(registry.consumeOnce(stale, contract(stale, "stale")))
            assertEquals(5, events.size)
            assertEquals(LocalModelMemoryLifecycle.Source.ANDROID, events.first().source)
            assertEquals(LocalModelMemoryLifecycle.Source.ANDROID, events[1].source)
            assertTrue(events.drop(2).all { it.source == LocalModelMemoryLifecycle.Source.SYNTHETIC_TEST })

            val forceRequested = proof(registry, "force-after-foreground")
            lifecycle.setForeground(false) // queues an automatic ticket
            lifecycle.setForeground(true) // that ticket will skip when it runs
            lifecycle.requestEviction() // must be preserved behind the active ticket
            advanceUntilIdle()
            assertEquals("explicit eviction survives a skipped background ticket", 3L, registry.lifecycle.currentGeneration())
            assertFalse(registry.consumeOnce(forceRequested, contract(forceRequested, "force-after-foreground")))

            lifecycle.close()
            assertTrue(owner.registered.isEmpty())
            assertEquals(1, owner.unregistered.size)
        } finally {
            lifecycle.close()
            scope.cancel()
            adapter.close()
            file.delete()
        }
    }

    private fun proof(registry: LocalModelAuthorizationRegistry, id: String): LocalModelConsentProof {
        val request = request(id)
        return registry.approve(request, "serea-local/test")
    }

    private fun request(id: String) = ModelChatRequest(
        conversationId = "conversation-1", message = id, sendId = id,
        classification = DataClassification.PUBLIC,
        bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
    )

    private fun contract(proof: LocalModelConsentProof, id: String) = EgressContract(
        conversationId = "conversation-1", currentMessageId = id,
        origin = DataOrigin.USER_AUTHORED, classification = DataClassification.PUBLIC,
        destination = ChatEgressGate.LOCAL_MODEL_DESTINATION, plannedBody = id, context = emptyList(),
        preparation = PayloadPreparation.RAW, explicitSendIntentAtMillis = null, expiresAtMillis = null,
        selectedModelId = "serea-local/test", target = EgressTarget.LOCAL_MODEL,
        plannedBodySourceLineage = setOf(DataOrigin.USER_AUTHORED), localModelConsent = proof,
    )

    private class FakeCallbacksOwner(
        private val failAfterRegister: Boolean = false,
        private val failOnUnregister: Boolean = false,
    ) : ComponentCallbacks2Owner {
        val registered = mutableListOf<ComponentCallbacks2>()
        val unregistered = mutableListOf<ComponentCallbacks2>()
        var unregisterCalls = 0
        override fun registerComponentCallbacks(callback: ComponentCallbacks2) {
            registered += callback
            if (failAfterRegister) throw IllegalStateException("component callback registration failure")
        }
        override fun unregisterComponentCallbacks(callback: ComponentCallbacks2) {
            unregisterCalls++
            if (failOnUnregister) throw IllegalStateException("component callback unregister failure")
            registered.remove(callback)
            unregistered += callback
        }
    }

    private class FakeProcessVisibilityOwner(
        private val failAfterRegister: Boolean = false,
        private val failOnUnregister: Boolean = false,
    ) : ProcessVisibilityOwner {
        val registered = mutableListOf<ProcessVisibilityObserver>()
        var unregisterCalls = 0
        override fun register(observer: ProcessVisibilityObserver) {
            if (observer !in registered) registered += observer
            if (failAfterRegister) throw IllegalStateException("process visibility registration failure")
        }
        override fun unregister(observer: ProcessVisibilityObserver) {
            unregisterCalls++
            if (failOnUnregister) throw IllegalStateException("process visibility unregister failure")
            registered.remove(observer)
        }
    }
}
