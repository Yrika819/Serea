package app.serea.android

import androidx.arch.core.executor.testing.InstantTaskExecutorRule
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.advanceTimeBy
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Rule
import org.junit.Test

@OptIn(ExperimentalCoroutinesApi::class)
class ChatViewModelTest {
    @get:Rule val instantTaskExecutorRule = InstantTaskExecutorRule()
    private val dispatcher = StandardTestDispatcher()

    @Before fun setUp() { Dispatchers.setMain(dispatcher) }
    @After fun tearDown() { Dispatchers.resetMain() }

    @Test fun sendsUserMessageAndDemoReply() = runTest(dispatcher) {
        val vm = ChatViewModel(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest) = ModelChatResponse("Demo: ${request.message}")
        })
        vm.updateInput(" hello ")
        vm.sendMessage()
        assertTrue(vm.state.value.isLoading)
        advanceUntilIdle()
        assertEquals(listOf(MessageRole.USER, MessageRole.ASSISTANT), vm.state.value.conversation.messages.map { it.role })
        assertEquals("Demo: hello", vm.state.value.conversation.messages.last().text)
        assertFalse(vm.state.value.isLoading)
    }

    @Test fun mockConversationUsesPreviousTurnsInTheNextRequestContext() = runTest(dispatcher) {
        val requests = mutableListOf<ModelChatRequest>()
        val vm = ChatViewModel(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                requests += request
                return MockModelChatPort(0).send(request)
            }
        })
        vm.updateInput("first turn")
        vm.sendMessage()
        advanceUntilIdle()
        vm.updateInput("second turn")
        vm.sendMessage()
        advanceUntilIdle()

        assertEquals(2, requests.size)
        assertTrue(requests[0].context.isEmpty())
        assertEquals(listOf(MessageRole.USER, MessageRole.ASSISTANT), requests[1].context.map { it.role })
        assertEquals("first turn", requests[1].context.first().text)
        assertTrue(vm.state.value.conversation.messages.last().text.contains("previous message: “first turn”"))
    }

    @Test fun contextIsBoundedAndKeepsSourceClassification() {
        val input = ChatMessage("user-1", MessageRole.USER, "a".repeat(25_000), java.time.Instant.EPOCH,
            DataClassification.PRIVATE, "COMPLETED")
        val pending = input.copy(id = "pending", text = "must not become context", status = "PENDING")
        val context = ChatContextBuilder.build(listOf(input, pending))
        assertEquals(1, context.size)
        assertTrue(context.single().text.length <= ChatContextBuilder.MAX_CONTEXT_TOKENS * 4)
        assertEquals(DataClassification.PRIVATE, context.single().classification)
        assertEquals("user-1", context.single().sourceMessageId)
    }

    @Test fun contextKeepsOriginAndExcludesOtherConversationMessages() {
        val current = ChatMessage("current", MessageRole.USER, "current text", java.time.Instant.EPOCH,
            DataClassification.PERSONAL, "COMPLETED", DataOrigin.CALENDAR, "conversation-current")
        val other = current.copy(id = "other", text = "other conversation", conversationId = "conversation-other")
        val context = ChatContextBuilder.build(listOf(other, current), expectedConversationId = "conversation-current")
        assertEquals(1, context.size)
        assertEquals("current", context.single().sourceMessageId)
        assertEquals(DataOrigin.CALENDAR, context.single().origin)
        assertEquals("conversation-current", context.single().conversationId)
    }

    @Test fun conversationTitlesNormalizeWhitespaceAndKeepJapaneseEmojiAndWholeCodePoints() {
        assertEquals("日本語の相談 😀", conversationTitle("  日本語の相談\n😀  "))
        assertEquals("新しい会話", conversationTitle(" \n\t "))
        val longTitle = conversationTitle("😀".repeat(60))
        assertEquals(48, longTitle.codePointCount(0, longTitle.length))
        assertTrue(longTitle.endsWith("…"))
        assertEquals(48, conversationTitle("😀".repeat(48)).codePointCount(0, conversationTitle("😀".repeat(48)).length))
        assertEquals("first second", conversationTitle("first\u0000\nsecond"))
    }

    @Test fun conversationTitlesAreDerivedPerConversationAndLegacyTitlesRemainReadable() {
        val first = Conversation("conversation-a", listOf(ChatMessage(
            "m1", MessageRole.USER, "Planning the garden", java.time.Instant.EPOCH,
        )))
        val second = Conversation("conversation-b", listOf(ChatMessage(
            "m2", MessageRole.USER, "Reviewing the budget", java.time.Instant.EPOCH,
        )))
        assertEquals("Planning the garden", conversationDisplayTitle(first))
        assertEquals("Reviewing the budget", conversationDisplayTitle(second))
        assertEquals("Conversation 1234abcd", storedConversationDisplayTitle(
            StoredConversation("legacy", "Conversation 1234abcd", 1, 1, "demo-local", emptyList()),
        ))
    }

    @Test fun softLimitValidationRejectsEmptyZeroAndNegativeValues() {
        assertTrue(runCatching { validateUsageLimits("", "2", "10", "20") }.isFailure)
        assertTrue(runCatching { validateUsageLimits("0", "2", "10", "20") }.isFailure)
        assertTrue(runCatching { validateUsageLimits("-1", "2", "10", "20") }.isFailure)
    }

    @Test fun softLimitValidationRejectsNonNumericAndLongOverflow() {
        assertTrue(runCatching { validateUsageLimits("abc", "2", "10", "20") }.isFailure)
        assertTrue(runCatching { validateUsageLimits("9223372036854775808", "2", "10", "20") }.isFailure)
    }

    @Test fun softLimitValidationRejectsDailyRequestsAboveMonthlyRequests() {
        assertTrue(runCatching { validateUsageLimits("11", "2", "10", "20") }.isFailure)
    }

    @Test fun softLimitValidationRejectsDailyTokensAboveMonthlyTokens() {
        assertTrue(runCatching { validateUsageLimits("1", "21", "10", "20") }.isFailure)
    }

    @Test fun softLimitValidationAcceptsCorrectedPositiveLongValues() {
        assertEquals(UsageLimits(10, 20, 10, 20), validateUsageLimits("10", "20", "10", "20"))
    }

    @Test fun staleConversationIdCannotDeleteCurrentConversation() = runTest(dispatcher) {
        val selected = Conversation(id = "selected-conversation")
        val vm = ChatViewModel(MockModelChatPort(0), initialConversation = selected)
        vm.deleteCurrentConversation("different-conversation")
        advanceUntilIdle()
        assertEquals(selected.id, vm.state.value.conversation.id)
        assertTrue(vm.state.value.errorMessage!!.contains("会話が切り替わった"))
        assertFalse(vm.state.value.isRestoring)
    }

    @Test fun oversizedInputIsRejectedWithoutDispatch() = runTest(dispatcher) {
        var calls = 0
        val vm = ChatViewModel(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                calls += 1
                return ModelChatResponse("unexpected")
            }
        })
        vm.updateInput("x".repeat(ChatContextBuilder.MAX_INPUT_CHARS + 1))
        vm.sendMessage()
        advanceUntilIdle()
        assertEquals(0, calls)
        assertTrue(vm.state.value.errorMessage!!.contains("local limit"))
        assertTrue(vm.state.value.conversation.messages.isEmpty())
    }

    @Test fun modelFailureIsShownAndLoadingEnds() = runTest(dispatcher) {
        val vm = ChatViewModel(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse = throw ModelChatFailure("offline")
        })
        vm.updateInput("hello")
        vm.sendMessage()
        advanceUntilIdle()
        assertFalse(vm.state.value.isLoading)
        assertTrue(vm.state.value.errorMessage!!.contains("offline"))
    }

    @Test fun timeoutEndsLoadingWithoutAutomaticRetry() = runTest(dispatcher) {
        var calls = 0
        val vm = ChatViewModel(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                calls += 1
                kotlinx.coroutines.delay(Long.MAX_VALUE)
                return ModelChatResponse("unreachable")
            }
        })
        vm.updateInput("hello")
        vm.sendMessage()
        advanceTimeBy(15_001)
        advanceUntilIdle()
        assertEquals(1, calls)
        assertFalse(vm.state.value.isLoading)
        assertTrue(vm.state.value.errorMessage!!.contains("timed out"))
        assertEquals("OUTCOME_UNKNOWN", vm.state.value.conversation.messages.last().status)
    }

    @Test fun explicitCancelStopsWaitingAndNeverRetries() = runTest(dispatcher) {
        var calls = 0
        val vm = ChatViewModel(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse {
                calls += 1
                kotlinx.coroutines.delay(Long.MAX_VALUE)
                return ModelChatResponse("late")
            }
        })
        vm.updateInput("hello")
        vm.sendMessage()
        runCurrent()
        vm.cancelCurrent()
        advanceUntilIdle()
        assertEquals(1, calls)
        assertFalse(vm.state.value.isLoading)
        assertEquals("OUTCOME_UNKNOWN", vm.state.value.conversation.messages.last().status)
        assertTrue(vm.state.value.errorMessage!!.contains("may still have completed remotely"))
    }

    @Test fun newConversationCannotReceiveLateReply() = runTest(dispatcher) {
        val reply = kotlinx.coroutines.CompletableDeferred<ModelChatResponse>()
        val vm = ChatViewModel(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse =
                kotlinx.coroutines.withContext(kotlinx.coroutines.NonCancellable) { reply.await() }
        })
        vm.updateInput("old conversation")
        vm.sendMessage()
        runCurrent()
        vm.newConversation()
        val newId = vm.state.value.conversation.id
        reply.complete(ModelChatResponse("late response"))
        advanceUntilIdle()
        assertEquals(newId, vm.state.value.conversation.id)
        assertTrue(vm.state.value.conversation.messages.none { it.text == "late response" })
    }
}
