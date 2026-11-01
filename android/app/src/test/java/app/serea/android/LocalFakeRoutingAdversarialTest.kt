package app.serea.android

import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Test
import java.time.Instant

/** Contract coverage for the offline model path; the fake below never opens a network connection. */
class LocalFakeRoutingAdversarialTest {
    @Test fun importedGoogleCurrentBodyIsDeniedBeforeUsageAndProvider() = runBlocking {
        val now = 50_000L
        val dao = MemoryUsageDao()
        val provider = LocalFakeModelChatPort()
        val egress = EgressCheckedModelChatPort(
            UsageControlledModelChatPort(provider, UsageRepository(dao) { now }),
            clock = { now },
        )
        val body = "synthetic imported message text"
        val allowedContract = EgressContract(
            conversationId = "body-lineage",
            currentMessageId = "allowed-user-body",
            origin = DataOrigin.USER_AUTHORED,
            classification = DataClassification.PUBLIC,
            destination = "https://ollama.com/api/chat",
            plannedBody = "synthetic typed prompt",
            context = emptyList(),
            preparation = PayloadPreparation.RAW,
            explicitSendIntentAtMillis = now,
            expiresAtMillis = now + 30_000,
            plannedBodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
        )
        assertEquals(EgressContractDecision.APPROVED, ChatEgressGate.evaluate(allowedContract, now).decision)

        val contaminatedLineages = listOf(
            setOf(DataOrigin.USER_AUTHORED, DataOrigin.GMAIL),
            setOf(DataOrigin.USER_AUTHORED, DataOrigin.CALENDAR),
            setOf(DataOrigin.USER_AUTHORED, DataOrigin.UNKNOWN),
            setOf(DataOrigin.UNKNOWN),
            emptySet(),
        )
        contaminatedLineages.forEachIndexed { index, lineage ->
            val sendId = "imported-body-denied-$index"
            val contract = EgressContract(
                conversationId = "body-lineage",
                currentMessageId = sendId,
                origin = DataOrigin.USER_AUTHORED,
                classification = DataClassification.PUBLIC,
                destination = "https://ollama.com/api/chat",
                plannedBody = body,
                context = emptyList(),
                preparation = PayloadPreparation.RAW,
                explicitSendIntentAtMillis = now,
                expiresAtMillis = now + 30_000,
                plannedBodySourceLineage = lineage,
            )
            val request = ModelChatRequest(
                conversationId = contract.conversationId,
                message = contract.plannedBody,
                sendId = sendId,
                classification = DataClassification.PUBLIC,
                egressContract = contract,
                bodySourceLineage = lineage,
            )
            assertEquals(EgressContractDecision.SOURCE_LINEAGE_DENIED, ChatEgressGate.evaluate(contract, now).decision)
            assertThrows(ModelChatFailure::class.java) { runBlocking { egress.send(request) } }
            assertNull("No Usage reservation for lineage $lineage", dao.reservation(sendId))
            assertEquals("No provider call for lineage $lineage", 0, provider.calls)
        }

        val mismatchedId = "body-lineage-mismatch"
        val mismatchedContract = EgressContract(
            conversationId = "body-lineage",
            currentMessageId = mismatchedId,
            origin = DataOrigin.USER_AUTHORED,
            classification = DataClassification.PUBLIC,
            destination = "https://ollama.com/api/chat",
            plannedBody = body,
            context = emptyList(),
            preparation = PayloadPreparation.RAW,
            explicitSendIntentAtMillis = now,
            expiresAtMillis = now + 30_000,
            plannedBodySourceLineage = setOf(DataOrigin.USER_AUTHORED, DataOrigin.GMAIL),
        )
        assertThrows(ModelChatFailure::class.java) {
            runBlocking {
                egress.send(ModelChatRequest(
                    conversationId = mismatchedContract.conversationId,
                    message = mismatchedContract.plannedBody,
                    sendId = mismatchedId,
                    classification = DataClassification.PUBLIC,
                    egressContract = mismatchedContract,
                    bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
                ))
            }
        }
        assertNull("A lineage mismatch is denied before Usage reservation", dao.reservation(mismatchedId))
        assertEquals(0, provider.calls)
    }

    @Test fun publicModelOutputDerivedFromGmailOrCalendarIsDeniedBeforeUsageAndProvider() = runBlocking {
        val now = 50_000L
        for (source in listOf(DataOrigin.GMAIL, DataOrigin.CALENDAR)) {
            val dao = MemoryUsageDao()
            val provider = LocalFakeModelChatPort()
            val usage = UsageControlledModelChatPort(provider, UsageRepository(dao) { now })
            val egress = EgressCheckedModelChatPort(usage, clock = { now })
            val conversationId = "lineage-${source.name.lowercase()}"
            val sendId = "send-${source.name.lowercase()}"
            val context = listOf(ContextMessage(
                role = MessageRole.ASSISTANT,
                text = "synthetic summarized source",
                classification = DataClassification.PUBLIC,
                sourceMessageId = "summary-${source.name.lowercase()}",
                origin = DataOrigin.MODEL_OUTPUT,
                conversationId = conversationId,
                sourceLineage = setOf(DataOrigin.MODEL_OUTPUT, DataOrigin.USER_AUTHORED, source),
            ))
            val contract = EgressContract(
                conversationId = conversationId,
                currentMessageId = sendId,
                origin = DataOrigin.USER_AUTHORED,
                classification = DataClassification.PUBLIC,
                destination = "https://ollama.com/api/chat",
                plannedBody = "synthetic public follow-up",
                context = context,
                preparation = PayloadPreparation.RAW,
                explicitSendIntentAtMillis = now,
                expiresAtMillis = now + 30_000,
                plannedBodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
            )
            val request = ModelChatRequest(
                conversationId = conversationId,
                message = contract.plannedBody,
                sendId = sendId,
                classification = DataClassification.PUBLIC,
                context = context,
                egressContract = contract,
                bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
            )

            assertEquals(EgressContractDecision.SOURCE_LINEAGE_DENIED, ChatEgressGate.evaluate(contract, now).decision)
            assertThrows(ModelChatFailure::class.java) { runBlocking { egress.send(request) } }
            assertNull("$source lineage is rejected before Usage reservation", dao.reservation(sendId))
            assertEquals("$source lineage is rejected before provider dispatch", 0, provider.calls)
        }
    }

    @Test fun staleGoogleHandoffAfterConfirmationIsDeniedBeforeUsageAndProvider() = runBlocking {
        val now = 50_000L
        val dao = MemoryUsageDao()
        val provider = LocalFakeModelChatPort()
        val authorization = object : LocalDemoGoogleHandoffAuthorization() {
            override val origin = DataOrigin.GMAIL
            override suspend fun consume(request: ModelChatRequest) = false
        }
        val usage = UsageControlledModelChatPort(provider, UsageRepository(dao) { now })
        val egress = EgressCheckedModelChatPort(usage, clock = { now }, expectedTarget = EgressTarget.LOCAL_DEMO)
        val sendId = "stale-confirmed-google-handoff"
        val body = "Google-origin content from an expired session"
        val lineage = setOf(DataOrigin.GMAIL)
        val contract = EgressContract(
            conversationId = "handoff-conversation",
            currentMessageId = sendId,
            origin = DataOrigin.USER_AUTHORED,
            classification = DataClassification.PUBLIC,
            destination = ChatEgressGate.LOCAL_DEMO_DESTINATION,
            plannedBody = body,
            context = emptyList(),
            preparation = PayloadPreparation.RAW,
            explicitSendIntentAtMillis = null,
            expiresAtMillis = null,
            selectedModelId = DemoChatComposition.DEMO_MODEL_ID,
            target = EgressTarget.LOCAL_DEMO,
            plannedBodySourceLineage = lineage,
            localDemoGoogleHandoffAuthorization = authorization,
        )
        val request = ModelChatRequest(
            conversationId = contract.conversationId,
            message = body,
            sendId = sendId,
            classification = DataClassification.PUBLIC,
            egressContract = contract,
            selectedModelId = DemoChatComposition.DEMO_MODEL_ID,
            bodySourceLineage = lineage,
            localDemoGoogleHandoffAuthorization = authorization,
        )

        assertEquals(EgressContractDecision.APPROVED, ChatEgressGate.evaluate(contract, now).decision)
        assertThrows(ModelChatFailure::class.java) { runBlocking { egress.send(request) } }
        assertNull("Expired/revoked handoff is rejected before reservation", dao.reservation(sendId))
        assertEquals("Expired/revoked handoff never reaches the local provider", 0, provider.calls)
    }

    @Test fun routedFakePassesEgressAndUsageAndDeniedRequestsStopBeforeReservation() = runBlocking {
        val now = 50_000L
        val nowInstant = Instant.ofEpochMilli(now)
        val dao = MemoryUsageDao()
        val fake = LocalFakeModelChatPort()
        val routed = DemoChatComposition.create(
            usage = UsageRepository(dao) { now },
            demoPort = fake,
            clock = { nowInstant },
        )

        val sendId = "fixture-local-approved"
        val body = "synthetic local prompt"
        val approved = routed.send(
            ModelChatRequest(
                conversationId = "fixture-conversation",
                message = body,
                sendId = sendId,
                classification = DataClassification.PUBLIC,
                bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
            ),
        )
        assertEquals("local fixture response", approved.text)
        assertEquals(DemoChatComposition.DEMO_MODEL_ID, fake.lastRequest?.selectedModelId)
        assertEquals(EgressTarget.LOCAL_DEMO, fake.lastRequest?.egressContract?.target)
        assertEquals("serea://local-demo", fake.lastRequest?.egressContract?.destination)
        assertEquals("serea-demo/local-chat", fake.lastRequest?.egressContract?.selectedModelId)
        assertEquals(setOf(DataOrigin.USER_AUTHORED), fake.lastRequest?.bodySourceLineage)
        assertEquals(fake.lastRequest?.bodySourceLineage, fake.lastRequest?.egressContract?.plannedBodySourceLineage)
        assertEquals(1, fake.calls)
        assertEquals(UsageLedgerState.SETTLED.name, dao.reservation(sendId)?.state)
        assertEquals(DemoChatComposition.DEMO_MODEL_ID, dao.ledger(sendId)?.modelId)
        assertEquals(DemoChatComposition.DEMO_PROVIDER_ID, dao.ledger(sendId)?.providerId)
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
                        bodySourceLineage = setOf(DataOrigin.GMAIL),
                        egressContract = contract(
                            "fixture-conversation", googleOriginSendId, "synthetic mail excerpt",
                            DataOrigin.GMAIL, DataClassification.PUBLIC,
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
                        bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
                        egressContract = contract(
                            "fixture-conversation", sensitiveSendId, "synthetic secret fixture",
                            DataOrigin.USER_AUTHORED, DataClassification.SECRET,
                        ),
                    ),
                )
            }
        }
        assertNull("Sensitive data is rejected before Usage reservation", dao.reservation(sensitiveSendId))

        val cloudTargetSendId = "fixture-cloud-target-denied"
        assertThrows(ModelChatFailure::class.java) {
            runBlocking {
                routed.send(
                    ModelChatRequest(
                        conversationId = "fixture-conversation",
                        message = "synthetic public fixture",
                        sendId = cloudTargetSendId,
                        classification = DataClassification.PUBLIC,
                        bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
                        egressContract = contract(
                            "fixture-conversation", cloudTargetSendId, "synthetic public fixture",
                            DataOrigin.USER_AUTHORED, DataClassification.PUBLIC,
                            target = EgressTarget.CLOUD, cloudIntentAtMillis = now,
                        ),
                    ),
                )
            }
        }
        assertNull("cloud target is rejected before Usage reservation", dao.reservation(cloudTargetSendId))
        assertEquals("denied requests never reach a provider delegate", 1, fake.calls)
    }

    private fun contract(
        conversationId: String,
        sendId: String,
        body: String,
        origin: DataOrigin,
        classification: DataClassification,
        target: EgressTarget = EgressTarget.LOCAL_DEMO,
        cloudIntentAtMillis: Long? = null,
    ) = EgressContract(
        conversationId = conversationId,
        currentMessageId = sendId,
        origin = origin,
        classification = classification,
        destination = if (target == EgressTarget.LOCAL_DEMO) "serea://local-demo" else "https://ollama.com/api/chat",
        plannedBody = body,
        context = emptyList(),
        preparation = PayloadPreparation.RAW,
        explicitSendIntentAtMillis = cloudIntentAtMillis,
        expiresAtMillis = cloudIntentAtMillis?.plus(30_000),
        selectedModelId = DemoChatComposition.DEMO_MODEL_ID,
        target = target,
        plannedBodySourceLineage = setOf(origin),
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
        override suspend fun insertReservation(row: UsageReservationRow) {
            check(row.requestId !in reservations)
            reservations[row.requestId] = row
        }
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
        override suspend fun insertLedger(row: ModelUsageLedgerRow) {
            check(row.requestId !in ledgers)
            ledgers[row.requestId] = row
        }
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
