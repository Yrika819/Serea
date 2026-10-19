package app.serea.android

import com.google.gson.JsonArray
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.HttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.MediaType.Companion.toMediaType
import java.util.concurrent.TimeUnit

/** Test-only HTTP provider. It accepts loopback MockWebServer endpoints and has no credentials. */
internal class MockHttpChatPort(endpoint: HttpUrl) : ModelChatPort {
    private val endpoint = endpoint.also {
        require(!it.isHttps && it.host in setOf("127.0.0.1", "localhost"))
    }
    private val client = OkHttpClient.Builder().followRedirects(false).followSslRedirects(false)
        .callTimeout(10, TimeUnit.SECONDS).build()

    override suspend fun send(request: ModelChatRequest): ModelChatResponse = withContext(Dispatchers.IO) {
        val messages = JsonArray().apply {
            request.context.forEach { add(JsonObject().apply {
                addProperty("role", it.role.name.lowercase())
                addProperty("content", it.text)
                addProperty("source_message_id", it.sourceMessageId)
                addProperty("classification", it.classification.name)
            }) }
            add(JsonObject().apply {
                addProperty("role", "user")
                addProperty("content", request.message)
                addProperty("source_message_id", request.sendId)
                addProperty("classification", request.classification.name)
            })
        }
        val body = JsonObject().apply {
            addProperty("model", "mock-http")
            addProperty("stream", false)
            add("messages", messages)
        }.toString().toRequestBody("application/json; charset=utf-8".toMediaType())
        val response = client.newCall(Request.Builder().url(endpoint).post(body).build()).execute()
        response.use {
            if (!it.isSuccessful) throw ModelChatFailure("Mock HTTP provider returned ${it.code}.")
            val json = try { JsonParser.parseString(it.body?.string().orEmpty()).asJsonObject }
            catch (_: Exception) { throw ModelChatFailure("Mock HTTP provider returned an invalid response.") }
            val text = json.getAsJsonObject("message")?.get("content")?.asString
                ?: throw ModelChatFailure("Mock HTTP provider omitted its response.")
            val usage = if (json.has("prompt_eval_count") && json.has("eval_count")) ProviderUsage(
                json.get("prompt_eval_count").asInt.coerceAtLeast(0), json.get("eval_count").asInt.coerceAtLeast(0),
            ) else null
            ModelChatResponse(text, usage)
        }
    }
}
