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
            "x86_64", consentLifecycle = registry.lifecycle,
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
            lifecycle.dispatchSyntheticTrimForTest(ComponentCallbacks2.TRIM_MEMORY_RUNNING_LOW)
            lifecycle.dispatchSyntheticTrimForTest(ComponentCallbacks2.TRIM_MEMORY_BACKGROUND)
            lifecycle.dispatchSyntheticTrimForTest(ComponentCallbacks2.TRIM_MEMORY_BACKGROUND)
            advanceUntilIdle()
            assertEquals("duplicate callback events coalesce into one eviction", 1L, registry.lifecycle.currentGeneration())
            assertFalse(registry.consumeOnce(stale, contract(stale, "stale")))
            assertEquals(5, events.size)
            assertEquals(LocalModelMemoryLifecycle.Source.ANDROID, events.first().source)
            assertEquals(LocalModelMemoryLifecycle.Source.ANDROID, events[1].source)
            assertTrue(events.drop(2).all { it.source == LocalModelMemoryLifecycle.Source.SYNTHETIC_TEST })

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

    private class FakeCallbacksOwner : ComponentCallbacks2Owner {
        val registered = mutableListOf<ComponentCallbacks2>()
        val unregistered = mutableListOf<ComponentCallbacks2>()
        override fun registerComponentCallbacks(callback: ComponentCallbacks2) { registered += callback }
        override fun unregisterComponentCallbacks(callback: ComponentCallbacks2) {
            registered.remove(callback)
            unregistered += callback
        }
    }
}
