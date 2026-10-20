package app.serea.android.gmail

import kotlinx.coroutines.CancellationException

enum class FakeGmailFailure { ACCESS_DENIED, RATE_LIMITED, TIMEOUT, CANCELLED, UNAVAILABLE }

/** Deterministic fictional mailbox. No real account, OAuth client, HTTP stack, or network code is used. */
class FakeGmailProvider(
    messages: List<GmailMessage> = fixtures.messages,
    private val inboxIds: Set<GmailMessageId> = fixtures.inboxIds,
    private val authorization: GmailAuthorizationStatus = GmailAuthorizationStatus(GmailAuthorizationState.MOCK_ONLY),
    private val failure: FakeGmailFailure? = null,
    private val failAtPageToken: String? = null,
    private val emptyPageAt: Int? = null,
    private val duplicateFirstMessageOnSecondPage: Boolean = false,
) : GmailReadPort, GmailAuthorizationPort {
    private val messages = messages.toList()

    override suspend fun status(): GmailAuthorizationStatus = authorization

    override suspend fun listInbox(pageToken: String?, pageSize: Int): GmailMessagePage {
        checkFailure(pageToken)
        requirePageSize(pageSize)
        val inbox = messages.filter { it.id in inboxIds }
        return page(inbox, pageToken, pageSize)
    }

    override suspend fun search(query: String, pageToken: String?, pageSize: Int): GmailMessagePage {
        checkFailure(pageToken)
        require(query.isNotBlank()) { "Search query must not be blank." }
        requirePageSize(pageSize)
        val needle = query.trim().lowercase()
        val matching = messages.filter { message ->
            listOfNotNull(message.from, message.subject, message.snippet, message.plainTextBody, message.htmlTextBody)
                .any { it.lowercase().contains(needle) }
        }
        return page(matching, pageToken, pageSize)
    }

    override suspend fun getMessage(id: GmailMessageId): GmailMessage {
        checkFailure()
        return messages.firstOrNull { it.id == id } ?: throw GmailReadFailure(GmailReadFailureCode.NOT_FOUND)
    }

    override suspend fun getThread(id: GmailThreadId): GmailThread {
        checkFailure()
        val threadMessages = messages.filter { it.threadId == id }
        if (threadMessages.isEmpty()) throw GmailReadFailure(GmailReadFailureCode.NOT_FOUND)
        return GmailThread(id, threadMessages)
    }

    private fun page(source: List<GmailMessage>, token: String?, pageSize: Int): GmailMessagePage {
        val start = token?.toIntOrNull() ?: 0
        if (token != null && (start.toString() != token || start < 0 || start > source.size)) {
            throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
        }
        val pageIndex = if (start == 0) 0 else start / pageSize
        if (emptyPageAt == pageIndex) return GmailMessagePage(emptyList(), if (start < source.size) (start + pageSize).toString() else null)
        val end = (start + pageSize).coerceAtMost(source.size)
        val selected = source.subList(start.coerceAtMost(source.size), end).toMutableList()
        if (duplicateFirstMessageOnSecondPage && pageIndex == 1 && source.isNotEmpty()) selected.add(0, source.first())
        return GmailMessagePage(selected, if (end < source.size) end.toString() else null)
    }

    private fun requirePageSize(pageSize: Int) {
        require(pageSize in 1..100) { "Page size is out of range." }
    }

    private fun checkFailure(pageToken: String? = null) {
        if (failAtPageToken != null && pageToken == failAtPageToken) {
            throw GmailReadFailure(GmailReadFailureCode.PARTIAL_FAILURE)
        }
        when (failure) {
            null -> Unit
            FakeGmailFailure.ACCESS_DENIED -> throw GmailReadFailure(GmailReadFailureCode.PERMISSION_DENIED)
            FakeGmailFailure.RATE_LIMITED -> throw GmailReadFailure(GmailReadFailureCode.RATE_LIMITED)
            FakeGmailFailure.TIMEOUT -> throw GmailReadFailure(GmailReadFailureCode.TIMEOUT)
            FakeGmailFailure.CANCELLED -> throw CancellationException("Fake Gmail operation cancelled.")
            FakeGmailFailure.UNAVAILABLE -> throw GmailReadFailure(GmailReadFailureCode.UNAVAILABLE)
        }
    }

    companion object {
        val fixtures: FakeGmailFixtures = FakeGmailFixtures.create()
    }
}

data class FakeGmailFixtures(val messages: List<GmailMessage>, val inboxIds: Set<GmailMessageId>) {
    companion object {
        fun create(): FakeGmailFixtures {
            fun message(id: String, thread: String, from: String, subject: String, snippet: String, body: String) = GmailMessage(
                id = GmailMessageId(id), threadId = GmailThreadId(thread), from = from,
                to = listOf("alex@example.invalid"), subject = subject, internalDateMillis = 1_792_000_000_000,
                snippet = snippet, plainTextBody = body, htmlTextBody = null, source = GmailMessageSource.MOCK,
            )
            val messages = buildList {
                add(message("fake-msg-001", "fake-thread-001", "sam@example.invalid", "Project Finch schedule", "Fictional schedule note", "The made-up Finch review starts at noon."))
                add(message("fake-msg-002", "fake-thread-001", "alex@example.invalid", "Re: Project Finch schedule", "Fictional reply", "Thanks, this is a fictional reply."))
                add(message("fake-msg-003", "fake-thread-002", "lee@example.invalid", "Garden plan", "Invented garden note", "The imaginary basil bed is ready."))
                for (index in 4..9) add(message(
                    "fake-msg-%03d".format(index), "fake-thread-%03d".format(index), "writer$index@example.invalid",
                    "Fictional note $index", "Synthetic fixture $index", "This is fictional body $index.",
                ))
            }
            return FakeGmailFixtures(messages, messages.take(7).mapTo(linkedSetOf()) { it.id })
        }
    }
}
