package app.serea.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test
import java.time.Instant

class UsageLedgerCostTest {
    private fun price(input: String = "1.25", output: String = "4.75", currency: String = "USD", source: String = "fictional test fixture", revision: String = "fixture-v1") =
        VerifiedPriceSnapshot(input, output, currency, source, "2026-10-20T00:00:00Z", revision = revision)

    @Test fun inputAndOutputPricesAreAppliedSeparatelyWithDecimalArithmetic() {
        val result = estimateUsageCost(price(), inputTokens = 1_000_000, outputTokens = 2_000_000)
        assertEquals(CostEstimationStatus.USAGE_REPORTED_COST_ESTIMATED, result.status)
        assertEquals("10.75", result.amount)
    }

    @Test fun zeroUsageHasKnownZeroCostOnlyWhenPriceIsVerified() {
        assertEquals("0", estimateUsageCost(price(), 0, 0).amount)
        assertEquals(CostEstimationStatus.UNKNOWN, estimateUsageCost(null, 0, 0).status)
    }

    @Test fun veryLargeTokenCountsDoNotUseFloatingPointOrOverflowLongMultiplication() {
        val result = estimateUsageCost(price(input = "0.000001", output = "0.000003"), Long.MAX_VALUE, Long.MAX_VALUE)
        assertEquals(CostEstimationStatus.USAGE_REPORTED_COST_ESTIMATED, result.status)
        assertEquals("36893488.147419103228", result.amount)
    }

    @Test fun missingUsageAndNegativeTokensRemainUnknown() {
        assertEquals(CostEstimationStatus.UNKNOWN, estimateUsageCost(price(), null, 1).status)
        assertEquals(CostEstimationStatus.UNKNOWN, estimateUsageCost(price(), 1, null).status)
        assertEquals(CostEstimationStatus.UNKNOWN, estimateUsageCost(price(), -1, 1).status)
    }

    @Test fun testFixturePricingRejectsNegativeMalformedCurrencyAndMissingSource() {
        assertThrows(IllegalArgumentException::class.java) { price(input = "-0.01") }
        assertThrows(IllegalArgumentException::class.java) { price(currency = "US") }
        assertThrows(IllegalArgumentException::class.java) { price(currency = "ZZZ") }
        assertThrows(IllegalArgumentException::class.java) { price(source = "") }
    }

    @Test fun pricingSnapshotsKeepTheirOwnRevisionAndCannotBeMutatedByLaterPrice() {
        val old = price(input = "2.00", output = "3.00", revision = "fictional-v1")
        val updated = price(input = "9.00", output = "12.00", revision = "fictional-v2")
        assertEquals("0.000007", estimateUsageCost(old, 2, 1).amount)
        assertEquals("0.00003", estimateUsageCost(updated, 2, 1).amount)
        assertEquals("fictional-v1", old.revision)
    }

    @Test fun catalogAttributionUsesOnlyExactFreshEntitledModelAndExplicitDecimalPriceEvidence() {
        val now = Instant.parse("2026-10-20T12:00:00Z")
        val modelId = "fixture/model-a"
        val model = ModelDescriptor(
            id = modelId, providerId = "fixture-provider", displayName = "Fictional A",
            pricing = ModelPricing(Knowledge.Known(1.25), Knowledge.Known(4.75), Knowledge.Known("USD")),
            availability = ModelAvailability.AVAILABLE, authorized = true,
            lastVerifiedAt = now.minusSeconds(30).toString(), source = "fictional fixture catalog",
            verificationStatus = ModelVerificationStatus.TEST_FIXTURE,
            expiresAt = now.plusSeconds(3600).toString(),
        )
        val snapshot = VerifiedPriceSnapshot(
            inputPricePerMillion = "1.2500", outputPricePerMillion = "4.7500", currency = "USD",
            source = "fictional signed fixture price feed", verifiedAt = now.minusSeconds(20).toString(),
            effectiveAt = now.minusSeconds(10).toString(), revision = "fixture-price-v1",
        )
        val prices = mapOf(modelId to VerifiedModelPriceSnapshot(modelId, snapshot, now.plusSeconds(600).toString()))
        val resolver = CatalogUsageAttributionResolver(
            modelCatalog = FakeModelCatalog(listOf(model), clock = { now }),
            priceCatalog = VerifiedModelPriceCatalog { id -> prices[id] },
            clock = { now },
        )

        val resolved = resolver.resolve(ModelChatRequest("conversation", "hello", selectedModelId = modelId))
        assertEquals(modelId, resolved.modelId)
        assertEquals("fixture-provider", resolved.providerId)
        assertEquals(snapshot, resolved.price)
        // The descriptor's Double values are never converted into billing evidence.
        assertEquals("1.2500", resolved.price?.inputPricePerMillion)
    }

    @Test fun catalogAttributionRejectsStaleOrUnauthorizedModelsBeforeUsageCanBeReserved() {
        val now = Instant.parse("2026-10-20T12:00:00Z")
        val stale = ModelDescriptor(
            id = "fixture/stale", providerId = "fixture-provider", displayName = "Stale",
            availability = ModelAvailability.AVAILABLE, authorized = true,
            lastVerifiedAt = now.minusSeconds(7200).toString(), source = "fictional fixture catalog",
            verificationStatus = ModelVerificationStatus.TEST_FIXTURE,
        )
        val notEntitled = stale.copy(id = "fixture/not-entitled", authorized = false, lastVerifiedAt = now.minusSeconds(10).toString())
        val catalog = FakeModelCatalog(listOf(stale, notEntitled), clock = { now }, maxAgeMillis = 60_000)
        val resolver = CatalogUsageAttributionResolver(catalog, clock = { now })

        assertThrows(PreDispatchModelChatFailure::class.java) {
            resolver.resolve(ModelChatRequest("conversation", "hello", selectedModelId = stale.id))
        }
        assertThrows(PreDispatchModelChatFailure::class.java) {
            resolver.resolve(ModelChatRequest("conversation", "hello", selectedModelId = notEntitled.id))
        }
        assertThrows(PreDispatchModelChatFailure::class.java) {
            resolver.resolve(ModelChatRequest("conversation", "hello", selectedModelId = "fixture/other"))
        }
    }

    @Test fun staleMismatchedOrNotYetEffectivePriceEvidenceStaysUnknown() {
        val now = Instant.parse("2026-10-20T12:00:00Z")
        val modelId = "fixture/model-a"
        val model = ModelDescriptor(
            id = modelId, providerId = "fixture-provider", displayName = "Fictional A",
            availability = ModelAvailability.AVAILABLE, authorized = true,
            lastVerifiedAt = now.minusSeconds(30).toString(), source = "fictional fixture catalog",
            verificationStatus = ModelVerificationStatus.TEST_FIXTURE,
        )
        val catalog = FakeModelCatalog(listOf(model), clock = { now })
        val stalePrice = VerifiedModelPriceSnapshot(modelId, price(), now.minusSeconds(1).toString())
        val mismatchedPrice = VerifiedModelPriceSnapshot("fixture/other", price(), now.plusSeconds(60).toString())
        val futurePrice = VerifiedModelPriceSnapshot(
            modelId, VerifiedPriceSnapshot("1", "2", "USD", "fictional fixture", now.toString(), now.plusSeconds(60).toString()),
            now.plusSeconds(120).toString(),
        )
        listOf(stalePrice, mismatchedPrice, futurePrice).forEach { evidence ->
            val result = CatalogUsageAttributionResolver(
                catalog, VerifiedModelPriceCatalog { evidence }, clock = { now },
            ).resolve(ModelChatRequest("conversation", "hello", selectedModelId = modelId))
            assertEquals("fixture-provider", result.providerId)
            assertEquals(null, result.price)
        }
    }
}
