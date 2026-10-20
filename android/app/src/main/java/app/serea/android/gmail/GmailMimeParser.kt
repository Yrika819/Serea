package app.serea.android.gmail

import com.google.gson.JsonElement
import com.google.gson.JsonObject
import com.google.gson.JsonParser
import com.google.gson.Strictness
import com.google.gson.stream.JsonReader
import java.io.StringReader
import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.nio.charset.Charset
import java.nio.charset.StandardCharsets
import java.util.Base64
import java.util.Locale

/** Parses the bounded, read-only subset of Gmail messages.get(format=full). */
object GmailMimeParser {
    const val MAX_JSON_BYTES = 2 * 1024 * 1024
    const val MAX_DECODED_BODY_BYTES = 1024 * 1024
    const val MAX_PART_DEPTH = 16
    private const val MAX_JSON_NESTING = 64
    private const val MAX_HEADER_COUNT = 200

    fun parseMessage(json: String, source: GmailMessageSource = GmailMessageSource.GMAIL_API): GmailMessage {
        if (json.toByteArray(StandardCharsets.UTF_8).size > MAX_JSON_BYTES) invalid()
        validateJsonNesting(json)
        val root = try {
            val reader = JsonReader(StringReader(json)).apply { setStrictness(Strictness.STRICT) }
            JsonParser.parseReader(reader).takeIf(JsonElement::isJsonObject)?.asJsonObject ?: invalid()
        } catch (_: Exception) { invalid() }
        try {
            val id = GmailMessageId(requiredString(root, "id"))
            val threadId = GmailThreadId(requiredString(root, "threadId"))
            val internalDate = optionalString(root, "internalDate")?.toLongOrNull()?.also { if (it < 0) invalid() }
            val payload = optionalObject(root, "payload")
            val headers = readHeaders(payload)
            var decodedBytes = 0
            val body = if (payload != null) collectBody(payload, 0) { bytes ->
                if (bytes > MAX_DECODED_BODY_BYTES - decodedBytes) invalid()
                decodedBytes += bytes
            } else BodyContent()
            return GmailMessage(
                id = id,
                threadId = threadId,
                from = headers["from"],
                to = headers["to"].orEmpty().splitAddressHeader(),
                subject = headers["subject"],
                internalDateMillis = internalDate,
                snippet = optionalString(root, "snippet"),
                plainTextBody = body.plain,
                htmlTextBody = body.html?.let(::htmlToSafeText),
                source = source,
            )
        } catch (_: GmailReadFailure) { throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE) }
        catch (_: Exception) { invalid() }
    }

    private data class BodyContent(val plain: String? = null, val html: String? = null)

    private fun collectBody(
        part: JsonObject,
        depth: Int,
        onDecoded: (Int) -> Unit,
    ): BodyContent {
        if (depth > MAX_PART_DEPTH) invalid()
        val contentType = contentType(part)
        val mimeType = contentType?.substringBefore(';')?.trim()?.lowercase(Locale.ROOT)
        val filename = optionalString(part, "filename")
        if (!filename.isNullOrEmpty()) return BodyContent() // Attachments are never downloaded or interpreted.
        val body = optionalObject(part, "body")
        val attachmentId = body?.let { optionalString(it, "attachmentId") }
        if (mimeType in setOf("text/plain", "text/html") && !attachmentId.isNullOrBlank() &&
            body?.let { optionalString(it, "data") } == null
        ) {
            // A text body available only through attachmentId is not a complete inline body.
            invalid()
        }
        val partsElement = part.get("parts")
        if (partsElement != null && !partsElement.isJsonNull) {
            if (!partsElement.isJsonArray) invalid()
            val children = partsElement.asJsonArray.map { child ->
                if (!child.isJsonObject) invalid()
                collectBody(child.asJsonObject, depth + 1, onDecoded)
            }
            if (children.isEmpty()) invalid()
            return when {
                mimeType == "multipart/alternative" -> {
                    // RFC 2046 alternatives are equivalent representations. Prefer plain text;
                    // only fall back to HTML when no plain text representation exists.
                    children.firstOrNull { it.plain != null }?.let { BodyContent(plain = it.plain) }
                        ?: children.firstOrNull { it.html != null }?.let { BodyContent(html = it.html) }
                        ?: BodyContent()
                }
                mimeType == "multipart/mixed" -> BodyContent(
                    plain = children.mapNotNull { it.plain }.takeIf { it.isNotEmpty() }?.joinToString("\n\n"),
                    html = children.mapNotNull { it.html }.takeIf { it.isNotEmpty() }?.joinToString("\n\n"),
                )
                else -> invalid() // Includes multipart/related and external-resource containers.
            }
        }
        if (mimeType?.startsWith("multipart/") == true) invalid()
        if (mimeType !in setOf("text/plain", "text/html")) {
            // Unknown inline leaf bodies cannot be represented safely as complete text.
            if (body?.let { optionalString(it, "data") } != null) invalid()
            return BodyContent()
        }
        body?.get("size")?.let { value ->
            if (!value.isJsonPrimitive || !value.asJsonPrimitive.isNumber) invalid()
            val rawSize = value.asString
            if (!rawSize.matches(Regex("0|[1-9][0-9]*"))) invalid()
            val declaredSize = rawSize.toLongOrNull() ?: invalid()
            if (declaredSize < 0 || declaredSize > MAX_DECODED_BODY_BYTES) invalid()
        }
        val encoded = body?.let { optionalString(it, "data") } ?: return BodyContent()
        val bytes = decodeBase64Url(encoded)
        body?.get("size")?.let { if (it.asString.toLongOrNull() != bytes.size.toLong()) invalid() }
        onDecoded(bytes.size)
        val charset = parseCharset(contentType)
        val text = decodeText(bytes, charset)
        return if (mimeType == "text/plain") BodyContent(plain = text) else BodyContent(html = text)
    }

    private fun decodeBase64Url(value: String): ByteArray {
        if (value.any(Char::isWhitespace) || !value.matches(Regex("[A-Za-z0-9_-]*={0,2}"))) invalid()
        val unpadded = value.trimEnd('=')
        if (unpadded.length % 4 == 1) invalid()
        val normalized = unpadded + "=".repeat((4 - unpadded.length % 4) % 4)
        return try { Base64.getUrlDecoder().decode(normalized).also { if (it.size > MAX_DECODED_BODY_BYTES) invalid() } }
        catch (_: IllegalArgumentException) { invalid() }
    }

    private fun parseCharset(contentType: String?): Charset {
        val raw = contentType?.split(';')?.drop(1)?.firstNotNullOfOrNull { parameter ->
            val pair = parameter.trim().split('=', limit = 2)
            if (pair.size == 2 && pair[0].trim().equals("charset", ignoreCase = true)) pair[1].trim().trim('"', '\'') else null
        } ?: "UTF-8"
        return try { Charset.forName(raw) } catch (_: Exception) { invalid() }
    }

    private fun decodeText(bytes: ByteArray, charset: Charset): String = try {
        charset.newDecoder().onMalformedInput(CodingErrorAction.REPORT).onUnmappableCharacter(CodingErrorAction.REPORT)
            .decode(ByteBuffer.wrap(bytes)).toString()
    } catch (_: CharacterCodingException) { invalid() }

    private fun readHeaders(payload: JsonObject?): Map<String, String> {
        val element = payload?.get("headers") ?: return emptyMap()
        if (!element.isJsonArray || element.asJsonArray.size() > MAX_HEADER_COUNT) invalid()
        val result = linkedMapOf<String, String>()
        element.asJsonArray.forEach { item ->
            if (!item.isJsonObject) invalid()
            val name = optionalString(item.asJsonObject, "name")?.lowercase(Locale.ROOT) ?: return@forEach
            val value = optionalString(item.asJsonObject, "value") ?: return@forEach
            if (name in setOf("from", "to", "subject")) {
                if (value.length > 16_384) invalid()
                result.putIfAbsent(name, value)
            }
        }
        return result
    }

    private fun contentType(part: JsonObject): String? {
        val headerArray = part.get("headers") ?: return optionalString(part, "mimeType")
        if (!headerArray.isJsonArray || headerArray.asJsonArray.size() > MAX_HEADER_COUNT) invalid()
        val header = headerArray.asJsonArray.firstNotNullOfOrNull { item ->
            if (!item.isJsonObject) invalid()
            val obj = item.asJsonObject
            if (optionalString(obj, "name").equals("content-type", ignoreCase = true)) optionalString(obj, "value") else null
        }
        return header ?: optionalString(part, "mimeType")
    }

    private fun String?.splitAddressHeader(): List<String> = this
        ?.split(',')?.map(String::trim)?.filter(String::isNotEmpty)?.take(100).orEmpty()

    private fun htmlToSafeText(html: String): String {
        val out = StringBuilder(minOf(html.length, MAX_DECODED_BODY_BYTES))
        var index = 0
        var hiddenTag: String? = null
        while (index < html.length && out.length < MAX_DECODED_BODY_BYTES) {
            if (html.startsWith("<!--", index)) {
                val end = html.indexOf("-->", index + 4)
                index = if (end < 0) html.length else end + 3
                continue
            }
            if (html[index] == '<') {
                val close = html.indexOf('>', index + 1)
                if (close < 0) { index++; continue }
                val rawTag = html.substring(index + 1, close).trim()
                val name = rawTag.trimStart('/').takeWhile { it.isLetterOrDigit() }.lowercase(Locale.ROOT)
                val closing = rawTag.startsWith('/')
                if (name in setOf("script", "style", "noscript")) {
                    if (!closing && !rawTag.endsWith('/')) hiddenTag = name
                    else if (closing && hiddenTag == name) hiddenTag = null
                } else if (hiddenTag == null && (name in setOf("br", "p", "div", "li", "tr", "h1", "h2", "h3", "hr"))) {
                    out.append('\n')
                }
                index = close + 1
                continue
            }
            if (hiddenTag != null) { index++; continue }
            if (html[index] == '&') {
                val semicolon = html.indexOf(';', index + 1).takeIf { it in index + 2..minOf(index + 12, html.lastIndex) }
                if (semicolon != null) {
                    val entity = html.substring(index + 1, semicolon)
                    out.append(decodeEntity(entity))
                    index = semicolon + 1
                    continue
                }
            }
            out.append(html[index++])
        }
        return out.toString().replace(Regex("[\\t\\x0B\\f\\r ]+"), " ")
            .replace(Regex(" *\\n *"), "\n").replace(Regex("\\n{3,}"), "\n\n").trim()
    }

    private fun decodeEntity(entity: String): String {
        val numeric = when {
            entity.startsWith("#x", true) -> entity.substring(2).toIntOrNull(16)
            entity.startsWith('#') -> entity.substring(1).toIntOrNull()
            else -> null
        }
        if (numeric != null) return String(Character.toChars(numeric.coerceIn(0, 0x10FFFF).takeUnless { it in 0xD800..0xDFFF } ?: 0xFFFD))
        return when (entity.lowercase(Locale.ROOT)) {
            "amp" -> "&"; "lt" -> "<"; "gt" -> ">"; "quot" -> "\""; "apos", "#39" -> "'"; "nbsp" -> " "
            else -> "&$entity;"
        }
    }

    /** Stops pathological JSON nesting before Gson builds a recursive object tree. */
    private fun validateJsonNesting(json: String) {
        var depth = 0
        var inString = false
        var escaped = false
        for (character in json) {
            if (inString) {
                if (escaped) escaped = false
                else if (character == '\\') escaped = true
                else if (character == '"') inString = false
                continue
            }
            when (character) {
                '"' -> inString = true
                '{', '[' -> if (++depth > MAX_JSON_NESTING) invalid()
                '}', ']' -> if (--depth < 0) invalid()
            }
        }
        if (depth != 0 || inString) invalid()
    }

    private fun requiredString(obj: JsonObject, key: String): String = optionalString(obj, key)?.takeIf(String::isNotBlank) ?: invalid()

    private fun optionalString(obj: JsonObject, key: String): String? {
        val element = obj.get(key) ?: return null
        if (element.isJsonNull) return null
        if (!element.isJsonPrimitive || !element.asJsonPrimitive.isString) invalid()
        return element.asString
    }

    private fun optionalObject(obj: JsonObject, key: String): JsonObject? {
        val element = obj.get(key) ?: return null
        if (element.isJsonNull) return null
        if (!element.isJsonObject) invalid()
        return element.asJsonObject
    }

    private fun invalid(): Nothing = throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
}
