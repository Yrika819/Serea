package app.serea.android

import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withContext
import okhttp3.Call
import okhttp3.Callback
import okhttp3.EventListener
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
import com.google.gson.stream.JsonReader
import java.io.IOException
import java.io.InterruptedIOException
import java.io.StringReader
import java.math.BigDecimal
import java.nio.ByteBuffer
import java.nio.charset.CodingErrorAction
import kotlin.coroutines.cancellation.CancellationException
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean
import okio.BufferedSource

/** Cloud mode is deliberately off until the Owner ratifies the data-egress contract. */
/** The historical model string is not a default; callers must name an explicitly reviewed candidate. */
data class CloudSettings(val enabled: Boolean = false, val modelId: String = "")
internal interface CredentialReader { fun readForRequest(): String }

data class ProviderUsage(val inputTokens: Int?, val outputTokens: Int?)
data class CloudReply(val text: String, val usage: ProviderUsage?)

enum class ProviderFailureKind { DISABLED, CREDENTIAL_MISSING, CREDENTIAL_INVALID, AUTHENTICATION, FORBIDDEN, MODEL_UNAVAILABLE, RATE_LIMITED, SERVER, NETWORK, TIMEOUT, INVALID_RESPONSE, LIMIT_EXCEEDED }
open class ProviderFailure(val kind: ProviderFailureKind, val mayHaveDispatched: Boolean = true) : Exception(
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
        ProviderFailureKind.TIMEOUT -> if (mayHaveDispatched) "The request timed out; its remote outcome is unknown. Retry only after review."
            else "The request timed out before it was sent."
        ProviderFailureKind.INVALID_RESPONSE -> "The provider returned an invalid response."
        ProviderFailureKind.LIMIT_EXCEEDED -> "The request exceeds the local size limit."
    }
)
internal class ProviderPreDispatchFailure(kind: ProviderFailureKind) : ProviderFailure(kind, mayHaveDispatched = false)

/** A gate-issued value. There is no public constructor from arbitrary user text. */
class EgressApprovedPayload private constructor(
    internal val messages: List<Pair<String, String>>,
    internal val conversationId: String?,
    internal val sendId: String?,
    internal val selectedModelId: String?,
    internal val destination: String,
    internal val origin: DataOrigin,
    internal val classification: DataClassification,
    internal val target: EgressTarget,
    internal val bodySourceLineage: Set<DataOrigin>,
) {
    internal val text: String get() = messages.lastOrNull()?.second.orEmpty()
    companion object {
        internal fun fixedPublicContractProbe(): EgressApprovedPayload =
            EgressApprovedPayload(listOf("user" to "Reply with exactly: Serea cloud contract probe passed."), null, null,
                null, "https://ollama.com/api/chat", DataOrigin.USER_AUTHORED, DataClassification.PUBLIC, EgressTarget.CLOUD,
                setOf(DataOrigin.USER_AUTHORED))
        internal fun fromApprovedContract(contract: EgressContract, messages: List<Pair<String, String>>) =
            EgressApprovedPayload(messages.toList(), contract.conversationId, contract.currentMessageId,
                contract.selectedModelId, contract.destination, contract.origin, contract.classification, contract.target,
                contract.plannedBodySourceLineage.toSet())
    }
}

/** Fixed-host Ollama Cloud adapter. Only the gated composition root can obtain an instance. */
class OllamaCloudAdapter private constructor(
    private val credentials: CredentialReader,
    private val settings: CloudSettings,
    suppliedClient: OkHttpClient,
    private val endpoint: HttpUrl,
    private val catalog: ModelCatalog,
    private val allowedProviderIds: Set<String>,
) {
    // Injected clients must not silently replay this POST or forward credentials on redirects.
    private val client = suppliedClient.newBuilder()
        .retryOnConnectionFailure(false)
        .followRedirects(false)
        .followSslRedirects(false)
        .build()
    internal constructor(credentials: CredentialReader, settings: CloudSettings) : this(
        credentials, settings,
        OkHttpClient.Builder()
            .followRedirects(false).followSslRedirects(false)
            .connectTimeout(10, TimeUnit.SECONDS).readTimeout(45, TimeUnit.SECONDS)
            .callTimeout(60, TimeUnit.SECONDS).build(),
        "https://ollama.com/api/chat".toHttpUrl(), LocalModelCatalog(), setOf("ollama"),
    )

    internal constructor(credentials: CredentialReader, settings: CloudSettings, catalog: ModelCatalog,
        allowedProviderIds: Set<String> = setOf("ollama")) : this(
        credentials, settings,
        OkHttpClient.Builder().followRedirects(false).followSslRedirects(false)
            .connectTimeout(10, TimeUnit.SECONDS).readTimeout(45, TimeUnit.SECONDS)
            .callTimeout(60, TimeUnit.SECONDS).build(),
        "https://ollama.com/api/chat".toHttpUrl(), catalog, allowedProviderIds,
    )

    /** Test seam accepts only Ollama's fixed API URL or a loopback MockWebServer endpoint. */
    internal constructor(credentials: CredentialReader, settings: CloudSettings, testEndpoint: HttpUrl, testClient: OkHttpClient,
        catalog: ModelCatalog = LocalModelCatalog(), allowedProviderIds: Set<String> = setOf("ollama")) : this(
        credentials, settings, testClient, testEndpoint.also {
            val fixedProvider = it.isHttps && it.host == "ollama.com" && it.port == 443 &&
                it.encodedPath == "/api/chat" && it.encodedQuery == null && it.encodedFragment == null
            val localTestServer = it.host == "127.0.0.1" || it.host == "localhost"
            require(fixedProvider || localTestServer) { "Only the fixed Ollama API or a loopback test server is allowed." }
        }, catalog, allowedProviderIds,
    )

    suspend fun send(payload: EgressApprovedPayload, onDelta: (String) -> Unit = {}, selectedModelId: String? = null): CloudReply {
        if (payload.target != EgressTarget.CLOUD) throw ProviderPreDispatchFailure(ProviderFailureKind.FORBIDDEN)
        if (!settings.enabled) throw ProviderPreDispatchFailure(ProviderFailureKind.DISABLED)
        val fixedProbe = payload.conversationId == null && payload.sendId == null && payload.selectedModelId == null
        val modelId = if (fixedProbe) selectedModelId ?: settings.modelId else selectedModelId
        if (modelId.isNullOrBlank() || !validProviderModelId(modelId)) throw ProviderPreDispatchFailure(ProviderFailureKind.MODEL_UNAVAILABLE)
        if (fixedProbe) {
            if (modelId !in LEGACY_PROBE_MODEL_IDS) throw ProviderPreDispatchFailure(ProviderFailureKind.MODEL_UNAVAILABLE)
        } else {
            val boundModelId = payload.selectedModelId
            val descriptor = catalog.all().firstOrNull { it.id == modelId }
            val fixtureModelAllowed = descriptor?.verificationStatus == ModelVerificationStatus.TEST_FIXTURE &&
                (endpoint.host == "127.0.0.1" || endpoint.host == "localhost")
            val modelEvidenceAllowed = descriptor?.verificationStatus == ModelVerificationStatus.VERIFIED || fixtureModelAllowed
            if (boundModelId != modelId || descriptor == null || descriptor.providerId !in allowedProviderIds ||
                descriptor.providerId != "ollama" || descriptor.availability != ModelAvailability.AVAILABLE ||
                !descriptor.authorized || !modelEvidenceAllowed ||
                descriptor.source.isNullOrBlank() || descriptor.lastVerifiedAt.isNullOrBlank() || !catalog.isFresh(descriptor)) {
                throw ProviderPreDispatchFailure(ProviderFailureKind.MODEL_UNAVAILABLE)
            }
            if (payload.destination != "https://ollama.com/api/chat" || payload.origin != DataOrigin.USER_AUTHORED ||
                payload.classification != DataClassification.PUBLIC || payload.conversationId.isNullOrBlank() || payload.sendId.isNullOrBlank()) {
                throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE)
            }
        }
        if (payload.messages.sumOf { it.first.length.toLong() + it.second.toByteArray(Charsets.UTF_8).size + 64L } > MAX_INPUT_BYTES) {
            throw ProviderPreDispatchFailure(ProviderFailureKind.LIMIT_EXCEEDED)
        }
        val request = withContext(Dispatchers.IO) {
            val key = try { credentials.readForRequest() } catch (_: IllegalStateException) { throw ProviderPreDispatchFailure(ProviderFailureKind.CREDENTIAL_MISSING) }
            catch (_: HistoryUnavailableException) { throw ProviderPreDispatchFailure(ProviderFailureKind.CREDENTIAL_INVALID) }
            val requestJson = JsonObject().apply {
                addProperty("model", modelId)
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
            val dispatchPossible = AtomicBoolean(false)
            val callClient = client.newBuilder().eventListener(object : EventListener() {
                override fun requestBodyStart(call: Call) { dispatchPossible.set(true) }
            }).build()
            val call = callClient.newCall(request)
            continuation.invokeOnCancellation { call.cancel() }
            call.enqueue(object : Callback {
                override fun onFailure(call: Call, error: IOException) {
                    if (!continuation.isActive) return
                    val failure = if (error is InterruptedIOException) {
                        ProviderFailure(ProviderFailureKind.TIMEOUT, dispatchPossible.get())
                    } else ProviderFailure(ProviderFailureKind.NETWORK, dispatchPossible.get())
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
                val nextLine = readBoundedLine(source) ?: break
                total += nextLine.bytesRead
                if (total > MAX_RESPONSE_BYTES) throw ProviderFailure(ProviderFailureKind.LIMIT_EXCEEDED)
                val line = nextLine.text
                if (line.isBlank()) continue
                if (completed) throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE)
                val json = try {
                    JsonReader(StringReader(line)).also { it.isLenient = false }.let { reader ->
                        Gson().getAdapter(JsonObject::class.java).read(reader).also {
                            if (reader.peek() != com.google.gson.stream.JsonToken.END_DOCUMENT) throw IllegalStateException()
                        }
                    }
                } catch (_: Exception) { throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE) }
                val delta = try { json.getAsJsonObject("message")?.get("content")?.asString.orEmpty() }
                    catch (_: Exception) { throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE) }
                answerBytes += delta.toByteArray(Charsets.UTF_8).size
                if (answerBytes > MAX_RESPONSE_BYTES) throw ProviderFailure(ProviderFailureKind.LIMIT_EXCEEDED)
                if (delta.isNotEmpty()) { answer.append(delta); onDelta(delta) }
                inputTokens = providerTokenCount(json.get("prompt_eval_count")) ?: inputTokens
                outputTokens = providerTokenCount(json.get("eval_count")) ?: outputTokens
                val done = json.get("done")
                if (done != null && !done.isJsonNull && (!done.isJsonPrimitive || !done.asJsonPrimitive.isBoolean)) {
                    throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE)
                }
                if (done?.asBoolean == true) completed = true
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
        // Retained solely for the old fixed public contract probe. This is not a Catalog entry or recommendation.
        private val LEGACY_PROBE_MODEL_IDS = setOf("gemma4:31b")
        private fun validProviderModelId(value: String) = value.matches(Regex("[A-Za-z0-9][A-Za-z0-9._:/-]{0,127}")) && !value.contains("://")
        private val JSON_MEDIA_TYPE = "application/json; charset=utf-8".toMediaType()

        private data class ProviderLine(val text: String, val bytesRead: Int)

        /** Read exact wire bytes so CRLF and invalid UTF-8 cannot bypass the response limits. */
        private fun readBoundedLine(source: BufferedSource): ProviderLine? {
            val bytes = java.io.ByteArrayOutputStream()
            var consumed = 0
            while (true) {
                if (!source.request(1)) {
                    if (consumed == 0) return null
                    break
                }
                val next = source.readByte()
                consumed++
                if (consumed > MAX_LINE_BYTES) throw ProviderFailure(ProviderFailureKind.LIMIT_EXCEEDED)
                if (next == '\n'.code.toByte()) break
                bytes.write(next.toInt() and 0xff)
            }
            val lineBytes = bytes.toByteArray()
            val contentLength = if (lineBytes.lastOrNull() == '\r'.code.toByte()) lineBytes.size - 1 else lineBytes.size
            val text = try {
                Charsets.UTF_8.newDecoder()
                    .onMalformedInput(CodingErrorAction.REPORT)
                    .onUnmappableCharacter(CodingErrorAction.REPORT)
                    .decode(ByteBuffer.wrap(lineBytes, 0, contentLength)).toString()
            } catch (_: Exception) {
                throw ProviderFailure(ProviderFailureKind.INVALID_RESPONSE)
            }
            return ProviderLine(text, consumed)
        }

        /** Invalid or out-of-range provider telemetry remains unknown; it never wraps or becomes zero. */
        private fun providerTokenCount(value: com.google.gson.JsonElement?): Int? {
            if (value == null || value.isJsonNull || !value.isJsonPrimitive || !value.asJsonPrimitive.isNumber) return null
            return try {
                val exact = BigDecimal(value.asString).toBigIntegerExact()
                if (exact.signum() < 0 || exact > Int.MAX_VALUE.toBigInteger()) null else exact.toInt()
            } catch (_: Exception) {
                null
            }
        }
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
        val contract = request.egressContract ?: throw PreDispatchModelChatFailure("Cloud send intent has not been approved.")
        val requestBodyLineage = request.bodySourceLineage.toSet()
        val contractBodyLineage = contract.plannedBodySourceLineage.toSet()
        if (contract.target != EgressTarget.CLOUD || contract.conversationId != request.conversationId || contract.currentMessageId != request.sendId ||
            contract.plannedBody != request.message || contract.context != request.context ||
            contractBodyLineage != requestBodyLineage ||
            (contract.selectedModelId != null && contract.selectedModelId != request.selectedModelId) || request.selectedModelId.isNullOrBlank() ||
            contract.classification != request.classification || contract.origin != DataOrigin.USER_AUTHORED) {
            throw PreDispatchModelChatFailure("The approved content no longer matches the request.")
        }
        val boundContract = contract.copy(
            selectedModelId = request.selectedModelId,
            plannedBodySourceLineage = contractBodyLineage,
        )
        val evaluation = ChatEgressGate.evaluate(boundContract, clock())
        if (evaluation.decision != EgressContractDecision.APPROVED) {
            throw PreDispatchModelChatFailure("Cloud egress was denied: ${evaluation.decision.name.lowercase()}.")
        }
        val payload = evaluation.payload ?: throw PreDispatchModelChatFailure("Cloud egress approval is unavailable.")
        if (payload.conversationId != request.conversationId || payload.sendId != request.sendId ||
            payload.selectedModelId != request.selectedModelId || payload.destination != boundContract.destination ||
            payload.bodySourceLineage.toSet() != requestBodyLineage) {
            throw PreDispatchModelChatFailure("The approved provider payload no longer matches the routed send.")
        }
        return try {
            adapter.send(payload, selectedModelId = request.selectedModelId).let { ModelChatResponse(it.text, it.usage) }
        } catch (failure: ProviderFailure) {
            if (failure is ProviderPreDispatchFailure || !failure.mayHaveDispatched) {
                throw PreDispatchModelChatFailure(failure.message ?: "The local provider request was rejected before dispatch.")
            }
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
        catalog: ModelCatalog = LocalModelCatalog(),
        routingPolicy: () -> ModelRoutingPolicy = { ModelRoutingPolicy(allowedProviderIds = setOf("ollama")) },
        testEndpoint: HttpUrl? = null,
        testClient: OkHttpClient? = null,
        usageAttributionResolver: UsageAttributionResolver? = null,
    ): ModelChatPort {
        require((testEndpoint == null) == (testClient == null)) { "A loopback test endpoint and client must be supplied together." }
        val adapter = if (testEndpoint != null && testClient != null)
            OllamaCloudAdapter(credentials, settings, testEndpoint, testClient, catalog, setOf("ollama"))
        else OllamaCloudAdapter(credentials, settings, catalog, setOf("ollama"))
        val gated = EgressCheckedModelChatPort(
            UsageControlledModelChatPort(
                OllamaModelChatPort(adapter), usage,
                attributionResolver = usageAttributionResolver ?: CatalogUsageAttributionResolver(catalog),
            ),
            expectedTarget = EgressTarget.CLOUD,
        )
        return RoutedModelChatPort(gated, AdaptiveModelRouter(catalog), policy = routingPolicy)
    }
}
