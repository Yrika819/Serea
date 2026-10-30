package app.serea.android

import kotlinx.coroutines.runBlocking
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class LocalModelChatFeatureRouterTest {
    private val localId = "serea-local/test-model"
    private val demoId = DemoChatComposition.DEMO_MODEL_ID

    @Test fun productBuildFlagIsDefaultOff() {
        assertFalse(BuildConfig.LOCAL_MODEL_CHAT_ENABLED)
    }

    @Test fun defaultRouterKeepsDemoRouteAndDoesNotTouchLocalDelegate() = runBlocking {
        val demo = RecordingPort("demo")
        val local = RecordingPort("local")
        val router = router(demo, local)
        assertEquals("demo", router.send(request(demoId)).text)
        assertEquals(1, demo.calls)
        assertEquals(0, local.calls)
    }

    @Test fun localRouteDeniedByDefaultWithoutCallingAnyDelegate() = runBlocking {
        val demo = RecordingPort("demo")
        val local = RecordingPort("local")
        val router = router(demo, local)
        val failure = runCatching { router.send(localRequest(withConsent = true)) }.exceptionOrNull()
        assertTrue(failure is PreDispatchModelChatFailure)
        assertEquals(0, demo.calls)
        assertEquals(0, local.calls)
    }

    @Test fun explicitlyEnabledTestRouteRequiresFreshLocalConsentContract() = runBlocking {
        val demo = RecordingPort("demo")
        val local = RecordingPort("local")
        val router = router(demo, local, enabled = true)
        val failure = runCatching { router.send(localRequest(withConsent = false)) }.exceptionOrNull()
        assertTrue(failure is PreDispatchModelChatFailure)
        assertEquals(0, local.calls)
        assertEquals(0, demo.calls)
    }

    @Test fun explicitLocalRouteDispatchesOnlyToLocalAndNeverFallsBackToDemo() = runBlocking {
        val demo = RecordingPort("demo")
        val local = RecordingPort("local")
        val router = router(demo, local, enabled = true)
        assertEquals("local", router.send(localRequest(withConsent = true)).text)
        assertEquals(1, local.calls)
        assertEquals(0, demo.calls)
    }

    @Test fun unknownAndCloudSelectionsFailClosedWithoutFallback() = runBlocking {
        val demo = RecordingPort("demo")
        val local = RecordingPort("local")
        val router = router(demo, local, enabled = true)
        val unknown = runCatching { router.send(request("https://provider.invalid/model")) }.exceptionOrNull()
        val cloud = runCatching {
            router.send(localRequest(withConsent = true).let { it.copy(egressContract = it.egressContract!!.copy(target = EgressTarget.CLOUD)) })
        }.exceptionOrNull()
        assertTrue(unknown is PreDispatchModelChatFailure)
        assertTrue(cloud is PreDispatchModelChatFailure)
        assertEquals(0, local.calls)
        assertEquals(0, demo.calls)
    }

    @Test fun oneUseConsentReplayIsDeniedBeforeUsageAndNeverFallsBack() = runBlocking {
        val demo = RecordingPort("demo")
        val localBackend = RecordingPort("local")
        val raw = request(localId)
        val registry = LocalModelAuthorizationRegistry(
            clock = { 1L },
            currentConversation = LocalModelCurrentConversationPort {
                LocalModelConversationBinding(raw.conversationId, 0L)
            },
        )
        val proof = registry.approve(raw, localId, conversationGeneration = 0L)
        val approved = raw.copy(egressContract = localContract(raw, proof))
        val dao = MemoryUsageDao()
        val usage = UsageControlledModelChatPort(
            delegate = localBackend,
            usage = UsageRepository(dao) { 1_800_000_000_000L },
            outputReservationTokens = 256,
            attributionResolver = UsageAttributionResolver {
                ModelUsageAttribution(modelId = localId, providerId = "local-device", price = null)
            },
        )
        val egress = EgressCheckedModelChatPort(
            delegate = usage,
            clock = { 1_800_000_000_000L },
            expectedTarget = EgressTarget.LOCAL_MODEL,
            localModelAuthorizations = registry,
            localMonotonicClock = { 1L },
        )
        val router = LocalModelChatFeatureRouter(
            demoModelId = demoId,
            localModelId = localId,
            demo = demo,
            local = egress,
            localModelChatEnabled = true,
        )

        assertEquals("local", router.send(approved).text)
        assertEquals(1, dao.insertedReservationCount)
        val replay = runCatching { router.send(approved) }.exceptionOrNull()
        assertTrue(replay is PreDispatchModelChatFailure)
        assertEquals("replayed proof must be denied before a second Usage reservation", 1, dao.insertedReservationCount)
        assertEquals("replayed proof must never reach the local backend", 1, localBackend.calls)
        assertEquals("local failure/denial must never route to Demo", 0, demo.calls)
    }

    @Test fun missingModelSelectionFailsClosed() = runBlocking {
        val demo = RecordingPort("demo")
        val local = RecordingPort("local")
        val failure = runCatching { router(demo, local, enabled = true).send(request(null)) }.exceptionOrNull()
        assertTrue(failure is PreDispatchModelChatFailure)
        assertEquals(0, local.calls)
        assertEquals(0, demo.calls)
    }

    private fun router(demo: RecordingPort, local: RecordingPort, enabled: Boolean = BuildConfig.LOCAL_MODEL_CHAT_ENABLED) =
        LocalModelChatFeatureRouter(demoModelId = demoId, localModelId = localId,
            demo = demo, local = local, localModelChatEnabled = enabled)

    private fun localContract(raw: ModelChatRequest, proof: LocalModelConsentProof) = EgressContract(
        conversationId = raw.conversationId, currentMessageId = raw.sendId,
        origin = DataOrigin.USER_AUTHORED, classification = DataClassification.PUBLIC,
        destination = ChatEgressGate.LOCAL_MODEL_DESTINATION, plannedBody = raw.message,
        context = raw.context, preparation = PayloadPreparation.RAW,
        explicitSendIntentAtMillis = null, expiresAtMillis = null,
        selectedModelId = localId, target = EgressTarget.LOCAL_MODEL,
        plannedBodySourceLineage = raw.bodySourceLineage, localModelConsent = proof,
    )

    private fun request(selected: String?): ModelChatRequest = ModelChatRequest(
        conversationId = "conversation-1", message = "hello", sendId = "send-1",
        classification = DataClassification.PUBLIC, selectedModelId = selected,
        bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
    )

    private fun localRequest(withConsent: Boolean): ModelChatRequest {
        val raw = request(localId)
        val proof = if (withConsent) LocalModelAuthorizationRegistry({ 1L }).approve(raw, localId) else null
        val contract = EgressContract(
            conversationId = raw.conversationId, currentMessageId = raw.sendId,
            origin = DataOrigin.USER_AUTHORED, classification = DataClassification.PUBLIC,
            destination = ChatEgressGate.LOCAL_MODEL_DESTINATION, plannedBody = raw.message,
            context = raw.context, preparation = PayloadPreparation.RAW,
            explicitSendIntentAtMillis = null, expiresAtMillis = null,
            selectedModelId = localId, target = EgressTarget.LOCAL_MODEL,
            plannedBodySourceLineage = raw.bodySourceLineage,
            localModelConsent = proof,
        )
        return raw.copy(egressContract = contract)
    }

    private class MemoryUsageDao : UsageDao {
        private val windows = mutableMapOf<String, UsageWindowRow>()
        private val reservations = mutableMapOf<String, UsageReservationRow>()
        private var savedSettings: UsageSettingsRow? = null
        private val ledgers = mutableMapOf<String, ModelUsageLedgerRow>()
        var insertedReservationCount = 0
            private set

        override suspend fun window(key: String) = windows[key]
        override suspend fun putWindow(row: UsageWindowRow) { windows[row.windowKey] = row }
        override suspend fun reservation(id: String) = reservations[id]
        override suspend fun insertReservation(row: UsageReservationRow) {
            check(row.requestId !in reservations)
            insertedReservationCount++
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
        override suspend fun insertLedger(row: ModelUsageLedgerRow) { check(row.requestId !in ledgers); ledgers[row.requestId] = row }
        override suspend fun putLedger(row: ModelUsageLedgerRow) { ledgers[row.requestId] = row }
        override suspend fun allLedgerRows() = ledgers.values.toList()
        override suspend fun markRequestUnknown(id: String) = markUnknown(id)
        override suspend fun totals(key: String) = UsageWindowTotals(0, 0, 0, 0)
        override suspend fun unresolvedCount() = reservations.values.count {
            it.state in setOf(UsageLedgerState.IN_FLIGHT.name, UsageLedgerState.OUTCOME_UNKNOWN.name, UsageLedgerState.USAGE_UNKNOWN.name)
        }.toLong()
    }

    private class RecordingPort(private val answer: String) : ModelChatPort {
        var calls = 0
        override suspend fun send(request: ModelChatRequest): ModelChatResponse {
            calls++
            return ModelChatResponse(answer)
        }
    }
}
