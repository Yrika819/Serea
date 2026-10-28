package app.serea.g3poc

import kotlinx.coroutines.test.runTest
import org.junit.Assert.assertEquals
import org.junit.Assert.fail
import org.junit.Test
import java.io.File
import java.security.MessageDigest

class ModelChatPortContractTest {
    @Test
    fun sendPassesCurrentAndPriorTurnsToTheLocalEngine() = runTest {
        val engine = RecordingEngine()
        val port = GgufModelChatPort("not-loaded-in-this-unit-test", "0".repeat(64), engine)

        val response = port.send(
            ModelChatRequest(
                conversationId = "synthetic-conversation",
                message = "What is the next step?",
                context = listOf(
                    ContextMessage("user", "I have a small task."),
                    ContextMessage("assistant", "What should happen next?"),
                ),
                maxOutputTokens = 12,
            ),
        )

        assertEquals("fixed test response", response.text)
        assertEquals(
            "user: I have a small task.\nassistant: What should happen next?\nuser: What is the next step?\nassistant:",
            engine.lastPrompt,
        )
        assertEquals(12, engine.lastTokenLimit)
    }

    @Test
    fun sendPreservesUnicodeInCurrentAndPriorTurns() = runTest {
        val engine = RecordingEngine()
        val port = GgufModelChatPort("not-loaded-in-this-unit-test", "0".repeat(64), engine)
        val response = port.send(
            ModelChatRequest(
                conversationId = "synthetic-unicode-conversation",
                message = "日本語の予定を確認して 🌱",
                context = listOf(ContextMessage("assistant", "Résumé déjà reçu.")),
                maxOutputTokens = 8,
            ),
        )

        assertEquals("fixed test response", response.text)
        assertEquals(
            "assistant: Résumé déjà reçu.\nuser: 日本語の予定を確認して 🌱\nassistant:",
            engine.lastPrompt,
        )
    }

    @Test
    fun modelHashIsCheckedBeforeCallingTheNativeLoader() = runTest {
        val fixture = File.createTempFile("serea-g3-model-integrity", ".gguf")
        fixture.writeText("synthetic integrity fixture; not a model")
        val engine = RecordingEngine()
        try {
            val accepted = GgufModelChatPort(fixture.absolutePath, sha256(fixture), engine)
            accepted.load()
            assertEquals(1, engine.loadCalls)

            val rejected = GgufModelChatPort(fixture.absolutePath, "0".repeat(64), engine)
            try {
                rejected.load()
                fail("wrong model digest must be rejected before native load")
            } catch (_: IllegalArgumentException) {
                assertEquals(1, engine.loadCalls)
            }
        } finally {
            engine.close()
            fixture.delete()
        }
    }

    @Test
    fun closedModelPortRejectsFurtherUse() = runTest {
        val engine = RecordingEngine()
        val port = GgufModelChatPort("not-loaded-in-this-unit-test", "0".repeat(64), engine)
        port.close()

        try {
            port.send(ModelChatRequest("synthetic", "must be rejected", maxOutputTokens = 4))
            fail("closed port must reject generation")
        } catch (_: IllegalStateException) {
            assertEquals(0, engine.generateCalls)
            assertEquals(1, engine.closeCalls)
        }
    }

    private fun sha256(file: File): String = MessageDigest.getInstance("SHA-256")
        .digest(file.readBytes())
        .joinToString("") { "%02x".format(it) }

    private class RecordingEngine : LocalInferenceEngine {
        var lastPrompt: String? = null
        var lastTokenLimit: Int? = null
        var loadCalls = 0
        var generateCalls = 0
        var closeCalls = 0

        override suspend fun load(path: String) { loadCalls++ }
        override suspend fun generate(prompt: String, maxOutputTokens: Int): String {
            generateCalls++
            lastPrompt = prompt
            lastTokenLimit = maxOutputTokens
            return "fixed test response"
        }
        override fun close() { closeCalls++ }
    }
}
