package app.serea.android

import org.junit.Assert.assertEquals
import org.junit.Assert.assertThrows
import org.junit.Test

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
}
