package app.serea.android.gmail.ui

import androidx.lifecycle.SavedStateHandle
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.createSavedStateHandle
import androidx.lifecycle.viewModelScope
import androidx.lifecycle.viewmodel.initializer
import androidx.lifecycle.viewmodel.viewModelFactory
import app.serea.android.DataClassification
import app.serea.android.DataOrigin
import app.serea.android.gmail.GmailAuthorizationPort
import app.serea.android.gmail.GmailAuthorizationState
import app.serea.android.gmail.GmailMessage
import app.serea.android.gmail.GmailMessageId
import app.serea.android.gmail.GmailMessagePage
import app.serea.android.gmail.GmailMessageSource
import app.serea.android.gmail.GmailReadFailure
import app.serea.android.gmail.GmailReadFailureCode
import app.serea.android.gmail.GmailReadPort
import app.serea.android.gmail.GmailThread
import app.serea.android.gmail.GmailThreadId
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.Job
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

enum class GmailListMode { INBOX, SEARCH }

sealed interface GmailRoute {
    data class List(val mode: GmailListMode, val query: String = "") : GmailRoute
    data class Message(val id: GmailMessageId) : GmailRoute
    data class Thread(val id: GmailThreadId, val selectedMessageId: GmailMessageId? = null) : GmailRoute
}

enum class GmailUiErrorCode {
    EMPTY_SEARCH,
    MOCK_PROVIDER_REQUIRED,
    AUTHORIZATION_REQUIRED,
    AUTHORIZATION_DENIED,
    AUTHORIZATION_REVOKED,
    AUTHORIZATION_EXPIRED,
    MISSING_SCOPE,
    PERMISSION_DENIED,
    RATE_LIMITED,
    TIMEOUT,
    CANCELLED,
    NOT_FOUND,
    INVALID_DATA,
    TOO_MANY_RESULTS,
    UNAVAILABLE,
}

data class GmailUiState(
    val route: GmailRoute = GmailRoute.List(GmailListMode.INBOX),
    val backStack: List<GmailRoute> = emptyList(),
    val messages: List<GmailMessage> = emptyList(),
    val nextPageToken: String? = null,
    val hasMore: Boolean = false,
    val isLoading: Boolean = true,
    val isLoadingMore: Boolean = false,
    val listError: GmailUiErrorCode? = null,
    val detail: GmailMessage? = null,
    val detailError: GmailUiErrorCode? = null,
    val thread: GmailThread? = null,
    val threadError: GmailUiErrorCode? = null,
    /** Raw search text stays in the Activity ViewModel and is never written to saved state. */
    val searchInput: String = "",
)

/**
 * State holder for the mock-only, read-only Gmail UI. Every operation checks the
 * mock authorization boundary again and rejects real-provider messages.
 */
class GmailViewModel(
    private val readPort: GmailReadPort,
    private val authorizationPort: GmailAuthorizationPort,
    private val savedStateHandle: SavedStateHandle = SavedStateHandle(),
    private val pageSize: Int = DEFAULT_PAGE_SIZE,
    private val maximumPages: Int = DEFAULT_MAXIMUM_PAGES,
    private val maximumMessages: Int = DEFAULT_MAXIMUM_MESSAGES,
) : ViewModel() {
    private val mutableState = MutableStateFlow(restoreState())
    val state: StateFlow<GmailUiState> = mutableState.asStateFlow()

    private var operationGeneration = 0L
    private var activeJob: Job? = null
    private var consumedTokens = mutableSetOf<String>()
    private var pagesRead = 0
    private var failedAction: FailedAction? = null

    init {
        require(pageSize in 1..100)
        require(maximumPages in 1..100)
        require(maximumMessages in 1..10_000)
        restoreContent()
    }

    fun showInbox() {
        consumedTokens.clear()
        failedAction = null
        navigateToList(GmailRoute.List(GmailListMode.INBOX), emptyList())
        loadFirstPage(GmailRoute.List(GmailListMode.INBOX))
    }

    fun updateSearchInput(value: String) {
        mutableState.value = mutableState.value.copy(searchInput = value)
    }

    fun submitSearch(query: String) {
        val normalized = query.trim()
        val current = mutableState.value
        val currentList = current.route as? GmailRoute.List
        val nextRoute = GmailRoute.List(GmailListMode.SEARCH, normalized)
        val stack = when {
            currentList?.mode == GmailListMode.SEARCH -> current.backStack
            currentList?.mode == GmailListMode.INBOX -> current.backStack + currentList
            else -> current.backStack
        }
        if (normalized.isEmpty()) {
            cancelActiveOperation()
            consumedTokens.clear()
            persistNavigation(nextRoute, stack)
            mutableState.value = current.copy(
                route = nextRoute,
                backStack = stack,
                searchInput = query,
                messages = emptyList(),
                nextPageToken = null,
                hasMore = false,
                isLoading = false,
                isLoadingMore = false,
                listError = GmailUiErrorCode.EMPTY_SEARCH,
                detail = null,
                detailError = null,
                thread = null,
                threadError = null,
            )
            return
        }
        consumedTokens.clear()
        failedAction = null
        navigateToList(nextRoute, stack)
        loadFirstPage(nextRoute)
    }

    /** Cancels a pending search and returns to the fictional inbox. */
    fun cancelSearch() {
        cancelActiveOperation()
        consumedTokens.clear()
        failedAction = null
        val inbox = GmailRoute.List(GmailListMode.INBOX)
        persistNavigation(inbox, emptyList())
        mutableState.value = GmailUiState(route = inbox, isLoading = true)
        loadFirstPage(inbox)
    }

    fun loadNextPage() {
        val current = mutableState.value
        val route = current.route as? GmailRoute.List ?: return
        val token = current.nextPageToken ?: return
        if (current.isLoading || current.isLoadingMore || !current.hasMore) return
        if (current.messages.size >= maximumMessages) {
            failedAction = null
            mutableState.value = current.copy(listError = GmailUiErrorCode.TOO_MANY_RESULTS, hasMore = false)
            return
        }
        loadPage(route, token, isFirstPage = false)
    }

    fun openMessage(id: GmailMessageId) {
        val current = mutableState.value
        if (current.route !is GmailRoute.List || current.messages.none { it.id == id }) return
        val route = GmailRoute.Message(id)
        val stack = current.backStack + current.route
        cancelActiveOperation()
        persistNavigation(route, stack)
        mutableState.value = current.copy(
            route = route,
            backStack = stack,
            isLoading = true,
            isLoadingMore = false,
            listError = null,
            detail = null,
            detailError = null,
            thread = null,
            threadError = null,
        )
        loadMessage(id)
    }

    fun openThread() {
        val current = mutableState.value
        val message = current.detail ?: return
        val route = GmailRoute.Thread(message.threadId)
        val stack = current.backStack + current.route
        cancelActiveOperation()
        persistNavigation(route, stack)
        mutableState.value = current.copy(route = route, backStack = stack, isLoading = true, thread = null, threadError = null)
        loadThread(route)
    }

    fun selectThreadMessage(id: GmailMessageId) {
        val current = mutableState.value
        val route = current.route as? GmailRoute.Thread ?: return
        if (current.thread?.messages?.any { it.id == id } != true) return
        val updated = route.copy(selectedMessageId = id)
        persistNavigation(updated, current.backStack)
        mutableState.value = current.copy(route = updated)
    }

    fun showThreadList() {
        val current = mutableState.value
        val route = current.route as? GmailRoute.Thread ?: return
        val updated = route.copy(selectedMessageId = null)
        persistNavigation(updated, current.backStack)
        mutableState.value = current.copy(route = updated)
    }

    fun back() {
        val current = mutableState.value
        if (current.backStack.isEmpty()) {
            if (current.route is GmailRoute.List && current.route.mode == GmailListMode.SEARCH) cancelSearch()
            return
        }
        cancelActiveOperation()
        failedAction = null
        val target = current.backStack.last()
        val remaining = current.backStack.dropLast(1)
        val returningFromDetailToList = target is GmailRoute.List &&
            (current.route is GmailRoute.Message || current.route is GmailRoute.Thread)
        val previousListContext = listContext(current.route, current.backStack)
        val targetListContext = listContext(target, remaining)
        if (previousListContext != targetListContext) {
            consumedTokens.clear()
            pagesRead = 0
        }
        persistNavigation(target, remaining)
        when (target) {
            is GmailRoute.List -> {
                mutableState.value = current.copy(
                    route = target,
                    backStack = remaining,
                    searchInput = if (returningFromDetailToList) current.searchInput else target.query,
                    isLoading = false,
                    isLoadingMore = false,
                    listError = null,
                    detail = null,
                    detailError = null,
                    thread = null,
                    threadError = null,
                )
                // List rows remain in memory while a detail route is open. If this
                // is process restoration, reload because message bodies are not saved.
                val returningFromDifferentList = current.route is GmailRoute.List && current.route != target
                if (returningFromDifferentList || mutableState.value.messages.isEmpty()) loadFirstPage(target)
            }
            is GmailRoute.Message -> {
                mutableState.value = current.copy(route = target, backStack = remaining, isLoading = true, thread = null, threadError = null)
                loadMessage(target.id)
            }
            is GmailRoute.Thread -> {
                mutableState.value = current.copy(route = target, backStack = remaining, isLoading = false)
            }
        }
    }

    fun retry() {
        when (val action = failedAction) {
            is FailedAction.FirstPage -> loadFirstPage(action.route)
            is FailedAction.NextPage -> loadPage(action.route, action.token, isFirstPage = false)
            is FailedAction.Restore -> loadFirstPage(action.route, action.target)
            is FailedAction.Message -> loadMessage(action.id)
            is FailedAction.Thread -> loadThread(action.route)
            null -> when (val route = mutableState.value.route) {
                is GmailRoute.List -> loadFirstPage(route)
                is GmailRoute.Message -> loadMessage(route.id)
                is GmailRoute.Thread -> loadThread(route)
            }
        }
    }

    private fun restoreContent() {
        val route = mutableState.value.route
        val listContext = (mutableState.value.backStack.firstOrNull { it is GmailRoute.List } as? GmailRoute.List)
            ?: (route as? GmailRoute.List)
            ?: GmailRoute.List(GmailListMode.INBOX)
        when (route) {
            is GmailRoute.List -> {
                if (route.mode == GmailListMode.SEARCH && route.query.isBlank()) {
                    consumedTokens.clear()
                    failedAction = null
                    mutableState.value = mutableState.value.copy(
                        messages = emptyList(),
                        nextPageToken = null,
                        hasMore = false,
                        isLoading = false,
                        isLoadingMore = false,
                        listError = GmailUiErrorCode.EMPTY_SEARCH,
                    )
                } else {
                    loadFirstPage(route)
                }
            }
            is GmailRoute.Message -> {
                mutableState.value = mutableState.value.copy(route = route, isLoading = true)
                loadFirstPage(listContext, restoreAfter = route)
            }
            is GmailRoute.Thread -> {
                mutableState.value = mutableState.value.copy(route = route, isLoading = true)
                loadFirstPage(listContext, restoreAfter = route)
            }
        }
    }

    private fun restoreState(): GmailUiState {
        val mode = runCatching { GmailListMode.valueOf(savedStateHandle[KEY_LIST_MODE] ?: GmailListMode.INBOX.name) }
            .getOrDefault(GmailListMode.INBOX)
        // A Gmail search can contain personal text. Older versions saved it in route IDs;
        // discard it on process recreation. The Activity-scoped ViewModel retains it through
        // configuration changes without placing it in the Bundle.
        savedStateHandle.remove<Any>(KEY_QUERY)
        val hadSavedSearch = mode == GmailListMode.SEARCH ||
            savedStateHandle.get<String>("${ROUTE_PREFIX}_type") == "search" ||
            (0 until MAX_SAVED_BACK_ROUTES).any { index ->
                savedStateHandle.get<String>("${BACK_PREFIX}${index}_type") == "search"
            }
        if (hadSavedSearch) {
            resetSavedNavigationToInbox()
            return GmailUiState(isLoading = true)
        }
        val fallbackList = GmailRoute.List(mode)
        val route = readRoute(ROUTE_PREFIX, fallbackList) ?: fallbackList
        val backStack = (0 until MAX_SAVED_BACK_ROUTES).mapNotNull { index ->
            readRoute("$BACK_PREFIX$index", fallbackList)
        }
        return GmailUiState(route = route, backStack = backStack, isLoading = true)
    }

    private fun resetSavedNavigationToInbox() {
        savedStateHandle[KEY_LIST_MODE] = GmailListMode.INBOX.name
        savedStateHandle.remove<Any>(KEY_QUERY)
        writeRoute(ROUTE_PREFIX, GmailRoute.List(GmailListMode.INBOX))
        for (index in 0 until MAX_SAVED_BACK_ROUTES) {
            savedStateHandle.remove<Any>("${BACK_PREFIX}${index}_type")
            savedStateHandle.remove<Any>("${BACK_PREFIX}${index}_id")
            savedStateHandle.remove<Any>("${BACK_PREFIX}${index}_selected")
        }
    }

    private fun navigateToList(route: GmailRoute.List, stack: List<GmailRoute> = mutableState.value.backStack) {
        cancelActiveOperation()
        persistNavigation(route, stack)
        mutableState.value = mutableState.value.copy(
            route = route,
            backStack = stack,
            messages = emptyList(),
            nextPageToken = null,
            hasMore = false,
            isLoading = true,
            isLoadingMore = false,
            listError = null,
            detail = null,
            detailError = null,
            thread = null,
            threadError = null,
            searchInput = route.query,
        )
    }

    private fun loadFirstPage(route: GmailRoute.List, restoreAfter: GmailRoute? = null) {
        consumedTokens.clear()
        pagesRead = 0
        loadPage(route, token = null, isFirstPage = true, restoreAfter = restoreAfter)
    }

    private fun loadPage(
        route: GmailRoute.List,
        token: String?,
        isFirstPage: Boolean,
        restoreAfter: GmailRoute? = null,
    ) {
        val generation = beginOperation()
        val current = mutableState.value
        failedAction = null
        mutableState.value = if (isFirstPage) {
            current.copy(isLoading = true, isLoadingMore = false, listError = null, messages = emptyList(), nextPageToken = null, hasMore = false)
        } else {
            current.copy(isLoadingMore = true, listError = null)
        }
        activeJob = viewModelScope.launch {
            try {
                requireMockAuthorization()
                if (!isFirstPage && token != null && (token.isBlank() || token.length > MAX_PAGE_TOKEN_LENGTH || token in consumedTokens)) {
                    throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
                }
                if (!isFirstPage && pagesRead >= maximumPages) {
                    throw GmailReadFailure(GmailReadFailureCode.TOO_MANY_PAGES)
                }
                val page = when (route.mode) {
                    GmailListMode.INBOX -> readPort.listInbox(token, pageSize)
                    GmailListMode.SEARCH -> readPort.search(route.query, token, pageSize)
                }
                ensureCurrent(generation)
                validatePage(page)
                val merged = mergePage(if (isFirstPage) emptyList() else mutableState.value.messages, page.messages)
                if (merged.size > maximumMessages) throw GmailReadFailure(GmailReadFailureCode.TOO_MANY_MESSAGES)
                val nextToken = page.nextPageToken
                if (nextToken != null && (nextToken.isBlank() || nextToken.length > MAX_PAGE_TOKEN_LENGTH || nextToken in consumedTokens || nextToken == token)) {
                    throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
                }
                if (token != null) consumedTokens += token
                pagesRead++
                ensureCurrent(generation)
                val exceededPageLimit = nextToken != null && pagesRead >= maximumPages
                val exceededMessageLimit = nextToken != null && merged.size >= maximumMessages
                mutableState.value = mutableState.value.copy(
                    messages = merged,
                    nextPageToken = nextToken,
                    hasMore = nextToken != null && !exceededPageLimit && !exceededMessageLimit,
                    isLoading = false,
                    isLoadingMore = false,
                    listError = if (exceededPageLimit || exceededMessageLimit) GmailUiErrorCode.TOO_MANY_RESULTS else null,
                )
                failedAction = null
                if (restoreAfter != null) {
                    when (restoreAfter) {
                        is GmailRoute.Message -> loadMessage(restoreAfter.id)
                        is GmailRoute.Thread -> loadThread(restoreAfter)
                        is GmailRoute.List -> Unit
                    }
                }
            } catch (failure: CancellationException) {
                if (failure is kotlinx.coroutines.TimeoutCancellationException) {
                    applyPageFailure(generation, GmailUiErrorCode.TIMEOUT, route, token, isFirstPage, restoreAfter)
                } else if (generation == operationGeneration) {
                    applyPageFailureNonCancellable(generation, GmailUiErrorCode.CANCELLED, route, token, isFirstPage, restoreAfter)
                }
            } catch (failure: GmailUiBoundaryFailure) {
                applyPageFailure(generation, failure.errorCode, route, token, isFirstPage, restoreAfter)
            } catch (failure: GmailReadFailure) {
                applyPageFailure(generation, failure.toUiError(), route, token, isFirstPage, restoreAfter)
            } catch (_: Exception) {
                applyPageFailure(generation, GmailUiErrorCode.UNAVAILABLE, route, token, isFirstPage, restoreAfter)
            }
        }
    }

    private fun loadMessage(id: GmailMessageId) {
        val generation = beginOperation()
        failedAction = null
        mutableState.value = mutableState.value.copy(isLoading = true, detail = null, detailError = null)
        activeJob = viewModelScope.launch {
            try {
                requireMockAuthorization()
                val message = readPort.getMessage(id)
                ensureCurrent(generation)
                validateMessage(message, expectedId = id)
                mutableState.value = mutableState.value.copy(isLoading = false, detail = message, detailError = null)
            } catch (failure: CancellationException) {
                if (failure is kotlinx.coroutines.TimeoutCancellationException) {
                    applyDetailFailure(generation, GmailUiErrorCode.TIMEOUT, FailedAction.Message(id))
                } else if (generation == operationGeneration) {
                    applyDetailFailureNonCancellable(generation, GmailUiErrorCode.CANCELLED, FailedAction.Message(id))
                }
            } catch (failure: GmailUiBoundaryFailure) {
                applyDetailFailure(generation, failure.errorCode, FailedAction.Message(id))
            } catch (failure: GmailReadFailure) {
                applyDetailFailure(generation, failure.toUiError(), FailedAction.Message(id))
            } catch (_: Exception) {
                applyDetailFailure(generation, GmailUiErrorCode.UNAVAILABLE, FailedAction.Message(id))
            }
        }
    }

    private fun loadThread(route: GmailRoute.Thread) {
        val generation = beginOperation()
        failedAction = null
        mutableState.value = mutableState.value.copy(isLoading = true, thread = null, threadError = null)
        activeJob = viewModelScope.launch {
            try {
                requireMockAuthorization()
                val thread = readPort.getThread(route.id)
                ensureCurrent(generation)
                val normalizedThread = validateThread(thread, route.id)
                mutableState.value = mutableState.value.copy(isLoading = false, thread = normalizedThread, threadError = null)
            } catch (failure: CancellationException) {
                if (failure is kotlinx.coroutines.TimeoutCancellationException) {
                    applyThreadFailure(generation, GmailUiErrorCode.TIMEOUT, FailedAction.Thread(route))
                } else if (generation == operationGeneration) {
                    applyThreadFailureNonCancellable(generation, GmailUiErrorCode.CANCELLED, FailedAction.Thread(route))
                }
            } catch (failure: GmailUiBoundaryFailure) {
                applyThreadFailure(generation, failure.errorCode, FailedAction.Thread(route))
            } catch (failure: GmailReadFailure) {
                applyThreadFailure(generation, failure.toUiError(), FailedAction.Thread(route))
            } catch (_: Exception) {
                applyThreadFailure(generation, GmailUiErrorCode.UNAVAILABLE, FailedAction.Thread(route))
            }
        }
    }

    private suspend fun requireMockAuthorization() {
        val status = authorizationPort.status()
        if (status.missingScopes.isNotEmpty()) throw GmailReadFailure(GmailReadFailureCode.MISSING_SCOPE)
        when (status.state) {
            GmailAuthorizationState.MOCK_ONLY -> Unit
            GmailAuthorizationState.DENIED -> throw GmailUiBoundaryFailure(GmailUiErrorCode.AUTHORIZATION_DENIED)
            GmailAuthorizationState.REVOKED -> throw GmailUiBoundaryFailure(GmailUiErrorCode.AUTHORIZATION_REVOKED)
            GmailAuthorizationState.EXPIRED -> throw GmailUiBoundaryFailure(GmailUiErrorCode.AUTHORIZATION_EXPIRED)
            GmailAuthorizationState.NOT_CONFIGURED,
            GmailAuthorizationState.NOT_CONNECTED,
            GmailAuthorizationState.AUTHORIZATION_REQUIRED -> throw GmailUiBoundaryFailure(GmailUiErrorCode.AUTHORIZATION_REQUIRED)
            GmailAuthorizationState.AUTHORIZED -> throw GmailUiBoundaryFailure(GmailUiErrorCode.MOCK_PROVIDER_REQUIRED)
            GmailAuthorizationState.ERROR -> throw GmailUiBoundaryFailure(GmailUiErrorCode.UNAVAILABLE)
        }
    }

    private fun validatePage(page: GmailMessagePage) {
        page.messages.forEach { validateMessage(it) }
    }

    private fun validateMessage(message: GmailMessage, expectedId: GmailMessageId? = null, expectedThreadId: GmailThreadId? = null) {
        if (message.source != GmailMessageSource.MOCK || message.dataOrigin != DataOrigin.GMAIL ||
            message.classification == DataClassification.PUBLIC ||
            (expectedId != null && message.id != expectedId) ||
            (expectedThreadId != null && message.threadId != expectedThreadId)
        ) throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
    }

    private fun validateThread(thread: GmailThread, expectedId: GmailThreadId): GmailThread {
        if (thread.id != expectedId || thread.messages.isEmpty()) throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
        val byId = linkedMapOf<GmailMessageId, GmailMessage>()
        thread.messages.forEach { message ->
            validateMessage(message, expectedThreadId = expectedId)
            val previous = byId.putIfAbsent(message.id, message)
            if (previous != null && previous != message) throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
        }
        val chronological = byId.values.withIndex().sortedWith(
            compareBy<IndexedValue<GmailMessage>> { it.value.internalDateMillis ?: Long.MAX_VALUE }
                .thenBy { it.index },
        ).map { it.value }
        return GmailThread(expectedId, chronological)
    }

    private fun mergePage(existing: List<GmailMessage>, incoming: List<GmailMessage>): List<GmailMessage> {
        val merged = linkedMapOf<GmailMessageId, GmailMessage>()
        existing.forEach { merged[it.id] = it }
        incoming.forEach { message ->
            val previous = merged[message.id]
            if (previous != null && previous != message) throw GmailReadFailure(GmailReadFailureCode.INVALID_RESPONSE)
            merged.putIfAbsent(message.id, message)
        }
        return merged.values.toList()
    }

    private fun beginOperation(): Long {
        activeJob?.cancel()
        val generation = ++operationGeneration
        return generation
    }

    private fun cancelActiveOperation() {
        operationGeneration++
        activeJob?.cancel()
        activeJob = null
    }

    private fun ensureCurrent(generation: Long) {
        if (generation != operationGeneration) throw CancellationException("Stale Gmail screen request.")
    }

    private fun applyFailure(generation: Long, code: GmailUiErrorCode, action: FailedAction) {
        if (generation != operationGeneration) return
        failedAction = action
        val state = mutableState.value
        mutableState.value = state.copy(isLoading = false, isLoadingMore = false, listError = code)
    }

    private fun applyPageFailure(
        generation: Long,
        code: GmailUiErrorCode,
        route: GmailRoute.List,
        token: String?,
        firstPage: Boolean,
        restoreAfter: GmailRoute?,
    ) {
        if (generation != operationGeneration) return
        val action = restoreAfter?.let { FailedAction.Restore(route, it) }
            ?: failedActionFor(route, token, firstPage)
        applyFailure(generation, code, action)
        when (restoreAfter) {
            is GmailRoute.Message -> mutableState.value = mutableState.value.copy(detailError = code)
            is GmailRoute.Thread -> mutableState.value = mutableState.value.copy(threadError = code)
            else -> Unit
        }
    }

    private suspend fun applyPageFailureNonCancellable(
        generation: Long,
        code: GmailUiErrorCode,
        route: GmailRoute.List,
        token: String?,
        firstPage: Boolean,
        restoreAfter: GmailRoute?,
    ) = withContext(kotlinx.coroutines.NonCancellable + Dispatchers.Main.immediate) {
        applyPageFailure(generation, code, route, token, firstPage, restoreAfter)
    }

    private fun applyDetailFailure(generation: Long, code: GmailUiErrorCode, action: FailedAction) {
        if (generation != operationGeneration) return
        failedAction = action
        mutableState.value = mutableState.value.copy(isLoading = false, detailError = code)
    }

    private suspend fun applyDetailFailureNonCancellable(generation: Long, code: GmailUiErrorCode, action: FailedAction) =
        withContext(kotlinx.coroutines.NonCancellable + Dispatchers.Main.immediate) { applyDetailFailure(generation, code, action) }

    private fun applyThreadFailure(generation: Long, code: GmailUiErrorCode, action: FailedAction) {
        if (generation != operationGeneration) return
        failedAction = action
        mutableState.value = mutableState.value.copy(isLoading = false, threadError = code)
    }

    private suspend fun applyThreadFailureNonCancellable(generation: Long, code: GmailUiErrorCode, action: FailedAction) =
        withContext(kotlinx.coroutines.NonCancellable + Dispatchers.Main.immediate) { applyThreadFailure(generation, code, action) }

    private fun failedActionFor(route: GmailRoute.List, token: String?, firstPage: Boolean): FailedAction =
        if (firstPage) FailedAction.FirstPage(route) else FailedAction.NextPage(route, token.orEmpty())

    private fun persistNavigation(route: GmailRoute, backStack: List<GmailRoute>) {
        val sensitiveSearch = listContext(route, backStack).mode == GmailListMode.SEARCH
        val safeRoute = if (sensitiveSearch) GmailRoute.List(GmailListMode.INBOX) else route
        val safeBackStack = if (sensitiveSearch) emptyList() else backStack
        savedStateHandle[KEY_LIST_MODE] = GmailListMode.INBOX.name
        savedStateHandle.remove<Any>(KEY_QUERY)
        writeRoute(ROUTE_PREFIX, safeRoute)
        safeBackStack.takeLast(MAX_SAVED_BACK_ROUTES).forEachIndexed { index, item -> writeRoute("$BACK_PREFIX$index", item) }
        for (index in safeBackStack.size.coerceAtMost(MAX_SAVED_BACK_ROUTES) until MAX_SAVED_BACK_ROUTES) {
            savedStateHandle.remove<Any>("$BACK_PREFIX${index}_type")
            savedStateHandle.remove<Any>("$BACK_PREFIX${index}_id")
            savedStateHandle.remove<Any>("$BACK_PREFIX${index}_selected")
        }
    }

    private fun listContext(route: GmailRoute, stack: List<GmailRoute>): GmailRoute.List =
        (route as? GmailRoute.List)
            ?: stack.lastOrNull { it is GmailRoute.List } as? GmailRoute.List
            ?: GmailRoute.List(GmailListMode.INBOX)

    private fun writeRoute(prefix: String, route: GmailRoute) {
        when (route) {
            is GmailRoute.List -> {
                savedStateHandle["${prefix}_type"] = if (route.mode == GmailListMode.INBOX) "inbox" else "search"
                savedStateHandle["${prefix}_id"] = route.query
                savedStateHandle["${prefix}_selected"] = ""
            }
            is GmailRoute.Message -> {
                savedStateHandle["${prefix}_type"] = "message"
                savedStateHandle["${prefix}_id"] = route.id.value
                savedStateHandle["${prefix}_selected"] = ""
            }
            is GmailRoute.Thread -> {
                savedStateHandle["${prefix}_type"] = "thread"
                savedStateHandle["${prefix}_id"] = route.id.value
                savedStateHandle["${prefix}_selected"] = route.selectedMessageId?.value.orEmpty()
            }
        }
    }

    private fun readRoute(prefix: String, fallbackList: GmailRoute.List): GmailRoute? {
        val type: String = savedStateHandle["${prefix}_type"] ?: return null
        val id: String = savedStateHandle["${prefix}_id"] ?: ""
        val selected: String = savedStateHandle["${prefix}_selected"] ?: ""
        return runCatching {
            when (type) {
                "inbox" -> GmailRoute.List(GmailListMode.INBOX)
                // Do not resurrect a query from an older Bundle. Search state is in-memory only.
                "search" -> GmailRoute.List(GmailListMode.INBOX)
                "message" -> GmailRoute.Message(GmailMessageId(id))
                "thread" -> GmailRoute.Thread(GmailThreadId(id), selected.takeIf(String::isNotEmpty)?.let(::GmailMessageId))
                else -> fallbackList
            }
        }.getOrDefault(fallbackList)
    }

    private sealed interface FailedAction {
        data class FirstPage(val route: GmailRoute.List) : FailedAction
        data class NextPage(val route: GmailRoute.List, val token: String) : FailedAction
        data class Restore(val route: GmailRoute.List, val target: GmailRoute) : FailedAction
        data class Message(val id: GmailMessageId) : FailedAction
        data class Thread(val route: GmailRoute.Thread) : FailedAction
    }

    private class GmailUiBoundaryFailure(val errorCode: GmailUiErrorCode) : Exception()

    private fun Throwable.toUiError(): GmailUiErrorCode = when (this) {
        is GmailUiBoundaryFailure -> errorCode
        is GmailReadFailure -> when (code) {
            GmailReadFailureCode.PERMISSION_DENIED -> GmailUiErrorCode.PERMISSION_DENIED
            GmailReadFailureCode.MISSING_SCOPE -> GmailUiErrorCode.MISSING_SCOPE
            GmailReadFailureCode.RATE_LIMITED -> GmailUiErrorCode.RATE_LIMITED
            GmailReadFailureCode.TIMEOUT -> GmailUiErrorCode.TIMEOUT
            GmailReadFailureCode.CANCELLED -> GmailUiErrorCode.CANCELLED
            GmailReadFailureCode.NOT_FOUND -> GmailUiErrorCode.NOT_FOUND
            GmailReadFailureCode.INVALID_RESPONSE -> GmailUiErrorCode.INVALID_DATA
            GmailReadFailureCode.TOO_MANY_PAGES,
            GmailReadFailureCode.TOO_MANY_MESSAGES -> GmailUiErrorCode.TOO_MANY_RESULTS
            GmailReadFailureCode.PARTIAL_FAILURE,
            GmailReadFailureCode.UNAVAILABLE -> GmailUiErrorCode.UNAVAILABLE
        }
        else -> GmailUiErrorCode.UNAVAILABLE
    }

    companion object {
        const val DEFAULT_PAGE_SIZE = 50
        const val DEFAULT_MAXIMUM_PAGES = 20
        const val DEFAULT_MAXIMUM_MESSAGES = 500
        const val MAX_PAGE_TOKEN_LENGTH = 2_048
        private const val MAX_SAVED_BACK_ROUTES = 2
        private const val ROUTE_PREFIX = "gmail_route"
        private const val BACK_PREFIX = "gmail_back_"
        private const val KEY_LIST_MODE = "gmail_list_mode"
        private const val KEY_QUERY = "gmail_search_query"

        fun factory(readPort: GmailReadPort, authorizationPort: GmailAuthorizationPort): ViewModelProvider.Factory =
            viewModelFactory {
                initializer {
                    GmailViewModel(readPort, authorizationPort, createSavedStateHandle())
                }
            }
    }
}
