package app.serea.android

import java.time.Instant
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

class ChatContextBuilderTest {
    private val now = Instant.parse("2026-11-01T00:00:00Z")

    private fun message(
        id: String,
        text: String,
        conversation: String? = "conversation-a",
        role: MessageRole = MessageRole.USER,
        status: String = "COMPLETED",
        classification: DataClassification = DataClassification.PUBLIC,
        origin: DataOrigin = DataOrigin.USER_AUTHORED,
        lineage: Set<DataOrigin> = setOf(origin),
    ) = ChatMessage(id, role, text, now, classification, status, origin, conversation, lineage)

    private fun model(context: Knowledge<Int>) = ModelDescriptor(
        id = "synthetic/model-v1",
        providerId = "synthetic",
        displayName = "Synthetic fixture",
        contextWindow = context,
        maxOutput = Knowledge.Known(1_000),
        capabilities = ModelCapabilities(input = setOf("text/plain")),
        availability = ModelAvailability.AVAILABLE,
        authorized = true,
        lastVerifiedAt = now.toString(),
        source = "synthetic C-04 test fixture",
        verificationStatus = ModelVerificationStatus.TEST_FIXTURE,
    )

    @Test fun knownModelBudgetReservesOutputAndBodyThenKeepsNewestHistoryFirst() {
        val oldLineage = setOf(DataOrigin.USER_AUTHORED, DataOrigin.CALENDAR)
        val old = message("old", "o".repeat(100), lineage = oldLineage)
        val recent = message("recent", "r".repeat(100), role = MessageRole.ASSISTANT,
            classification = DataClassification.PERSONAL, origin = DataOrigin.MODEL_OUTPUT,
            lineage = setOf(DataOrigin.MODEL_OUTPUT, DataOrigin.USER_AUTHORED))

        val built = ChatContextBuilder.buildForModel(
            history = listOf(old, recent), currentInput = "x".repeat(20),
            expectedConversationId = "conversation-a", model = model(Knowledge.Known(1_000)),
            requestedOutputTokens = 400, maxInputTokens = 800,
        )

        assertEquals(130, built.sumOf { it.text.length })
        assertEquals(listOf("old", "recent"), built.map { it.sourceMessageId })
        assertEquals(30, built.first().text.length)
        assertEquals(100, built.last().text.length)
        assertEquals(MessageRole.ASSISTANT, built.last().role)
        assertEquals(DataClassification.PERSONAL, built.last().classification)
        assertEquals(DataOrigin.MODEL_OUTPUT, built.last().origin)
        assertEquals(setOf(DataOrigin.MODEL_OUTPUT, DataOrigin.USER_AUTHORED), built.last().sourceLineage)
        assertEquals("conversation-a", built.last().conversationId)
    }

    @Test fun legacyAndUnknownModelLimitsKeepTheFixedContextBudget() {
        val history = listOf(message("legacy", "c".repeat(ChatContextBuilder.MAX_CONTEXT_CHARS + 5)))
        val legacy = ChatContextBuilder.build(history, currentInput = "i".repeat(5), expectedConversationId = "conversation-a")
        val unknown = ChatContextBuilder.buildForModel(
            history, "i".repeat(5), "conversation-a", model(Knowledge.UNKNOWN), requestedOutputTokens = 512,
        )

        assertEquals(ChatContextBuilder.MAX_CONTEXT_CHARS - 5, legacy.single().text.length)
        assertEquals(legacy, unknown)
    }

    @Test fun incompleteFailedAndOtherConversationHistoryAreExcluded() {
        val selected = ChatContextBuilder.build(
            listOf(
                message("other", "other", conversation = "conversation-b"),
                message("pending", "pending", status = "PENDING"),
                message("failed", "failed", status = "FAILED"),
                message("complete", "current"),
            ),
            expectedConversationId = "conversation-a",
        )

        assertEquals(listOf("complete"), selected.map { it.sourceMessageId })
        assertEquals("current", selected.single().text)
    }

    @Test fun codePointSafeTruncationRetainsValidUnicodeAndMessageMetadata() {
        val source = message(
            "unicode", "a😀bc", role = MessageRole.ASSISTANT,
            classification = DataClassification.PRIVATE, origin = DataOrigin.MODEL_OUTPUT,
            lineage = setOf(DataOrigin.MODEL_OUTPUT, DataOrigin.GMAIL),
        ).let { original ->
            ContextMessage(original.role, original.text, original.classification, original.id, original.origin,
                original.status, original.conversationId, original.sourceLineage)
        }

        val clipped = ChatContextBuilder.fitForModel(
            listOf(source), currentInput = "", expectedConversationId = "conversation-a",
            modelContextWindowTokens = 20, requestedOutputTokens = 9,
        )

        assertEquals("a", clipped.single().text)
        assertFalse(clipped.single().text.any { Character.isSurrogate(it) })
        assertEquals(source.role, clipped.single().role)
        assertEquals(source.classification, clipped.single().classification)
        assertEquals(source.sourceMessageId, clipped.single().sourceMessageId)
        assertEquals(source.origin, clipped.single().origin)
        assertEquals(source.sourceLineage, clipped.single().sourceLineage)
        assertEquals(source.conversationId, clipped.single().conversationId)
        assertTrue(clipped.single().text.isNotEmpty())

        val noRoomForPair = ChatContextBuilder.fitForModel(
            listOf(source.copy(text = "😀")), "", "conversation-a", 4, requestedOutputTokens = 3,
        )
        assertTrue(noRoomForPair.isEmpty())
    }

    @Test fun routedRequestFitsModelBudgetBeforeBuildingEgressContract() = kotlinx.coroutines.runBlocking {
        val catalog = FakeModelCatalog(listOf(model(Knowledge.Known(1_000))), clock = { now })
        val received = kotlinx.coroutines.CompletableDeferred<ModelChatRequest>()
        val delegate = object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                received.complete(request)
                return ModelChatResponse("synthetic reply")
            }
        }
        val routed = RoutedModelChatPort(
            delegate = delegate,
            router = AdaptiveModelRouter(catalog),
            contractFactory = { request, selected ->
                assertEquals(listOf("same-conversation"), request.context.map { it.sourceMessageId })
                assertEquals(selected.id, request.selectedModelId)
                EgressContract(
                    conversationId = request.conversationId,
                    currentMessageId = request.sendId,
                    origin = DataOrigin.USER_AUTHORED,
                    classification = DataClassification.PUBLIC,
                    destination = ChatEgressGate.LOCAL_DEMO_DESTINATION,
                    plannedBody = request.message,
                    context = request.context,
                    preparation = PayloadPreparation.RAW,
                    explicitSendIntentAtMillis = null,
                    expiresAtMillis = null,
                    selectedModelId = selected.id,
                    target = EgressTarget.LOCAL_DEMO,
                    plannedBodySourceLineage = request.bodySourceLineage,
                )
            },
            policy = { ModelRoutingPolicy(
                mode = ModelSelectionMode.MANUAL,
                manualModelId = "synthetic/model-v1",
                allowedProviderIds = setOf("synthetic"),
                requestedOutputTokens = 200,
            ) },
        )
        val request = ModelChatRequest(
            conversationId = "conversation-a", message = "hello", sendId = "send-c04",
            context = listOf(
                ContextMessage(MessageRole.USER, "x".repeat(800), DataClassification.PUBLIC, "same-conversation",
                    DataOrigin.USER_AUTHORED, conversationId = "conversation-a"),
                ContextMessage(MessageRole.USER, "wrong", DataClassification.PUBLIC, "other-conversation",
                    DataOrigin.USER_AUTHORED, conversationId = "conversation-b"),
                ContextMessage(MessageRole.ASSISTANT, "pending", DataClassification.PUBLIC, "incomplete",
                    DataOrigin.MODEL_OUTPUT, status = "FAILED", conversationId = "conversation-a"),
            ),
            bodySourceLineage = setOf(DataOrigin.USER_AUTHORED),
        )

        assertEquals("synthetic reply", routed.send(request).text)
        val adapted = received.await()
        assertEquals("synthetic/model-v1", adapted.selectedModelId)
        assertEquals(195, adapted.context.single().text.length)
        assertEquals("conversation-a", adapted.egressContract?.conversationId)
        assertEquals(adapted.context, adapted.egressContract?.context)
        assertNull(request.selectedModelId)
    }
}
