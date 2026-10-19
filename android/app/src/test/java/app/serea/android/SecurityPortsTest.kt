package app.serea.android

import org.junit.Assert.assertEquals
import org.junit.Test
import kotlinx.coroutines.runBlocking

class SecurityPortsTest {
    @Test fun modelRoutingCannotChangeClassificationOrBypassEgressGate() = runBlocking {
        var providerCalls = 0
        val descriptor = ModelDescriptor("fixture/public-only", "fixture", "Fixture",
            Knowledge.Known(4096), Knowledge.Known(512), ModelCapabilities(setOf("text/plain")),
            ModelPricing(Knowledge.UNKNOWN, Knowledge.UNKNOWN), ModelAvailability.AVAILABLE, authorized = true, lastVerifiedAt = "2026-10-19T00:00:00Z", source = "unit-test fixture")
        val now = 50_000L
        val provider = EgressCheckedModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                providerCalls += 1
                return ModelChatResponse("unexpected")
            }
        }, { now })
        val routed = RoutedModelChatPort(provider, AdaptiveModelRouter(LocalModelCatalog(listOf(descriptor)))) {
            ModelRoutingPolicy(ModelSelectionMode.MANUAL, manualModelId = descriptor.id)
        }
        try {
            val context = emptyList<ContextMessage>()
            val sendId = "route-test"
            routed.send(ModelChatRequest("conversation", "classified sensitive", sendId = sendId,
                classification = DataClassification.SECRET, egressContract = EgressContract("conversation", sendId,
                    DataOrigin.USER_AUTHORED, DataClassification.SECRET, "https://ollama.com/api/chat",
                    "classified sensitive", context, PayloadPreparation.RAW, now, now + 30_000)))
            throw AssertionError("routing cannot authorize SECRET data")
        } catch (_: ModelChatFailure) { }
        assertEquals(0, providerCalls)
    }

    @Test fun publicMayPassAndUnpreparedPersonalFailsClosed() {
        assertEquals(EgressDecision.ALLOW, ChatEgressGate.decide(DataClassification.PUBLIC))
        assertEquals(EgressDecision.DENY_UNPREPARED, ChatEgressGate.decide(DataClassification.PERSONAL))
    }

    @Test fun preparationMarkerCannotAuthorizePersonalData() {
        assertEquals(EgressDecision.DENY_UNPREPARED, ChatEgressGate.decide(DataClassification.PERSONAL, PreparedPersonalData("redacted")))
    }

    @Test fun privateSecretCredentialAndUnknownNeverPass() {
        listOf(DataClassification.PRIVATE, DataClassification.SECRET, DataClassification.CREDENTIAL).forEach {
            assertEquals(EgressDecision.DENY_SENSITIVE, ChatEgressGate.decide(it, PreparedPersonalData("cannot lower class")))
        }
        assertEquals(EgressDecision.DENY_UNCLASSIFIED, ChatEgressGate.decide(DataClassification.UNKNOWN))
    }

    @Test fun rejectedCloudRequestNeverReachesProvider() = runBlocking {
        var calls = 0
        val gated = EgressCheckedModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                calls += 1
                return ModelChatResponse("provider response")
            }
        })
        listOf(DataClassification.UNKNOWN, DataClassification.PRIVATE, DataClassification.SECRET,
            DataClassification.CREDENTIAL, DataClassification.PERSONAL).forEach { classification ->
            try {
                gated.send(ModelChatRequest("conversation", "sensitive", classification = classification))
                throw AssertionError("request should have been rejected")
            } catch (_: ModelChatFailure) { }
        }
        assertEquals(0, calls)
    }

    @Test fun preparationMarkerNeverReachesProvider() = runBlocking {
        var calls = 0
        val gated = EgressCheckedModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                calls += 1
                return ModelChatResponse("ok")
            }
        })
        try {
            gated.send(ModelChatRequest("conversation", "raw private text", classification = DataClassification.PERSONAL,
                preparedPersonal = PreparedPersonalData("minimized and redacted")))
            throw AssertionError("Personal data must be denied until a trusted preparation policy exists")
        } catch (_: ModelChatFailure) { }
        assertEquals(0, calls)
    }

    @Test fun sensitiveClassificationInPriorHistoryBlocksWholeRequest() = runBlocking {
        var calls = 0
        val gated = EgressCheckedModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                calls += 1
                return ModelChatResponse("unexpected")
            }
        })
        val prior = ContextMessage(MessageRole.USER, "private historical content", DataClassification.PRIVATE, "message-1")
        try {
            gated.send(ModelChatRequest("conversation", "a public probe", classification = DataClassification.PUBLIC,
                context = listOf(prior)))
            throw AssertionError("private history must not reach the provider")
        } catch (_: ModelChatFailure) { }
        assertEquals(0, calls)
    }

    @Test fun overLimitContextIsRejectedBeforeDelegate() = runBlocking {
        var calls = 0
        val gated = EgressCheckedModelChatPort(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                calls += 1
                return ModelChatResponse("unexpected")
            }
        })
        try {
            gated.send(ModelChatRequest("conversation", "prompt", classification = DataClassification.PUBLIC,
                context = listOf(ContextMessage(MessageRole.USER, "x".repeat(ChatContextBuilder.MAX_CONTEXT_CHARS + 1), DataClassification.PUBLIC, "large"))))
            throw AssertionError("oversized context must not reach the provider")
        } catch (_: ModelChatFailure) { }
        assertEquals(0, calls)
    }

    @Test fun typedEgressContractRequiresPublicClassificationOriginIntentAndFreshness() {
        val now = 50_000L
        fun contract(
            origin: DataOrigin = DataOrigin.USER_AUTHORED,
            classification: DataClassification = DataClassification.PUBLIC,
            intent: Long? = now,
            expiry: Long? = now + 30_000,
            context: List<ContextMessage> = emptyList(),
        ) = EgressContract("c", "m", origin, classification, "https://ollama.com/api/chat", "planned text",
            context, PayloadPreparation.RAW, intent, expiry)

        assertEquals(EgressContractDecision.APPROVED, ChatEgressGate.evaluate(contract(), now).decision)
        assertEquals(EgressContractDecision.CLASSIFICATION_DENIED,
            ChatEgressGate.evaluate(contract(classification = DataClassification.UNKNOWN), now).decision)
        assertEquals(EgressContractDecision.ORIGIN_DENIED,
            ChatEgressGate.evaluate(contract(origin = DataOrigin.CALENDAR), now).decision)
        assertEquals(EgressContractDecision.MISSING_INTENT,
            ChatEgressGate.evaluate(contract(intent = null), now).decision)
        assertEquals(EgressContractDecision.EXPIRED_INTENT,
            ChatEgressGate.evaluate(contract(intent = now - 60_001, expiry = now - 1), now).decision)
        assertEquals(EgressContractDecision.CLASSIFICATION_DENIED, ChatEgressGate.evaluate(contract(context = listOf(
            ContextMessage(MessageRole.USER, "private", DataClassification.PRIVATE, "source", DataOrigin.USER_AUTHORED),
        )), now).decision)
        assertEquals(EgressContractDecision.CONTENT_MISMATCH, ChatEgressGate.evaluate(contract(context = listOf(
            ContextMessage(MessageRole.USER, "partial", DataClassification.PUBLIC, "source", DataOrigin.USER_AUTHORED, "PENDING"),
        )), now).decision)
    }
}
