package app.serea.android.gmail.ui

import app.serea.android.gmail.FakeGmailProvider
import app.serea.android.gmail.GmailAuthorizationPort
import app.serea.android.gmail.GmailAuthorizationState
import app.serea.android.gmail.GmailAuthorizationStatus
import app.serea.android.gmail.GmailMessage
import app.serea.android.gmail.GmailMessageId
import app.serea.android.gmail.GmailMessagePage
import app.serea.android.gmail.GmailReadPort
import app.serea.android.gmail.GmailThread
import app.serea.android.gmail.GmailThreadId

/** UI-only port fake. It has no transport, HTTP client, credentials, or network path. */
internal class GmailUiTestFake(
    private val allMessages: List<GmailMessage> = FakeGmailProvider.fixtures.messages,
    private val inboxIds: Set<GmailMessageId> = FakeGmailProvider.fixtures.inboxIds,
    var authorization: GmailAuthorizationStatus = GmailAuthorizationStatus(GmailAuthorizationState.MOCK_ONLY),
) : GmailReadPort, GmailAuthorizationPort {
    val inboxTokens = mutableListOf<String?>()
    val searchQueries = mutableListOf<String>()
    val searchTokens = mutableListOf<String?>()
    val messageIds = mutableListOf<GmailMessageId>()
    val threadIds = mutableListOf<GmailThreadId>()

    var inboxHandler: suspend (String?, Int) -> GmailMessagePage = { token, pageSize ->
        page(allMessages.filter { it.id in inboxIds }, token, pageSize)
    }
    var searchHandler: suspend (String, String?, Int) -> GmailMessagePage = { query, token, pageSize ->
        val needle = query.lowercase()
        page(allMessages.filter { message ->
            listOfNotNull(message.from, message.subject, message.snippet, message.plainTextBody, message.htmlTextBody)
                .any { it.lowercase().contains(needle) }
        }, token, pageSize)
    }
    var messageHandler: suspend (GmailMessageId) -> GmailMessage = { id ->
        allMessages.firstOrNull { it.id == id } ?: throw app.serea.android.gmail.GmailReadFailure(
            app.serea.android.gmail.GmailReadFailureCode.NOT_FOUND,
        )
    }
    var threadHandler: suspend (GmailThreadId) -> GmailThread = { id ->
        val messages = allMessages.filter { it.threadId == id }
        if (messages.isEmpty()) throw app.serea.android.gmail.GmailReadFailure(app.serea.android.gmail.GmailReadFailureCode.NOT_FOUND)
        GmailThread(id, messages)
    }

    override suspend fun status(): GmailAuthorizationStatus = authorization

    override suspend fun listInbox(pageToken: String?, pageSize: Int): GmailMessagePage {
        inboxTokens += pageToken
        return inboxHandler(pageToken, pageSize)
    }

    override suspend fun search(query: String, pageToken: String?, pageSize: Int): GmailMessagePage {
        searchQueries += query
        searchTokens += pageToken
        return searchHandler(query, pageToken, pageSize)
    }

    override suspend fun getMessage(id: GmailMessageId): GmailMessage {
        messageIds += id
        return messageHandler(id)
    }

    override suspend fun getThread(id: GmailThreadId): GmailThread {
        threadIds += id
        return threadHandler(id)
    }

    private fun page(messages: List<GmailMessage>, token: String?, pageSize: Int): GmailMessagePage {
        val start = token?.toIntOrNull() ?: 0
        if (token != null && (start.toString() != token || start < 0 || start > messages.size)) {
            throw app.serea.android.gmail.GmailReadFailure(app.serea.android.gmail.GmailReadFailureCode.INVALID_RESPONSE)
        }
        val end = (start + pageSize).coerceAtMost(messages.size)
        return GmailMessagePage(messages.subList(start.coerceAtMost(messages.size), end), end.toString().takeIf { end < messages.size })
    }
}
