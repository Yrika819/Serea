package app.serea.android

import androidx.arch.core.executor.testing.InstantTaskExecutorRule
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.advanceTimeBy
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

    @Test fun modelFailureIsShownAndLoadingEnds() = runTest(dispatcher) {
        val vm = ChatViewModel(object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse = throw ModelChatFailure("offline")
        })
        vm.updateInput("hello")
        vm.sendMessage()
        advanceUntilIdle()
        assertFalse(vm.state.value.isLoading)
        assertTrue(vm.state.value.errorMessage!!.contains("failed"))
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
    }
}
