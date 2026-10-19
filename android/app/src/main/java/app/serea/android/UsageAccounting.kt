package app.serea.android

import androidx.room.Entity
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.Query
import androidx.room.Transaction
import java.time.Instant
import java.time.ZoneOffset
import java.time.format.DateTimeFormatter
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

data class UsageReservation(
    val requestId: String,
    val estimatedInputTokens: Long,
    val reservedOutputTokens: Long,
)

class UsageLimitExceededException : Exception("The local daily or monthly usage limit has been reached.")

@Entity(tableName = "usage_windows", primaryKeys = ["windowKey"])
data class UsageWindowRow(
    val windowKey: String,
    val requests: Long,
    val inputTokens: Long,
    val outputTokens: Long,
)

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

data class UsageWindowTotals(
    val estimatedInputTokens: Long,
    val estimatedOutputTokens: Long,
    val providerInputTokens: Long,
    val providerOutputTokens: Long,
)

@androidx.room.Dao
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
    @Query("SELECT COALESCE(SUM(CASE WHEN state != 'SETTLED' THEN inputTokens ELSE 0 END), 0) AS estimatedInputTokens, COALESCE(SUM(CASE WHEN state != 'SETTLED' THEN outputTokens ELSE 0 END), 0) AS estimatedOutputTokens, COALESCE(SUM(CASE WHEN state = 'SETTLED' THEN inputTokens ELSE 0 END), 0) AS providerInputTokens, COALESCE(SUM(CASE WHEN state = 'SETTLED' THEN outputTokens ELSE 0 END), 0) AS providerOutputTokens FROM usage_reservations WHERE dayKey = :key OR monthKey = :key")
    suspend fun totals(key: String): UsageWindowTotals

    @Transaction
    suspend fun reserve(requestId: String, requestedAt: Long, input: Long, outputCap: Long): UsageReservation {
        require(requestId.isNotBlank() && input >= 0 && outputCap >= 0)
        check(reservation(requestId) == null) { "A send ID can be reserved only once." }
        val saved = settings() ?: UsageSettingsRow().also { putSettings(it) }
        // Wall-clock rollback cannot reopen a previously observed daily/monthly window.
        val effectiveAt = maxOf(requestedAt, saved.lastObservedAt)
        val observed = saved.copy(lastObservedAt = effectiveAt)
        putSettings(observed)
        val day = dayKey(effectiveAt)
        val month = monthKey(effectiveAt)
        val limits = observed.toLimits()
        val dayUsage = window("D:$day") ?: UsageWindowRow("D:$day", 0, 0, 0)
        val monthUsage = window("M:$month") ?: UsageWindowRow("M:$month", 0, 0, 0)
        if (dayUsage.requests + 1 > limits.dailyRequests ||
            dayUsage.inputTokens + dayUsage.outputTokens + input + outputCap > limits.dailyTokens ||
            monthUsage.requests + 1 > limits.monthlyRequests ||
            monthUsage.inputTokens + monthUsage.outputTokens + input + outputCap > limits.monthlyTokens) {
            throw UsageLimitExceededException()
        }
        putWindow(dayUsage.copy(requests = dayUsage.requests + 1, inputTokens = dayUsage.inputTokens + input,
            outputTokens = dayUsage.outputTokens + outputCap))
        putWindow(monthUsage.copy(requests = monthUsage.requests + 1, inputTokens = monthUsage.inputTokens + input,
            outputTokens = monthUsage.outputTokens + outputCap))
        insertReservation(UsageReservationRow(requestId, "D:$day", "M:$month", input, outputCap, "IN_FLIGHT"))
        return UsageReservation(requestId, input, outputCap)
    }

    @Transaction
    suspend fun settle(requestId: String, inputTokens: Long?, outputTokens: Long?) {
        val item = reservation(requestId) ?: return
        if (item.state != "IN_FLIGHT") return
        if (inputTokens == null || outputTokens == null) {
            putReservation(item.copy(state = "USAGE_UNKNOWN"))
            return // Keep the complete reservation when provider usage is missing.
        }
        val actualInput = inputTokens.coerceAtLeast(0)
        val actualOutput = outputTokens.coerceAtLeast(0)
        listOf(item.dayKey, item.monthKey).forEach { key ->
            val current = window(key) ?: return@forEach
            putWindow(current.copy(
                inputTokens = (current.inputTokens - item.inputTokens + actualInput).coerceAtLeast(0),
                outputTokens = (current.outputTokens - item.outputTokens + actualOutput).coerceAtLeast(0),
            ))
        }
        putReservation(item.copy(inputTokens = actualInput, outputTokens = actualOutput, state = "SETTLED"))
    }

    @Transaction
    suspend fun markInterruptedRequestsUnknown() {
        inFlight().forEach { putReservation(it.copy(state = "OUTCOME_UNKNOWN")) }
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

    private fun UsageSettingsRow.toLimits() = UsageLimits(dailyRequests, dailyTokens, monthlyRequests, monthlyTokens)

    private fun dayKey(epochMillis: Long): String = Instant.ofEpochMilli(epochMillis).atZone(ZoneOffset.UTC)
        .toLocalDate().format(DateTimeFormatter.ISO_LOCAL_DATE)

    private fun monthKey(epochMillis: Long): String = Instant.ofEpochMilli(epochMillis).atZone(ZoneOffset.UTC)
        .format(DateTimeFormatter.ofPattern("yyyy-MM"))
}

internal class UsageRepository(private val dao: UsageDao, private val clock: () -> Long = System::currentTimeMillis) {
    /** Reserves four tokens per UTF-16 code unit plus the response cap; this is conservative budgeting, not tokenizer output. */
    suspend fun reserve(requestId: String, promptChars: Int, maxOutputTokens: Int = 512): UsageReservation {
        require(promptChars >= 0 && maxOutputTokens > 0)
        val estimate = Math.multiplyExact(promptChars.toLong(), 4L)
        return dao.reserve(requestId, clock(), estimate, maxOutputTokens.toLong())
    }
    suspend fun complete(requestId: String, providerUsage: ProviderUsage?) = dao.settle(
        requestId, providerUsage?.inputTokens?.toLong(), providerUsage?.outputTokens?.toLong(),
    )
    suspend fun outcomeUnknown(requestId: String) = dao.markUnknown(requestId)
    suspend fun recoverAfterProcessRestart() = dao.markInterruptedRequestsUnknown()
    suspend fun updateLimits(limits: UsageLimits) = dao.updateLimits(limits)
    suspend fun snapshot() = dao.snapshot(clock())
}

/** Reserves both UTC windows before dispatch and retains the reservation on ambiguous outcomes. */
internal class UsageControlledModelChatPort(
    private val delegate: ModelChatPort,
    private val usage: UsageRepository,
    private val outputReservationTokens: Int = 512,
) : ModelChatPort {
    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        val promptChars = request.message.length + request.context.sumOf { it.text.length }
        usage.reserve(request.sendId, promptChars, outputReservationTokens)
        try {
            val response = delegate.send(request)
            withContext(NonCancellable) { usage.complete(request.sendId, response.usage) }
            return response
        } catch (cancelled: CancellationException) {
            withContext(NonCancellable) { usage.outcomeUnknown(request.sendId) }
            throw cancelled
        } catch (failure: Exception) {
            withContext(NonCancellable) { usage.outcomeUnknown(request.sendId) }
            throw failure
        }
    }
}
