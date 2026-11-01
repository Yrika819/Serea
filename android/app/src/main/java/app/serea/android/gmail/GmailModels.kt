package app.serea.android.gmail

import app.serea.android.DataClassification
import app.serea.android.DataOrigin

@JvmInline
value class GmailMessageId(val value: String) {
    init { require(value.length in 1..512 && value.matches(Regex("[A-Za-z0-9_-]+"))) { "Invalid Gmail message ID." } }
}

@JvmInline
value class GmailThreadId(val value: String) {
    init { require(value.length in 1..512 && value.matches(Regex("[A-Za-z0-9_-]+"))) { "Invalid Gmail thread ID." } }
}

enum class GmailMessageSource { GMAIL_API, MOCK }
enum class GmailAuthorizationState {
    NOT_CONFIGURED, NOT_CONNECTED, AUTHORIZATION_REQUIRED, AUTHORIZED, DENIED,
    REVOKED, EXPIRED, ERROR, MOCK_ONLY,
}

data class GmailAuthorizationStatus(
    val state: GmailAuthorizationState,
    val missingScopes: Set<String> = emptySet(),
    val detailCode: String? = null,
) {
    init { require(detailCode == null || detailCode.matches(Regex("[A-Z0-9_]{1,64}"))) { "Authorization detail must be a stable code." } }
}

data class GmailMessage(
    val id: GmailMessageId,
    val threadId: GmailThreadId,
    val from: String?,
    val to: List<String>,
    val subject: String?,
    val internalDateMillis: Long?,
    val snippet: String?,
    val plainTextBody: String?,
    val htmlTextBody: String?,
    val source: GmailMessageSource,
    val dataOrigin: DataOrigin = DataOrigin.GMAIL,
    val classification: DataClassification = DataClassification.PRIVATE,
) {
    init {
        require(dataOrigin == DataOrigin.GMAIL) { "Gmail messages must retain GMAIL origin." }
        require(classification != DataClassification.PUBLIC) { "Gmail data cannot be classified PUBLIC automatically." }
    }
}

data class GmailThread(val id: GmailThreadId, val messages: List<GmailMessage>) {
    init { require(messages.all { it.threadId == id && it.dataOrigin == DataOrigin.GMAIL }) }
}

data class GmailMessagePage(val messages: List<GmailMessage>, val nextPageToken: String? = null)

enum class GmailReadFailureCode {
    NOT_FOUND, PERMISSION_DENIED, MISSING_SCOPE, RATE_LIMITED, TIMEOUT, CANCELLED,
    INVALID_RESPONSE, TOO_MANY_PAGES, TOO_MANY_MESSAGES, PARTIAL_FAILURE, UNAVAILABLE,
}

/** Deliberately contains only a stable code. Provider responses and mail fields are never copied to messages. */
class GmailReadFailure(val code: GmailReadFailureCode) : RuntimeException("Gmail read failed (${code.name}).")
