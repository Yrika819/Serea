package app.serea.android

import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test

class LocalModelChatFeatureRouterTest {
    private val modelId = "serea-local/fake-fixture"

    private class Port(private val response: String) : ModelChatPort {
        var sends = 0
        override suspend fun send(request: ModelChatRequest): ModelChatResponse {
            sends++
            return ModelChatResponse(response)
        }
    }

    @Test fun featureIsOffByDefaultAndMissingSelectionFailsClosed() {
        val demo = Port("demo")
        val local = Port("fake")
        val router = LocalModelChatFeatureRouter(demo, local, modelId)
        assertTrue(runCatching { router.resolve(null) }.exceptionOrNull() is PreDispatchModelChatFailure)
        assertTrue(runCatching {
            router.resolve(LocalModelChatFeatureRouter.Selection.LocalModel(modelId))
        }.exceptionOrNull() is PreDispatchModelChatFailure)
        assertSame(demo, router.resolve(LocalModelChatFeatureRouter.Selection.Demo))
        assertEqualsNoDispatch(demo, local)
    }

    @Test fun explicitValidTestSelectionReturnsOnlyTheInjectedFakePort() = runBlocking {
        val demo = Port("demo")
        val local = Port("deterministic fake")
        val router = LocalModelChatFeatureRouter(demo, local, modelId, enabledForExplicitTestSelection = true)
        assertSame(local, router.resolve(LocalModelChatFeatureRouter.Selection.LocalModel(modelId)))
        val result = router.resolve(LocalModelChatFeatureRouter.Selection.LocalModel(modelId))
            .send(ModelChatRequest("conversation", "hello", sendId = "send"))
        org.junit.Assert.assertEquals("deterministic fake", result.text)
        assertEqualsNoDispatch(demo, local, expectedLocal = 1)
    }

    @Test fun invalidAndMismatchedModelSelectionsNeverFallBackToDemo() {
        val demo = Port("demo")
        val local = Port("fake")
        val router = LocalModelChatFeatureRouter(demo, local, modelId, enabledForExplicitTestSelection = true)
        val bad = listOf("", "https://example.test/model", "other/model")
        bad.forEach { id ->
            assertTrue(runCatching {
                router.resolve(LocalModelChatFeatureRouter.Selection.LocalModel(id))
            }.exceptionOrNull() is PreDispatchModelChatFailure)
        }
        assertEqualsNoDispatch(demo, local)
    }

    @Test fun explicitSelectionWithoutAnInjectedLocalPortFailsClosed() {
        val demo = Port("demo")
        val router = LocalModelChatFeatureRouter(
            demoPort = demo,
            localModelPort = null,
            expectedLocalModelId = modelId,
            enabledForExplicitTestSelection = true,
        )
        assertTrue(runCatching {
            router.resolve(LocalModelChatFeatureRouter.Selection.LocalModel(modelId))
        }.exceptionOrNull() is PreDispatchModelChatFailure)
        assertEquals(0, demo.sends)
    }

    private fun assertEqualsNoDispatch(demo: Port, local: Port, expectedLocal: Int = 0) {
        org.junit.Assert.assertEquals(0, demo.sends)
        org.junit.Assert.assertEquals(expectedLocal, local.sends)
    }
}
