package app.serea.android

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import okhttp3.Call
import okhttp3.Callback
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody
import okhttp3.Response
import com.google.gson.Gson
import com.google.gson.JsonArray
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import java.io.IOException
import java.io.InterruptedIOException
import kotlin.coroutines.cancellation.CancellationException
import java.util.concurrent.TimeUnit

/** Cloud mode is deliberately off until the Owner ratifies the data-egress contract. */
data class CloudSettings(val enabled: Boolean = false, val modelId: String = "gemma4:31b")
internal interface CredentialReader { fun readForRequest(): String }

data class ProviderUsage(val inputTokens: Int?, val outputTokens: Int?)
data class CloudReply(val text: String, val usage: ProviderUsage?)

enum class ProviderFailureKind { DISABLED, CREDENTIAL_MISSING, CREDENTIAL_INVALID, AUTHENTICATION, FORBIDDEN, MODEL_UNAVAILABLE, RATE_LIMITED, SERVER, NETWORK, TIMEOUT, INVALID_RESPONSE, LIMIT_EXCEEDED }
class ProviderFailure(val kind: ProviderFailureKind) : Exception(
    when (kind) {
        ProviderFailureKind.DISABLED -> "Cloud mode is disabled."
        ProviderFailureKind.CREDENTIAL_MISSING -> "Set an API key in Settings before using cloud mode."
        ProviderFailureKind.CREDENTIAL_INVALID -> "The saved API key is unavailable. Delete it in Settings, then add it again."
        ProviderFailureKind.AUTHENTICATION -> "The provider rejected the API key. Review or replace it in Settings."
        ProviderFailureKind.FORBIDDEN -> "The provider account is not permitted to use this model."
        ProviderFailureKind.MODEL_UNAVAILABLE -> "The selected cloud model is unavailable."
        ProviderFailureKind.RATE_LIMITED -> "The provider rate limit was reached. Wait before sending again."
        ProviderFailureKind.SERVER -> "The provider is temporarily unavailable. No automatic retry was made."
        ProviderFailureKind.NETWORK -> "The network request could not be completed."
        ProviderFailureKind.TIMEOUT -> "The request timed out; its remote outcome is unknown. Retry only after review."
        ProviderFailureKind.INVALID_RESPONSE -> "The provider returned an invalid response."
        ProviderFailureKind.LIMIT_EXCEEDED -> "The request exceeds the local size limit."
    }
)

/** A gate-issued value. There is no public constructor from arbitrary user text. */
class EgressApprovedPayload private constructor(internal val messages: List<Pair<String, String>>) {
    internal val text: String get() = messages.lastOrNull()?.second.orEmpty()
    companion object {
        internal fun fixedPublicContractProbe(): EgressApprovedPayload =
            EgressApprovedPayload(listOf("user" to "Reply with exactly: Serea cloud contract probe passed."))
        internal fun fromApprovedContract(messages: List<Pair<String, String>>): EgressApprovedPayload =
            EgressApprovedPayload(messages.toList())
    }
}

/** Fixed-host Ollama Cloud adapter. Only the gated composition root can obtain an instance. */
class OllamaCloudAdapter private constructor(
    private val credentials: CredentialReader,
    private val settings: CloudSettings,
    private val client: OkHttpClient,
    private val endpoint: HttpUrl,
) {
    internal constructor(credentials: CredentialReader, settings: CloudSettings) : this(
        credentials, settings,
        OkHttpClient.Builder()
            .followRedirects(false).followSslRedirects(false)
            .connectTimeout(10, TimeUnit.SECONDS).readTimeout(45, TimeUnit.SECONDS)
            .callTimeout(60, TimeUnit.SECONDS).build(),
        "https://ollama.com/api/chat".toHttpUrl(),
    )

    /** Test seam accepts only Ollama's fixed API URL or a loopback MockWebServer endpoint. */
    internal constructor(credentials: CredentialReader, settings: CloudSettings, testEndpoint: HttpUrl, testClient: OkHttpClient) : this(
        credentials, settings, testClient, testEndpoint.also {
            val fixedProvider = it.isHttps && it.host == "ollama.com" && it.port == 443 &&
                it.encodedPath == "/api/chat" && it.encodedQuery == null && it.encodedFragment == null
            val localTestServer = it.host == "127.0.0.1" || it.host == "localhost"
            require(fixedProvider || localTestServer) { "Only the fixed Ollama API or a loopback test server is allowed." }
        },
    )

    suspend fun send(payload: EgressApprovedPayload, onDelta: (String) -> Unit = {}): CloudReply {
        if (!settings.enabled) throw ProviderFailure(ProviderFailureKind.DISABLED)
        if (settings.modelId !in SUPPORTED_MODEL_IDS) {
            throw ProviderFailure(ProviderFailureKind.MODEL_UNAVAILABLE)
        }
        if (payload.messages.sumOf { it.first.length.toLong() + it.second.toByteArray(Charsets.UTF_8).size + 64L } > MAX_INPUT_BYTES) {
            throw ProviderFailure(ProviderFailureKind.LIMIT_EXCEEDED)
        }
        val request = withContext(Dispatchers.IO) {
            val key = try { credentials.readForRequest() } catch (_: IllegalStateException) { throw ProviderFailure(ProviderFailureKind.CREDENTIAL_MISSING) }
            catch (_: HistoryUnavailableException) { throw ProviderFailure(ProviderFailureKind.CREDENTIAL_INVALID) }
            val requestJson = JsonObject().apply {
                addProperty("model", settings.modelId)
                add("messages", JsonArray().apply {
                    payload.messages.forEach { (role, content) -> add(JsonObject().apply { addProperty("role", role); addProperty("content", content) }) }
                })
                addProperty("stream", true)
                add("options", JsonObject().apply { addProperty("num_predict", MAX_OUTPUT_TOKENS) })
            }
            Request.Builder().url(endpoint)
                .header("Authorization", "Bearer $key")
                .header("Accept", "application/x-ndjson")
                .post(Gson().toJson(requestJson).toRequestBody(JSON_MEDIA_TYPE)).build()
        }
        return suspendCancellableCoroutine { continuation ->
            val call = client.newCall(request)
            continuation.invokeOnCancellation { call.cancel() }
            call.enqueue(object : Callback {
                override fun onFailure(call: Call, error: IOException) {
                    if (!continuation.isActive) return
                    val failure = if (error is InterruptedIOException) {
                        ProviderFailure(ProviderFailureKind.TIMEOUT)
                    } else ProviderFailure(ProviderFailureKind.NETWORK)
                    continuation.resumeWith(Result.failure(failure))
                }

                override fun onResponse(call: Call, response: Response) {
                    if (!continuation.isActive) { response.close(); return }
                    try {
                        val result = consume(response, onDelta) { continuation.isActive }
                        if (continuation.isActive) continuation.resumeWith(Result.success(result))
                    } catch (error: Exception) {
                        if (continuation.isActive) {
                            val mapped = when (error) {
                                is ProviderFailure, is CancellationException -> error
                                is InterruptedIOException -> ProviderFailure(ProviderFailureKind.TIMEOUT)
                                is IOException -> ProviderFailure(ProviderFailureKind.NETWORK)
                                else -> ProviderFailure(ProviderFailureKind.INVALID_RESPONSE)
                            }
                            continuation.resumeWith(Result.failure(mapped))
                        }
                    }
                }
            })
        }
    }

    private fun consume(response: Response, onDelta: (String) -> Unit, active: () -> Boolean): CloudReply {
        response.use {
            if (!response.isSuccessful) throw ProviderFailure(when (response.code) {
                401 -> ProviderFailureKind.AUTHENTICATION
                403 -> ProviderFailureKind.FORBIDDEN
                404 -> ProviderFailureKind.MODEL_UNAVAILABLE
                429 -> ProviderFailureKind.RATE_LIMITED
                in 500..599 -> ProviderFailureKind.SERVER
                else -> ProviderFailureKind.INVALID_RESPONSE
            })
            val body = response.body ?: throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE)
            if ((body.contentLength() > MAX_RESPONSE_BYTES) || response.header("Content-Length")?.toLongOrNull()?.let { it > MAX_RESPONSE_BYTES } == true) {
                throw ProviderFailure(ProviderFailureKind.LIMIT_EXCEEDED)
            }
            var total = 0
            var answerBytes = 0
            var completed = false
            val answer = StringBuilder()
            var inputTokens: Int? = null
            var outputTokens: Int? = null
            val source = body.source()
            while (!source.exhausted()) {
                if (!active()) throw CancellationException("Provider request cancelled")
                val line = try { source.readUtf8LineStrict(MAX_LINE_BYTES.toLong()) }
                    catch (_: java.io.EOFException) {
                        // NDJSON producers may omit the final newline. At EOF the bounded
                        // line reader has already buffered at most MAX_LINE_BYTES + 1 bytes.
                        if (source.buffer.size > MAX_LINE_BYTES) throw ProviderFailure(ProviderFailureKind.LIMIT_EXCEEDED)
                        source.readUtf8()
                    }
                val lineBytes = line.toByteArray(Charsets.UTF_8).size + 1
                total += lineBytes
                if (total > MAX_RESPONSE_BYTES || lineBytes > MAX_LINE_BYTES) throw ProviderFailure(ProviderFailureKind.LIMIT_EXCEEDED)
                if (line.isBlank()) continue
                val json = try { JsonParser.parseString(line).asJsonObject } catch (_: Exception) { throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE) }
                val delta = try { json.getAsJsonObject("message")?.get("content")?.asString.orEmpty() }
                    catch (_: Exception) { throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE) }
                answerBytes += delta.toByteArray(Charsets.UTF_8).size
                if (answerBytes > MAX_RESPONSE_BYTES) throw ProviderFailure(ProviderFailureKind.LIMIT_EXCEEDED)
                if (delta.isNotEmpty()) { answer.append(delta); onDelta(delta) }
                try {
                    inputTokens = json.get("prompt_eval_count")?.takeUnless { it.isJsonNull }?.asInt?.coerceAtLeast(0) ?: inputTokens
                    outputTokens = json.get("eval_count")?.takeUnless { it.isJsonNull }?.asInt?.coerceAtLeast(0) ?: outputTokens
                    if (json.get("done")?.asBoolean == true) { completed = true; break }
                } catch (_: Exception) { throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE) }
            }
            if (!completed) throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE)
            if (answer.isEmpty()) throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE)
            return CloudReply(answer.toString(), if (inputTokens != null || outputTokens != null) ProviderUsage(inputTokens, outputTokens) else null)
        }
    }

    companion object {
        private const val MAX_INPUT_BYTES = 16 * 1024
        private const val MAX_RESPONSE_BYTES = 1024 * 1024
        private const val MAX_LINE_BYTES = 64 * 1024
        private const val MAX_OUTPUT_TOKENS = 512
        private val SUPPORTED_MODEL_IDS = setOf("gemma4:31b")
        private val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()
    }
}

/** The only UI-facing cloud composition: all ordinary messages remain UNKNOWN and are denied. */
class CloudCandidatePort internal constructor(private val adapter: OllamaCloudAdapter) {
    suspend fun runFixedPublicContractProbe(onDelta: (String) -> Unit = {}): CloudReply =
        adapter.send(EgressApprovedPayload.fixedPublicContractProbe(), onDelta)
}

/** Ordinary chat path. An exact, fresh gate approval must exist before any credential or socket use. */
internal class OllamaModelChatPort(
    private val adapter: OllamaCloudAdapter,
    private val clock: () -> Long = System::currentTimeMillis,
) : ModelChatPort {
    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        val contract = request.egressContract ?: throw ModelChatFailure("Cloud send intent has not been approved.")
        if (contract.conversationId != request.conversationId || contract.currentMessageId != request.sendId ||
            contract.plannedBody != request.message || contract.context != request.context ||
            contract.classification != request.classification || contract.origin != DataOrigin.USER_AUTHORED) {
            throw ModelChatFailure("The approved content no longer matches the request.")
        }
        val evaluation = ChatEgressGate.evaluate(contract, clock())
        if (evaluation.decision != EgressContractDecision.APPROVED) {
            throw ModelChatFailure("Cloud egress was denied: ${evaluation.decision.name.lowercase()}.")
        }
        val payload = evaluation.payload ?: throw ModelChatFailure("Cloud egress approval is unavailable.")
        return try {
            adapter.send(payload).let { ModelChatResponse(it.text, it.usage) }
        } catch (failure: ProviderFailure) {
            throw ModelChatFailure(failure.message ?: "Cloud request failed.")
        }
    }
}

/** Direct Android composition root: gate -> durable usage reservation -> typed adapter. */
internal object DirectProviderChatComposition {
    fun create(
        credentials: CredentialReader,
        usage: UsageRepository,
        settings: CloudSettings = CloudSettings(),
    ): ModelChatPort = EgressCheckedModelChatPort(
        UsageControlledModelChatPort(
            OllamaModelChatPort(OllamaCloudAdapter(credentials, settings)),
            usage,
        ),
    )
}
