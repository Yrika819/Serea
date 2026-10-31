package app.serea.android

import android.content.Context
import android.database.sqlite.SQLiteDatabase
import java.io.File
import java.security.KeyStore
import java.util.UUID
import androidx.room.Room
import androidx.test.core.app.ApplicationProvider
import androidx.test.ext.junit.runners.AndroidJUnit4
import kotlinx.coroutines.runBlocking
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Test
import org.junit.runner.RunWith

/** Non-required real-device migration proof using only synthetic app-private rows. */
@RunWith(AndroidJUnit4::class)
class LegacyHistoryV4MigrationDiagnosticTest {
    private val context = ApplicationProvider.getApplicationContext<Context>()
    private val databaseName = "legacy-history-v4-${UUID.randomUUID()}.db"
    private val keyAlias = "serea-v4-migration-${UUID.randomUUID()}"
    private var migratedDatabase: SereaHistoryDatabase? = null

    @After fun cleanup() {
        migratedDatabase?.close()
        context.deleteDatabase(databaseName)
        KeyStore.getInstance("AndroidKeyStore").apply {
            load(null)
            if (containsAlias(keyAlias)) deleteEntry(keyAlias)
        }
    }

    @Test fun v4HistoryDatabaseMigratesToLatestPreservingSyntheticEncryptedHistoryAndUsageMetadata() = runBlocking {
        val cipher = KeystoreAesGcm(keyAlias)
        val conversationId = "synthetic-v4-conversation"
        val userMessageId = "synthetic-v4-user"
        val assistantMessageId = "synthetic-v4-assistant"
        val encryptedTitle = cipher.encrypt("Synthetic v4 title".toByteArray(), "conversation:$conversationId:title".toByteArray())
        val encryptedUserBody = cipher.encrypt("Synthetic v4 user body".toByteArray(), "message:$userMessageId:body".toByteArray())
        val encryptedAssistantBody = cipher.encrypt("Synthetic v4 assistant body".toByteArray(), "message:$assistantMessageId:body".toByteArray())
        val settledReservation = UsageReservationRow(
            requestId = "synthetic-v4-settled",
            dayKey = "D:2026-10-29",
            monthKey = "M:2026-10",
            inputTokens = 41,
            outputTokens = 17,
            state = UsageLedgerState.SETTLED.name,
        )
        val unknownReservation = UsageReservationRow(
            requestId = "synthetic-v4-unknown",
            dayKey = "D:2026-10-29",
            monthKey = "M:2026-10",
            inputTokens = 23,
            outputTokens = 11,
            state = UsageLedgerState.OUTCOME_UNKNOWN.name,
        )
        val settings = UsageSettingsRow(1, 17, 1700, 177, 17700, 1793232000000L)
        val window = UsageWindowRow("D:2026-10-29", 2, 64, 28)
        val settledLedger = ModelUsageLedgerRow(
            requestId = settledReservation.requestId,
            modelId = "synthetic-model-v4",
            providerId = "synthetic-provider-v4",
            createdAt = 1793232000000L,
            dayKey = settledReservation.dayKey,
            monthKey = settledReservation.monthKey,
            estimatedInputTokens = 41,
            reservedOutputTokens = 17,
            providerInputTokens = 39,
            providerOutputTokens = 15,
            usageStatus = UsageLedgerState.SETTLED.name,
            costStatus = CostEstimationStatus.USAGE_REPORTED_COST_ESTIMATED.name,
            estimatedCost = "0.000123",
            currency = "USD",
            pricingSource = "synthetic migration fixture",
            pricingVerifiedAt = "2026-10-29T00:00:00Z",
            inputPricePerMillion = "2",
            outputPricePerMillion = "6",
            pricingEffectiveAt = "2026-10-01T00:00:00Z",
            pricingRevision = "synthetic-v4-revision",
            reservationState = UsageLedgerState.SETTLED.name,
            settledAt = 1793232001000L,
            uncertainOutcomeReason = null,
            modelAttributionStatus = "ATTRIBUTED",
        )
        val unknownLedger = ModelUsageLedgerRow(
            requestId = unknownReservation.requestId,
            modelId = "synthetic-model-v4",
            providerId = "synthetic-provider-v4",
            createdAt = 1793232002000L,
            dayKey = unknownReservation.dayKey,
            monthKey = unknownReservation.monthKey,
            estimatedInputTokens = 23,
            reservedOutputTokens = 11,
            providerInputTokens = null,
            providerOutputTokens = null,
            usageStatus = UsageLedgerState.OUTCOME_UNKNOWN.name,
            costStatus = CostEstimationStatus.UNKNOWN.name,
            estimatedCost = null,
            currency = null,
            pricingSource = null,
            pricingVerifiedAt = null,
            inputPricePerMillion = null,
            outputPricePerMillion = null,
            pricingEffectiveAt = null,
            pricingRevision = null,
            reservationState = UsageLedgerState.OUTCOME_UNKNOWN.name,
            settledAt = null,
            uncertainOutcomeReason = "Synthetic v4 interrupted request",
            modelAttributionStatus = "UNKNOWN",
        )

        createSyntheticV4Database(
            encryptedTitle = encryptedTitle,
            userMessageId = userMessageId,
            encryptedUserBody = encryptedUserBody,
            assistantMessageId = assistantMessageId,
            encryptedAssistantBody = encryptedAssistantBody,
            conversationId = conversationId,
            settings = settings,
            window = window,
            reservations = listOf(settledReservation, unknownReservation),
            ledgers = listOf(settledLedger, unknownLedger),
        )

        val migrated = Room.databaseBuilder(context, SereaHistoryDatabase::class.java, databaseName)
            .addMigrations(SereaHistoryDatabase.MIGRATION_4_5)
            .allowMainThreadQueries()
            .build()
            .also { migratedDatabase = it }

        assertEquals("the v4 file must reach the current schema", 5, migrated.openHelper.writableDatabase.version)
        val history = assertNotNullAndGet(ConversationRepository(migrated.historyDao(), cipher).get(conversationId))
        assertEquals("Synthetic v4 title", history.title)
        assertEquals(2, history.messages.size)
        assertEquals("Synthetic v4 user body", history.messages[0].text)
        assertEquals(DataOrigin.USER_AUTHORED, history.messages[0].origin)
        assertEquals(setOf(DataOrigin.UNKNOWN), history.messages[0].sourceLineage)
        assertEquals("Synthetic v4 assistant body", history.messages[1].text)
        assertEquals(DataOrigin.MODEL_OUTPUT, history.messages[1].origin)
        assertEquals(setOf(DataOrigin.UNKNOWN), history.messages[1].sourceLineage)

        assertEquals(settings, migrated.usageDao().settings())
        assertEquals(window, migrated.usageDao().window(window.windowKey))
        assertEquals(settledReservation, migrated.usageDao().reservation(settledReservation.requestId))
        assertEquals(unknownReservation, migrated.usageDao().reservation(unknownReservation.requestId))
        assertEquals(settledLedger, migrated.usageDao().ledger(settledLedger.requestId))
        assertEquals(unknownLedger, migrated.usageDao().ledger(unknownLedger.requestId))

        val storedTitle = migrated.openHelper.readableDatabase.query(
            "SELECT encryptedTitle FROM conversations WHERE id = ?", arrayOf(conversationId),
        ).use { cursor -> check(cursor.moveToFirst()); cursor.getString(0) }
        assertEquals("migration must leave existing ciphertext unchanged", encryptedTitle, storedTitle)
        val storedBodies = migrated.openHelper.readableDatabase.query(
            "SELECT id, encryptedBody, encryptedSourceLineage FROM messages ORDER BY createdAt, id",
        ).use { cursor ->
            buildList {
                while (cursor.moveToNext()) add(Triple(cursor.getString(0), cursor.getString(1), cursor.getString(2)))
            }
        }
        assertEquals(listOf(userMessageId, assistantMessageId), storedBodies.map { it.first })
        assertEquals(listOf(encryptedUserBody, encryptedAssistantBody), storedBodies.map { it.second })
        assertEquals(listOf("UNKNOWN", "UNKNOWN"), storedBodies.map { it.third })
    }

    private fun createSyntheticV4Database(
        encryptedTitle: String,
        userMessageId: String,
        encryptedUserBody: String,
        assistantMessageId: String,
        encryptedAssistantBody: String,
        conversationId: String,
        settings: UsageSettingsRow,
        window: UsageWindowRow,
        reservations: List<UsageReservationRow>,
        ledgers: List<ModelUsageLedgerRow>,
    ) {
        val file: File = context.getDatabasePath(databaseName)
        file.parentFile?.mkdirs()
        val raw = SQLiteDatabase.openOrCreateDatabase(file, null)
        raw.setForeignKeyConstraintsEnabled(true)
        raw.execSQL("CREATE TABLE conversations (id TEXT NOT NULL, encryptedTitle TEXT NOT NULL, createdAt INTEGER NOT NULL, updatedAt INTEGER NOT NULL, modelId TEXT NOT NULL, deletedAt INTEGER, PRIMARY KEY(id))")
        raw.execSQL("CREATE TABLE messages (id TEXT NOT NULL, conversationId TEXT NOT NULL, role TEXT NOT NULL, encryptedBody TEXT NOT NULL, classification TEXT NOT NULL, createdAt INTEGER NOT NULL, status TEXT NOT NULL, inputTokens INTEGER, outputTokens INTEGER, origin TEXT NOT NULL DEFAULT 'UNKNOWN', PRIMARY KEY(id))")
        raw.execSQL("CREATE TABLE usage_windows (windowKey TEXT NOT NULL, requests INTEGER NOT NULL, inputTokens INTEGER NOT NULL, outputTokens INTEGER NOT NULL, PRIMARY KEY(windowKey))")
        raw.execSQL("CREATE TABLE usage_reservations (requestId TEXT NOT NULL, dayKey TEXT NOT NULL, monthKey TEXT NOT NULL, inputTokens INTEGER NOT NULL, outputTokens INTEGER NOT NULL, state TEXT NOT NULL, PRIMARY KEY(requestId))")
        raw.execSQL("CREATE TABLE usage_settings (id INTEGER NOT NULL, dailyRequests INTEGER NOT NULL, dailyTokens INTEGER NOT NULL, monthlyRequests INTEGER NOT NULL, monthlyTokens INTEGER NOT NULL, lastObservedAt INTEGER NOT NULL, PRIMARY KEY(id))")
        raw.execSQL("CREATE TABLE model_usage_ledger (requestId TEXT NOT NULL, modelId TEXT NOT NULL, providerId TEXT NOT NULL, createdAt INTEGER NOT NULL, dayKey TEXT NOT NULL, monthKey TEXT NOT NULL, estimatedInputTokens INTEGER NOT NULL, reservedOutputTokens INTEGER NOT NULL, providerInputTokens INTEGER, providerOutputTokens INTEGER, usageStatus TEXT NOT NULL, costStatus TEXT NOT NULL, estimatedCost TEXT, currency TEXT, pricingSource TEXT, pricingVerifiedAt TEXT, inputPricePerMillion TEXT, outputPricePerMillion TEXT, pricingEffectiveAt TEXT, pricingRevision TEXT, reservationState TEXT NOT NULL, settledAt INTEGER, uncertainOutcomeReason TEXT, modelAttributionStatus TEXT NOT NULL, PRIMARY KEY(requestId), FOREIGN KEY(requestId) REFERENCES usage_reservations(requestId) ON UPDATE NO ACTION ON DELETE CASCADE)")
        raw.execSQL("CREATE INDEX index_model_usage_ledger_dayKey_providerId_modelId ON model_usage_ledger(dayKey, providerId, modelId)")
        raw.execSQL("CREATE INDEX index_model_usage_ledger_monthKey_providerId_modelId ON model_usage_ledger(monthKey, providerId, modelId)")
        raw.execSQL(
            "INSERT INTO conversations VALUES (?, ?, ?, ?, ?, NULL)",
            arrayOf<Any?>(conversationId, encryptedTitle, 1793232000000L, 1793232003000L, "synthetic-v4-model"),
        )
        raw.execSQL(
            "INSERT INTO messages (id, conversationId, role, encryptedBody, classification, createdAt, status, origin) VALUES (?, ?, 'USER', ?, 'PRIVATE', 1793232001000, 'COMPLETED', 'USER_AUTHORED')",
            arrayOf(userMessageId, conversationId, encryptedUserBody),
        )
        raw.execSQL(
            "INSERT INTO messages (id, conversationId, role, encryptedBody, classification, createdAt, status, origin) VALUES (?, ?, 'ASSISTANT', ?, 'PUBLIC', 1793232002000, 'COMPLETED', 'MODEL_OUTPUT')",
            arrayOf(assistantMessageId, conversationId, encryptedAssistantBody),
        )
        raw.execSQL("INSERT INTO usage_settings VALUES (?, ?, ?, ?, ?, ?)", arrayOf<Any?>(settings.id, settings.dailyRequests, settings.dailyTokens, settings.monthlyRequests, settings.monthlyTokens, settings.lastObservedAt))
        raw.execSQL("INSERT INTO usage_windows VALUES (?, ?, ?, ?)", arrayOf<Any?>(window.windowKey, window.requests, window.inputTokens, window.outputTokens))
        reservations.forEach { row ->
            raw.execSQL("INSERT INTO usage_reservations VALUES (?, ?, ?, ?, ?, ?)", arrayOf<Any?>(row.requestId, row.dayKey, row.monthKey, row.inputTokens, row.outputTokens, row.state))
        }
        ledgers.forEach { row ->
            raw.execSQL(
                "INSERT INTO model_usage_ledger VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
                arrayOf<Any?>(row.requestId, row.modelId, row.providerId, row.createdAt, row.dayKey, row.monthKey,
                    row.estimatedInputTokens, row.reservedOutputTokens, row.providerInputTokens, row.providerOutputTokens,
                    row.usageStatus, row.costStatus, row.estimatedCost, row.currency, row.pricingSource,
                    row.pricingVerifiedAt, row.inputPricePerMillion, row.outputPricePerMillion,
                    row.pricingEffectiveAt, row.pricingRevision, row.reservationState, row.settledAt,
                    row.uncertainOutcomeReason, row.modelAttributionStatus),
            )
        }
        raw.version = 4
        raw.close()
    }

    private fun <T : Any> assertNotNullAndGet(value: T?): T {
        assertNotNull(value)
        return checkNotNull(value)
    }
}
