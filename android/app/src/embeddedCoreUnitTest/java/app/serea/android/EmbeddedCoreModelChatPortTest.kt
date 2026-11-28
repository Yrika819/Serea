package app.serea.android

import app.serea.android.core.EmbeddedCoreAdapter
import app.serea.android.core.SyntheticCoreNative
import androidx.lifecycle.ViewModelStore
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestScope
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.launch
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test

@OptIn(kotlinx.coroutines.ExperimentalCoroutinesApi::class)
class EmbeddedCoreModelChatPortTest {
    private fun mockPort(block: suspend (ModelChatRequest) -> ModelChatResponse): ModelChatPort =
        object : ModelChatPort {
            override suspend fun send(request: ModelChatRequest): ModelChatResponse = block(request)
        }

    private class FakeCore : SyntheticCoreNative {
        var handle: ULong? = null
        var initializeCount = 0
        var statusCount = 0
        var schemaCount = 0
        var closeCount = 0
        val initialized = CountDownLatch(1)
        val statusRead = CountDownLatch(1)
        val schemaRead = CountDownLatch(1)
        val closed = CountDownLatch(1)
        var failStatus = false
        var failInitialize = false
        override fun initialize(path: String): ULong {
            try {
                check(handle == null)
                initializeCount++
                if (failInitialize) error("synthetic Core initialization failure")
                return 41uL.also { handle = it }
            } finally {
                initialized.countDown()
            }
        }
        override fun status(handle: ULong): Boolean {
            try {
                statusCount++
                if (failStatus) error("synthetic Core status failure")
                return this.handle == handle
            } finally {
                statusRead.countDown()
            }
        }
        override fun schema(handle: ULong): UInt {
            schemaCount++
            check(this.handle == handle)
            schemaRead.countDown()
            return 1u
        }
        override fun close(handle: ULong) {
            check(this.handle == handle)
            closeCount++
            this.handle = null
            closed.countDown()
        }
    }

    private suspend fun TestScope.awaitUntil(description: String, condition: () -> Boolean) {
        val deadline = System.nanoTime() + TimeUnit.SECONDS.toNanos(5)
        while (!condition() && System.nanoTime() < deadline) {
            runCurrent()
            Thread.sleep(2)
        }
        runCurrent()
        assertTrue(description, condition())
    }

    private suspend fun TestScope.awaitLatch(latch: CountDownLatch, description: String) =
        awaitUntil(description) { latch.count == 0L }

    private suspend fun TestScope.closePort(port: EmbeddedCoreModelChatPort) {
        val closing = launch { port.close() }
        awaitUntil("Embedded Core did not finish explicit cleanup") { closing.isCompleted }
        closing.join()
    }

    private suspend fun TestScope.awaitCoreReady(native: FakeCore) {
        runCurrent()
        awaitLatch(native.initialized, "Core initialize not observed")
        runCurrent()
        awaitLatch(native.statusRead, "Core status not observed; init=${native.initializeCount}, handle=${native.handle}")
        runCurrent()
        awaitLatch(native.schemaRead, "Core schema not observed; status=${native.statusCount}, handle=${native.handle}")
        runCurrent()
    }

    /** Ephemeral synthetic-only history fixture; it cannot open the app's durable Room database. */
    private class SyntheticConversationDb : ConversationHistoryPort {
        private val rows = linkedMapOf<String, MutableList<StoredMessage>>()
        override suspend fun list(): List<StoredConversation> = rows.map { (id, messages) ->
            StoredConversation(id, "Synthetic conversation", 1L, 1L, "mock-synthetic", messages.toList())
        }
        override suspend fun get(id: String): StoredConversation? = rows[id]?.let { messages ->
            StoredConversation(id, "Synthetic conversation", 1L, 1L, "mock-synthetic", messages.toList())
        }
        override suspend fun addMessage(message: StoredMessage) {
            rows.getOrPut(message.conversationId) { mutableListOf() }.add(message)
        }
        override suspend fun createWithFirstMessage(
            id: String, title: String, message: StoredMessage, modelId: String,
        ) {
            require(id == message.conversationId && message.role == MessageRole.USER)
            check(id !in rows)
            rows[id] = mutableListOf(message)
        }
        override suspend fun completeTurn(userMessageId: String, assistant: StoredMessage) {
            require(assistant.role == MessageRole.ASSISTANT && assistant.status == "COMPLETED")
            val messages = checkNotNull(rows[assistant.conversationId])
            val index = messages.indexOfFirst { it.id == userMessageId && it.status == "PENDING" }
            check(index >= 0)
            messages[index] = messages[index].copy(status = "COMPLETED")
            messages += assistant
        }
        override suspend fun updateStatus(messageId: String, conversationId: String, status: String): Boolean {
            val messages = rows[conversationId] ?: return false
            val index = messages.indexOfFirst { it.id == messageId }
            if (index < 0) return false
            messages[index] = messages[index].copy(status = status)
            return true
        }
        override suspend fun delete(id: String, now: Long) { rows.remove(id) }
        override suspend fun deleteAll() { rows.clear() }
    }

    @Test fun chatViewModelUsesReadyCoreAndMockAndWritesSyntheticTurn() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        Dispatchers.setMain(dispatcher)
        val native = FakeCore()
        val adapter = EmbeddedCoreAdapter({ "synthetic-core" }, native)
        var mockCalls = 0
        val mockEntered = CompletableDeferred<Unit>()
        val mock = mockPort { request ->
            mockCalls++
            mockEntered.complete(Unit)
            assertEquals("synthetic: hello", request.message)
            MockModelChatPort(responseDelayMillis = 100).send(request)
        }
        val port = EmbeddedCoreModelChatPort(adapter, mock)
        val db = SyntheticConversationDb()
        val chat = ChatViewModel(port, db)
        val viewModelStore = ViewModelStore().apply { put("synthetic-chat", chat) }
        try {
            advanceUntilIdle() // Finish the empty synthetic-history restore before the first turn.
            chat.updateInput("synthetic: hello")
            chat.sendMessage()
            awaitCoreReady(native)
            awaitUntil("Core-ready Chat request must enter Mock AI or surface an error") {
                mockEntered.isCompleted || chat.state.value.errorMessage != null
            }
            assertTrue("Mock AI was not entered; Chat error=${chat.state.value.errorMessage}", mockEntered.isCompleted)
            advanceUntilIdle()

            val saved = db.get(chat.state.value.conversation.id)
            assertNotNull(saved)
            assertEquals(listOf(MessageRole.USER, MessageRole.ASSISTANT), saved!!.messages.map { it.role })
            assertEquals(listOf("COMPLETED", "COMPLETED"), saved.messages.map { it.status })
            assertTrue(chat.state.value.conversation.messages.last().text.contains("No cloud AI was contacted."))
            assertEquals(1, native.initializeCount)
            assertTrue(native.statusCount > 0)
            assertTrue(native.schemaCount > 0)
            assertEquals(1, mockCalls)
        } finally {
            viewModelStore.clear()
            closePort(port)
            Dispatchers.resetMain()
        }
        assertEquals(1, native.closeCount)
    }

    @Test fun cancellationDuringDelayedMockNeverAddsLateAssistantMessage() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        Dispatchers.setMain(dispatcher)
        val native = FakeCore()
        val adapter = EmbeddedCoreAdapter({ "synthetic-core" }, native)
        val mockStarted = CompletableDeferred<Unit>()
        val delayed = CompletableDeferred<ModelChatResponse>()
        val port = EmbeddedCoreModelChatPort(adapter, mockPort {
            mockStarted.complete(Unit)
            delayed.await()
        })
        val db = SyntheticConversationDb()
        val chat = ChatViewModel(port, db)
        val viewModelStore = ViewModelStore().apply { put("synthetic-chat", chat) }
        try {
            advanceUntilIdle()
            chat.updateInput("synthetic: cancel me")
            chat.sendMessage()
            awaitCoreReady(native)
            awaitUntil("Core-ready Chat request must enter delayed Mock AI or surface an error") {
                mockStarted.isCompleted || chat.state.value.errorMessage != null
            }
            assertTrue("delayed Mock AI was not entered; Chat error=${chat.state.value.errorMessage}", mockStarted.isCompleted)
            assertFalse("the controlled synthetic response must still be pending before cancel", delayed.isCompleted)
            chat.cancelCurrent()
            runCurrent()
            val lateResponse = ModelChatResponse("synthetic: late reply")
            assertTrue("the delayed response gate must complete after cancellation", delayed.complete(lateResponse))
            advanceUntilIdle()

            val saved = db.get(chat.state.value.conversation.id)
            assertNotNull(saved)
            assertEquals(listOf(MessageRole.USER), saved!!.messages.map { it.role })
            assertEquals("OUTCOME_UNKNOWN", saved.messages.single().status)
            assertFalse(chat.state.value.conversation.messages.any { it.role == MessageRole.ASSISTANT })
            assertFalse(chat.state.value.conversation.messages.any { it.text == lateResponse.text })
        } finally {
            viewModelStore.clear()
            closePort(port)
            Dispatchers.resetMain()
        }
        assertEquals(1, native.closeCount)
    }

    @Test fun coreHealthFailureStopsMockAndSurfacesChatError() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        Dispatchers.setMain(dispatcher)
        val native = FakeCore().apply { failStatus = true }
        val adapter = EmbeddedCoreAdapter({ "synthetic-core" }, native)
        var mockCalls = 0
        val port = EmbeddedCoreModelChatPort(adapter, mockPort {
            mockCalls++
            ModelChatResponse("unexpected")
        })
        val db = SyntheticConversationDb()
        val chat = ChatViewModel(port, db)
        val viewModelStore = ViewModelStore().apply { put("synthetic-chat", chat) }
        try {
            advanceUntilIdle()
            chat.updateInput("synthetic: core failure")
            chat.sendMessage()
            runCurrent()
            awaitLatch(native.initialized, "failed Core initialization not observed")
            runCurrent()
            awaitLatch(native.statusRead, "failed Core status not observed")
            runCurrent()
            awaitLatch(native.closed, "failed Core cleanup not observed")
            runCurrent()
            advanceUntilIdle()

            assertEquals(0, mockCalls)
            assertTrue(chat.state.value.errorMessage?.contains("Embedded Core is unavailable") == true)
            val saved = db.get(chat.state.value.conversation.id)
            assertEquals("FAILED", saved?.messages?.singleOrNull()?.status)
            assertEquals(1, native.closeCount)
        } finally {
            viewModelStore.clear()
            closePort(port)
            Dispatchers.resetMain()
        }
    }

    @Test fun coreInitializationFailureSkipsMockAndMarksTurnFailed() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        Dispatchers.setMain(dispatcher)
        val native = FakeCore().apply { failInitialize = true }
        val adapter = EmbeddedCoreAdapter({ "synthetic-core" }, native)
        var mockCalls = 0
        val port = EmbeddedCoreModelChatPort(adapter, mockPort {
            mockCalls++
            ModelChatResponse("unexpected")
        })
        val db = SyntheticConversationDb()
        val chat = ChatViewModel(port, db)
        val viewModelStore = ViewModelStore().apply { put("synthetic-chat", chat) }
        try {
            advanceUntilIdle()
            chat.updateInput("synthetic: init failure")
            chat.sendMessage()
            runCurrent()
            awaitLatch(native.initialized, "failed Core initialization not observed")
            runCurrent()
            awaitUntil("Embedded Core initialization failure was not surfaced") {
                chat.state.value.errorMessage?.contains("Embedded Core is unavailable") == true
            }
            assertEquals(0, mockCalls)
            assertTrue(
                "Expected an Embedded Core error, got: ${chat.state.value.errorMessage}",
                chat.state.value.errorMessage?.contains("Embedded Core is unavailable") == true,
            )
            assertEquals("FAILED", db.get(chat.state.value.conversation.id)?.messages?.singleOrNull()?.status)
            assertEquals(0, native.closeCount)
        } finally {
            viewModelStore.clear()
            closePort(port)
            Dispatchers.resetMain()
        }
    }

    @Test fun mockFailureMarksSyntheticTurnFailedAndCloseIsIdempotent() = runTest {
        val dispatcher = StandardTestDispatcher(testScheduler)
        Dispatchers.setMain(dispatcher)
        val native = FakeCore()
        val adapter = EmbeddedCoreAdapter({ "synthetic-core" }, native)
        val mockEntered = CompletableDeferred<Unit>()
        val port = EmbeddedCoreModelChatPort(adapter, mockPort {
            mockEntered.complete(Unit)
            throw ModelChatFailure("synthetic mock failure")
        })
        val db = SyntheticConversationDb()
        val chat = ChatViewModel(port, db)
        val viewModelStore = ViewModelStore().apply { put("synthetic-chat", chat) }
        try {
            advanceUntilIdle()
            chat.updateInput("synthetic: fail")
            chat.sendMessage()
            awaitCoreReady(native)
            awaitUntil("Core-ready request did not enter the failing Mock") {
                mockEntered.isCompleted || chat.state.value.errorMessage != null
            }
            assertTrue("Mock failure was not reached; Chat error=${chat.state.value.errorMessage}", mockEntered.isCompleted)
            advanceUntilIdle()
            assertEquals("FAILED", db.get(chat.state.value.conversation.id)?.messages?.singleOrNull()?.status)
            assertEquals(1, native.initializeCount)
            closePort(port)
            closePort(port)
        } finally {
            viewModelStore.clear()
            closePort(port)
            Dispatchers.resetMain()
        }
        assertEquals(1, native.closeCount)
    }
}
