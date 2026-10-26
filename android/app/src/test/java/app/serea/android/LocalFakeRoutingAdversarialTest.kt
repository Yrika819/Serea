package app.serea.android

import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test
import java.time.Instant

/** Contract coverage for the offline model path; the fake below never opens a network connection. */
class LocalFakeRoutingAdversarialTest {
    @Test fun routedFakePassesEgressAndUsageAndDeniedRequestsStopBeforeReservation() = runBlocking {
        val now = 50_000L
        val nowInstant = Instant.ofEpochMilli(now)
        val model = ModelDescriptor(
            id = "fixture/local-fake",
            providerId = "fixture",
            displayName = "Local fictional fixture",
            contextWindow = Knowledge.Known(4096),
            maxOutput = Knowledge.Known(1024),
            capabilities = ModelCapabilities(setOf("text/plain")),
            availability = ModelAvailability.AVAILABLE,
            authorized = true,
            lastVerifiedAt = nowInstant.minusSeconds(1).toString(),
            source = "fictional local test fixture",
            verificationStatus = ModelVerificationStatus.TEST_FIXTURE,
        )
        val catalog = FakeModelCatalog(listOf(model), clock = { nowInstant })
        val dao = MemoryUsageDao()
        val fake = LocalFakeModelChatPort()
        val usage = UsageControlledModelChatPort(
            delegate = fake,
            usage = UsageRepository(dao) { now },
            attributionResolver = CatalogUsageAttributionResolver(catalog, clock = { nowInstant }),
        )
        val egress = EgressCheckedModelChatPort(usage) { now }
        val routed = RoutedModelChatPort(
            delegate = egress,
            router = AdaptiveModelRouter(catalog),
            policy = {
                ModelRoutingPolicy(
                    mode = ModelSelectionMode.MANUAL,
                    manualModelId = model.id,
                    allowedProviderIds = setOf(model.providerId),
                )
            },
        )

        val sendId = "fixture-local-approved"
        val body = "synthetic local prompt"
        val approved = routed.send(
            ModelChatRequest(
                conversationId = "fixture-conversation",
                message = body,
                sendId = sendId,
                classification = DataClassification.PUBLIC,
                egressContract = contract("fixture-conversation", sendId, body, DataOrigin.USER_AUTHORED, DataClassification.PUBLIC, now),
            ),
        )
        assertEquals("local fixture response", approved.text)
        assertEquals(model.id, fake.lastRequest?.selectedModelId)
        assertEquals(1, fake.calls)
        assertEquals(UsageLedgerState.SETTLED.name, dao.reservation(sendId)?.state)
        assertEquals(model.id, dao.ledger(sendId)?.modelId)
        assertEquals(model.providerId, dao.ledger(sendId)?.providerId)
        assertEquals(3L, dao.ledger(sendId)?.providerInputTokens)
        assertEquals(2L, dao.ledger(sendId)?.providerOutputTokens)

        val googleOriginSendId = "fixture-gmail-denied"
        assertThrows(ModelChatFailure::class.java) {
            runBlocking {
                routed.send(
                    ModelChatRequest(
                        conversationId = "fixture-conversation",
                        message = "synthetic mail excerpt",
                        sendId = googleOriginSendId,
                        classification = DataClassification.PUBLIC,
                        egressContract = contract(
                            "fixture-conversation", googleOriginSendId, "synthetic mail excerpt",
                            DataOrigin.GMAIL, DataClassification.PUBLIC, now,
                        ),
                    ),
                )
            }
        }
        assertNull("Google-origin data is rejected before Usage reservation", dao.reservation(googleOriginSendId))

        val sensitiveSendId = "fixture-secret-denied"
        assertThrows(ModelChatFailure::class.java) {
            runBlocking {
                routed.send(
                    ModelChatRequest(
                        conversationId = "fixture-conversation",
                        message = "synthetic secret fixture",
                        sendId = sensitiveSendId,
                        classification = DataClassification.SECRET,
                        egressContract = contract(
                            "fixture-conversation", sensitiveSendId, "synthetic secret fixture",
                            DataOrigin.USER_AUTHORED, DataClassification.SECRET, now,
                        ),
                    ),
                )
            }
        }
        assertNull("Sensitive data is rejected before Usage reservation", dao.reservation(sensitiveSendId))
        assertEquals("denied requests never reach the local provider delegate", 1, fake.calls)
    }

    private fun contract(
        conversationId: String,
        sendId: String,
        body: String,
        origin: DataOrigin,
        classification: DataClassification,
        now: Long,
    ) = EgressContract(
        conversationId = conversationId,
        currentMessageId = sendId,
        origin = origin,
        classification = classification,
        destination = "https://ollama.com/api/chat",
        plannedBody = body,
        context = emptyList(),
        preparation = PayloadPreparation.RAW,
        explicitSendIntentAtMillis = now,
        expiresAtMillis = now + 30_000,
    )

    private class LocalFakeModelChatPort : ModelChatPort {
        var calls = 0
            private set
        var lastRequest: ModelChatRequest? = null
            private set

        override suspend fun send(request: ModelChatRequest): ModelChatResponse {
            calls++
            lastRequest = request
            return ModelChatResponse("local fixture response", ProviderUsage(3, 2))
        }
    }

    /** Minimal in-memory DAO for exercising the real UsageDao default reservation/settlement logic. */
    private class MemoryUsageDao : UsageDao {
        private val windows = mutableMapOf<String, UsageWindowRow>()
        private val reservations = mutableMapOf<String, UsageReservationRow>()
        private var savedSettings: UsageSettingsRow? = null
        private val ledgers = mutableMapOf<String, ModelUsageLedgerRow>()

        override suspend fun window(key: String) = windows[key]
        override suspend fun putWindow(row: UsageWindowRow) { windows[row.windowKey] = row }
        override suspend fun reservation(id: String) = reservations[id]
        override suspend fun insertReservation(row: UsageReservationRow) { check(reservations.putIfAbsent(row.requestId, row) == null) }
        override suspend fun putReservation(row: UsageReservationRow) { reservations[row.requestId] = row }
        override suspend fun deleteInFlightReservation(id: String): Int {
            if (reservations[id]?.state != UsageLedgerState.IN_FLIGHT.name) return 0
            reservations.remove(id)
            ledgers.remove(id)
            return 1
        }
        override suspend fun inFlight() = reservations.values.filter { it.state == UsageLedgerState.IN_FLIGHT.name }
        override suspend fun markUnknown(id: String): Int {
            val old = reservations[id] ?: return 0
            if (old.state != UsageLedgerState.IN_FLIGHT.name) return 0
            reservations[id] = old.copy(state = UsageLedgerState.OUTCOME_UNKNOWN.name)
            return 1
        }
        override suspend fun settings() = savedSettings
        override suspend fun putSettings(row: UsageSettingsRow) { savedSettings = row }
        override suspend fun ledger(id: String) = ledgers[id]
        override suspend fun insertLedger(row: ModelUsageLedgerRow) { check(ledgers.putIfAbsent(row.requestId, row) == null) }
        override suspend fun putLedger(row: ModelUsageLedgerRow) { ledgers[row.requestId] = row }
        override suspend fun allLedgerRows() = ledgers.values.toList()
        override suspend fun markRequestUnknown(id: String) = markUnknown(id)
        override suspend fun totals(key: String): UsageWindowTotals {
            val matching = reservations.values.filter { it.dayKey == key || it.monthKey == key }
            return UsageWindowTotals(
                estimatedInputTokens = matching.filter { it.state != UsageLedgerState.SETTLED.name }.sumOf { it.inputTokens },
                estimatedOutputTokens = matching.filter { it.state != UsageLedgerState.SETTLED.name }.sumOf { it.outputTokens },
                providerInputTokens = matching.filter { it.state == UsageLedgerState.SETTLED.name }.sumOf { it.inputTokens },
                providerOutputTokens = matching.filter { it.state == UsageLedgerState.SETTLED.name }.sumOf { it.outputTokens },
            )
        }
        override suspend fun unresolvedCount() = reservations.values.count {
            it.state in setOf(UsageLedgerState.IN_FLIGHT.name, UsageLedgerState.OUTCOME_UNKNOWN.name, UsageLedgerState.USAGE_UNKNOWN.name)
        }.toLong()
    }
}
