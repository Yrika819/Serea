package app.serea.android.gmail.transport

import app.serea.android.gmail.GmailAuthorizationPort
import app.serea.android.gmail.GmailAuthorizationState
import app.serea.android.gmail.GmailMessage
import app.serea.android.gmail.GmailMessageId
import app.serea.android.gmail.GmailMessagePage
import app.serea.android.gmail.GmailMessageSource
import app.serea.android.gmail.GmailPermissionPolicy
import app.serea.android.gmail.GmailReadFailure
import app.serea.android.gmail.GmailReadFailureCode
import app.serea.android.gmail.GmailReadPort
import app.serea.android.gmail.GmailThread
import app.serea.android.gmail.GmailThreadId
import app.serea.android.gmail.GmailMimeParser
import com.google.gson.JsonParser
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.currentCoroutineContext
import okhttp3.Call
import okhttp3.Callback
import okhttp3.HttpUrl
import okhttp3.HttpUrl.Companion.toHttpUrl
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.util.concurrent.TimeUnit
import kotlin.coroutines.resume

/** Gmail REST read adapter. The API surface is intentionally limited to list, message, and thread GETs. */
class GmailHttpReadPort internal constructor(
    private val authorization: GmailAuthorizationPort,
    private val tokens: GmailAccessTokenPort,
    client: OkHttpClient = productionClient(),
    private val baseUrl: HttpUrl = GMAIL_API_BASE,
) : GmailReadPort {
    private val safeClient = client.newBuilder()
        .followRedirects(false)
        .followSslRedirects(false)
        .retryOnConnectionFailure(false)
        .build()

    init {
        val production = baseUrl.scheme == "https" && baseUrl.host == GMAIL_API_HOST
        val loopbackTest = baseUrl.scheme == "http" && baseUrl.host in TEST_HOSTS
        require((production || loopbackTest) && baseUrl.encodedPath == "/gmail/v1/")
    }

    override suspend fun listInbox(pageToken: String?, pageSize: Int): GmailMessagePage =
        listMessages(query = null, inboxOnly = true, pageToken = pageToken, pageSize = pageSize)

    override suspend fun search(query: String, pageToken: String?, pageSize: Int): GmailMessagePage {
        val normalized = query.trim()
        if (normalized.isEmpty() || normalized.length > MAX_QUERY_CHARS || normalized.any(Char::isISOControl)) invalid()
        return listMessages(normalized, inboxOnly = false, pageToken = pageToken, pageSize = pageSize)
    }

    override suspend fun getMessage(id: GmailMessageId): GmailMessage {
        val url = userCollection("messages").newBuilder()
            .addPathSegment(id.value).addQueryParameter("format", "full").build()
        val raw = get(url, MAX_MESSAGE_RESPONSE_BYTES)
        val message = parseMessage(raw)
        if (message.id != id) invalid()
        return message
    }

    override suspend fun getThread(id: GmailThreadId): GmailThread {
        val url = userCollection("threads").newBuilder()
            .addPathSegment(id.value).addQueryParameter("format", "full").build()
        val raw = get(url, MAX_THREAD_RESPONSE_BYTES)
        try {
            val root = JsonParser.parseString(raw).asJsonObject
            if (root.stringOrNull("id") != id.value) invalid()
            val entries = root.get("messages")?.takeIf { it.isJsonArray }?.asJsonArray ?: invalid()
            if (entries.isEmpty() || entries.size() > MAX_THREAD_MESSAGES) invalid()
            val messages = entries.map { entry ->
                if (!entry.isJsonObject) invalid()
                val message = parseMessage(entry.toString())
                if (message.threadId != id) invalid()
                message
            }
            return GmailThread(id, messages)
        } catch (failure: GmailReadFailure) {
            throw failure
        } catch (_: Exception) {
            invalid()
        }
    }

    private suspend fun listMessages(
        query: String?,
        inboxOnly: Boolean,
        pageToken: String?,
        pageSize: Int,
    ): GmailMessagePage {
        if (pageSize !in 1..MAX_PAGE_SIZE) invalid()
        pageToken?.let(::validatePageToken)
        val url = userCollection("messages").newBuilder()
            .addQueryParameter("maxResults", pageSize.toString())
            .apply {
                if (inboxOnly) addQueryParameter("labelIds", "INBOX")
                query?.let { addQueryParameter("q", it) }
                pageToken?.let { addQueryParameter("pageToken", it) }
            }.build()
        val indexResponse = getBounded(url, MAX_LIST_RESPONSE_BYTES)
        val index = try { JsonParser.parseString(indexResponse.first).asJsonObject } catch (_: Exception) { invalid() }
        val entries = index.get("messages")?.let { value ->
            if (!value.isJsonArray) invalid()
            value.asJsonArray
        }
        if (entries != null && entries.size() > pageSize) invalid()
        val ids = entries?.map { item ->
            if (!item.isJsonObject) invalid()
            val obj = item.asJsonObject
            val id = try { GmailMessageId(obj.stringOrNull("id") ?: invalid()) } catch (_: Exception) { invalid() }
            try { GmailThreadId(obj.stringOrNull("threadId") ?: invalid()) } catch (_: Exception) { invalid() }
            id
        }.orEmpty()
        if (ids.distinct().size != ids.size) invalid()
        val messages = ArrayList<GmailMessage>(ids.size)
        var totalBytes = indexResponse.second
        for (id in ids) {
            val (raw, bytes) = getBounded(
                userCollection("messages").newBuilder().addPathSegment(id.value)
                    .addQueryParameter("format", "full").build(),
                MAX_MESSAGE_RESPONSE_BYTES,
            )
            totalBytes = checkedAdd(totalBytes, bytes)
            if (totalBytes > MAX_PAGE_RESPONSE_BYTES) invalid()
            val message = parseMessage(raw)
            if (message.id != id) invalid()
            messages += message
        }
        val next = index.stringOrNull("nextPageToken")?.also(::validatePageToken)
        return GmailMessagePage(messages, next)
    }

    private fun parseMessage(json: String): GmailMessage = try {
        GmailMimeParser.parseMessage(json, GmailMessageSource.GMAIL_API)
    } catch (_: Exception) {
        invalid()
    }

    private suspend fun get(url: HttpUrl, maximumBytes: Int): String = getBounded(url, maximumBytes).first

    private suspend fun getBounded(url: HttpUrl, maximumBytes: Int): Pair<String, Int> {
        val status = authorization.status()
        if (!GmailPermissionPolicy.mayRead(status)) {
            val code = if (status.state == GmailAuthorizationState.AUTHORIZATION_REQUIRED &&
                GmailPermissionPolicy.GMAIL_READ_ONLY in status.missingScopes
            ) {
                GmailReadFailureCode.MISSING_SCOPE
            } else {
                GmailReadFailureCode.PERMISSION_DENIED
            }
            throw GmailReadFailure(code)
        }
        val token = try { tokens.accessToken(GmailPermissionPolicy.GMAIL_READ_ONLY) }
        catch (cancelled: CancellationException) { throw cancelled }
        catch (_: Exception) { throw GmailReadFailure(GmailReadFailureCode.PERMISSION_DENIED) }
        if (token == null) throw GmailReadFailure(GmailReadFailureCode.PERMISSION_DENIED)
        try {
            currentCoroutineContext().ensureActive()
            val request = token.useToken { value ->
                Request.Builder().url(url).header("Authorization", "Bearer $value")
                    .header("Accept", "application/json").get().build()
            }
            currentCoroutineContext().ensureActive()
            execute(safeClient.newCall(request)).use { response ->
                if (response.code !in 200..299) throw responseFailure(response)
                val body = response.body ?: throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
                val bytes = try { body.byteStream().use { readBoundedResponse(it, maximumBytes) } }
                catch (cancelled: CancellationException) { throw cancelled }
                catch (_: GmailResponseTooLargeException) { throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE) }
                catch (_: java.io.InterruptedIOException) { throw GmailReadFailure(GmailReadFailureCode.TIMEOUT) }
                catch (_: IOException) { throw GmailReadFailure(GmailReadFailureCode.UNAVAILABLE) }
                return bytes.toString(Charsets.UTF_8) to bytes.size
            }
        } catch (cancelled: CancellationException) {
            throw cancelled
        } catch (_: IllegalStateException) {
            throw GmailReadFailure(GmailReadFailureCode.PERMISSION_DENIED)
        } finally {
            token.close()
        }
    }

    private suspend fun execute(call: Call): Response = suspendCancellableCoroutine { continuation ->
        continuation.invokeOnCancellation { call.cancel() }
        call.enqueue(object : Callback {
            override fun onFailure(call: Call, error: IOException) {
                if (!continuation.isActive) return
                val code = when {
                    error is java.io.InterruptedIOException -> GmailReadFailureCode.TIMEOUT
                    call.isCanceled() -> GmailReadFailureCode.CANCELLED
                    else -> GmailReadFailureCode.UNAVAILABLE
                }
                runCatching { continuation.resumeWith(Result.failure(GmailReadFailure(code))) }
            }

            override fun onResponse(call: Call, response: Response) {
                if (continuation.isActive) runCatching { continuation.resume(response) }.onFailure { response.close() }
                else response.close()
            }
        })
    }

    private fun responseFailure(response: Response): GmailReadFailure {
        val code = when (response.code) {
            401 -> GmailReadFailureCode.PERMISSION_DENIED
            403 -> when (errorReason(response)) {
                "insufficientPermissions", "forbidden" -> GmailReadFailureCode.MISSING_SCOPE
                "rateLimitExceeded", "userRateLimitExceeded", "quotaExceeded" -> GmailReadFailureCode.RATE_LIMITED
                else -> GmailReadFailureCode.PERMISSION_DENIED
            }
            404 -> GmailReadFailureCode.NOT_FOUND
            429 -> GmailReadFailureCode.RATE_LIMITED
            in 500..599 -> GmailReadFailureCode.UNAVAILABLE
            else -> GmailReadFailureCode.INVALID_RESPONSE
        }
        return GmailReadFailure(code)
    }

    private fun errorReason(response: Response): String? = try {
        val bytes = response.peekBody(MAX_ERROR_BYTES.toLong()).bytes()
        JsonParser.parseString(bytes.toString(Charsets.UTF_8)).asJsonObject
            .getAsJsonObject("error")?.getAsJsonArray("errors")?.firstOrNull()?.asJsonObject
            ?.get("reason")?.asString
    } catch (_: Exception) { null }

    private fun userCollection(resource: String): HttpUrl = baseUrl.newBuilder()
        .addPathSegment("users").addPathSegment("me").addPathSegment(resource).build()

    private fun validatePageToken(token: String) {
        if (token.isBlank() || token.length > MAX_PAGE_TOKEN_CHARS) invalid()
    }

    private fun com.google.gson.JsonObject.stringOrNull(name: String): String? = try {
        get(name)?.takeUnless { it.isJsonNull }?.asString
    } catch (_: Exception) { invalid() }

    private fun checkedAdd(left: Int, right: Int): Int = try { Math.addExact(left, right) }
    catch (_: ArithmeticException) { invalid() }

    private fun invalid(): Nothing = throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)

    companion object {
        private const val GMAIL_API_HOST = "gmail.googleapis.com"
        private val TEST_HOSTS = setOf("127.0.0.1", "localhost", "[::1]")
        private val GMAIL_API_BASE = "https://gmail.googleapis.com/gmail/v1/".toHttpUrl()
        private const val MAX_QUERY_CHARS = 256
        private const val MAX_PAGE_SIZE = 100
        private const val MAX_PAGE_TOKEN_CHARS = 2_048
        private const val MAX_LIST_RESPONSE_BYTES = 512 * 1024
        private const val MAX_MESSAGE_RESPONSE_BYTES = GmailMimeParser.MAX_JSON_BYTES
        private const val MAX_THREAD_RESPONSE_BYTES = 8 * 1024 * 1024
        private const val MAX_PAGE_RESPONSE_BYTES = 8 * 1024 * 1024
        private const val MAX_THREAD_MESSAGES = 100
        private const val MAX_ERROR_BYTES = 16 * 1024

        private fun productionClient() = OkHttpClient.Builder()
            .connectTimeout(5, TimeUnit.SECONDS).readTimeout(10, TimeUnit.SECONDS)
            .callTimeout(30, TimeUnit.SECONDS).followRedirects(false).followSslRedirects(false)
            .retryOnConnectionFailure(false).build()
    }
}

private class GmailResponseTooLargeException : IOException("Gmail response exceeds the configured byte limit.")

/** Reads bounded chunks and stops after at most one byte beyond the accepted limit. */
private fun readBoundedResponse(input: InputStream, maximumBytes: Int): ByteArray {
    require(maximumBytes >= 0)
    val output = ByteArrayOutputStream(minOf(maximumBytes, 8 * 1024))
    val chunk = ByteArray(8 * 1024)
    while (true) {
        val remainingWithSentinel = maximumBytes.toLong() + 1L - output.size()
        if (remainingWithSentinel <= 0) throw GmailResponseTooLargeException()
        val count = input.read(chunk, 0, minOf(chunk.size.toLong(), remainingWithSentinel).toInt())
        if (count < 0) return output.toByteArray()
        if (count == 0) {
            val one = input.read()
            if (one < 0) return output.toByteArray()
            output.write(one)
        } else output.write(chunk, 0, count)
        if (output.size() > maximumBytes) throw GmailResponseTooLargeException()
    }
}
