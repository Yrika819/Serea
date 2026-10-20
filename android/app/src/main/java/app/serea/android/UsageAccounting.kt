package app.serea.android

import androidx.room.Dao
import androidx.room.Entity
import androidx.room.Index
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import java.math.BigDecimal
import java.time.Instant
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
import java.util.Currency
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.withContext

/** Local soft limits only. Provider billing and account-side limits remain authoritative. */
data class UsageLimits(
    val dailyRequests: Long = 20,
    val dailyTokens: Long = 10_000,
    val monthlyRequests: Long = 100,
    val monthlyTokens: Long = 50_000,
) {
    init {
        require(dailyRequests > 0 && dailyTokens > 0 && monthlyRequests > 0 && monthlyTokens > 0)
        require(monthlyRequests >= dailyRequests)
        require(monthlyTokens >= dailyTokens)
    }
}

data class UsageSnapshot(
    val limits: UsageLimits,
    val dailyRequests: Long,
    val dailyEstimatedInputTokens: Long,
    val dailyEstimatedOutputTokens: Long,
    val dailyProviderInputTokens: Long,
    val dailyProviderOutputTokens: Long,
    val monthlyRequests: Long,
    val monthlyEstimatedInputTokens: Long,
    val monthlyEstimatedOutputTokens: Long,
    val monthlyProviderInputTokens: Long,
    val monthlyProviderOutputTokens: Long,
    val uncertainRequests: Long,
)

data class UsageReservation(val requestId: String, val estimatedInputTokens: Long, val reservedOutputTokens: Long)

class UsageLimitExceededException : Exception("The local daily or monthly usage limit has been reached.")
class UsageAccountingOverflowException : Exception("Usage totals exceed the supported integer range.")
class DuplicateUsageRequestException(message: String) : Exception(message)

enum class UsageLedgerState { IN_FLIGHT, SETTLED, USAGE_UNKNOWN, OUTCOME_UNKNOWN }
enum class CostEstimationStatus { ESTIMATED, USAGE_REPORTED_COST_ESTIMATED, UNKNOWN }
enum class UsageTimeWindow { UTC_DAILY, UTC_MONTHLY, ALL_TIME }

/** Prices are immutable request-time evidence. Values are decimal strings, never binary floats. */
data class VerifiedPriceSnapshot(
    val inputPricePerMillion: String,
    val outputPricePerMillion: String,
    val currency: String,
    val source: String,
    val verifiedAt: String,
    val effectiveAt: String? = null,
    val revision: String? = null,
) {
    init {
        require(validPrice(inputPricePerMillion) && validPrice(outputPricePerMillion)) { "Pricing must be finite non-negative decimal values." }
        require(validCurrency(currency)) { "Currency must be a supported ISO 4217 code." }
        require(source.isNotBlank() && source.length <= 200)
        require(verifiedAt.length <= 40 && runCatching { Instant.parse(verifiedAt) }.isSuccess)
        require(effectiveAt == null || (effectiveAt.length <= 40 && runCatching { Instant.parse(effectiveAt) }.isSuccess))
        require(revision == null || (revision.isNotBlank() && revision.length <= 100))
    }
}

/** Provider identity must come from trusted explicit attribution, never from parsing the model ID. */
data class ModelUsageAttribution(
    val modelId: String = UNKNOWN_ATTRIBUTION,
    val providerId: String = UNKNOWN_ATTRIBUTION,
    val price: VerifiedPriceSnapshot? = null,
) {
    init {
        require(validAttributionId(modelId)) { "Invalid model attribution." }
        require(validAttributionId(providerId)) { "Invalid provider attribution." }
    }

    companion object { const val UNKNOWN_ATTRIBUTION = "UNKNOWN" }
}

fun interface UsageAttributionResolver {
    fun resolve(request: ModelChatRequest): ModelUsageAttribution
}

/** Safe compatibility resolver. It records an explicit selectedModelId and never guesses a provider. */
object RequestModelAttribution : UsageAttributionResolver {
    override fun resolve(request: ModelChatRequest) = ModelUsageAttribution(
        modelId = request.selectedModelId?.takeIf { it.isNotBlank() } ?: ModelUsageAttribution.UNKNOWN_ATTRIBUTION,
    )
}

@Entity(tableName = "usage_windows", primaryKeys = ["windowKey"])
data class UsageWindowRow(val windowKey: String, val requests: Long, val inputTokens: Long, val outputTokens: Long)

@Entity(tableName = "usage_reservations", primaryKeys = ["requestId"])
data class UsageReservationRow(
    val requestId: String,
    val dayKey: String,
    val monthKey: String,
    val inputTokens: Long,
    val outputTokens: Long,
    val state: String,
)

@Entity(tableName = "usage_settings", primaryKeys = ["id"])
data class UsageSettingsRow(
    val id: Int = 1,
    val dailyRequests: Long = 20,
    val dailyTokens: Long = 10_000,
    val monthlyRequests: Long = 100,
    val monthlyTokens: Long = 50_000,
    val lastObservedAt: Long = 0,
)

/** One durable attribution/cost row per usage reservation; requestId is a SQLite primary key. */
@Entity(
    tableName = "model_usage_ledger",
    primaryKeys = ["requestId"],
    indices = [Index(value = ["dayKey", "providerId", "modelId"]), Index(value = ["monthKey", "providerId", "modelId"])],
)
data class ModelUsageLedgerRow(
    val requestId: String,
    val modelId: String,
    val providerId: String,
    val createdAt: Long,
    val dayKey: String,
    val monthKey: String,
    val estimatedInputTokens: Long,
    val reservedOutputTokens: Long,
    val providerInputTokens: Long?,
    val providerOutputTokens: Long?,
    val usageStatus: String,
    val costStatus: String,
    val estimatedCost: String?,
    val currency: String?,
    val pricingSource: String?,
    val pricingVerifiedAt: String?,
    val inputPricePerMillion: String?,
    val outputPricePerMillion: String?,
    val pricingEffectiveAt: String?,
    val pricingRevision: String?,
    val reservationState: String,
    val settledAt: Long?,
    val uncertainOutcomeReason: String?,
    val modelAttributionStatus: String,
)

data class UsageWindowTotals(val estimatedInputTokens: Long, val estimatedOutputTokens: Long, val providerInputTokens: Long, val providerOutputTokens: Long)

data class ModelUsageSummary(
    val timeWindow: UsageTimeWindow,
    val windowKey: String?,
    val providerId: String?,
    val modelId: String?,
    val requestCount: Long,
    val estimatedInputTokens: Long,
    val reservedOutputTokens: Long,
    val providerReportedInputTokens: Long,
    val providerReportedOutputTokens: Long,
    val uncertainRequestCount: Long,
    val estimatedCostsByCurrency: Map<String, String>,
    val costUnknownCount: Long,
)

@Dao
interface UsageDao {
    @Query("SELECT * FROM usage_windows WHERE windowKey = :key") suspend fun window(key: String): UsageWindowRow?
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun putWindow(row: UsageWindowRow)
    @Query("SELECT * FROM usage_reservations WHERE requestId = :id") suspend fun reservation(id: String): UsageReservationRow?
    @Insert(onConflict = OnConflictStrategy.ABORT) suspend fun insertReservation(row: UsageReservationRow)
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun putReservation(row: UsageReservationRow)
    @Query("SELECT * FROM usage_reservations WHERE state = 'IN_FLIGHT'") suspend fun inFlight(): List<UsageReservationRow>
    @Query("UPDATE usage_reservations SET state = 'OUTCOME_UNKNOWN' WHERE requestId = :id AND state = 'IN_FLIGHT'") suspend fun markUnknown(id: String): Int
    @Query("SELECT * FROM usage_settings WHERE id = 1") suspend fun settings(): UsageSettingsRow?
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun putSettings(row: UsageSettingsRow)
    @Query("SELECT * FROM model_usage_ledger WHERE requestId = :id") suspend fun ledger(id: String): ModelUsageLedgerRow?
    @Insert(onConflict = OnConflictStrategy.ABORT) suspend fun insertLedger(row: ModelUsageLedgerRow)
    @Insert(onConflict = OnConflictStrategy.REPLACE) suspend fun putLedger(row: ModelUsageLedgerRow)
    @Query("SELECT * FROM model_usage_ledger") suspend fun allLedgerRows(): List<ModelUsageLedgerRow>
    @Query("UPDATE usage_reservations SET state = 'OUTCOME_UNKNOWN' WHERE requestId = :id AND state = 'IN_FLIGHT'") suspend fun markRequestUnknown(id: String): Int
    @Query("SELECT COALESCE(SUM(CASE WHEN state != 'SETTLED' THEN inputTokens ELSE 0 END), 0) AS estimatedInputTokens, COALESCE(SUM(CASE WHEN state != 'SETTLED' THEN outputTokens ELSE 0 END), 0) AS estimatedOutputTokens, COALESCE(SUM(CASE WHEN state = 'SETTLED' THEN inputTokens ELSE 0 END), 0) AS providerInputTokens, COALESCE(SUM(CASE WHEN state = 'SETTLED' THEN outputTokens ELSE 0 END), 0) AS providerOutputTokens FROM usage_reservations WHERE dayKey = :key OR monthKey = :key")
    suspend fun totals(key: String): UsageWindowTotals

    @Transaction
    suspend fun reserve(requestId: String, requestedAt: Long, input: Long, outputCap: Long, attribution: ModelUsageAttribution = ModelUsageAttribution()): UsageReservation {
        validateRequestId(requestId)
        require(input >= 0 && outputCap >= 0)
        val old = reservation(requestId)
        if (old != null) {
            val oldLedger = ledger(requestId)
            if (oldLedger != null && (oldLedger.modelId != attribution.modelId || oldLedger.providerId != attribution.providerId)) {
                throw DuplicateUsageRequestException("The send ID is already attributed to different model/provider values.")
            }
            throw DuplicateUsageRequestException("A send ID can be reserved only once.")
        }

        val saved = settings() ?: UsageSettingsRow().also { putSettings(it) }
        // Wall-clock rollback cannot reopen a previously observed daily/monthly window.
        val effectiveAt = maxOf(requestedAt, saved.lastObservedAt)
        val day = dayKey(effectiveAt)
        val month = monthKey(effectiveAt)
        val limits = saved.toLimits()
        val dayUsage = window("D:$day") ?: UsageWindowRow("D:$day", 0, 0, 0)
        val monthUsage = window("M:$month") ?: UsageWindowRow("M:$month", 0, 0, 0)
        val dayRequests = safeAdd(dayUsage.requests, 1)
        val monthRequests = safeAdd(monthUsage.requests, 1)
        val dayTokens = safeAdd(safeAdd(dayUsage.inputTokens, dayUsage.outputTokens), safeAdd(input, outputCap))
        val monthTokens = safeAdd(safeAdd(monthUsage.inputTokens, monthUsage.outputTokens), safeAdd(input, outputCap))
        if (dayRequests > limits.dailyRequests || dayTokens > limits.dailyTokens ||
            monthRequests > limits.monthlyRequests || monthTokens > limits.monthlyTokens) throw UsageLimitExceededException()

        val reservationRow = UsageReservationRow(requestId, "D:$day", "M:$month", input, outputCap, "IN_FLIGHT")
        insertReservation(reservationRow)
        insertLedger(attribution.toLedger(requestId, effectiveAt, "D:$day", "M:$month", input, outputCap))
        putSettings(saved.copy(lastObservedAt = effectiveAt))
        putWindow(dayUsage.copy(requests = dayRequests, inputTokens = safeAdd(dayUsage.inputTokens, input), outputTokens = safeAdd(dayUsage.outputTokens, outputCap)))
        putWindow(monthUsage.copy(requests = monthRequests, inputTokens = safeAdd(monthUsage.inputTokens, input), outputTokens = safeAdd(monthUsage.outputTokens, outputCap)))
        return UsageReservation(requestId, input, outputCap)
    }

    @Transaction
    suspend fun settle(requestId: String, inputTokens: Long?, outputTokens: Long?, settledAt: Long) {
        val item = reservation(requestId) ?: return
        val savedLedger = ledger(requestId) ?: throw IllegalStateException("Usage reservation is missing its atomic ledger row.")
        if (item.state != "IN_FLIGHT" || savedLedger.usageStatus != UsageLedgerState.IN_FLIGHT.name) return
        if (inputTokens == null || outputTokens == null) {
            putReservation(item.copy(state = UsageLedgerState.USAGE_UNKNOWN.name))
            putLedger(savedLedger.copy(usageStatus = UsageLedgerState.USAGE_UNKNOWN.name, reservationState = UsageLedgerState.USAGE_UNKNOWN.name,
                costStatus = CostEstimationStatus.UNKNOWN.name, estimatedCost = null, uncertainOutcomeReason = "Provider usage was absent or incomplete."))
            return // Keep the full reservation; missing usage is not zero.
        }
        require(inputTokens >= 0 && outputTokens >= 0) { "Provider token usage must be non-negative." }
        val actualInput = inputTokens
        val actualOutput = outputTokens
        listOf(item.dayKey, item.monthKey).forEach { key ->
            val current = window(key) ?: return@forEach
            putWindow(current.copy(
                inputTokens = safeAdd((current.inputTokens - item.inputTokens).coerceAtLeast(0), actualInput),
                outputTokens = safeAdd((current.outputTokens - item.outputTokens).coerceAtLeast(0), actualOutput),
            ))
        }
        val cost = savedLedger.toCost(actualInput, actualOutput)
        putReservation(item.copy(inputTokens = actualInput, outputTokens = actualOutput, state = UsageLedgerState.SETTLED.name))
        putLedger(savedLedger.copy(providerInputTokens = actualInput, providerOutputTokens = actualOutput,
            usageStatus = UsageLedgerState.SETTLED.name, reservationState = UsageLedgerState.SETTLED.name,
            costStatus = cost.status.name, estimatedCost = cost.amount, settledAt = settledAt, uncertainOutcomeReason = null))
    }

    @Transaction
    suspend fun markInterruptedRequestsUnknown(reason: String = "Process ended before a response outcome was recorded.") {
        require(reason.length <= 240)
        inFlight().forEach { item ->
            if (markUnknown(item.requestId) == 1) {
                ledger(item.requestId)?.let { row -> putLedger(row.copy(usageStatus = UsageLedgerState.OUTCOME_UNKNOWN.name,
                    reservationState = UsageLedgerState.OUTCOME_UNKNOWN.name, costStatus = CostEstimationStatus.UNKNOWN.name,
                    estimatedCost = null, uncertainOutcomeReason = reason)) }
            }
        }
    }

    @Transaction
    suspend fun markRequestOutcomeUnknown(requestId: String, reason: String) {
        require(reason.length <= 240)
        if (markRequestUnknown(requestId) == 1) {
            ledger(requestId)?.let { row -> putLedger(row.copy(usageStatus = UsageLedgerState.OUTCOME_UNKNOWN.name,
                reservationState = UsageLedgerState.OUTCOME_UNKNOWN.name, costStatus = CostEstimationStatus.UNKNOWN.name,
                estimatedCost = null, uncertainOutcomeReason = reason)) }
        }
    }

    @Transaction
    suspend fun updateLimits(limits: UsageLimits) {
        val previous = settings() ?: UsageSettingsRow()
        putSettings(previous.copy(dailyRequests = limits.dailyRequests, dailyTokens = limits.dailyTokens,
            monthlyRequests = limits.monthlyRequests, monthlyTokens = limits.monthlyTokens))
    }

    @Transaction
    suspend fun snapshot(now: Long): UsageSnapshot {
        val saved = settings() ?: UsageSettingsRow().also { putSettings(it) }
        val effectiveAt = maxOf(now, saved.lastObservedAt)
        if (effectiveAt != saved.lastObservedAt) putSettings(saved.copy(lastObservedAt = effectiveAt))
        val dayKey = "D:${dayKey(effectiveAt)}"
        val monthKey = "M:${monthKey(effectiveAt)}"
        val dayWindow = window(dayKey)
        val monthWindow = window(monthKey)
        val day = totals(dayKey)
        val month = totals(monthKey)
        return UsageSnapshot(saved.toLimits(), dayWindow?.requests ?: 0, day.estimatedInputTokens,
            day.estimatedOutputTokens, day.providerInputTokens, day.providerOutputTokens,
            monthWindow?.requests ?: 0, month.estimatedInputTokens, month.estimatedOutputTokens,
            month.providerInputTokens, month.providerOutputTokens, unresolvedCount())
    }

    @Query("SELECT COUNT(*) FROM usage_reservations WHERE state IN ('IN_FLIGHT', 'OUTCOME_UNKNOWN', 'USAGE_UNKNOWN')")
    suspend fun unresolvedCount(): Long

    @Transaction
    suspend fun summarize(timeWindow: UsageTimeWindow, now: Long, providerId: String? = null, modelId: String? = null): List<ModelUsageSummary> {
        val saved = settings() ?: UsageSettingsRow().also { putSettings(it) }
        val effectiveAt = maxOf(now, saved.lastObservedAt)
        if (effectiveAt != saved.lastObservedAt) putSettings(saved.copy(lastObservedAt = effectiveAt))
        val key = when (timeWindow) {
            UsageTimeWindow.UTC_DAILY -> dayKey(effectiveAt)
            UsageTimeWindow.UTC_MONTHLY -> monthKey(effectiveAt)
            UsageTimeWindow.ALL_TIME -> null
        }
        val rows = allLedgerRows().filter { row ->
            (key == null || if (timeWindow == UsageTimeWindow.UTC_DAILY) row.dayKey == "D:$key" else row.monthKey == "M:$key") &&
                (providerId == null || row.providerId == providerId) && (modelId == null || row.modelId == modelId)
        }
        return rows.groupBy { it.providerId to it.modelId }.map { (attribution, group) ->
            val costs = group.filter { it.costStatus != CostEstimationStatus.UNKNOWN.name && it.estimatedCost != null && it.currency != null }
                .groupBy { it.currency!! }.mapValues { (_, currencyRows) ->
                    currencyRows.fold(BigDecimal.ZERO) { total, row -> total.add(BigDecimal(row.estimatedCost!!)) }.toPlainString()
                }
            ModelUsageSummary(timeWindow, key, attribution.first, attribution.second, group.size.toLong(),
                checkedSum(group.map { it.estimatedInputTokens }), checkedSum(group.map { it.reservedOutputTokens }),
                checkedSum(group.mapNotNull { it.providerInputTokens }), checkedSum(group.mapNotNull { it.providerOutputTokens }),
                group.count { it.usageStatus == UsageLedgerState.OUTCOME_UNKNOWN.name || it.usageStatus == UsageLedgerState.USAGE_UNKNOWN.name || it.usageStatus == UsageLedgerState.IN_FLIGHT.name }.toLong(),
                costs, group.count { it.costStatus == CostEstimationStatus.UNKNOWN.name || it.estimatedCost == null }.toLong())
        }.sortedWith(compareBy<ModelUsageSummary> { it.providerId }.thenBy { it.modelId })
    }

    private fun UsageSettingsRow.toLimits() = UsageLimits(dailyRequests, dailyTokens, monthlyRequests, monthlyTokens)
    private fun dayKey(epochMillis: Long): String = Instant.ofEpochMilli(epochMillis).atZone(ZoneOffset.UTC).toLocalDate().format(DateTimeFormatter.ISO_LOCAL_DATE)
    private fun monthKey(epochMillis: Long): String = Instant.ofEpochMilli(epochMillis).atZone(ZoneOffset.UTC).format(DateTimeFormatter.ofPattern("yyyy-MM"))
}
internal class UsageRepository(private val dao: UsageDao, private val clock: () -> Long = System::currentTimeMillis) {
    /** Conservative request budgeting, not tokenizer output. */
    suspend fun reserve(requestId: String, promptChars: Int, maxOutputTokens: Int = 512, attribution: ModelUsageAttribution = ModelUsageAttribution()): UsageReservation {
        require(promptChars >= 0 && maxOutputTokens > 0)
        val estimate = try { Math.multiplyExact(promptChars.toLong(), 4L) } catch (_: ArithmeticException) { throw UsageAccountingOverflowException() }
        return dao.reserve(requestId, clock(), estimate, maxOutputTokens.toLong(), attribution)
    }
    suspend fun complete(requestId: String, providerUsage: ProviderUsage?) = dao.settle(
        requestId, providerUsage?.inputTokens?.toLong(), providerUsage?.outputTokens?.toLong(), clock(),
    )
    suspend fun outcomeUnknown(requestId: String, reason: String = "Request failed before its remote outcome was known.") = dao.markRequestOutcomeUnknown(requestId, reason)
    suspend fun recoverAfterProcessRestart() = dao.markInterruptedRequestsUnknown()
    suspend fun updateLimits(limits: UsageLimits) = dao.updateLimits(limits)
    suspend fun snapshot() = dao.snapshot(clock())
    suspend fun summaries(timeWindow: UsageTimeWindow = UsageTimeWindow.ALL_TIME, providerId: String? = null, modelId: String? = null) =
        dao.summarize(timeWindow, clock(), providerId, modelId)
    suspend fun modelUsage(modelId: String, timeWindow: UsageTimeWindow = UsageTimeWindow.ALL_TIME) = summaries(timeWindow, modelId = modelId)
    suspend fun providerUsage(providerId: String, timeWindow: UsageTimeWindow = UsageTimeWindow.ALL_TIME) = summaries(timeWindow, providerId = providerId)
}

/** Reserves both UTC windows and the attribution ledger atomically before dispatch. */
internal class UsageControlledModelChatPort(
    private val delegate: ModelChatPort,
    private val usage: UsageRepository,
    private val outputReservationTokens: Int = 512,
    private val attributionResolver: UsageAttributionResolver = RequestModelAttribution,
) : ModelChatPort {
    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        val promptChars = try { Math.addExact(request.message.length.toLong(), request.context.fold(0L) { total, item -> Math.addExact(total, item.text.length.toLong()) }).also { require(it <= Int.MAX_VALUE) }.toInt() }
            catch (_: ArithmeticException) { throw UsageAccountingOverflowException() }
        val attribution = attributionResolver.resolve(request)
        usage.reserve(request.sendId, promptChars, outputReservationTokens, attribution)
        try {
            val response = delegate.send(request)
            withContext(NonCancellable) { usage.complete(request.sendId, response.usage) }
            return response
        } catch (cancelled: CancellationException) {
            withContext(NonCancellable) { usage.outcomeUnknown(request.sendId, "Request was cancelled after reservation; remote outcome may be unknown.") }
            throw cancelled
        } catch (failure: Exception) {
            withContext(NonCancellable) { usage.outcomeUnknown(request.sendId, "Request failed after reservation; remote outcome may be unknown.") }
            throw failure
        }
    }
}

internal data class UsageCostResult(val status: CostEstimationStatus, val amount: String?)

private fun ModelUsageAttribution.toLedger(requestId: String, createdAt: Long, day: String, month: String, input: Long, output: Long) = ModelUsageLedgerRow(
    requestId = requestId, modelId = modelId, providerId = providerId, createdAt = createdAt, dayKey = day, monthKey = month,
    estimatedInputTokens = input, reservedOutputTokens = output, providerInputTokens = null, providerOutputTokens = null,
    usageStatus = UsageLedgerState.IN_FLIGHT.name, costStatus = CostEstimationStatus.UNKNOWN.name, estimatedCost = null,
    currency = price?.currency, pricingSource = price?.source, pricingVerifiedAt = price?.verifiedAt,
    inputPricePerMillion = price?.inputPricePerMillion, outputPricePerMillion = price?.outputPricePerMillion,
    pricingEffectiveAt = price?.effectiveAt, pricingRevision = price?.revision,
    reservationState = UsageLedgerState.IN_FLIGHT.name, settledAt = null, uncertainOutcomeReason = null,
    modelAttributionStatus = if (modelId == ModelUsageAttribution.UNKNOWN_ATTRIBUTION) "UNKNOWN" else "ATTRIBUTED",
)

private fun ModelUsageLedgerRow.toCost(input: Long, output: Long): UsageCostResult {
    if (inputPricePerMillion == null || outputPricePerMillion == null || currency == null || pricingSource.isNullOrBlank() || pricingVerifiedAt.isNullOrBlank()) {
        return UsageCostResult(CostEstimationStatus.UNKNOWN, null)
    }
    val snapshot = runCatching { VerifiedPriceSnapshot(inputPricePerMillion, outputPricePerMillion, currency, pricingSource, pricingVerifiedAt, pricingEffectiveAt, pricingRevision) }.getOrNull()
    return estimateUsageCost(snapshot, input, output)
}

internal fun estimateUsageCost(price: VerifiedPriceSnapshot?, inputTokens: Long?, outputTokens: Long?): UsageCostResult {
    if (price == null || inputTokens == null || outputTokens == null || inputTokens < 0 || outputTokens < 0) {
        return UsageCostResult(CostEstimationStatus.UNKNOWN, null)
    }
    return try {
        val million = BigDecimal("1000000")
        val amount = BigDecimal(price.inputPricePerMillion).multiply(BigDecimal.valueOf(inputTokens))
            .add(BigDecimal(price.outputPricePerMillion).multiply(BigDecimal.valueOf(outputTokens)))
            .divide(million).stripTrailingZeros().toPlainString()
        UsageCostResult(CostEstimationStatus.USAGE_REPORTED_COST_ESTIMATED, amount)
    } catch (_: ArithmeticException) {
        UsageCostResult(CostEstimationStatus.UNKNOWN, null)
    } catch (_: NumberFormatException) {
        UsageCostResult(CostEstimationStatus.UNKNOWN, null)
    }
}

private fun safeAdd(left: Long, right: Long): Long = try { Math.addExact(left, right) } catch (_: ArithmeticException) { throw UsageAccountingOverflowException() }
private fun checkedSum(values: List<Long>): Long = values.fold(0L, ::safeAdd)
private fun validateRequestId(value: String) { require(value.matches(Regex("[A-Za-z0-9][A-Za-z0-9._:-]{0,127}"))) { "Invalid send ID." } }
private fun validAttributionId(value: String) = value == ModelUsageAttribution.UNKNOWN_ATTRIBUTION || value.matches(Regex("[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}")) && !value.contains("://")
private fun validCurrency(value: String): Boolean = value.matches(Regex("[A-Z]{3}")) && runCatching { Currency.getInstance(value) }.isSuccess
private fun validPrice(value: String): Boolean = value.length in 1..40 && runCatching {
    val number = BigDecimal(value)
    number.signum() >= 0 && number.precision() <= 30 && number.scale() <= 18
}.getOrDefault(false)
