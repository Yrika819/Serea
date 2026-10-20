package app.serea.android

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import java.time.Instant
import java.util.UUID
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.async
import kotlinx.coroutines.awaitAll
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

@RunWith(AndroidJUnit4::class)
class UsageLedgerDatabaseTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private val dbName = "usage-ledger-${UUID.randomUUID()}.db"
    private var database: SereaHistoryDatabase? = null

    @After fun cleanup() {
        database?.close()
        context.deleteDatabase(dbName)
    }

    private fun open() = Room.inMemoryDatabaseBuilder(context, SereaHistoryDatabase::class.java).build().also { database = it }
    private fun fixturePrice(input: String = "2", output: String = "6", revision: String = "fictional-price-v1") =
        VerifiedPriceSnapshot(input, output, "USD", "fictional test fixture", "2026-10-20T00:00:00Z", revision = revision)
    private fun rowCount(table: String): Long = database!!.openHelper.readableDatabase
        .query("SELECT COUNT(*) FROM $table").use { cursor -> cursor.moveToFirst(); cursor.getLong(0) }

    @Test fun atomicReservationSettlementAndProviderModelSummaries() = runBlocking {
        val db = open()
        val usage = UsageRepository(db.usageDao()) { Instant.parse("2026-10-20T00:00:00Z").toEpochMilli() }
        usage.reserve("send-a", 3, 2, ModelUsageAttribution("model/a", "provider-a", fixturePrice()))
        val reservation = checkNotNull(db.usageDao().reservation("send-a"))
        assertEquals(12L, reservation.inputTokens)
        assertEquals(2L, reservation.outputTokens)
        val inFlight = checkNotNull(db.usageDao().ledger("send-a"))
        assertEquals(UsageLedgerState.IN_FLIGHT.name, inFlight.usageStatus)
        assertEquals(CostEstimationStatus.ESTIMATED.name, inFlight.costStatus)
        assertEquals(12L, inFlight.estimatedInputTokens)
        assertEquals(2L, inFlight.reservedOutputTokens)
        assertEquals("2", inFlight.inputPricePerMillion)
        assertEquals("6", inFlight.outputPricePerMillion)
        assertEquals("fictional-price-v1", inFlight.pricingRevision)
        assertEquals("0.000036", inFlight.estimatedCost)
        usage.complete("send-a", ProviderUsage(4, 2))
        val settledOnce = checkNotNull(db.usageDao().ledger("send-a"))
        val settledUsageOnce = usage.snapshot()
        assertEquals(CostEstimationStatus.USAGE_REPORTED_COST_ESTIMATED.name, settledOnce.costStatus)
        assertEquals("0.00002", settledOnce.estimatedCost)
        assertEquals(4L, settledOnce.providerInputTokens)
        assertEquals(2L, settledOnce.providerOutputTokens)
        usage.complete("send-a", ProviderUsage(400, 200)) // repeated settle is an idempotent no-op
        assertEquals(settledOnce, db.usageDao().ledger("send-a"))
        val settledUsageTwice = usage.snapshot()
        assertEquals(settledUsageOnce.dailyProviderInputTokens, settledUsageTwice.dailyProviderInputTokens)
        assertEquals(settledUsageOnce.dailyProviderOutputTokens, settledUsageTwice.dailyProviderOutputTokens)
        assertEquals(settledUsageOnce.dailyEstimatedInputTokens, settledUsageTwice.dailyEstimatedInputTokens)
        assertEquals(settledUsageOnce.dailyEstimatedOutputTokens, settledUsageTwice.dailyEstimatedOutputTokens)
        usage.reserve("send-b", 5, 3, ModelUsageAttribution("model/b", "provider-b", fixturePrice("10", "20", "fictional-price-v2")))
        usage.complete("send-b", ProviderUsage(8, 1))

        val rows = usage.summaries(UsageTimeWindow.UTC_DAILY)
        assertEquals(2, rows.size)
        val first = rows.single { it.modelId == "model/a" }
        assertEquals("provider-a", first.providerId)
        assertEquals(1L, first.requestCount)
        assertEquals(4L, first.providerReportedInputTokens)
        assertEquals(2L, first.providerReportedOutputTokens)
        assertEquals("0.00002", first.estimatedCostsByCurrency["USD"])
        assertEquals(0L, first.costUnknownCount)
        assertEquals(null, usage.providerUsage("provider-b").single().modelId)
        assertEquals(1L, usage.providerUsage("provider-b").single().requestCount)
        assertEquals(null, usage.modelUsage("model/b").single().providerId)
        assertEquals(1, usage.modelUsage("model/b").size)
        assertEquals("provider-a", usage.providerModelUsage("provider-a", "model/a").single().providerId)
        val overall = usage.overallUsage().single()
        assertEquals(null, overall.providerId)
        assertEquals(null, overall.modelId)
        assertEquals(2L, overall.requestCount)
        assertEquals(2L, usage.summaries(UsageTimeWindow.ALL_TIME).sumOf { it.requestCount })
    }

    @Test fun duplicateSendIdAndAttributionConflictAreRejectedBySqlitePrimaryKey() = runBlocking {
        val db = open()
        val usage = UsageRepository(db.usageDao())
        val original = ModelUsageAttribution("model/a", "provider-a")
        usage.reserve("same-send", 1, 4, original)
        assertThrows(DuplicateUsageRequestException::class.java) { runBlocking { usage.reserve("same-send", 1, 4, original) } }
        val conflict = assertThrows(DuplicateUsageRequestException::class.java) {
            runBlocking { usage.reserve("same-send", 1, 4, ModelUsageAttribution("model/b", "provider-b")) }
        }
        assertTrue(conflict.message.orEmpty().contains("different model/provider"))
        assertEquals(1L, usage.snapshot().dailyRequests)
        assertEquals(1, usage.summaries().single().requestCount.toInt())
    }

    @Test fun unknownAttributionMissingUsageTimeoutAndRecoveryKeepReservation() = runBlocking {
        val db = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, dbName).build().also { database = it }
        var now = Instant.parse("2026-10-20T12:00:00Z").toEpochMilli()
        val usage = UsageRepository(db.usageDao()) { now }
        usage.reserve("unknown-model", 10, 5)
        usage.complete("unknown-model", null)
        val unknownReservation = checkNotNull(db.usageDao().reservation("unknown-model"))
        val unknownLedger = checkNotNull(db.usageDao().ledger("unknown-model"))
        assertEquals(40L, unknownReservation.inputTokens)
        assertEquals(5L, unknownReservation.outputTokens)
        assertEquals(UsageLedgerState.USAGE_UNKNOWN.name, unknownReservation.state)
        assertEquals(UsageLedgerState.USAGE_UNKNOWN.name, unknownLedger.usageStatus)
        assertEquals(CostEstimationStatus.UNKNOWN.name, unknownLedger.costStatus)
        assertEquals(null, unknownLedger.estimatedCost)
        val missing = usage.summaries().single()
        assertEquals(ModelUsageAttribution.UNKNOWN_ATTRIBUTION, missing.modelId)
        assertEquals(ModelUsageAttribution.UNKNOWN_ATTRIBUTION, missing.providerId)
        assertEquals(1L, missing.uncertainRequestCount)
        assertEquals(1L, missing.costUnknownCount)

        val port = UsageControlledModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse = throw java.util.concurrent.TimeoutException("fixture timeout")
        }, usage, attributionResolver = RequestModelAttribution)
        try {
            port.send(ModelChatRequest("conversation", "prompt", sendId = "timeout-send", selectedModelId = "fixture/model"))
            throw AssertionError("expected timeout")
        } catch (_: java.util.concurrent.TimeoutException) { }
        val timeoutRow = usage.summaries().single { it.modelId == "fixture/model" }
        assertEquals(1L, timeoutRow.uncertainRequestCount)
        val timeoutReservation = checkNotNull(db.usageDao().reservation("timeout-send"))
        val timeoutLedger = checkNotNull(db.usageDao().ledger("timeout-send"))
        assertEquals(24L, timeoutReservation.inputTokens)
        assertEquals(512L, timeoutReservation.outputTokens)
        assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, timeoutReservation.state)
        assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, timeoutLedger.usageStatus)
        assertEquals(CostEstimationStatus.UNKNOWN.name, timeoutLedger.costStatus)
        assertEquals(null, timeoutLedger.estimatedCost)
        assertEquals(2L, usage.snapshot().uncertainRequests)
        val unknownSummary = usage.unknownUsageSummary().single()
        assertEquals(2L, unknownSummary.requestCount)
        assertEquals(2L, unknownSummary.uncertainRequestCount)
        assertEquals(2L, unknownSummary.costUnknownCount)

        database?.close()
        database = null
        val reopened = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, dbName).build().also { database = it }
        val afterRestart = UsageRepository(reopened.usageDao()) { now }
        afterRestart.recoverAfterProcessRestart()
        afterRestart.recoverAfterProcessRestart()
        assertEquals(2L, afterRestart.snapshot().uncertainRequests)
        assertEquals(2, reopened.usageDao().allLedgerRows().size)
        assertEquals(UsageLedgerState.USAGE_UNKNOWN.name, reopened.usageDao().ledger("unknown-model")?.usageStatus)
        assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, reopened.usageDao().ledger("timeout-send")?.usageStatus)
        assertEquals(UsageLedgerState.USAGE_UNKNOWN.name, reopened.usageDao().reservation("unknown-model")?.state)
        assertEquals(UsageLedgerState.OUTCOME_UNKNOWN.name, reopened.usageDao().reservation("timeout-send")?.state)
        assertEquals(2L, afterRestart.unknownUsageSummary().single().requestCount)
        assertEquals(1, afterRestart.summaries().count { it.modelId == "fixture/model" && it.uncertainRequestCount == 1L })
    }

    @Test fun concurrentReservationsEnforceSoftLimitAtomically() = runBlocking {
        val db = open()
        val usage = UsageRepository(db.usageDao())
        usage.updateLimits(UsageLimits(dailyRequests = 1, dailyTokens = 1000, monthlyRequests = 2, monthlyTokens = 2000))
        val results = (0 until 8).map { index -> async(Dispatchers.IO) {
            runCatching { usage.reserve("concurrent-$index", 5, 5, ModelUsageAttribution("model/$index", "provider-a")) }
        } }.awaitAll()
        assertEquals(1, results.count { it.isSuccess })
        assertEquals(7, results.count { it.exceptionOrNull() is UsageLimitExceededException })
        assertEquals(1L, usage.snapshot().dailyRequests)
        assertEquals(1L, usage.snapshot().monthlyRequests)
        assertEquals(1L, usage.summaries().sumOf { it.requestCount })
        val ledgerRows = db.usageDao().allLedgerRows()
        assertEquals(1, ledgerRows.size)
        assertEquals(1L, rowCount("usage_reservations"))
        assertEquals(1L, rowCount("model_usage_ledger"))
        assertNotNull(db.usageDao().reservation(ledgerRows.single().requestId))
    }

    @Test fun ledgerInsertFailureRollsBackReservationAndUsageWindows() = runBlocking {
        val db = open()
        val usage = UsageRepository(db.usageDao()) { Instant.parse("2026-10-20T12:00:00Z").toEpochMilli() }
        usage.reserve("kept-send", 2, 3, ModelUsageAttribution("model/kept", "provider-a"))
        usage.complete("kept-send", ProviderUsage(4, 1))
        val beforeSnapshot = usage.snapshot()
        val beforeSettings = checkNotNull(db.usageDao().settings())
        val beforeLedgerRows = db.usageDao().allLedgerRows()
        val beforeReservationCount = rowCount("usage_reservations")
        val beforeLedgerCount = rowCount("model_usage_ledger")
        val beforeSummary = usage.summaries()

        db.openHelper.writableDatabase.execSQL("CREATE TRIGGER reject_usage_ledger BEFORE INSERT ON model_usage_ledger BEGIN SELECT RAISE(ABORT, 'fixture ledger failure'); END")
        val triggerFailure = assertThrows(android.database.sqlite.SQLiteConstraintException::class.java) {
            runBlocking { usage.reserve("rollback-send", 1, 1, ModelUsageAttribution("model/rollback", "provider-a")) }
        }
        assertTrue(triggerFailure.message.orEmpty().contains("fixture ledger failure"))
        assertEquals(null, db.usageDao().reservation("rollback-send"))
        assertEquals(null, db.usageDao().ledger("rollback-send"))
        assertEquals(beforeReservationCount, rowCount("usage_reservations"))
        assertEquals(beforeLedgerCount, rowCount("model_usage_ledger"))
        val afterSnapshot = usage.snapshot()
        val afterSettings = checkNotNull(db.usageDao().settings())
        assertEquals(beforeSnapshot.dailyRequests, afterSnapshot.dailyRequests)
        assertEquals(beforeSnapshot.monthlyRequests, afterSnapshot.monthlyRequests)
        assertEquals(beforeSnapshot.dailyEstimatedInputTokens, afterSnapshot.dailyEstimatedInputTokens)
        assertEquals(beforeSnapshot.dailyEstimatedOutputTokens, afterSnapshot.dailyEstimatedOutputTokens)
        assertEquals(beforeSnapshot.monthlyEstimatedInputTokens, afterSnapshot.monthlyEstimatedInputTokens)
        assertEquals(beforeSnapshot.monthlyEstimatedOutputTokens, afterSnapshot.monthlyEstimatedOutputTokens)
        assertEquals(beforeSnapshot.dailyProviderInputTokens, afterSnapshot.dailyProviderInputTokens)
        assertEquals(beforeSnapshot.dailyProviderOutputTokens, afterSnapshot.dailyProviderOutputTokens)
        assertEquals(beforeSnapshot.monthlyProviderInputTokens, afterSnapshot.monthlyProviderInputTokens)
        assertEquals(beforeSnapshot.monthlyProviderOutputTokens, afterSnapshot.monthlyProviderOutputTokens)
        assertEquals(beforeSnapshot.uncertainRequests, afterSnapshot.uncertainRequests)
        assertEquals(beforeSnapshot, afterSnapshot)
        assertEquals(beforeSettings.lastObservedAt, afterSettings.lastObservedAt)
        assertEquals(beforeLedgerRows, db.usageDao().allLedgerRows())
        assertEquals(beforeSummary, usage.summaries())
        assertNotNull(db.usageDao().reservation("kept-send"))
        assertNotNull(db.usageDao().ledger("kept-send"))
    }

    @Test fun cancellationAndNetworkFailureRemainConservativelyUnknown() = runBlocking {
        val db = open()
        val usage = UsageRepository(db.usageDao())
        val port = UsageControlledModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse = when (request.sendId) {
                "cancel-send" -> throw kotlinx.coroutines.CancellationException("fixture cancellation")
                else -> throw java.io.IOException("fixture network failure")
            }
        }, usage)
        for (sendId in listOf("cancel-send", "network-send")) {
            try {
                port.send(ModelChatRequest("conversation", "prompt", sendId = sendId))
                throw AssertionError("expected failure")
            } catch (_: Exception) { }
        }
        val summary = usage.summaries().single()
        assertEquals(2L, summary.requestCount)
        assertEquals(2L, summary.uncertainRequestCount)
        assertEquals(0L, summary.providerReportedInputTokens + summary.providerReportedOutputTokens)
        assertEquals(2L, summary.costUnknownCount)
        assertEquals(2L * ("prompt".length * 4L + 512L), summary.estimatedInputTokens + summary.reservedOutputTokens)
    }

    @Test fun egressDenyOccursBeforeUsageReservation() = runBlocking {
        val db = open()
        val usage = UsageRepository(db.usageDao())
        var providerCalls = 0
        val usagePort = UsageControlledModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                providerCalls++
                return ModelChatResponse("must not dispatch")
            }
        }, usage)
        val gated = EgressCheckedModelChatPort(usagePort)
        assertThrows(ModelChatFailure::class.java) { runBlocking { gated.send(ModelChatRequest("conversation", "private prompt", sendId = "egress-deny")) } }
        assertEquals(0, providerCalls)
        assertEquals(null, db.usageDao().reservation("egress-deny"))
        assertTrue(usage.summaries().isEmpty())
        assertEquals(0L, usage.snapshot().dailyRequests)
    }

    @Test fun tokenAndWindowLongOverflowRejectsReservationWithoutPartialWrites() = runBlocking {
        val db = open()
        val usage = UsageRepository(db.usageDao())
        assertThrows(UsageAccountingOverflowException::class.java) {
            runBlocking { db.usageDao().reserve("overflow-input", 1L, Long.MAX_VALUE, 1L, ModelUsageAttribution("model/overflow", "provider-a")) }
        }
        usage.updateLimits(UsageLimits(dailyRequests = 5, dailyTokens = Long.MAX_VALUE, monthlyRequests = 10, monthlyTokens = Long.MAX_VALUE))
        usage.reserve("max-window", 1, 1, ModelUsageAttribution("model/overflow", "provider-a"))
        // Exercise the checked DAO boundary with the largest legal token count.
        db.usageDao().settle("max-window", Long.MAX_VALUE - 1, 1, 2L)
        assertThrows(UsageAccountingOverflowException::class.java) {
            runBlocking { db.usageDao().reserve("overflow-window", 3L, 1L, 0L, ModelUsageAttribution("model/overflow", "provider-a")) }
        }
        assertEquals(null, db.usageDao().reservation("overflow-input"))
        assertEquals(null, db.usageDao().reservation("overflow-window"))
        assertEquals(1L, usage.snapshot().dailyRequests)
        assertEquals(1L, usage.summaries().sumOf { it.requestCount })
    }

    @Test fun utcDayMonthLeapDayAndYearBoundariesNeverReopenObservedWindows() = runBlocking {
        var now = Instant.parse("2024-02-29T23:59:59Z").toEpochMilli()
        val db = open()
        val usage = UsageRepository(db.usageDao()) { now }
        usage.reserve("leap-day-send", 1, 1, ModelUsageAttribution("model/calendar", "provider-calendar"))
        usage.complete("leap-day-send", ProviderUsage(1, 1))
        now = Instant.parse("2024-03-01T00:00:00Z").toEpochMilli()
        assertEquals(0L, usage.summaries(UsageTimeWindow.UTC_DAILY).sumOf { it.requestCount })
        assertEquals(0L, usage.summaries(UsageTimeWindow.UTC_MONTHLY).sumOf { it.requestCount })
        assertEquals(1L, usage.summaries(UsageTimeWindow.ALL_TIME).sumOf { it.requestCount })
        now = Instant.parse("2024-02-29T23:00:00Z").toEpochMilli() // wall clock rollback uses the last observed UTC instant
        assertEquals(0L, usage.summaries(UsageTimeWindow.UTC_DAILY).sumOf { it.requestCount })

        now = Instant.parse("2024-12-31T23:59:59Z").toEpochMilli()
        usage.reserve("year-end-send", 1, 1, ModelUsageAttribution("model/calendar", "provider-calendar"))
        usage.complete("year-end-send", ProviderUsage(1, 1))
        now = Instant.parse("2025-01-01T00:00:00Z").toEpochMilli()
        assertEquals(0L, usage.summaries(UsageTimeWindow.UTC_DAILY).sumOf { it.requestCount })
        assertEquals(0L, usage.summaries(UsageTimeWindow.UTC_MONTHLY).sumOf { it.requestCount })
        assertEquals(2L, usage.summaries(UsageTimeWindow.ALL_TIME).sumOf { it.requestCount })
    }

    @Test fun oldPriceSnapshotIsUsedAfterLaterCatalogPriceChange() = runBlocking {
        val db = open()
        val usage = UsageRepository(db.usageDao()) { Instant.parse("2026-10-20T12:00:00Z").toEpochMilli() }
        usage.reserve("before-price-change", 1, 1, ModelUsageAttribution("model/priced", "provider-priced", fixturePrice("1", "2", "old-test-price")))
        usage.complete("before-price-change", ProviderUsage(1, 1))
        // A later request receives new catalog evidence; the previous row remains tied to its own immutable snapshot.
        usage.reserve("after-price-change", 1, 1, ModelUsageAttribution("model/priced", "provider-priced", fixturePrice("100", "200", "new-test-price")))
        usage.complete("after-price-change", ProviderUsage(1, 1))
        val summary = usage.modelUsage("model/priced").single()
        assertEquals("0.000303", summary.estimatedCostsByCurrency["USD"])
        val rows = db.usageDao().allLedgerRows().associateBy { it.requestId }
        assertEquals("old-test-price", rows.getValue("before-price-change").pricingRevision)
        assertEquals("new-test-price", rows.getValue("after-price-change").pricingRevision)
    }

    @Test fun migrationsFromVersionsOneTwoAndThreePreserveHistoryAndUnknownAttribution() = runBlocking {
        for (version in 1..3) {
            val name = "usage-migration-$version-${UUID.randomUUID()}.db"
            val file = context.getDatabasePath(name)
            file.parentFile?.mkdirs()
            val raw = SQLiteDatabase.openOrCreateDatabase(file, null)
            raw.execSQL("CREATE TABLE conversations (id TEXT NOT NULL PRIMARY KEY, encryptedTitle TEXT NOT NULL, createdAt INTEGER NOT NULL, updatedAt INTEGER NOT NULL, modelId TEXT NOT NULL, deletedAt INTEGER)")
            val originColumn = if (version >= 3) ", origin TEXT NOT NULL DEFAULT 'UNKNOWN'" else ""
            raw.execSQL("CREATE TABLE messages (id TEXT NOT NULL PRIMARY KEY, conversationId TEXT NOT NULL, role TEXT NOT NULL, encryptedBody TEXT NOT NULL, classification TEXT NOT NULL, createdAt INTEGER NOT NULL, status TEXT NOT NULL, inputTokens INTEGER, outputTokens INTEGER$originColumn)")
            raw.execSQL("INSERT INTO conversations VALUES ('conversation', 'encrypted-title-fixture', 1, 2, 'legacy-model', NULL)")
            if (version >= 3) raw.execSQL("INSERT INTO messages VALUES ('message', 'conversation', 'USER', 'ciphertext-fixture', 'PRIVATE', 2, 'COMPLETED', NULL, NULL, 'USER_AUTHORED')")
            else raw.execSQL("INSERT INTO messages VALUES ('message', 'conversation', 'USER', 'ciphertext-fixture', 'PRIVATE', 2, 'COMPLETED', NULL, NULL)")
            if (version >= 2) {
                raw.execSQL("CREATE TABLE usage_windows (windowKey TEXT NOT NULL PRIMARY KEY, requests INTEGER NOT NULL, inputTokens INTEGER NOT NULL, outputTokens INTEGER NOT NULL)")
                raw.execSQL("CREATE TABLE usage_reservations (requestId TEXT NOT NULL PRIMARY KEY, dayKey TEXT NOT NULL, monthKey TEXT NOT NULL, inputTokens INTEGER NOT NULL, outputTokens INTEGER NOT NULL, state TEXT NOT NULL)")
                raw.execSQL("CREATE TABLE usage_settings (id INTEGER NOT NULL PRIMARY KEY, dailyRequests INTEGER NOT NULL, dailyTokens INTEGER NOT NULL, monthlyRequests INTEGER NOT NULL, monthlyTokens INTEGER NOT NULL, lastObservedAt INTEGER NOT NULL)")
                raw.execSQL("INSERT INTO usage_settings VALUES (1, 12, 1200, 120, 12000, 1792454400000)")
                raw.execSQL("INSERT INTO usage_windows VALUES ('D:2026-10-20', 1, 8, 16)")
                raw.execSQL("INSERT INTO usage_reservations VALUES ('legacy-send', 'D:2026-10-20', 'M:2026-10', 8, 16, 'OUTCOME_UNKNOWN')")
            }
            raw.version = version
            raw.close()

            val migrated = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, name)
                .addMigrations(SereaHistoryDatabase.MIGRATION_1_2, SereaHistoryDatabase.MIGRATION_2_3, SereaHistoryDatabase.MIGRATION_3_4)
                .allowMainThreadQueries().build()
            assertEquals(4, migrated.openHelper.writableDatabase.version)
            assertNotNull(migrated.historyDao().conversation("conversation"))
            val savedBody = migrated.openHelper.readableDatabase.query("SELECT encryptedBody FROM messages WHERE id = 'message'")
            assertTrue(savedBody.moveToFirst())
            assertEquals("ciphertext-fixture", savedBody.getString(0))
            savedBody.close()
            if (version >= 2) {
                assertEquals(12L, migrated.usageDao().settings()?.dailyRequests)
                assertEquals("OUTCOME_UNKNOWN", migrated.usageDao().reservation("legacy-send")?.state)
                val legacyLedger = migrated.usageDao().ledger("legacy-send")!!
                assertEquals("UNKNOWN", legacyLedger.modelId)
                assertEquals("UNKNOWN", legacyLedger.providerId)
                assertEquals("UNKNOWN", legacyLedger.costStatus)
                assertEquals("OUTCOME_UNKNOWN", legacyLedger.usageStatus)
            }
            migrated.close()
            context.deleteDatabase(name)
        }
    }

    @Test fun failedMigrationKeepsLegacyDatabaseAndDoesNotFallBackToDestructiveCreate() {
        val name = "usage-migration-failure-${UUID.randomUUID()}.db"
        val file = context.getDatabasePath(name)
        file.parentFile?.mkdirs()
        val raw = SQLiteDatabase.openOrCreateDatabase(file, null)
        raw.execSQL("CREATE TABLE conversations (id TEXT NOT NULL PRIMARY KEY, encryptedTitle TEXT NOT NULL, createdAt INTEGER NOT NULL, updatedAt INTEGER NOT NULL, modelId TEXT NOT NULL, deletedAt INTEGER)")
        raw.execSQL("CREATE TABLE messages (id TEXT NOT NULL PRIMARY KEY, conversationId TEXT NOT NULL, role TEXT NOT NULL, encryptedBody TEXT NOT NULL, classification TEXT NOT NULL, createdAt INTEGER NOT NULL, status TEXT NOT NULL, inputTokens INTEGER, outputTokens INTEGER, origin TEXT NOT NULL DEFAULT 'UNKNOWN')")
        raw.execSQL("CREATE TABLE usage_windows (windowKey TEXT NOT NULL PRIMARY KEY, requests INTEGER NOT NULL, inputTokens INTEGER NOT NULL, outputTokens INTEGER NOT NULL)")
        // Missing the legacy state column forces the backfill SQL to fail transactionally.
        raw.execSQL("CREATE TABLE usage_reservations (requestId TEXT NOT NULL PRIMARY KEY, dayKey TEXT NOT NULL, monthKey TEXT NOT NULL, inputTokens INTEGER NOT NULL, outputTokens INTEGER NOT NULL)")
        raw.execSQL("CREATE TABLE usage_settings (id INTEGER NOT NULL PRIMARY KEY, dailyRequests INTEGER NOT NULL, dailyTokens INTEGER NOT NULL, monthlyRequests INTEGER NOT NULL, monthlyTokens INTEGER NOT NULL, lastObservedAt INTEGER NOT NULL)")
        raw.execSQL("INSERT INTO conversations VALUES ('preserved', 'ciphertext', 1, 1, 'UNKNOWN', NULL)")
        raw.version = 3
        raw.close()

        val failedDatabase = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, name)
            .addMigrations(SereaHistoryDatabase.MIGRATION_1_2, SereaHistoryDatabase.MIGRATION_2_3, SereaHistoryDatabase.MIGRATION_3_4)
            .allowMainThreadQueries().build()
        try {
            assertThrows(Exception::class.java) { failedDatabase.openHelper.writableDatabase }
        } finally {
            failedDatabase.close()
        }
        val afterFailure = SQLiteDatabase.openDatabase(file.path, null, SQLiteDatabase.OPEN_READONLY)
        assertEquals(3, afterFailure.version)
        val preserved = afterFailure.rawQuery("SELECT id FROM conversations WHERE id = 'preserved'", null)
        assertTrue(preserved.moveToFirst())
        preserved.close()
        val ledgerTable = afterFailure.rawQuery("SELECT name FROM sqlite_master WHERE type='table' AND name='model_usage_ledger'", null)
        assertEquals(false, ledgerTable.moveToFirst())
        ledgerTable.close()
        afterFailure.close()
        context.deleteDatabase(name)
    }
}
