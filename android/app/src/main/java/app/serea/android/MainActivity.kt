package app.serea.android

import android.app.Activity
import android.content.ContextWrapper
import android.os.Bundle
import android.view.WindowManager
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.imePadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.DropdownMenu
import androidx.compose.material3.DropdownMenuItem
import androidx.compose.material3.HorizontalDivider
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.ui.text.input.KeyboardType
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.withFrameNanos
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.Job
import kotlinx.coroutines.NonCancellable
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.currentCoroutineContext
import kotlinx.coroutines.delay
import kotlinx.coroutines.ensureActive
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext
import kotlinx.coroutines.withTimeout
import java.time.Instant
import java.util.UUID

class MainActivity : ComponentActivity() {
    private val database by lazy { SereaHistoryDatabase.open(this) }
    private val history by lazy {
        ConversationRepository(database.historyDao(), KeystoreAesGcm("serea-history-v1"))
    }
    private val usage by lazy { UsageRepository(database.usageDao()) }
    private val credentials by lazy { CredentialRepository(this) }
    private val tasksDatabase by lazy { app.serea.android.tasks.TasksDatabase.open(this) }
    private val taskClock: () -> Instant = Instant::now
    private val taskRepository by lazy {
        app.serea.android.tasks.EncryptedTaskRepository(
            tasksDatabase.taskDao(), app.serea.android.tasks.KeystoreTaskCipher(), taskClock,
        )
    }
    internal val shellTaskViewModel: app.serea.android.tasks.TaskViewModel by viewModels {
        app.serea.android.tasks.TaskViewModel.factory(taskRepository, taskClock)
    }
    internal val chatViewModel: ChatViewModel by viewModels {
        ChatViewModel.factory(MockModelChatPort(), history, usage)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { SereaTheme { SereaAppShell(chatViewModel, credentials, usage, shellTaskViewModel) } }
    }
}

data class ChatMessage(
    val id: String,
    val role: MessageRole,
    val text: String,
    val createdAt: Instant,
    val classification: DataClassification = DataClassification.UNKNOWN,
    val status: String = "COMPLETED",
    val origin: DataOrigin = DataOrigin.UNKNOWN,
    val conversationId: String? = null,
)

enum class MessageRole { USER, ASSISTANT }
enum class DataClassification { PUBLIC, PERSONAL, PRIVATE, SECRET, CREDENTIAL, UNKNOWN }
enum class DataOrigin { USER_AUTHORED, MODEL_OUTPUT, CALENDAR, GMAIL, DEVICE, UNKNOWN }

data class Conversation(
    val id: String,
    val messages: List<ChatMessage> = emptyList(),
    val createdAt: Instant = Instant.now(),
    val expiresAt: Instant? = null,
    val deletedAt: Instant? = null,
)

data class ChatUiState(
    val conversation: Conversation = Conversation(id = UUID.randomUUID().toString()),
    val input: String = "",
    val isLoading: Boolean = false,
    val isFinalizing: Boolean = false,
    val isRestoring: Boolean = false,
    val showingHistory: Boolean = false,
    val history: List<StoredConversation> = emptyList(),
    val errorMessage: String? = null,
    val statusMessage: String? = null,
)

data class ModelChatRequest(
    val conversationId: String,
    val message: String,
    val sendId: String = UUID.randomUUID().toString(),
    val timeoutMillis: Long = 15_000,
    val classification: DataClassification = DataClassification.UNKNOWN,
    val preparedPersonal: PreparedPersonalData? = null,
    val context: List<ContextMessage> = emptyList(),
    val egressContract: EgressContract? = null,
    val selectedModelId: String? = null,
)
data class ModelChatResponse(val text: String, val usage: ProviderUsage? = null)

interface ModelChatPort {
    /** Must complete within request.timeoutMillis; throws ModelChatFailure on a definite failure. */
    suspend fun send(request: ModelChatRequest): ModelChatResponse
}

open class ModelChatFailure(message: String) : Exception(message)

/** A local validation failed before the provider adapter could dispatch a request. */
internal class PreDispatchModelChatFailure(message: String) : ModelChatFailure(message)

/** Deterministic demo only. It makes no network calls and never claims cloud inference. */
class MockModelChatPort(private val responseDelayMillis: Long = 450) : ModelChatPort {
    override suspend fun send(request: ModelChatRequest): ModelChatResponse {
        kotlinx.coroutines.delay(responseDelayMillis)
        val turns = request.context.size / 2
        val prior = request.context.lastOrNull { it.role == MessageRole.USER }?.text
        val contextNote = prior?.let { " I remember your previous message: “$it”." }.orEmpty()
        return ModelChatResponse("Demo response: I received “${request.message}”.$contextNote Context turns: $turns. No cloud AI was contacted.")
    }
}

internal class ChatViewModel(
    private val modelChat: ModelChatPort,
    private val historyRepository: ConversationHistoryPort? = null,
    private val usageRepository: UsageRepository? = null,
    initialConversation: Conversation? = null,
) : ViewModel() {
    private val mutableState = MutableStateFlow(ChatUiState(
        conversation = initialConversation ?: Conversation(id = UUID.randomUUID().toString()),
        isRestoring = historyRepository != null && initialConversation == null,
    ))
    val state: StateFlow<ChatUiState> = mutableState.asStateFlow()
    private var activeJob: Job? = null
    private var activeMessageId: String? = null
    private val finalizingMessageIds = mutableSetOf<String>()
    private var selectionGeneration = 0L
    private val persistedConversationIds = mutableSetOf<String>().apply {
        if (historyRepository != null && initialConversation != null) add(initialConversation.id)
    }

    init { if (historyRepository != null && initialConversation == null) restoreLatest() }

    fun updateInput(value: String) = mutableState.update { it.copy(input = value, errorMessage = null) }

    fun sendMessage() {
        val snapshot = mutableState.value
        val messageText = snapshot.input.trim()
        if (messageText.isEmpty() || snapshot.isLoading || snapshot.isRestoring || snapshot.showingHistory) return
        if (messageText.length > ChatContextBuilder.MAX_INPUT_CHARS) {
            mutableState.update { it.copy(errorMessage = "Message exceeds the ${ChatContextBuilder.MAX_INPUT_CHARS}-character local limit.") }
            return
        }
        val conversationId = snapshot.conversation.id
        val userMessage = ChatMessage(UUID.randomUUID().toString(), MessageRole.USER, messageText, nextMessageTime(snapshot.conversation), status = "PENDING",
            origin = DataOrigin.USER_AUTHORED, conversationId = conversationId)
        activeMessageId = userMessage.id
        mutableState.update {
            it.copy(conversation = it.conversation.copy(messages = it.conversation.messages + userMessage), input = "", isLoading = true, isFinalizing = false, errorMessage = null)
        }
        activeJob = viewModelScope.launch {
            var turnFinalized = false
            try {
                if (historyRepository != null) {
                    if (conversationId in persistedConversationIds) {
                        persist(userMessage, conversationId)
                    } else {
                        historyRepository.createWithFirstMessage(
                            conversationId, conversationTitle(messageText), userMessage.toStoredMessage(conversationId),
                        )
                        persistedConversationIds += conversationId
                    }
                }
                val context = ChatContextBuilder.build(snapshot.conversation.messages, messageText, conversationId)
                val request = ModelChatRequest(conversationId, messageText, sendId = userMessage.id, context = context)
                val reply = withTimeout(request.timeoutMillis) { modelChat.send(request) }
                currentCoroutineContext().ensureActive()
                if (activeMessageId != userMessage.id || mutableState.value.conversation.id != conversationId) {
                    markUnknown(userMessage, conversationId, "Request cancelled. A transmitted request may still have completed remotely.")
                    return@launch
                }
                val assistant = ChatMessage(UUID.randomUUID().toString(), MessageRole.ASSISTANT, reply.text,
                    nextMessageTime(snapshot.conversation.copy(messages = snapshot.conversation.messages + userMessage)),
                    origin = DataOrigin.MODEL_OUTPUT, conversationId = conversationId)
                mutableState.update { current ->
                    if (current.conversation.id != conversationId || activeMessageId != userMessage.id) current
                    else current.copy(isFinalizing = true)
                }
                finalizingMessageIds += userMessage.id
                withContext(NonCancellable) {
                    historyRepository?.completeTurn(userMessage.id, assistant.toStoredMessage(conversationId))
                    turnFinalized = true
                    mutableState.update { current ->
                        if (current.conversation.id != conversationId) current
                        else {
                            val ownsActiveRequest = activeMessageId == userMessage.id
                            current.copy(
                                conversation = current.conversation.copy(messages = current.conversation.messages
                                    .map { if (it.id == userMessage.id) it.copy(status = "COMPLETED") else it } + assistant),
                                isLoading = if (ownsActiveRequest) false else current.isLoading,
                                isFinalizing = if (ownsActiveRequest) false else current.isFinalizing,
                                errorMessage = if (ownsActiveRequest) null else current.errorMessage,
                            )
                        }
                    }
                }
                refreshHistory()
            } catch (_: TimeoutCancellationException) {
                markUnknown(userMessage, conversationId, "The request timed out. Its remote outcome is unknown; retry explicitly.")
            } catch (cancelled: CancellationException) {
                if (!turnFinalized) {
                    withContext(NonCancellable) { markUnknown(userMessage, conversationId, "Request cancelled. A transmitted request may still have completed remotely.") }
                }
            } catch (failure: ModelChatFailure) {
                markFailed(userMessage, conversationId, failure.message ?: "The model request failed.")
            } catch (_: UsageLimitExceededException) {
                markFailed(userMessage, conversationId, "ローカルの日次または月次Soft Limitを超えました。設定で上限を確認してください。")
            } catch (_: Exception) {
                markFailed(userMessage, conversationId)
            } finally {
                finalizingMessageIds -= userMessage.id
                if (activeMessageId == userMessage.id) { activeMessageId = null; activeJob = null }
            }
        }
    }

    fun cancelCurrent() {
        val id = activeMessageId ?: return
        if (id in finalizingMessageIds) return
        val conversationId = mutableState.value.conversation.id
        activeJob?.cancel()
        mutableState.update { it.copy(isLoading = false, errorMessage = "Request cancelled. A transmitted request may still have completed remotely.") }
        viewModelScope.launch { historyRepository?.updateStatus(id, conversationId, "OUTCOME_UNKNOWN") }
    }

    fun newConversation() {
        cancelCurrent()
        selectionGeneration += 1
        val id = UUID.randomUUID().toString()
        mutableState.update { it.copy(conversation = Conversation(id), input = "", isLoading = false, isFinalizing = false, showingHistory = false,
            isRestoring = false, errorMessage = null, statusMessage = null) }
    }

    fun showHistory() {
        if (mutableState.value.isRestoring) return
        if (finalizingMessageIds.isNotEmpty()) {
            mutableState.update { it.copy(errorMessage = "応答の保存中は履歴を開けません。") }
            return
        }
        val generation = ++selectionGeneration
        viewModelScope.launch {
            try {
                val conversations = historyRepository?.list().orEmpty()
                if (generation != selectionGeneration) return@launch
                mutableState.update { it.copy(history = conversations, showingHistory = true, isLoading = false, isFinalizing = false) }
            } catch (_: HistoryUnavailableException) {
                if (generation == selectionGeneration) mutableState.update { it.copy(errorMessage = "Saved history is unavailable because its encryption key or data is invalid.") }
            } catch (_: Exception) {
                if (generation == selectionGeneration) mutableState.update { it.copy(errorMessage = "Could not open saved history.") }
            }
        }
    }

    fun openConversation(id: String) {
        if (mutableState.value.isRestoring) return
        if (finalizingMessageIds.isNotEmpty()) {
            mutableState.update { it.copy(errorMessage = "応答の保存中は履歴を切り替えられません。") }
            return
        }
        cancelCurrent()
        val generation = ++selectionGeneration
        viewModelScope.launch {
            try {
                val saved = historyRepository?.get(id) ?: return@launch
                if (generation != selectionGeneration) return@launch
                persistedConversationIds += saved.id
                val messages = saved.messages.map { message ->
                    if (message.status == "PENDING") {
                        historyRepository?.updateStatus(message.id, saved.id, "OUTCOME_UNKNOWN")
                        message.copy(status = "OUTCOME_UNKNOWN").toChatMessage()
                    } else message.toChatMessage()
                }
                if (generation != selectionGeneration) return@launch
                mutableState.update { it.copy(conversation = Conversation(saved.id, messages, Instant.ofEpochMilli(saved.createdAt)), showingHistory = false, isLoading = false, isFinalizing = false) }
            } catch (_: HistoryUnavailableException) {
                if (generation == selectionGeneration) mutableState.update { it.copy(errorMessage = "Saved history is unavailable because its encryption key or data is invalid.") }
            } catch (_: Exception) {
                if (generation == selectionGeneration) mutableState.update { it.copy(errorMessage = "Could not open the selected conversation.") }
            }
        }
    }

    fun deleteCurrentConversation(expectedId: String) {
        if (finalizingMessageIds.isNotEmpty()) {
            mutableState.update { it.copy(errorMessage = "応答の保存中は会話を削除できません。") }
            return
        }
        val current = mutableState.value
        if (current.isRestoring || current.conversation.id != expectedId) {
            mutableState.update { it.copy(errorMessage = "削除対象の会話が切り替わったため、削除を中止しました。") }
            return
        }
        val id = expectedId
        cancelCurrent()
        val generation = ++selectionGeneration
        mutableState.update { it.copy(isRestoring = true, isLoading = false, isFinalizing = false, statusMessage = null) }
        viewModelScope.launch {
            try {
                historyRepository?.delete(id)
                if (generation != selectionGeneration) return@launch
                newConversation()
                refreshHistory()
                val completedGeneration = selectionGeneration
                mutableState.update { if (completedGeneration == selectionGeneration) it.copy(statusMessage = "この会話を削除しました。") else it }
            } catch (_: Exception) {
                if (generation == selectionGeneration) mutableState.update { it.copy(errorMessage = "この会話を削除できませんでした。", isRestoring = false) }
            }
        }
    }

    fun deleteAllHistory() {
        if (mutableState.value.isRestoring) return
        if (finalizingMessageIds.isNotEmpty()) {
            mutableState.update { it.copy(errorMessage = "応答の保存中は履歴を削除できません。") }
            return
        }
        cancelCurrent()
        val generation = ++selectionGeneration
        mutableState.update { it.copy(isRestoring = true, isLoading = false, isFinalizing = false, statusMessage = null) }
        viewModelScope.launch {
            try {
                historyRepository?.deleteAll()
                if (generation != selectionGeneration) return@launch
                newConversation()
                val completedGeneration = selectionGeneration
                mutableState.update { if (completedGeneration == selectionGeneration) it.copy(history = emptyList(), statusMessage = "全会話履歴を削除しました。") else it }
            } catch (_: Exception) {
                if (generation == selectionGeneration) mutableState.update { it.copy(errorMessage = "全会話履歴を削除できませんでした。", isRestoring = false) }
            }
        }
    }

    private fun restoreLatest() {
        val generation = selectionGeneration
        viewModelScope.launch {
            try {
                usageRepository?.recoverAfterProcessRestart()
                val conversations = historyRepository!!.list()
                if (generation != selectionGeneration) return@launch
                val latest = conversations.firstOrNull()
                if (latest == null) {
                    // Keep the new empty conversation in memory until its first message is sent.
                } else {
                    persistedConversationIds += latest.id
                    val messages = latest.messages.map { message ->
                        if (message.status == "PENDING") historyRepository.updateStatus(message.id, latest.id, "OUTCOME_UNKNOWN")
                        message.copy(status = if (message.status == "PENDING") "OUTCOME_UNKNOWN" else message.status).toChatMessage()
                    }
                    if (generation != selectionGeneration) return@launch
                    mutableState.update { it.copy(conversation = Conversation(latest.id, messages, Instant.ofEpochMilli(latest.createdAt))) }
                }
            } catch (_: HistoryUnavailableException) {
                if (generation == selectionGeneration) mutableState.update { it.copy(errorMessage = "Saved history is unavailable because its encryption key or data is invalid.") }
            } catch (_: Exception) {
                if (generation == selectionGeneration) mutableState.update { it.copy(errorMessage = "Could not restore saved history.") }
            } finally {
                if (generation == selectionGeneration) mutableState.update { it.copy(isRestoring = false) }
            }
        }
    }

    private suspend fun persist(message: ChatMessage, conversationId: String) {
        historyRepository?.addMessage(message.toStoredMessage(conversationId))
    }
    private fun nextMessageTime(conversation: Conversation): Instant = maxOf(
        Instant.now(), (conversation.messages.maxOfOrNull { it.createdAt } ?: Instant.EPOCH).plusMillis(1),
    )
    private suspend fun markUnknown(message: ChatMessage, conversationId: String, reason: String) {
        historyRepository?.updateStatus(message.id, conversationId, "OUTCOME_UNKNOWN")
        mutableState.update { current ->
            if (current.conversation.id != conversationId) current
            else {
                val ownsActiveRequest = activeMessageId == message.id
                current.copy(
                    isLoading = if (ownsActiveRequest) false else current.isLoading,
                    isFinalizing = if (ownsActiveRequest) false else current.isFinalizing,
                    conversation = current.conversation.copy(messages = current.conversation.messages
                        .map { if (it.id == message.id) it.copy(status = "OUTCOME_UNKNOWN") else it }),
                    errorMessage = if (ownsActiveRequest) reason else current.errorMessage,
                )
            }
        }
    }
    private suspend fun markFailed(message: ChatMessage, conversationId: String, reason: String = "Demo response failed. Please try again.") {
        historyRepository?.updateStatus(message.id, conversationId, "FAILED")
        mutableState.update { current ->
            if (current.conversation.id != conversationId) current
            else {
                val ownsActiveRequest = activeMessageId == message.id
                current.copy(
                    isLoading = if (ownsActiveRequest) false else current.isLoading,
                    isFinalizing = if (ownsActiveRequest) false else current.isFinalizing,
                    conversation = current.conversation.copy(messages = current.conversation.messages
                        .map { if (it.id == message.id) it.copy(status = "FAILED") else it }),
                    errorMessage = if (ownsActiveRequest) reason else current.errorMessage,
                )
            }
        }
    }
    private fun refreshHistory() {
        val generation = selectionGeneration
        viewModelScope.launch {
            runCatching { historyRepository?.list().orEmpty() }.onSuccess { list ->
                if (generation == selectionGeneration) mutableState.update { it.copy(history = list) }
            }
        }
    }

    companion object {
        fun factory(port: ModelChatPort, history: ConversationHistoryPort? = null, usage: UsageRepository? = null): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T = ChatViewModel(port, history, usage) as T
        }
    }
}

internal fun validateUsageLimits(dailyRequests: String, dailyTokens: String, monthlyRequests: String, monthlyTokens: String): UsageLimits {
    fun positiveLong(raw: String, label: String): Long {
        val value = raw.trim().toLongOrNull() ?: throw IllegalArgumentException("$label はLong範囲内の整数で入力してください。")
        require(value > 0) { "$label は1以上で入力してください。" }
        return value
    }
    val dailyRequestLimit = positiveLong(dailyRequests, "日次requests")
    val dailyTokenLimit = positiveLong(dailyTokens, "日次tokens")
    val monthlyRequestLimit = positiveLong(monthlyRequests, "月次requests")
    val monthlyTokenLimit = positiveLong(monthlyTokens, "月次tokens")
    require(dailyRequestLimit <= monthlyRequestLimit) { "日次requestsは月次requests以下にしてください。" }
    require(dailyTokenLimit <= monthlyTokenLimit) { "日次tokensは月次tokens以下にしてください。" }
    return UsageLimits(dailyRequestLimit, dailyTokenLimit, monthlyRequestLimit, monthlyTokenLimit)
}

private fun ChatMessage.toStoredMessage(conversationId: String) = StoredMessage(
    id, conversationId, role, text, classification, createdAt.toEpochMilli(), status, origin,
)

internal fun conversationTitle(firstMessage: String): String {
    val clean = buildString(firstMessage.length) {
        firstMessage.forEach { char -> append(if (char.isISOControl() || char == '\u2028' || char == '\u2029') ' ' else char) }
    }.replace(Regex("\\s+"), " ").trim()
    if (clean.isEmpty()) return "新しい会話"
    val limit = 48
    val codePointCount = clean.codePointCount(0, clean.length)
    val truncated = codePointCount > limit
    val visibleCodePoints = if (truncated) limit - 1 else limit
    val end = clean.offsetByCodePoints(0, minOf(codePointCount, visibleCodePoints))
    return clean.substring(0, end).trimEnd() + if (truncated) "…" else ""
}

internal fun conversationDisplayTitle(conversation: Conversation): String =
    conversation.messages.firstOrNull { it.role == MessageRole.USER }?.text?.let(::conversationTitle)
        ?: "新しい会話 · ${conversation.id.take(8)}"

internal fun storedConversationDisplayTitle(conversation: StoredConversation): String {
    val legacyTitle = Regex("^Conversation [0-9a-fA-F]{8}$").matches(conversation.title)
    if (!legacyTitle) return conversation.title
    return conversation.messages.firstOrNull { it.role == MessageRole.USER }?.text?.let(::conversationTitle)
        ?: conversation.title
}

private fun <T> Knowledge<T>.displayValue(): String = when (this) {
    Knowledge.UNKNOWN -> "UNKNOWN"
    is Knowledge.Known<*> -> value.toString()
}

private fun StoredMessage.toChatMessage() = ChatMessage(id, role, text, Instant.ofEpochMilli(createdAt), classification,
    status, origin, conversationId)

@Composable
internal fun ChatScreen(
    chatViewModel: ChatViewModel = androidx.lifecycle.viewmodel.compose.viewModel(),
    credentials: CredentialRepository? = null,
    usage: UsageRepository? = null,
    onOpenTasks: (() -> Unit)? = null,
    onOpenSettings: (() -> Unit)? = null,
    settingsDestination: Boolean = false,
    onSettingsClosed: (() -> Unit)? = null,
) {
    val state by chatViewModel.state.collectAsState()
    val keyboard = LocalSoftwareKeyboardController.current
    val context = LocalContext.current
    val hostActivity = remember(context) {
        var current: android.content.Context? = context
        while (current != null && current !is Activity) current = (current as? ContextWrapper)?.baseContext
        current as? Activity
    }
    val listState = rememberLazyListState()
    val scope = rememberCoroutineScope()
    var settingsVisible by rememberSaveable(settingsDestination) { mutableStateOf(settingsDestination) }
    var previewVisible by remember { mutableStateOf(false) }
    var secretText by remember { mutableStateOf("") }
    var settingsMessage by remember { mutableStateOf("") }
    var overflowExpanded by remember { mutableStateOf(false) }
    var deleteConfirmation by rememberSaveable { mutableStateOf("") }
    var deleteTargetId by rememberSaveable { mutableStateOf("") }
    var deleteTargetTitle by rememberSaveable { mutableStateOf("") }
    var credentialReady by remember { mutableStateOf(credentials?.isConfigured() == true) }
    var dailyRequestsText by remember { mutableStateOf("20") }
    var dailyTokensText by remember { mutableStateOf("10000") }
    var monthlyRequestsText by remember { mutableStateOf("100") }
    var monthlyTokensText by remember { mutableStateOf("50000") }
    var usageSnapshot by remember { mutableStateOf<UsageSnapshot?>(null) }
    val modelSettings = remember(context) { ModelSelectionSettings(context.getSharedPreferences("model_selection", android.content.Context.MODE_PRIVATE)) }
    val modelRouter = remember { AdaptiveModelRouter(LocalModelCatalog()) }
    var modelPolicy by remember { mutableStateOf(modelSettings.load()) }
    var budgetAmountText by remember { mutableStateOf(modelPolicy.maxEstimatedCost?.toString().orEmpty()) }
    var budgetCurrencyText by remember { mutableStateOf(modelPolicy.budgetCurrencyCode.orEmpty()) }
    val modelCatalog by remember { mutableStateOf(LocalModelCatalog().all()) }
    val routePreview = remember(modelPolicy, state.input, state.conversation.messages.size) {
        modelRouter.select(RoutingRequest(state.input.trim(), state.conversation.messages.sumOf { it.text.length }), modelPolicy)
    }
    LaunchedEffect(settingsVisible) {
        if (settingsVisible) {
            usageSnapshot = runCatching { usage?.snapshot() }.getOrNull()
            usageSnapshot?.limits?.let { limits ->
                dailyRequestsText = limits.dailyRequests.toString()
                dailyTokensText = limits.dailyTokens.toString()
                monthlyRequestsText = limits.monthlyRequests.toString()
                monthlyTokensText = limits.monthlyTokens.toString()
            }
        }
    }
    LaunchedEffect(state.conversation.messages.size, state.isLoading, state.showingHistory, settingsVisible) {
        if (!state.showingHistory && !settingsVisible) {
            val targetIndex = state.conversation.messages.size + if (state.isLoading) 1 else 0
            if (targetIndex > 0) {
                withFrameNanos { }
                withFrameNanos { }
                listState.scrollToItem(targetIndex - 1)
            }
        }
    }
    DisposableEffect(hostActivity, settingsVisible) {
        val window = hostActivity?.window
        if (settingsVisible) window?.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        else window?.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
        onDispose {
            if (settingsVisible) window?.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
        }
    }
    val keystoreProfile = remember(credentials, settingsVisible) {
        if (settingsVisible) runCatching { credentials?.securityProfile() }.getOrNull() ?: "未確認" else "未確認"
    }
    Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
        val screenModifier = Modifier.fillMaxSize().imePadding()
            .then(if (settingsVisible) Modifier.verticalScroll(rememberScrollState()) else Modifier)
            .padding(16.dp)
        Column(modifier = screenModifier, verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text("Serea", style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.Bold,
                modifier = Modifier.semantics { heading() })
            if (!settingsDestination) {
                Button(onClick = {
                    if (onOpenTasks != null) onOpenTasks()
                    else context.startActivity(android.content.Intent(context, app.serea.android.tasks.TasksActivity::class.java))
                }, modifier = Modifier.testTag("open_tasks")) { Text("タスク") }
            }
            Text("DEMO · Mock会話 · クラウドAI未接続", color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelLarge)
            Text("Cloud未利用 · Provider利用条件/送信対象未承認 · 資格情報: ${if (credentialReady) "端末内に保存済み" else "未設定"} · オフラインでもデモ会話は利用できます。",
                style = MaterialTheme.typography.bodySmall)
            if (settingsVisible) {
                Text("モデル選択", style = MaterialTheme.typography.titleMedium, modifier = Modifier.semantics { heading() })
                Text("選択設定は端末内に保存されます。現在は検証済みCatalogが空のため、外部モデルは選択できません。デモ会話とCloud無効状態は維持されます。",
                    style = MaterialTheme.typography.bodySmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(onClick = {
                        modelPolicy = modelPolicy.copy(mode = ModelSelectionMode.AUTO, manualModelId = null)
                        modelSettings.save(modelPolicy)
                    }, enabled = modelPolicy.mode != ModelSelectionMode.AUTO) { Text("自動") }
                    Button(onClick = {
                        modelPolicy = modelPolicy.copy(mode = ModelSelectionMode.MANUAL)
                        modelSettings.save(modelPolicy)
                    }, enabled = modelPolicy.mode != ModelSelectionMode.MANUAL) { Text("手動") }
                }
                Text("自動選択方針", style = MaterialTheme.typography.labelLarge)
                Row(horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                    AutoRoutingPolicy.entries.forEach { option ->
                        TextButton(onClick = {
                            modelPolicy = modelPolicy.copy(autoPolicy = option)
                            modelSettings.save(modelPolicy)
                        }, enabled = modelPolicy.autoPolicy != option) {
                            Text(when (option) { AutoRoutingPolicy.BALANCED -> "バランス重視"; AutoRoutingPolicy.COST_PRIORITY -> "費用優先"; AutoRoutingPolicy.QUALITY_PRIORITY -> "品質優先" })
                        }
                    }
                }
                Text("選択方式: ${if (modelPolicy.mode == ModelSelectionMode.AUTO) "自動" else "手動"} · 方針: ${when (modelPolicy.autoPolicy) { AutoRoutingPolicy.BALANCED -> "バランス重視"; AutoRoutingPolicy.COST_PRIORITY -> "費用優先"; AutoRoutingPolicy.QUALITY_PRIORITY -> "品質優先" }}", style = MaterialTheme.typography.bodyMedium)
                Text("選択モデルID: ${routePreview.selectedModelId ?: "なし"}" +
                    (modelPolicy.manualModelId?.let { " · 保存済み手動指定: $it" } ?: ""), style = MaterialTheme.typography.bodyMedium)
                routePreview.estimatedCost?.let { cost ->
                    Text("1要求の推定費用: ${cost.amount} ${cost.currencyCode} · 入力予約 ${cost.estimatedInputTokens} トークン / 出力予約 ${cost.reservedOutputTokens} トークン。Provider請求額ではありません。",
                        style = MaterialTheme.typography.bodySmall)
                } ?: Text("推定費用: 料金または通貨が未確認のため算出できません。", style = MaterialTheme.typography.bodySmall)
                routePreview.model?.let { model ->
                    Text("${model.displayName} · ${model.providerId} · Context ${model.contextWindow.knownInt()?.toString() ?: "UNKNOWN"} · 最大出力 ${model.maxOutput.knownInt()?.toString() ?: "UNKNOWN"} · 入力 ${model.capabilities.input.joinToString().ifEmpty { "UNKNOWN" }} · Tool ${model.capabilities.toolCalling.displayValue()} · Structured ${model.capabilities.structuredOutput.displayValue()} · Reasoning ${model.capabilities.reasoning.displayValue()} · 利用枠 ${model.contractLimits.displayValue()} · 利用可否 ${model.availability} · 最終確認 ${model.lastVerifiedAt ?: "UNKNOWN"} · 情報源 ${model.source ?: "UNKNOWN"}",
                        style = MaterialTheme.typography.bodySmall)
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(value = budgetAmountText, onValueChange = { budgetAmountText = it.filter { c -> c.isDigit() || c == '.' } },
                        modifier = Modifier.weight(2f), label = { Text("任意の1要求費用上限") },
                        keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Decimal), singleLine = true)
                    OutlinedTextField(value = budgetCurrencyText, onValueChange = { budgetCurrencyText = it.filter(Char::isLetter).take(3).uppercase() },
                        modifier = Modifier.weight(1f), label = { Text("通貨コード") }, singleLine = true)
                }
                Button(onClick = {
                    val amount = budgetAmountText.trim().takeIf { it.isNotEmpty() }?.toDoubleOrNull()
                    val currency = budgetCurrencyText.trim().uppercase()
                    runCatching {
                        require(budgetAmountText.isBlank() || amount != null && amount.isFinite() && amount >= 0)
                        require(amount == null || currency.matches(Regex("[A-Z]{3}")))
                        modelPolicy.copy(maxEstimatedCost = amount, budgetCurrencyCode = currency.takeIf { amount != null })
                    }.onSuccess { updated ->
                        modelSettings.save(updated); modelPolicy = updated
                        settingsMessage = "費用上限を保存しました。ローカル推定でありProvider請求上限ではありません。"
                    }.onFailure { settingsMessage = "費用上限を保存できません。金額と3文字の通貨コードを確認してください。" }
                }) { Text("ルーター設定を保存") }
                Text(if (modelCatalog.isEmpty()) "利用可能なモデル: 0 · Catalog未取得または未確認" else "利用可能なモデル: ${modelCatalog.size}")
                if (modelCatalog.isEmpty()) Text("手動モデル選択: 選択可能なモデルはありません。任意のモデルIDやProvider Hostは入力できません。",
                    style = MaterialTheme.typography.bodySmall)
                Text("判定: ${routePreview.task} · ${routePreview.reason}", style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite })
                Text("制約: ${routePreview.constraints.joinToString(" · ")}", style = MaterialTheme.typography.bodySmall)
                Text("候補除外: ${routePreview.excluded.joinToString { it.reason }.ifEmpty { "なし" }} · 未確認: ${routePreview.unknownFields.joinToString().ifEmpty { "なし" }}",
                    style = MaterialTheme.typography.bodySmall)
                Text("料金: 確認済みモデルがないため表示なし · Context: Catalog未確認 · 利用可否: 未確認 · 最終確認日時: なし · 本番Cloud: 無効",
                    style = MaterialTheme.typography.bodySmall)
                usageSnapshot?.let { usageState ->
                    Text("ローカルソフト上限（UTC）: 今日 ${usageState.dailyRequests}/${usageState.limits.dailyRequests} 件 · 予約推定 ${usageState.dailyEstimatedInputTokens + usageState.dailyEstimatedOutputTokens} / 実測 ${usageState.dailyProviderInputTokens + usageState.dailyProviderOutputTokens} トークン（上限 ${usageState.limits.dailyTokens}）",
                        style = MaterialTheme.typography.bodySmall)
                    Text("今月 ${usageState.monthlyRequests}/${usageState.limits.monthlyRequests} 件 · 予約推定 ${usageState.monthlyEstimatedInputTokens + usageState.monthlyEstimatedOutputTokens} / 実測 ${usageState.monthlyProviderInputTokens + usageState.monthlyProviderOutputTokens} トークン（上限 ${usageState.limits.monthlyTokens}） · 未確定 ${usageState.uncertainRequests}件",
                        style = MaterialTheme.typography.bodySmall)
                }
                Text("トークン数は入力長からの概算予約とProvider実測を区別します。Timeout/Cancel/使用量欠落では予約を戻しません。端末時計に基づくソフト上限で、Provider請求上限やクレジット制御ではありません。",
                    style = MaterialTheme.typography.bodySmall)
                Text("このカウンターはProvider要求のみ対象です。ローカルDemoとMock HTTPテストは含みません。日次/月次はUTC境界です。",
                    style = MaterialTheme.typography.bodySmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(value = dailyRequestsText, onValueChange = { dailyRequestsText = it; settingsMessage = "" },
                        modifier = Modifier.weight(1f), label = { Text("日次requests") }, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true)
                    OutlinedTextField(value = dailyTokensText, onValueChange = { dailyTokensText = it; settingsMessage = "" },
                        modifier = Modifier.weight(1f), label = { Text("日次tokens") }, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true)
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(value = monthlyRequestsText, onValueChange = { monthlyRequestsText = it; settingsMessage = "" },
                        modifier = Modifier.weight(1f), label = { Text("月次requests") }, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true)
                    OutlinedTextField(value = monthlyTokensText, onValueChange = { monthlyTokensText = it; settingsMessage = "" },
                        modifier = Modifier.weight(1f), label = { Text("月次tokens") }, keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Number), singleLine = true)
                }
                Button(onClick = {
                    scope.launch {
                        runCatching {
                            val limits = validateUsageLimits(dailyRequestsText, dailyTokensText, monthlyRequestsText, monthlyTokensText)
                            usage?.updateLimits(limits) ?: error("Usage settings unavailable")
                            limits
                        }.onSuccess { limits ->
                            usageSnapshot = usage?.snapshot()
                            settingsMessage = "Soft Limit設定を保存しました。Provider請求上限ではありません。"
                            dailyRequestsText = limits.dailyRequests.toString()
                            dailyTokensText = limits.dailyTokens.toString()
                            monthlyRequestsText = limits.monthlyRequests.toString()
                            monthlyTokensText = limits.monthlyTokens.toString()
                        }.onFailure {
                            settingsMessage = "Soft Limit設定を保存できません。日次/月次上限と正の数値を確認してください。"
                        }
                    }
                }, enabled = usage != null && runCatching {
                    validateUsageLimits(dailyRequestsText, dailyTokensText, monthlyRequestsText, monthlyTokensText)
                }.isSuccess) { Text("ソフト上限を保存") }
                runCatching { validateUsageLimits(dailyRequestsText, dailyTokensText, monthlyRequestsText, monthlyTokensText) }
                    .exceptionOrNull()?.message?.let { validationMessage ->
                        Text(validationMessage, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall,
                            modifier = Modifier.semantics { liveRegion = LiveRegionMode.Assertive })
                    }
            }
            if (!settingsDestination) {
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(onClick = chatViewModel::newConversation, enabled = !state.isRestoring, modifier = Modifier.weight(1f)) { Text("新しい会話") }
                    Button(onClick = chatViewModel::showHistory, enabled = !state.isRestoring, modifier = Modifier.weight(1f)) { Text("履歴") }
                }
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(onClick = {
                        if (onOpenSettings != null) onOpenSettings()
                        else { settingsVisible = !settingsVisible; secretText = "" }
                    }, enabled = !state.isRestoring, modifier = Modifier.weight(1f).testTag("open_settings")) { Text("設定") }
                    Box(modifier = Modifier.weight(1f)) {
                        TextButton(onClick = { overflowExpanded = true }, enabled = !state.isRestoring,
                            modifier = Modifier.fillMaxWidth().semantics { contentDescription = "その他の操作" }) { Text("その他の操作") }
                        DropdownMenu(expanded = overflowExpanded, onDismissRequest = { overflowExpanded = false }) {
                            HorizontalDivider()
                            DropdownMenuItem(text = { Text("この会話を削除", color = MaterialTheme.colorScheme.error) }, enabled = !state.isRestoring,
                                onClick = { overflowExpanded = false; deleteTargetId = state.conversation.id; deleteTargetTitle = conversationDisplayTitle(state.conversation); deleteConfirmation = "current" })
                            DropdownMenuItem(text = { Text("全履歴を削除", color = MaterialTheme.colorScheme.error) }, enabled = !state.isRestoring,
                                onClick = { overflowExpanded = false; deleteTargetId = ""; deleteConfirmation = "all" })
                        }
                    }
                }
            }
            state.statusMessage?.let { Text(it, color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }) }
            if (settingsVisible) {
                Text("Provider候補: Ollama Cloud", style = MaterialTheme.typography.titleMedium)
                Text("利用条件: 公式利用規約は18歳以上を要求します。利用資格を確認していないため、このアプリからのCloud通信を無効にしています。",
                    color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                Text("Cloud送信: 無効 · 既存Provider候補のモデルIDと価格情報は正式な推奨・既定値として扱いません。",
                    style = MaterialTheme.typography.bodyMedium)
                Text("APIキー: ${if (credentialReady) "設定済み" else "未設定"}", style = MaterialTheme.typography.bodyMedium)
                Text("保存先: Android Keystoreで暗号化 · $keystoreProfile",
                    style = MaterialTheme.typography.bodySmall)
                OutlinedTextField(value = secretText, onValueChange = { secretText = it },
                    modifier = Modifier.fillMaxWidth(), label = { Text("Ollama Cloud APIキー") },
                    visualTransformation = PasswordVisualTransformation(), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
                    singleLine = true, supportingText = { Text("端末内に暗号化保存します。キーは会話・保存状態・ログへ保存しません。設定を開いている間は画面キャプチャを無効にします。") })
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(onClick = {
                        val secret = secretText
                        secretText = ""
                        scope.launch {
                            runCatching { credentials?.store(secret.toCharArray()) }
                                .onSuccess { credentialReady = credentials?.isConfigured() == true; settingsMessage = "APIキーを暗号化保存しました。Cloud送信は無効のままです。" }
                                .onFailure { error -> settingsMessage = if (error is HardwareBackedKeyRequiredException)
                                    "Cloud APIキーにはHardware-backed Android Keystoreが必要です。この端末ではキーを保存しません。"
                                else "APIキーを安全に保存できませんでした。" }
                        }
                    }, enabled = secretText.isNotEmpty() && credentials != null) { Text("保存") }
                    TextButton(onClick = {
                        scope.launch {
                            runCatching { credentials?.currentHandle()?.let { credentials.delete(it) } }
                                .onSuccess { credentialReady = false; settingsMessage = "APIキーを削除しました。" }
                                .onFailure { settingsMessage = "APIキーを削除できませんでした。" }
                        }
                    }, enabled = credentialReady && credentials != null) { Text("削除") }
                }
                Text("Provider利用不可: 利用条件・データ処理条件・Ownerの有効化判断が未確認です。送信対象も未許可のためCloud接続は無効です。資格情報があってもMock HTTP以外へ接続しません。",
                    color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                Button(onClick = {}, enabled = false) { Text("Cloud接続テスト（未承認）") }
                TextButton(onClick = { previewVisible = !previewVisible }) { Text(if (previewVisible) "送信予定プレビューを閉じる" else "送信予定内容をプレビュー") }
                if (previewVisible) {
                    val contextPreview = ChatContextBuilder.build(state.conversation.messages, state.input.trim(), state.conversation.id)
                    Text("ローカル表示のみ · Cloud送信は無効 · 秘密情報が含まれないことを証明するものではありません。",
                        style = MaterialTheme.typography.bodySmall)
                    Text("送信先候補: https://ollama.com/api/chat · 出所: USER_AUTHORED · 分類: UNKNOWN · 送信意思: なし · 状態: 無効",
                        style = MaterialTheme.typography.bodySmall)
                    Text("予定本文: ${state.input.trim()}", style = MaterialTheme.typography.bodyMedium)
                    Text("送信コンテキスト (${contextPreview.size} 件、文字数ベースの保守上限。厳密なトークン数ではありません):", style = MaterialTheme.typography.labelMedium)
                    contextPreview.forEach { item ->
                        Text("${item.role} · ${item.sourceMessageId} · ${item.origin} · ${item.classification} · ${item.status}: ${item.text}",
                            style = MaterialTheme.typography.bodySmall)
                    }
                }
                if (settingsMessage.isNotEmpty()) Text(settingsMessage, style = MaterialTheme.typography.bodySmall)
                Text("Freeプランの利用枠とモデル数には上限があります。通常会話のCloud送信はOwnerポリシー承認まで無効です。",
                    color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                if (onSettingsClosed == null) {
                    Button(onClick = {
                        secretText = ""
                        settingsVisible = false
                    }) { Text("設定を閉じる") }
                }
            } else if (state.showingHistory) {
                Text("保存された会話", style = MaterialTheme.typography.titleMedium)
                LazyColumn(modifier = Modifier.weight(1f).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(state.history, key = { it.id }) { conversation ->
                        TextButton(onClick = { chatViewModel.openConversation(conversation.id) }, modifier = Modifier.fillMaxWidth()) {
                            Text("${storedConversationDisplayTitle(conversation)} · ${conversation.messages.size}件")
                        }
                    }
                    if (state.history.isEmpty()) item { Text("履歴はありません。") }
                }
            } else {
                Text("会話: ${conversationDisplayTitle(state.conversation)}", style = MaterialTheme.typography.titleSmall,
                    maxLines = 1, overflow = androidx.compose.ui.text.style.TextOverflow.Ellipsis)
                if (state.conversation.messages.isEmpty()) {
                    Text(if (state.isRestoring) "暗号化された履歴を復元しています…" else "端末内の暗号化履歴に保存されるデモ会話です。Cloud送信は無効です。",
                        style = MaterialTheme.typography.bodyLarge)
                }
                LazyColumn(modifier = Modifier.weight(1f).fillMaxWidth().testTag("conversation-list"), state = listState,
                    verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(state.conversation.messages, key = { it.id }) { message ->
                        Surface(color = if (message.role == MessageRole.USER) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surfaceVariant,
                            shape = MaterialTheme.shapes.medium) {
                            Column(Modifier.fillMaxWidth().padding(12.dp)) {
                                Text(if (message.role == MessageRole.USER) "あなた · ${message.status}" else "デモ応答",
                                    style = MaterialTheme.typography.labelMedium, fontWeight = FontWeight.Bold)
                                Text(message.text, style = MaterialTheme.typography.bodyLarge)
                            }
                        }
                    }
                    if (state.isLoading) item { Row(horizontalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }) {
                        CircularProgressIndicator(modifier = Modifier.padding(4.dp), strokeWidth = 2.dp)
                        Text(if (state.isFinalizing) "応答を保存しています…" else "Demo応答を準備しています…",
                            modifier = Modifier.padding(top = 8.dp))
                    } }
                }
            }
            state.errorMessage?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Assertive }) }
            if (!settingsDestination) {
                Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedTextField(value = state.input, onValueChange = chatViewModel::updateInput,
                        modifier = Modifier.weight(1f).testTag("chat-composer"), label = { Text("メッセージ") },
                        enabled = !state.isLoading && !state.isRestoring && !state.showingHistory,
                        maxLines = 4, keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                        keyboardActions = KeyboardActions(onSend = { chatViewModel.sendMessage(); keyboard?.hide() }),
                        supportingText = { Text("デモ専用。Cloudへは送信されません。") })
                    if (state.isLoading && !state.isFinalizing) Button(onClick = chatViewModel::cancelCurrent, modifier = Modifier.padding(top = 8.dp)) { Text("キャンセル") }
                    else if (!state.isLoading) Button(onClick = { chatViewModel.sendMessage(); keyboard?.hide() },
                        enabled = state.input.isNotBlank() && !state.isRestoring && !state.showingHistory,
                        modifier = Modifier.padding(top = 8.dp)) { Text("送信") }
                }
            }
            if (deleteConfirmation == "current") {
                AlertDialog(
                    onDismissRequest = { deleteConfirmation = ""; deleteTargetId = ""; deleteTargetTitle = "" },
                    title = { Text("この会話を削除しますか？") },
                    text = { Text("対象: $deleteTargetTitle\n選択した会話のローカル履歴を削除します。この操作はアプリ内から取り消せません。") },
                    confirmButton = { TextButton(enabled = !state.isRestoring, onClick = {
                        val confirmedId = deleteTargetId
                        deleteConfirmation = ""
                        deleteTargetId = ""
                        deleteTargetTitle = ""
                        chatViewModel.deleteCurrentConversation(confirmedId)
                    }) { Text("この会話を削除") } },
                    dismissButton = { TextButton(onClick = { deleteConfirmation = ""; deleteTargetId = ""; deleteTargetTitle = "" }) { Text("キャンセル") } },
                )
            } else if (deleteConfirmation == "all") {
                AlertDialog(
                    onDismissRequest = { deleteConfirmation = "" },
                    title = { Text("全会話履歴を削除しますか？") },
                    text = { Text("すべてのローカル会話履歴が削除されます。この操作はアプリ内から取り消せません。") },
                    confirmButton = { TextButton(enabled = !state.isRestoring, onClick = {
                        deleteConfirmation = ""
                        chatViewModel.deleteAllHistory()
                    }) { Text("全履歴を削除") } },
                    dismissButton = { TextButton(onClick = { deleteConfirmation = "" }) { Text("キャンセル") } },
                )
            }
        }
    }
}

@Composable
fun SereaTheme(darkTheme: Boolean = isSystemInDarkTheme(), content: @Composable () -> Unit) {
    val colors = if (darkTheme) darkColorScheme() else lightColorScheme()
    MaterialTheme(colorScheme = colors, content = content)
}
