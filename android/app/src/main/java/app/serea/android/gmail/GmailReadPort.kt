package app.serea.android.gmail

/** Read-only adapter seam. There are no methods that change message or mailbox state. */
interface GmailReadPort {
    suspend fun listInbox(pageToken: String? = null, pageSize: Int = 50): GmailMessagePage
    suspend fun search(query: String, pageToken: String? = null, pageSize: Int = 50): GmailMessagePage
    suspend fun getMessage(id: GmailMessageId): GmailMessage
    suspend fun getThread(id: GmailThreadId): GmailThread
}

/** Reports authorization state only; OAuth and credential handling belong to a separate adapter. */
interface GmailAuthorizationPort {
    suspend fun status(): GmailAuthorizationStatus
}

object GmailPermissionPolicy {
    const val GMAIL_READ_ONLY = "https://www.googleapis.com/auth/gmail.readonly"

    fun requiredScopes(): Set<String> = setOf(GMAIL_READ_ONLY)
    fun missingScopes(grantedScopes: Set<String>): Set<String> = requiredScopes() - grantedScopes
    /** Contract for a future real HTTP adapter: mock authorization is never valid there. */
    fun mayRead(status: GmailAuthorizationStatus): Boolean =
        status.state == GmailAuthorizationState.AUTHORIZED && status.missingScopes.isEmpty()

    /** MOCK_ONLY is a fixture capability, never a grant for a real Gmail adapter. */
    fun mayReadFake(status: GmailAuthorizationStatus): Boolean =
        status.state == GmailAuthorizationState.MOCK_ONLY && status.missingScopes.isEmpty()
}

data class GmailReadLimits(val maximumPages: Int = 20, val maximumMessages: Int = 1_000) {
    init { require(maximumPages in 1..100 && maximumMessages in 1..10_000) }
}

/** Collects every page or fails without returning a misleading partial result. */
class GmailSearchService(private val port: GmailReadPort, private val limits: GmailReadLimits = GmailReadLimits()) {
    suspend fun searchAll(query: String, pageSize: Int = 50): List<GmailMessage> = collect { token ->
        port.search(query, token, pageSize)
    }

    suspend fun inboxAll(pageSize: Int = 50): List<GmailMessage> = collect { token ->
        port.listInbox(token, pageSize)
    }

    private suspend fun collect(fetch: suspend (String?) -> GmailMessagePage): List<GmailMessage> {
        val seenTokens = HashSet<String>()
        val byId = LinkedHashMap<GmailMessageId, GmailMessage>()
        var token: String? = null
        var pageCount = 0
        do {
            if (++pageCount > limits.maximumPages) throw GmailReadFailure(GmailReadFailureCode.TOO_MANY_PAGES)
            if (token != null && !seenTokens.add(token)) throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
            val page = try { fetch(token) } catch (failure: kotlinx.coroutines.CancellationException) {
                throw failure
            } catch (failure: GmailReadFailure) {
                throw failure
            } catch (_: Exception) {
                throw GmailReadFailure(GmailReadFailureCode.PARTIAL_FAILURE)
            }
            val uniquePage = LinkedHashMap<GmailMessageId, GmailMessage>()
            page.messages.forEach { message ->
                if (message.dataOrigin != app.serea.android.DataOrigin.GMAIL || message.classification == app.serea.android.DataClassification.PUBLIC) {
                    throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
                }
                val samePage = uniquePage.putIfAbsent(message.id, message)
                if (samePage != null && samePage != message) throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
                val previous = byId[message.id]
                if (previous != null && previous != message) throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
            }
            val newUniqueCount = uniquePage.keys.count { it !in byId }
            if (newUniqueCount > limits.maximumMessages - byId.size) throw GmailReadFailure(GmailReadFailureCode.TOO_MANY_MESSAGES)
            uniquePage.forEach { (id, message) ->
                byId.putIfAbsent(id, message)
            }
            token = page.nextPageToken?.also {
                if (it.isBlank() || it.length > 2048) throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
            }
        } while (token != null)
        return byId.values.toList()
    }
}
