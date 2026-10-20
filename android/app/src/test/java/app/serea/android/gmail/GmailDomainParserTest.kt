package app.serea.android.gmail

import app.serea.android.ChatEgressGate
import app.serea.android.DataClassification
import app.serea.android.DataOrigin
import app.serea.android.EgressContract
import app.serea.android.EgressContractDecision
import app.serea.android.PayloadPreparation
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.runBlocking
import org.junit.Assert.*
import org.junit.Test
import java.nio.charset.Charset
import java.util.Base64

class GmailSecurityPortsTest {
    @Test fun permissionIsReadOnlyAndMockOnlyIsNotAuthorized() = runBlocking {
        val fake = FakeGmailProvider()
        assertEquals(GmailAuthorizationState.MOCK_ONLY, fake.status().state)
        assertTrue(GmailPermissionPolicy.mayReadFake(fake.status()))
        assertFalse(GmailPermissionPolicy.mayRead(fake.status()))
        assertEquals(setOf(GmailPermissionPolicy.GMAIL_READ_ONLY), GmailPermissionPolicy.requiredScopes())
        assertEquals(setOf(GmailPermissionPolicy.GMAIL_READ_ONLY), GmailPermissionPolicy.missingScopes(emptySet()))
        for (state in GmailAuthorizationState.entries) {
            val status = GmailAuthorizationStatus(state)
            assertEquals(state == GmailAuthorizationState.AUTHORIZED, GmailPermissionPolicy.mayRead(status))
            assertEquals(state == GmailAuthorizationState.MOCK_ONLY, GmailPermissionPolicy.mayReadFake(status))
        }
        assertFalse(GmailPermissionPolicy.mayRead(GmailAuthorizationStatus(GmailAuthorizationState.AUTHORIZED, setOf(GmailPermissionPolicy.GMAIL_READ_ONLY))))
    }

    @Test fun fakeReadsRequireFakeOnlyMockAuthorizationAndRejectRevokedExpiredAndMissingScopes() = runBlocking {
        val operations: List<suspend (FakeGmailProvider) -> Unit> = listOf(
            { it.listInbox() }, { it.search("Finch") },
            { it.getMessage(GmailMessageId("fake-msg-001")) },
            { it.getThread(GmailThreadId("fake-thread-001")) },
        )
        for (state in listOf(GmailAuthorizationState.DENIED, GmailAuthorizationState.REVOKED, GmailAuthorizationState.EXPIRED, GmailAuthorizationState.AUTHORIZED)) {
            val provider = FakeGmailProvider(authorization = GmailAuthorizationStatus(state))
            operations.forEach { operation -> assertEquals(GmailReadFailureCode.PERMISSION_DENIED, failure { operation(provider) }.code) }
        }
        val missing = GmailAuthorizationStatus(GmailAuthorizationState.MOCK_ONLY, setOf(GmailPermissionPolicy.GMAIL_READ_ONLY))
        val provider = FakeGmailProvider(authorization = missing)
        operations.forEach { operation -> assertEquals(GmailReadFailureCode.MISSING_SCOPE, failure { operation(provider) }.code) }
        val realStatus = GmailAuthorizationStatus(GmailAuthorizationState.AUTHORIZED)
        assertTrue(GmailPermissionPolicy.mayRead(realStatus))
        assertFalse(GmailPermissionPolicy.mayRead(GmailAuthorizationStatus(GmailAuthorizationState.AUTHORIZED, setOf("gmail.modify"))))
    }

    @Test fun portHasOnlyReadOperationsAndCancellationPropagates() = runBlocking {
        val portMethods = GmailReadPort::class.java.declaredMethods.map { it.name.substringBefore('-') }.toSet()
        assertTrue(portMethods.containsAll(setOf("listInbox", "search", "getMessage", "getThread")))
        assertFalse(portMethods.any { it.contains("send", ignoreCase = true) || it.contains("delete", ignoreCase = true) || it.contains("modify", ignoreCase = true) })
        try {
            FakeGmailProvider(failure = FakeGmailFailure.CANCELLED).listInbox()
            fail("Expected cancellation")
        } catch (_: CancellationException) { }
    }

    @Test fun fakeInboxSearchThreadAndPaginationAreFictionalAndDeterministic() = runBlocking {
        val fake = FakeGmailProvider()
        assertEquals(7, fake.listInbox(pageSize = 50).messages.size)
        assertEquals(1, fake.search("Finch", pageSize = 1).messages.size)
        assertEquals(2, fake.getThread(GmailThreadId("fake-thread-001")).messages.size)
        val search = GmailSearchService(fake)
        assertEquals(2, search.searchAll("finch", pageSize = 1).size)
        assertEquals(GmailMessageId("fake-msg-001"), fake.listInbox(pageSize = 1).messages.single().id)
        assertEquals(7, search.inboxAll(pageSize = 2).size)
    }

    @Test fun fakeSupportsDuplicateIdsEmptyPagesAndTokenValidation() = runBlocking {
        val duplicate = FakeGmailProvider(duplicateFirstMessageOnSecondPage = true)
        assertEquals(7, GmailSearchService(duplicate).inboxAll(pageSize = 2).size)
        val empty = FakeGmailProvider(emptyPageAt = 1)
        assertEquals(5, GmailSearchService(empty).inboxAll(pageSize = 2).size)
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { empty.listInbox("bad-token", 2) }.code)
        val tooMany = GmailSearchService(FakeGmailProvider(), GmailReadLimits(maximumPages = 1, maximumMessages = 50))
        assertEquals(GmailReadFailureCode.TOO_MANY_PAGES, failure { tooMany.inboxAll(pageSize = 2) }.code)
        val tooManyMessages = GmailSearchService(FakeGmailProvider(), GmailReadLimits(maximumPages = 20, maximumMessages = 2))
        assertEquals(GmailReadFailureCode.TOO_MANY_MESSAGES, failure { tooManyMessages.inboxAll(pageSize = 3) }.code)
    }

    @Test fun pagingCountsUniqueIdsWithinEachPageAndRejectsConflictingMessagesAndRepeatedTokens() = runBlocking {
        val fixture = FakeGmailProvider.fixtures.messages
        val repeatedPage = object : GmailReadPort {
            override suspend fun listInbox(pageToken: String?, pageSize: Int) = GmailMessagePage(listOf(fixture[0], fixture[0], fixture[1]))
            override suspend fun search(query: String, pageToken: String?, pageSize: Int) = listInbox(pageToken, pageSize)
            override suspend fun getMessage(id: GmailMessageId) = fixture.first()
            override suspend fun getThread(id: GmailThreadId) = GmailThread(id, fixture.filter { it.threadId == id })
        }
        val twoUniqueAllowed = GmailSearchService(repeatedPage, GmailReadLimits(maximumMessages = 2))
        assertEquals(2, twoUniqueAllowed.inboxAll().size)
        val oneUniqueAllowed = GmailSearchService(repeatedPage, GmailReadLimits(maximumMessages = 1))
        assertEquals(GmailReadFailureCode.TOO_MANY_MESSAGES, failure { oneUniqueAllowed.inboxAll() }.code)

        val contradictory = object : GmailReadPort by repeatedPage {
            override suspend fun listInbox(pageToken: String?, pageSize: Int): GmailMessagePage {
                val changed = fixture[0].copy(subject = "conflicting content")
                return GmailMessagePage(listOf(fixture[0], changed))
            }
        }
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { GmailSearchService(contradictory).inboxAll() }.code)

        val crossPageContradictory = object : GmailReadPort by repeatedPage {
            override suspend fun listInbox(pageToken: String?, pageSize: Int): GmailMessagePage = if (pageToken == null) {
                GmailMessagePage(listOf(fixture[0]), "next")
            } else {
                GmailMessagePage(listOf(fixture[0].copy(subject = "different content")))
            }
        }
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { GmailSearchService(crossPageContradictory).inboxAll() }.code)

        val repeating = object : GmailReadPort by repeatedPage {
            override suspend fun listInbox(pageToken: String?, pageSize: Int) =
                GmailMessagePage(listOf(fixture[0]), "same-token")
        }
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { GmailSearchService(repeating).inboxAll() }.code)
    }

    @Test fun fakeFailureFixturesCoverDenialRateLimitTimeoutAndPartialFailure() = runBlocking {
        assertEquals(GmailReadFailureCode.PERMISSION_DENIED, failure { FakeGmailProvider(failure = FakeGmailFailure.ACCESS_DENIED).listInbox() }.code)
        assertEquals(GmailReadFailureCode.RATE_LIMITED, failure { FakeGmailProvider(failure = FakeGmailFailure.RATE_LIMITED).listInbox() }.code)
        assertEquals(GmailReadFailureCode.TIMEOUT, failure { FakeGmailProvider(failure = FakeGmailFailure.TIMEOUT).listInbox() }.code)
        assertEquals(GmailReadFailureCode.PARTIAL_FAILURE, failure {
            GmailSearchService(FakeGmailProvider(failAtPageToken = "2")).inboxAll(pageSize = 2)
        }.code)
        assertEquals(GmailReadFailureCode.UNAVAILABLE, failure { FakeGmailProvider(failure = FakeGmailFailure.UNAVAILABLE).getMessage(GmailMessageId("fake-msg-001")) }.code)
    }

    @Test fun parserReadsPlainBodyAndNestedAlternativePrefersPlain() {
        val plain = GmailMimeParser.parseMessage(messageJson(payload = part("text/plain", "Hello, Finch.")))
        assertEquals("Hello, Finch.", plain.plainTextBody)
        assertEquals(DataOrigin.GMAIL, plain.dataOrigin)
        assertNotEquals(DataClassification.PUBLIC, plain.classification)

        val nested = """{"mimeType":"multipart/mixed","parts":[{"mimeType":"multipart/alternative","parts":[${part("text/html", "<b>HTML version</b>")},${part("text/plain", "Plain version")}]}]}"""
        val parsed = GmailMimeParser.parseMessage(messageJson(payload = nested))
        assertEquals("Plain version", parsed.plainTextBody)
        assertNull(parsed.htmlTextBody)
    }

    @Test fun parserKeepsIndependentMixedBodiesAndRejectsUnsupportedIncompleteMime() = runBlocking {
        val mixed = """{"mimeType":"multipart/mixed","parts":[${part("text/plain", "First paragraph")},${part("text/plain", "Second paragraph")},${part("text/html", "<p>HTML one</p>")},${part("text/html", "<p>HTML two</p>") }]}"""
        val parsed = GmailMimeParser.parseMessage(messageJson(payload = mixed))
        assertEquals("First paragraph\n\nSecond paragraph", parsed.plainTextBody)
        assertEquals("HTML one\n\nHTML two", parsed.htmlTextBody)
        val unsupportedRelated = """{"mimeType":"multipart/related","parts":[${part("text/html", "body")},${part("image/png", "not interpreted")}]}"""
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { GmailMimeParser.parseMessage(messageJson(payload = unsupportedRelated)) }.code)
        val unsupportedInline = """{"mimeType":"application/octet-stream","body":{"data":"AA"}}"""
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { GmailMimeParser.parseMessage(messageJson(payload = unsupportedInline)) }.code)
    }

    @Test fun parserSupportsBase64UrlAndDeclaredCharset() {
        val text = "München — 東京"
        val bytes = text.toByteArray(Charset.forName("UTF-8"))
        val encoded = Base64.getUrlEncoder().withoutPadding().encodeToString(bytes)
        val payload = """{"mimeType":"text/plain","headers":[{"name":"Content-Type","value":"text/plain; charset=UTF-8"}],"body":{"data":"$encoded"}}"""
        assertEquals(text, GmailMimeParser.parseMessage(messageJson(payload = payload)).plainTextBody)
        val latin = "café".toByteArray(Charset.forName("ISO-8859-1"))
        val latinEncoded = Base64.getUrlEncoder().withoutPadding().encodeToString(latin)
        val latinPayload = """{"mimeType":"text/plain","headers":[{"name":"Content-Type","value":"text/plain; charset=ISO-8859-1"}],"body":{"data":"$latinEncoded"}}"""
        assertEquals("café", GmailMimeParser.parseMessage(messageJson(payload = latinPayload)).plainTextBody)
    }

    @Test fun htmlIsReducedToTextWithoutActiveContentOrRemoteAttributes() {
        val html = "<p>Hello <a href=\"https://outside.invalid/track\">world</a></p><script>secret()</script><img src=\"https://outside.invalid/pixel\"><style>.x{}</style>"
        val parsed = GmailMimeParser.parseMessage(messageJson(payload = part("text/html", html)))
        assertEquals("Hello world", parsed.htmlTextBody)
        assertFalse(parsed.htmlTextBody.orEmpty().contains("outside.invalid"))
        assertFalse(parsed.htmlTextBody.orEmpty().contains("secret"))
        assertFalse(parsed.htmlTextBody.orEmpty().contains("<"))
    }

    @Test fun parserIgnoresAttachmentSubtreesAndRejectsInconsistentBodySize() = runBlocking {
        val attached = """{"mimeType":"multipart/mixed","parts":[{"mimeType":"text/plain","filename":"fixture.txt","body":{"data":"${Base64.getUrlEncoder().withoutPadding().encodeToString("do not read".toByteArray())}"}}]}"""
        assertNull(GmailMimeParser.parseMessage(messageJson(payload = attached)).plainTextBody)
        val deferredTextBody = """{"mimeType":"text/plain","body":{"attachmentId":"fixture-body-attachment"}}"""
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure {
            GmailMimeParser.parseMessage(messageJson(payload = deferredTextBody))
        }.code)
        val mismatch = """{"mimeType":"text/plain","body":{"size":99,"data":"${Base64.getUrlEncoder().withoutPadding().encodeToString("small".toByteArray())}"}}"""
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { GmailMimeParser.parseMessage(messageJson(payload = mismatch)) }.code)
    }

    @Test fun parserAcceptsMissingOptionalFieldsButRejectsMissingRequiredFields() = runBlocking {
        val minimal = GmailMimeParser.parseMessage("""{"id":"message-1","threadId":"thread-1"}""")
        assertNull(minimal.subject)
        assertNull(minimal.plainTextBody)
        assertEquals(emptyList<String>(), minimal.to)
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure {
            GmailMimeParser.parseMessage("""{"threadId":"thread-1"}""")
        }.code)
    }

    @Test fun parserEnforcesJsonBodyDepthAndMalformedEncodingLimits() = runBlocking {
        val largeJson = " ".repeat(GmailMimeParser.MAX_JSON_BYTES + 1)
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { GmailMimeParser.parseMessage(largeJson) }.code)
        val pathologicalNesting = """{"id":"message-1","threadId":"thread-1","payload":${"{".repeat(200)}${"}".repeat(200)}}"""
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { GmailMimeParser.parseMessage(pathologicalNesting) }.code)
        val oversizedBody = "x".repeat(GmailMimeParser.MAX_DECODED_BODY_BYTES + 1)
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure {
            GmailMimeParser.parseMessage(messageJson(payload = part("text/plain", oversizedBody)))
        }.code)
        var deep = part("text/plain", "x")
        repeat(GmailMimeParser.MAX_PART_DEPTH + 1) { deep = """{"mimeType":"multipart/mixed","parts":[$deep]}""" }
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure { GmailMimeParser.parseMessage(messageJson(payload = deep)) }.code)
        val invalidUtf8 = Base64.getUrlEncoder().withoutPadding().encodeToString(byteArrayOf(0xC3.toByte(), 0x28))
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure {
            GmailMimeParser.parseMessage(messageJson(payload = """{"mimeType":"text/plain","body":{"data":"$invalidUtf8"}}"""))
        }.code)
        assertEquals(GmailReadFailureCode.INVALID_RESPONSE, failure {
            GmailMimeParser.parseMessage(messageJson(payload = """{"mimeType":"text/plain","body":{"data":"***"}}"""))
        }.code)
    }

    @Test fun messageLineageAndReadOnlyBoundaryBlockModelEgressAndPreventLogLeaks() = runBlocking {
        val message = FakeGmailProvider().getMessage(GmailMessageId("fake-msg-001"))
        assertEquals(DataOrigin.GMAIL, message.dataOrigin)
        assertNotEquals(DataClassification.PUBLIC, message.classification)
        val contract = EgressContract(
            conversationId = "fake-conversation", currentMessageId = message.id.value,
            origin = message.dataOrigin, classification = message.classification,
            destination = "https://ollama.com/api/chat", plannedBody = message.plainTextBody.orEmpty(),
            context = emptyList(), preparation = PayloadPreparation.RAW,
            explicitSendIntentAtMillis = 1_000, expiresAtMillis = 1_010,
        )
        assertEquals(EgressContractDecision.ORIGIN_DENIED, ChatEgressGate.evaluate(contract, 1_005).decision)
        val failure = failure { FakeGmailProvider(failure = FakeGmailFailure.ACCESS_DENIED).search("Private Person") }
        assertFalse(failure.message.orEmpty().contains("Private Person"))
        assertFalse(failure.message.orEmpty().contains("example.invalid"))
    }

    private fun messageJson(payload: String): String = """{"id":"message-1","threadId":"thread-1","internalDate":"1792000000000","snippet":"fixture","payload":$payload}"""

    private fun part(mime: String, text: String): String {
        val encoded = Base64.getUrlEncoder().withoutPadding().encodeToString(text.toByteArray(Charsets.UTF_8))
        return """{"mimeType":"$mime","body":{"size":${text.toByteArray().size},"data":"$encoded"}}"""
    }

    private suspend fun failure(block: suspend () -> Unit): GmailReadFailure {
        try { block(); throw AssertionError("Expected GmailReadFailure") } catch (failure: GmailReadFailure) { return failure }
    }
}
