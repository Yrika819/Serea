package app.serea.android

import org.junit.Assert.*
import org.junit.Test

class AdaptiveModelRouterTest {
    private fun model(id: String, cost: Double, context: Int = 4096, tools: Boolean = false) = ModelDescriptor(
        id, "fake", id, Knowledge.Known(context), Knowledge.Known(1024),
        ModelCapabilities(setOf("text/plain"), Knowledge.Known(tools), Knowledge.Known(true), Knowledge.Known(false)),
        ModelPricing(Knowledge.Known(cost), Knowledge.Known(cost * 2), Knowledge.Known("USD")), ModelAvailability.AVAILABLE, authorized = true,
        source = "unit-test fixture", lastVerifiedAt = "2026-10-19T00:00:00Z",
    )

    @Test fun emptyCatalogDoesNotInventOrChooseDefaultModel() {
        val result = AdaptiveModelRouter(LocalModelCatalog()).select(RoutingRequest("hello"), ModelRoutingPolicy())
        assertNull(result.model)
        assertEquals(ModelTask.SIMPLE, result.task)
        assertTrue(result.reason.contains("ありません"))
    }
    @Test fun urlShapedIdsAndModelsWithoutSourceVerificationCannotBeSelected() {
        assertThrows(IllegalArgumentException::class.java) {
            model("https://attacker.example", .1)
        }
        val unverified = model("unverified", .1).copy(source = null, lastVerifiedAt = null)
        val result = AdaptiveModelRouter(LocalModelCatalog(listOf(unverified))).select(RoutingRequest("hi"), ModelRoutingPolicy())
        assertNull(result.model)
        assertTrue(result.excluded.single().reason.contains("情報源"))
    }
    @Test fun taskClassificationSeparatesComplexityFromCapabilities() {
        val router = AdaptiveModelRouter(LocalModelCatalog(listOf(model("a", 1.0), model("b", 2.0))))
        assertEquals(ModelTask.COMPLEX, router.classify(RoutingRequest("Please compare these approaches")))
        assertEquals(ModelTask.STRUCTURED, router.classify(RoutingRequest("json", requiresStructuredOutput = true)))
        assertEquals(ModelTask.TOOL_REQUIRED, router.classify(RoutingRequest("search", requiresTools = true)))
        assertEquals(ModelTask.LONG_CONTEXT, router.classify(RoutingRequest("question", contextCharacters = 12_001)))
    }
    @Test fun autoSimpleAndComplexChooseOnlyVerifiedRequiredCapabilities() {
        val simple = model("a-simple", .1)
        val reasoning = model("z-reasoning", .2).copy(capabilities = ModelCapabilities(
            setOf("text/plain"), Knowledge.Known(false), Knowledge.Known(true), Knowledge.Known(true)))
        val router = AdaptiveModelRouter(LocalModelCatalog(listOf(simple, reasoning)))
        assertEquals("a-simple", router.select(RoutingRequest("hello"), ModelRoutingPolicy()).selectedModelId)
        val complex = router.select(RoutingRequest("compare these options"), ModelRoutingPolicy())
        assertEquals(ModelTask.COMPLEX, complex.task)
        assertEquals("z-reasoning", complex.selectedModelId)
        assertTrue(complex.excluded.any { it.modelId == "a-simple" && it.reason.contains("Reasoning") })
    }
    @Test fun selectionIsDeterministicAndCostPolicyRequiresVerifiedPrices() {
        val router = AdaptiveModelRouter(LocalModelCatalog(listOf(model("z", 1.0), model("a", .2))))
        val request = RoutingRequest("hello")
        assertEquals("a", router.select(request, ModelRoutingPolicy()).selectedModelId)
        assertEquals("a", router.select(request, ModelRoutingPolicy()).selectedModelId)
        val withinBudget = router.select(request, ModelRoutingPolicy(maxEstimatedCost = 0.0003, budgetCurrencyCode = "USD"))
        assertEquals("a", withinBudget.selectedModelId)
        assertEquals("USD", withinBudget.estimatedCost?.currencyCode)
        assertNull(router.select(request, ModelRoutingPolicy(maxEstimatedCost = 0.0001, budgetCurrencyCode = "USD")).model)
        val unknown = model("unknown", 1.0).copy(pricing = ModelPricing(Knowledge.UNKNOWN, Knowledge.UNKNOWN, Knowledge.UNKNOWN))
        val result = AdaptiveModelRouter(LocalModelCatalog(listOf(unknown))).select(request,
            ModelRoutingPolicy(autoPolicy = AutoRoutingPolicy.COST_PRIORITY))
        assertNull(result.model)
        assertTrue(result.excluded.single().reason.contains("料金が未確認"))
    }
    @Test fun capabilityContextAvailabilityAuthorizationAndBudgetAreHardConstraints() {
        val request = RoutingRequest("x".repeat(200), contextCharacters = 8_000, requestedOutputTokens = 256, requiresTools = true)
        val candidates = listOf(
            model("context", .1, context = 1024, tools = true),
            model("capability", .1, context = 65_536, tools = false),
            model("unauthorized", .1, context = 65_536, tools = true).copy(authorized = false),
            model("unavailable", .1, context = 65_536, tools = true).copy(availability = ModelAvailability.UNAVAILABLE),
            model("expensive", 1_000.0, context = 65_536, tools = true),
            model("short-output", .1, context = 65_536, tools = true).copy(maxOutput = Knowledge.Known(64)),
        )
        val result = AdaptiveModelRouter(LocalModelCatalog(candidates)).select(request,
            ModelRoutingPolicy(maxEstimatedCost = 0.000001, budgetCurrencyCode = "USD"))
        assertNull(result.model)
        assertEquals(6, result.excluded.size)
        assertTrue(result.excluded.any { it.reason.contains("Context") })
        assertTrue(result.excluded.any { it.reason.contains("能力") })
        assertTrue(result.excluded.any { it.reason.contains("権限") })
        assertTrue(result.excluded.any { it.reason.contains("利用不可") })
        assertTrue(result.excluded.any { it.reason.contains("費用") })
        assertTrue(result.excluded.any { it.reason.contains("最大出力") })
    }
    @Test fun manualUnavailableDoesNotFallback() {
        val router = AdaptiveModelRouter(LocalModelCatalog(listOf(model("a", .1), model("b", .2).copy(availability = ModelAvailability.UNAVAILABLE))))
        val result = router.select(RoutingRequest("hello"), ModelRoutingPolicy(ModelSelectionMode.MANUAL, manualModelId = "b"))
        assertNull(result.model)
        assertTrue(result.reason.contains("自動切替はしません"))
    }
}
