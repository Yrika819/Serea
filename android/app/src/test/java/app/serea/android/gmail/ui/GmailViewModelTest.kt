package app.serea.android.gmail.ui

import androidx.lifecycle.SavedStateHandle
import app.serea.android.DataClassification
import app.serea.android.DataOrigin
import app.serea.android.gmail.FakeGmailProvider
import app.serea.android.gmail.GmailAuthorizationState
import app.serea.android.gmail.GmailAuthorizationStatus
import app.serea.android.gmail.GmailMessage
import app.serea.android.gmail.GmailMessageId
import app.serea.android.gmail.GmailMessagePage
import app.serea.android.gmail.GmailMessageSource
import app.serea.android.gmail.GmailReadFailure
import app.serea.android.gmail.GmailReadFailureCode
import app.serea.android.gmail.GmailThread
import app.serea.android.gmail.GmailThreadId
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.test.StandardTestDispatcher
import kotlinx.coroutines.test.TestDispatcher
import kotlinx.coroutines.test.advanceUntilIdle
import kotlinx.coroutines.test.runCurrent
import kotlinx.coroutines.test.runTest
import kotlinx.coroutines.test.setMain
import kotlinx.coroutines.test.resetMain
import kotlinx.coroutines.withContext
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.Description
import org.junit.runners.model.Statement
import kotlinx.coroutines.Dispatchers

@OptIn(ExperimentalCoroutinesApi::class)
class GmailViewModelTest {
    @get:Rule val mainDispatcher = MainDispatcherRule()

    @Test fun inboxLoadsFictionalMessagesAndRetainsGmailLineage() = runTest(mainDispatcher.dispatcher) {
        val fake = GmailUiTestFake()
        val viewModel = viewModel(fake)
        advanceUntilIdle()

        assertEquals(GmailListMode.INBOX, (viewModel.state.value.route as GmailRoute.List).mode)
        assertEquals(7, viewModel.state.value.messages.size)
        assertFalse(viewModel.state.value.isLoading)
        viewModel.state.value.messages.forEach { message ->
            assertEquals(DataOrigin.GMAIL, message.dataOrigin)
            assertNotEquals(DataClassification.PUBLIC, message.classification)
            assertEquals(GmailMessageSource.MOCK, message.source)
        }
    }

    @Test fun emptyInboxIsAStableEmptyState() = runTest(mainDispatcher.dispatcher) {
        val fake = GmailUiTestFake(inboxIds = emptySet())
        val viewModel = viewModel(fake)
        advanceUntilIdle()

        assertTrue(viewModel.state.value.messages.isEmpty())
        assertNull(viewModel.state.value.listError)
        assertFalse(viewModel.state.value.isLoading)
        assertFalse(viewModel.state.value.hasMore)
    }

    @Test fun pageTokensArePassedThroughAndDuplicateIdsAcrossPagesAreCollapsed() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        val fake = GmailUiTestFake()
        fake.inboxHandler = { token, _ ->
            when (token) {
                null -> GmailMessagePage(fixture.take(2), "cursor/+==")
                "cursor/+==" -> GmailMessagePage(listOf(fixture[1], fixture[2]), null)
                else -> error("Unexpected page cursor")
            }
        }
        val viewModel = viewModel(fake)
        runCurrent()
        assertTrue(viewModel.state.value.hasMore)

        viewModel.loadNextPage()
        viewModel.loadNextPage() // Must not dispatch a second concurrent request.
        advanceUntilIdle()

        assertEquals(listOf(null, "cursor/+=="), fake.inboxTokens)
        assertEquals(fixture.take(3).map { it.id }, viewModel.state.value.messages.map { it.id })
        assertFalse(viewModel.state.value.hasMore)
    }

    @Test fun openingInboxFromSearchResetsBackStackToTheRootList() = runTest(mainDispatcher.dispatcher) {
        val fake = GmailUiTestFake()
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        viewModel.submitSearch("Finch")
        advanceUntilIdle()

        viewModel.showInbox()
        advanceUntilIdle()

        assertEquals(GmailRoute.List(GmailListMode.INBOX), viewModel.state.value.route)
        assertTrue(viewModel.state.value.backStack.isEmpty())
    }

    @Test fun emptyPageWithContinuationCanAdvanceToLaterMessages() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        val fake = GmailUiTestFake()
        fake.inboxHandler = { token, _ ->
            if (token == null) GmailMessagePage(emptyList(), "empty-page") else GmailMessagePage(listOf(fixture.first()))
        }
        val viewModel = viewModel(fake)
        runCurrent()
        assertTrue(viewModel.state.value.messages.isEmpty())
        assertTrue(viewModel.state.value.hasMore)

        viewModel.loadNextPage()
        advanceUntilIdle()

        assertEquals(listOf(fixture.first().id), viewModel.state.value.messages.map { it.id })
        assertNull(viewModel.state.value.listError)
    }

    @Test fun repeatedAndBlankPagingTokensBecomeSafeDataErrors() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        val repeated = GmailUiTestFake().apply {
            inboxHandler = { token, _ ->
                if (token == null) GmailMessagePage(listOf(fixture[0]), "cycle") else GmailMessagePage(listOf(fixture[1]), "cycle")
            }
        }
        val repeatedVm = viewModel(repeated)
        runCurrent()
        repeatedVm.loadNextPage()
        advanceUntilIdle()
        assertEquals(GmailUiErrorCode.INVALID_DATA, repeatedVm.state.value.listError)
        assertEquals(listOf(fixture[0].id), repeatedVm.state.value.messages.map { it.id })

        val blank = GmailUiTestFake().apply { inboxHandler = { _, _ -> GmailMessagePage(emptyList(), "   ") } }
        val blankVm = viewModel(blank)
        advanceUntilIdle()
        assertEquals(GmailUiErrorCode.INVALID_DATA, blankVm.state.value.listError)
    }

    @Test fun cursorLoopDetectionSurvivesOpeningAndLeavingMessageDetail() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        val fake = GmailUiTestFake().apply {
            inboxHandler = { token, _ ->
                when (token) {
                    null -> GmailMessagePage(fixture.take(2), "cursor-a")
                    "cursor-a" -> GmailMessagePage(listOf(fixture[2]), "cursor-b")
                    "cursor-b" -> GmailMessagePage(listOf(fixture[3]), "cursor-a")
                    else -> error("Unexpected cursor")
                }
            }
        }
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        viewModel.loadNextPage()
        advanceUntilIdle()
        viewModel.openMessage(fixture[0].id)
        advanceUntilIdle()
        viewModel.back()
        advanceUntilIdle()

        viewModel.loadNextPage()
        advanceUntilIdle()

        assertEquals(GmailUiErrorCode.INVALID_DATA, viewModel.state.value.listError)
        assertEquals(listOf(null, "cursor-a", "cursor-b"), fake.inboxTokens)
    }

    @Test fun conflictingDuplicateIdsAreRejectedAndExactDuplicatesCollapse() = runTest(mainDispatcher.dispatcher) {
        val message = FakeGmailProvider.fixtures.messages.first()
        val exactFake = GmailUiTestFake().apply {
            inboxHandler = { _, _ -> GmailMessagePage(listOf(message, message)) }
        }
        val exactVm = viewModel(exactFake)
        advanceUntilIdle()
        assertEquals(listOf(message.id), exactVm.state.value.messages.map { it.id })

        val conflictingFake = GmailUiTestFake().apply {
            inboxHandler = { _, _ -> GmailMessagePage(listOf(message, message.copy(subject = "different fixture"))) }
        }
        val conflictingVm = viewModel(conflictingFake)
        advanceUntilIdle()
        assertEquals(GmailUiErrorCode.INVALID_DATA, conflictingVm.state.value.listError)
        assertTrue(conflictingVm.state.value.messages.isEmpty())
    }

    @Test fun pageAndMessageLimitsStopPaginationWithAnExplicitError() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        val pageLimitFake = GmailUiTestFake().apply {
            inboxHandler = { _, _ -> GmailMessagePage(listOf(fixture.first()), "next") }
        }
        val pageLimited = viewModel(pageLimitFake, maximumPages = 1)
        advanceUntilIdle()
        assertEquals(GmailUiErrorCode.TOO_MANY_RESULTS, pageLimited.state.value.listError)
        assertFalse(pageLimited.state.value.hasMore)
        assertEquals("next", pageLimited.state.value.nextPageToken)

        val messageLimitFake = GmailUiTestFake().apply {
            inboxHandler = { _, _ -> GmailMessagePage(fixture.take(2), "next") }
        }
        val messageLimited = viewModel(messageLimitFake, maximumMessages = 2)
        advanceUntilIdle()
        assertEquals(GmailUiErrorCode.TOO_MANY_RESULTS, messageLimited.state.value.listError)
        assertFalse(messageLimited.state.value.hasMore)
        assertEquals(2, messageLimited.state.value.messages.size)
    }

    @Test fun searchSuccessAndNoResultsAreDistinctFromErrors() = runTest(mainDispatcher.dispatcher) {
        val fake = GmailUiTestFake()
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        viewModel.submitSearch("  Finch ")
        advanceUntilIdle()
        assertEquals(listOf("Finch"), fake.searchQueries)
        assertEquals(2, viewModel.state.value.messages.size)
        assertEquals(GmailRoute.List(GmailListMode.SEARCH, "Finch"), viewModel.state.value.route)

        viewModel.submitSearch("no-match-term")
        advanceUntilIdle()
        assertTrue(viewModel.state.value.messages.isEmpty())
        assertNull(viewModel.state.value.listError)
        assertFalse(viewModel.state.value.isLoading)
    }

    @Test fun searchProviderFailureShowsSafeRecoverableError() = runTest(mainDispatcher.dispatcher) {
        val fake = GmailUiTestFake().apply {
            searchHandler = { _, _, _ -> throw GmailReadFailure(GmailReadFailureCode.UNAVAILABLE) }
        }
        val viewModel = viewModel(fake)
        advanceUntilIdle()

        viewModel.submitSearch("fictional probe")
        advanceUntilIdle()

        assertEquals(GmailUiErrorCode.UNAVAILABLE, viewModel.state.value.listError)
        assertFalse(errorMessage(viewModel.state.value.listError!!).contains("fictional probe"))
        assertFalse(errorMessage(viewModel.state.value.listError!!).contains("example.invalid"))
    }

    @Test fun blankSearchDoesNotReachTheProvider() = runTest(mainDispatcher.dispatcher) {
        val fake = GmailUiTestFake()
        val viewModel = viewModel(fake)
        advanceUntilIdle()

        viewModel.submitSearch("   ")

        assertEquals(GmailUiErrorCode.EMPTY_SEARCH, viewModel.state.value.listError)
        assertTrue(fake.searchQueries.isEmpty())
    }

    @Test fun searchAThenBDiscardsLateAResult() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        val releaseA = CompletableDeferred<Unit>()
        val startedA = CompletableDeferred<Unit>()
        val fake = GmailUiTestFake().apply {
            searchHandler = { query, _, _ ->
                when (query) {
                    "A" -> {
                        startedA.complete(Unit)
                        withContext(NonCancellable) { releaseA.await() }
                        GmailMessagePage(listOf(fixture[0]))
                    }
                    "B" -> GmailMessagePage(listOf(fixture[1]))
                    else -> GmailMessagePage(emptyList())
                }
            }
        }
        val viewModel = viewModel(fake)
        advanceUntilIdle()

        viewModel.submitSearch("A")
        runCurrent()
        assertTrue(startedA.isCompleted)
        viewModel.submitSearch("B")
        runCurrent()
        assertEquals(listOf(fixture[1].id), viewModel.state.value.messages.map { it.id })
        releaseA.complete(Unit)
        advanceUntilIdle()

        assertEquals(GmailRoute.List(GmailListMode.SEARCH, "B"), viewModel.state.value.route)
        assertEquals(listOf(fixture[1].id), viewModel.state.value.messages.map { it.id })
    }

    @Test fun leavingSearchCancelsItAndLateResultsCannotReplaceInbox() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        val release = CompletableDeferred<Unit>()
        val fake = GmailUiTestFake().apply {
            searchHandler = { _, _, _ ->
                withContext(NonCancellable) { release.await() }
                GmailMessagePage(listOf(fixture.last()))
            }
        }
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        val inboxIds = viewModel.state.value.messages.map { it.id }

        viewModel.submitSearch("pending")
        runCurrent()
        viewModel.cancelSearch()
        advanceUntilIdle()
        release.complete(Unit)
        advanceUntilIdle()

        assertEquals(GmailRoute.List(GmailListMode.INBOX), viewModel.state.value.route)
        assertEquals(inboxIds, viewModel.state.value.messages.map { it.id })
        assertFalse(viewModel.state.value.isLoading)
    }

    @Test fun backFromSearchReloadsInboxInsteadOfShowingSearchRows() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        val fake = GmailUiTestFake()
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        val inboxIds = viewModel.state.value.messages.map { it.id }

        viewModel.submitSearch("Finch")
        advanceUntilIdle()
        assertNotEquals(inboxIds, viewModel.state.value.messages.map { it.id })
        viewModel.back()
        advanceUntilIdle()

        assertEquals(GmailRoute.List(GmailListMode.INBOX), viewModel.state.value.route)
        assertEquals(inboxIds, viewModel.state.value.messages.map { it.id })
        assertTrue(fake.inboxTokens.size >= 2)
        assertEquals(fixture.take(7).map { it.id }, inboxIds)
    }

    @Test fun detailChecksRequestedIdAndKeepsPlainTextPriority() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        val chosen = fixture.first().copy(plainTextBody = "plain version", htmlTextBody = "safe html fallback")
        val fake = GmailUiTestFake().apply {
            inboxHandler = { _, _ -> GmailMessagePage(listOf(chosen)) }
            messageHandler = { chosen }
        }
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        viewModel.openMessage(chosen.id)
        advanceUntilIdle()

        assertEquals(chosen, viewModel.state.value.detail)
        assertEquals("plain version", displayBody(chosen))

        fake.messageHandler = { fixture[1] }
        viewModel.retry()
        advanceUntilIdle()
        assertEquals(GmailUiErrorCode.INVALID_DATA, viewModel.state.value.detailError)
        assertNull(viewModel.state.value.detail)
    }

    @Test fun htmlSafeTextFallbackAndMissingBodyHaveExplicitPresentation() {
        val fixture = FakeGmailProvider.fixtures.messages.first()
        assertEquals("safe text only", displayBody(fixture.copy(plainTextBody = null, htmlTextBody = "safe text only")))
        assertEquals("本文はありません", displayBody(fixture.copy(plainTextBody = null, htmlTextBody = null)))
        assertEquals("html fallback", displayBody(fixture.copy(plainTextBody = "", htmlTextBody = "html fallback")))
    }

    @Test fun longBodyChunksPreserveAllTextAndNeverSplitSurrogatePairs() {
        val body = "a".repeat(3_999) + "😀" + "b".repeat(5_000) + "\n" + "c".repeat(5_000)
        val chunks = bodyChunks(FakeGmailProvider.fixtures.messages.first().copy(plainTextBody = body))

        assertTrue(chunks.size > 1)
        assertTrue(chunks.all { it.length <= 4_000 })
        assertEquals(body, chunks.joinToString(separator = ""))
        chunks.zipWithNext().forEach { (left, right) ->
            assertFalse(Character.isHighSurrogate(left.last()) && Character.isLowSurrogate(right.first()))
        }
    }

    @Test fun threadMessagesAreOrderedAndSelectableInsideTheThread() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        val first = fixture[0].copy(internalDateMillis = 200L)
        val second = fixture[1].copy(internalDateMillis = 100L)
        val fake = GmailUiTestFake().apply {
            inboxHandler = { _, _ -> GmailMessagePage(listOf(first)) }
            messageHandler = { first }
            threadHandler = { id -> GmailThread(id, listOf(first, second)) }
        }
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        viewModel.openMessage(first.id)
        advanceUntilIdle()
        viewModel.openThread()
        advanceUntilIdle()

        assertEquals(listOf(second.id, first.id), viewModel.state.value.thread?.messages?.map { it.id })
        assertEquals(GmailRoute.Thread(first.threadId), viewModel.state.value.route)
        viewModel.selectThreadMessage(second.id)
        assertEquals(GmailRoute.Thread(first.threadId, second.id), viewModel.state.value.route)
        viewModel.showThreadList()
        assertEquals(GmailRoute.Thread(first.threadId), viewModel.state.value.route)
    }

    @Test fun wrongThreadIdOrCrossThreadMemberIsADataError() = runTest(mainDispatcher.dispatcher) {
        val message = FakeGmailProvider.fixtures.messages.first()
        val fake = GmailUiTestFake().apply {
            inboxHandler = { _, _ -> GmailMessagePage(listOf(message)) }
            messageHandler = { message }
            threadHandler = { _ ->
                val foreign = GmailThreadId("foreign-thread")
                GmailThread(foreign, listOf(message.copy(threadId = foreign)))
            }
        }
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        viewModel.openMessage(message.id)
        advanceUntilIdle()
        viewModel.openThread()
        advanceUntilIdle()

        assertEquals(GmailUiErrorCode.INVALID_DATA, viewModel.state.value.threadError)
        assertNull(viewModel.state.value.thread)
    }

    @Test fun authorizationBoundariesCoverDeniedMissingScopeExpiredRevokedAndRealStatus() = runTest(mainDispatcher.dispatcher) {
        val expected = listOf(
            GmailAuthorizationStatus(GmailAuthorizationState.DENIED) to GmailUiErrorCode.AUTHORIZATION_DENIED,
            GmailAuthorizationStatus(GmailAuthorizationState.REVOKED) to GmailUiErrorCode.AUTHORIZATION_REVOKED,
            GmailAuthorizationStatus(GmailAuthorizationState.EXPIRED) to GmailUiErrorCode.AUTHORIZATION_EXPIRED,
            GmailAuthorizationStatus(GmailAuthorizationState.MOCK_ONLY, setOf("gmail.readonly")) to GmailUiErrorCode.MISSING_SCOPE,
            GmailAuthorizationStatus(GmailAuthorizationState.AUTHORIZED) to GmailUiErrorCode.MOCK_PROVIDER_REQUIRED,
        )
        for ((status, error) in expected) {
            val fake = GmailUiTestFake(authorization = status)
            val viewModel = viewModel(fake)
            advanceUntilIdle()
            assertEquals(error, viewModel.state.value.listError)
            assertTrue("No mailbox read after authorization gate: $status", fake.inboxTokens.isEmpty())
        }
    }

    @Test fun permissionRateLimitTimeoutAndRetryUseStableSafeErrors() = runTest(mainDispatcher.dispatcher) {
        val fake = GmailUiTestFake()
        var attempts = 0
        fake.inboxHandler = { _, _ ->
            attempts++
            if (attempts == 1) throw GmailReadFailure(GmailReadFailureCode.RATE_LIMITED)
            GmailMessagePage(FakeGmailProvider.fixtures.messages.take(1))
        }
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        assertEquals(GmailUiErrorCode.RATE_LIMITED, viewModel.state.value.listError)
        viewModel.retry()
        advanceUntilIdle()
        assertEquals(1, viewModel.state.value.messages.size)
        assertNull(viewModel.state.value.listError)

        fake.inboxHandler = { _, _ -> throw GmailReadFailure(GmailReadFailureCode.TIMEOUT) }
        viewModel.showInbox()
        advanceUntilIdle()
        assertEquals(GmailUiErrorCode.TIMEOUT, viewModel.state.value.listError)
        assertTrue(errorMessage(viewModel.state.value.listError!!).contains("タイムアウト"))
        assertFalse(errorMessage(viewModel.state.value.listError!!).contains("example.invalid"))

        fake.inboxHandler = { _, _ -> throw GmailReadFailure(GmailReadFailureCode.PERMISSION_DENIED) }
        viewModel.showInbox()
        advanceUntilIdle()
        assertEquals(GmailUiErrorCode.PERMISSION_DENIED, viewModel.state.value.listError)
    }

    @Test fun retryAfterPagedFailureUsesTheSameOpaqueCursor() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages
        var continuationAttempts = 0
        val fake = GmailUiTestFake().apply {
            inboxHandler = { token, _ ->
                if (token == null) GmailMessagePage(listOf(fixture[0]), "cursor/retry+1")
                else {
                    continuationAttempts++
                    if (continuationAttempts == 1) throw GmailReadFailure(GmailReadFailureCode.TIMEOUT)
                    GmailMessagePage(listOf(fixture[1]))
                }
            }
        }
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        viewModel.loadNextPage()
        advanceUntilIdle()
        assertEquals(GmailUiErrorCode.TIMEOUT, viewModel.state.value.listError)

        viewModel.retry()
        advanceUntilIdle()

        assertEquals(listOf(null, "cursor/retry+1", "cursor/retry+1"), fake.inboxTokens)
        assertEquals(listOf(fixture[0].id, fixture[1].id), viewModel.state.value.messages.map { it.id })
        assertNull(viewModel.state.value.listError)
    }

    @Test fun providerCancellationStopsProgressAndDoesNotLogFixtureContent() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages.first()
        val started = CompletableDeferred<Unit>()
        val never = CompletableDeferred<GmailMessagePage>()
        val fake = GmailUiTestFake().apply {
            searchHandler = { _, _, _ ->
                started.complete(Unit)
                never.await()
            }
        }
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        viewModel.submitSearch("private probe")
        runCurrent()
        assertTrue(started.isCompleted)
        viewModel.cancelSearch()
        advanceUntilIdle()

        assertFalse(viewModel.state.value.isLoading)
        assertEquals(GmailRoute.List(GmailListMode.INBOX), viewModel.state.value.route)
        assertFalse(errorMessage(GmailUiErrorCode.UNAVAILABLE).contains(fixture.from.orEmpty()))
    }

    @Test fun processRestorationReloadsDataWithoutSavingBodiesOrTokens() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages.first()
        val savedState = SavedStateHandle()
        val fake = GmailUiTestFake().apply { inboxHandler = { _, _ -> GmailMessagePage(listOf(fixture), "opaque-secret-cursor") } }
        val original = GmailViewModel(fake, fake, savedState, pageSize = 1)
        advanceUntilIdle()
        original.openMessage(fixture.id)
        advanceUntilIdle()
        assertEquals(GmailRoute.Message(fixture.id), original.state.value.route)

        val recreatedFake = GmailUiTestFake().apply { inboxHandler = { _, _ -> GmailMessagePage(listOf(fixture)) } }
        val savedValues = savedState.keys().associateWith { savedState.get<Any?>(it) }
        val recreated = GmailViewModel(recreatedFake, recreatedFake, SavedStateHandle(savedValues), pageSize = 1)
        advanceUntilIdle()

        assertEquals(GmailRoute.Message(fixture.id), recreated.state.value.route)
        assertEquals(fixture.id, recreated.state.value.detail?.id)
        val restoredValues = savedState.keys().mapNotNull { savedState.get<Any?>(it) }
        assertFalse(restoredValues.any { it == "opaque-secret-cursor" })
        assertFalse(restoredValues.any { it == fixture.plainTextBody })
    }

    @Test fun restoredEmptySearchDoesNotCallProviderAgain() = runTest(mainDispatcher.dispatcher) {
        val handle = SavedStateHandle(mapOf(
            "gmail_list_mode" to GmailListMode.SEARCH.name,
            "gmail_search_query" to "",
            "gmail_route_type" to "search",
            "gmail_route_id" to "",
            "gmail_route_selected" to "",
        ))
        val fake = GmailUiTestFake()
        val viewModel = GmailViewModel(fake, fake, handle)
        advanceUntilIdle()

        assertEquals(GmailUiErrorCode.EMPTY_SEARCH, viewModel.state.value.listError)
        assertTrue(fake.searchQueries.isEmpty())
        assertTrue(fake.inboxTokens.isEmpty())
    }

    @Test fun restoredDetailInboxFailureIsShownAndRetryContinuesRestoration() = runTest(mainDispatcher.dispatcher) {
        val fixture = FakeGmailProvider.fixtures.messages.first()
        var attempts = 0
        val handle = SavedStateHandle(mapOf(
            "gmail_list_mode" to GmailListMode.INBOX.name,
            "gmail_search_query" to "",
            "gmail_route_type" to "message",
            "gmail_route_id" to fixture.id.value,
            "gmail_route_selected" to "",
            "gmail_back_0_type" to "inbox",
            "gmail_back_0_id" to "",
            "gmail_back_0_selected" to "",
        ))
        val fake = GmailUiTestFake().apply {
            inboxHandler = { _, _ ->
                attempts++
                if (attempts == 1) throw GmailReadFailure(GmailReadFailureCode.TIMEOUT)
                GmailMessagePage(listOf(fixture))
            }
        }
        val viewModel = GmailViewModel(fake, fake, handle)
        advanceUntilIdle()

        assertEquals(GmailUiErrorCode.TIMEOUT, viewModel.state.value.detailError)
        viewModel.retry()
        advanceUntilIdle()

        assertEquals(fixture.id, viewModel.state.value.detail?.id)
        assertNull(viewModel.state.value.detailError)
    }

    @Test fun uiUsesOnlyInjectedFakeReadPortsAndCompiledUiHasNoTransportReferences() = runTest(mainDispatcher.dispatcher) {
        val fake = GmailUiTestFake()
        val viewModel = viewModel(fake)
        advanceUntilIdle()
        viewModel.submitSearch("Finch")
        advanceUntilIdle()
        val message = viewModel.state.value.messages.first()
        viewModel.openMessage(message.id)
        advanceUntilIdle()
        viewModel.openThread()
        advanceUntilIdle()

        assertTrue(fake.inboxTokens.isNotEmpty())
        assertTrue(fake.searchQueries.isNotEmpty())
        assertTrue(fake.messageIds.isNotEmpty())
        assertTrue(fake.threadIds.isNotEmpty())

        val classes = listOf(
            GmailViewModel::class.java,
            Class.forName("app.serea.android.gmail.ui.GmailScreenKt"),
        )
        val forbiddenReferences = listOf(
            "java/net/", "okhttp3/", "retrofit2/", "WebView", "GoogleSignIn",
            "com/google/android/gms/auth/", "ModelChatPort", "CloudProvider", "OpenAI",
        )
        classes.forEach { type ->
            val classBytes = type.getResourceAsStream("/${type.name.replace('.', '/')}.class")!!.use { it.readBytes() }
            val constantPoolText = classBytes.toString(Charsets.ISO_8859_1)
            forbiddenReferences.forEach { forbidden ->
                assertFalse("${type.simpleName} must not link to $forbidden", constantPoolText.contains(forbidden))
            }
        }
    }

    private fun viewModel(
        fake: GmailUiTestFake,
        pageSize: Int = 50,
        maximumPages: Int = GmailViewModel.DEFAULT_MAXIMUM_PAGES,
        maximumMessages: Int = GmailViewModel.DEFAULT_MAXIMUM_MESSAGES,
    ) = GmailViewModel(fake, fake, pageSize = pageSize, maximumPages = maximumPages, maximumMessages = maximumMessages)
}

@OptIn(ExperimentalCoroutinesApi::class)
class MainDispatcherRule(
    val dispatcher: TestDispatcher = StandardTestDispatcher(),
) : org.junit.rules.TestRule {
    override fun apply(base: Statement, description: Description): Statement = object : Statement() {
        override fun evaluate() {
            Dispatchers.setMain(dispatcher)
            try {
                base.evaluate()
            } finally {
                Dispatchers.resetMain()
            }
        }
    }
}
