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
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.Checkbox
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
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.LiveRegionMode
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
import kotlinx.coroutines.cancelAndJoin
import kotlinx.coroutines.delay
import kotlinx.coroutines.isActive
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
    private val history by lazy {
        val database = SereaHistoryDatabase.open(this)
        ConversationRepository(database.historyDao(), KeystoreAesGcm("serea-history-v1"))
    }
    private val credentials by lazy { CredentialRepository(this) }
    internal val chatViewModel: ChatViewModel by viewModels {
        ChatViewModel.factory(MockModelChatPort(), history)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { SereaTheme { ChatScreen(chatViewModel, credentials) } }
    }
}

data class ChatMessage(
    val id: String,
    val role: MessageRole,
    val text: String,
    val createdAt: Instant,
    val classification: DataClassification = DataClassification.UNKNOWN,
    val status: String = "COMPLETED",
)

enum class MessageRole { USER, ASSISTANT }
enum class DataClassification { PUBLIC, PERSONAL, PRIVATE, SECRET, CREDENTIAL, UNKNOWN }

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
    val isRestoring: Boolean = false,
    val showingHistory: Boolean = false,
    val history: List<StoredConversation> = emptyList(),
    val errorMessage: String? = null,
)

data class ModelChatRequest(
    val conversationId: String,
    val message: String,
    val timeoutMillis: Long = 15_000,
    val classification: DataClassification = DataClassification.UNKNOWN,
    val preparedPersonal: PreparedPersonalData? = null,
    val context: List<ContextMessage> = emptyList(),
)
data class ModelChatResponse(val text: String)

interface ModelChatPort {
    /** Must complete within request.timeoutMillis; throws ModelChatFailure on a definite failure. */
    suspend fun send(request: ModelChatRequest): ModelChatResponse
}

class ModelChatFailure(message: String) : Exception(message)

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

internal class ChatViewModel(private val modelChat: ModelChatPort, private val historyRepository: ConversationRepository? = null) : ViewModel() {
    private val mutableState = MutableStateFlow(ChatUiState(isRestoring = historyRepository != null))
    val state: StateFlow<ChatUiState> = mutableState.asStateFlow()
    private var activeJob: Job? = null
    private var activeMessageId: String? = null

    init { if (historyRepository != null) restoreLatest() }

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
        val userMessage = ChatMessage(UUID.randomUUID().toString(), MessageRole.USER, messageText, nextMessageTime(snapshot.conversation), status = "PENDING")
        activeMessageId = userMessage.id
        mutableState.update {
            it.copy(conversation = it.conversation.copy(messages = it.conversation.messages + userMessage), input = "", isLoading = true, errorMessage = null)
        }
        activeJob = viewModelScope.launch {
            try {
                persist(userMessage, conversationId)
                val context = ChatContextBuilder.build(snapshot.conversation.messages, messageText)
                val request = ModelChatRequest(conversationId, messageText, context = context)
                val reply = withTimeout(request.timeoutMillis) { modelChat.send(request) }
                val assistant = ChatMessage(UUID.randomUUID().toString(), MessageRole.ASSISTANT, reply.text,
                    nextMessageTime(snapshot.conversation.copy(messages = snapshot.conversation.messages + userMessage)))
                if (historyRepository != null) {
                    historyRepository.completeTurn(userMessage.id, assistant.toStoredMessage(conversationId))
                }
                mutableState.update { current ->
                    if (current.conversation.id != conversationId) current
                    else current.copy(conversation = current.conversation.copy(messages = current.conversation.messages
                        .map { if (it.id == userMessage.id) it.copy(status = "COMPLETED") else it } + assistant), isLoading = false)
                }
                refreshHistory()
            } catch (_: TimeoutCancellationException) {
                markUnknown(userMessage, conversationId, "The request timed out. Its remote outcome is unknown; retry explicitly.")
            } catch (cancelled: CancellationException) {
                withContext(NonCancellable) { markUnknown(userMessage, conversationId, "Request cancelled. A transmitted request may still have completed remotely.") }
            } catch (_: ModelChatFailure) {
                markFailed(userMessage, conversationId)
            } catch (_: Exception) {
                markFailed(userMessage, conversationId)
            } finally {
                if (activeMessageId == userMessage.id) { activeMessageId = null; activeJob = null }
            }
        }
    }

    fun cancelCurrent() {
        val id = activeMessageId ?: return
        val conversationId = mutableState.value.conversation.id
        activeJob?.cancel()
        mutableState.update { it.copy(isLoading = false, errorMessage = "Request cancelled. A transmitted request may still have completed remotely.") }
        viewModelScope.launch { historyRepository?.updateStatus(id, conversationId, "OUTCOME_UNKNOWN") }
    }

    fun newConversation() {
        cancelCurrent()
        val id = UUID.randomUUID().toString()
        mutableState.update { it.copy(conversation = Conversation(id), input = "", isLoading = false, showingHistory = false,
            isRestoring = historyRepository != null, errorMessage = null) }
        viewModelScope.launch {
            try { historyRepository?.create(id, "Conversation ${id.take(8)}") }
            catch (_: Exception) { mutableState.update { it.copy(errorMessage = "Could not save this conversation securely.") } }
            finally { mutableState.update { it.copy(isRestoring = false) } }
        }
    }

    fun showHistory() {
        viewModelScope.launch {
            try { mutableState.update { it.copy(history = historyRepository?.list().orEmpty(), showingHistory = true, isLoading = false) } }
            catch (_: HistoryUnavailableException) { mutableState.update { it.copy(errorMessage = "Saved history is unavailable because its encryption key or data is invalid.") } }
            catch (_: Exception) { mutableState.update { it.copy(errorMessage = "Could not open saved history.") } }
        }
    }

    fun openConversation(id: String) {
        cancelCurrent()
        viewModelScope.launch {
            try {
                val saved = historyRepository?.get(id) ?: return@launch
                val messages = saved.messages.map { message ->
                    if (message.status == "PENDING") {
                        historyRepository?.updateStatus(message.id, saved.id, "OUTCOME_UNKNOWN")
                        message.copy(status = "OUTCOME_UNKNOWN").toChatMessage()
                    } else message.toChatMessage()
                }
                mutableState.update { it.copy(conversation = Conversation(saved.id, messages, Instant.ofEpochMilli(saved.createdAt)), showingHistory = false, isLoading = false) }
            } catch (_: HistoryUnavailableException) { mutableState.update { it.copy(errorMessage = "Saved history is unavailable because its encryption key or data is invalid.") } }
        }
    }

    fun deleteCurrentConversation() {
        val id = mutableState.value.conversation.id
        cancelCurrent()
        mutableState.update { it.copy(isRestoring = true, isLoading = false) }
        viewModelScope.launch {
            try { historyRepository?.delete(id); newConversation(); refreshHistory() }
            catch (_: Exception) { mutableState.update { it.copy(errorMessage = "Could not delete this conversation.", isRestoring = false) } }
        }
    }

    fun deleteAllHistory() {
        cancelCurrent()
        mutableState.update { it.copy(isRestoring = true, isLoading = false) }
        viewModelScope.launch {
            try { historyRepository?.deleteAll(); newConversation(); mutableState.update { it.copy(history = emptyList()) } }
            catch (_: Exception) { mutableState.update { it.copy(errorMessage = "Could not delete saved history.", isRestoring = false) } }
        }
    }

    private fun restoreLatest() {
        viewModelScope.launch {
            try {
                val conversations = historyRepository!!.list()
                val latest = conversations.firstOrNull()
                if (latest == null) {
                    val id = mutableState.value.conversation.id
                    historyRepository.create(id, "Conversation ${id.take(8)}")
                } else {
                    val messages = latest.messages.map { message ->
                        if (message.status == "PENDING") historyRepository.updateStatus(message.id, latest.id, "OUTCOME_UNKNOWN")
                        message.copy(status = if (message.status == "PENDING") "OUTCOME_UNKNOWN" else message.status).toChatMessage()
                    }
                    mutableState.update { it.copy(conversation = Conversation(latest.id, messages, Instant.ofEpochMilli(latest.createdAt))) }
                }
            } catch (_: HistoryUnavailableException) {
                mutableState.update { it.copy(errorMessage = "Saved history is unavailable because its encryption key or data is invalid.") }
            } catch (_: Exception) {
                mutableState.update { it.copy(errorMessage = "Could not restore saved history.") }
            } finally { mutableState.update { it.copy(isRestoring = false) } }
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
        mutableState.update { current -> if (current.conversation.id == conversationId) current.copy(isLoading = false,
            conversation = current.conversation.copy(messages = current.conversation.messages.map { if (it.id == message.id) it.copy(status = "OUTCOME_UNKNOWN") else it }),
            errorMessage = reason) else current }
    }
    private suspend fun markFailed(message: ChatMessage, conversationId: String) {
        historyRepository?.updateStatus(message.id, conversationId, "FAILED")
        mutableState.update { current -> if (current.conversation.id == conversationId) current.copy(isLoading = false,
            conversation = current.conversation.copy(messages = current.conversation.messages.map { if (it.id == message.id) it.copy(status = "FAILED") else it }),
            errorMessage = "Demo response failed. Please try again.") else current }
    }
    private fun refreshHistory() { viewModelScope.launch { runCatching { historyRepository?.list().orEmpty() }.onSuccess { list -> mutableState.update { it.copy(history = list) } } } }

    companion object {
        fun factory(port: ModelChatPort, history: ConversationRepository? = null): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T = ChatViewModel(port, history) as T
        }
    }
}

private fun ChatMessage.toStoredMessage(conversationId: String) = StoredMessage(
    id, conversationId, role, text, classification, createdAt.toEpochMilli(), status,
)

private fun StoredMessage.toChatMessage() = ChatMessage(id, role, text, Instant.ofEpochMilli(createdAt), classification, status)

@Composable
internal fun ChatScreen(chatViewModel: ChatViewModel = androidx.lifecycle.viewmodel.compose.viewModel(), credentials: CredentialRepository? = null) {
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
    var settingsVisible by remember { mutableStateOf(false) }
    var secretText by remember { mutableStateOf("") }
    var settingsMessage by remember { mutableStateOf("") }
    var credentialReady by remember { mutableStateOf(credentials?.isConfigured() == true) }
    var cloudEnabled by remember { mutableStateOf(false) }
    var cloudOutput by remember { mutableStateOf("") }
    var cloudWorking by remember { mutableStateOf(false) }
    var cloudJob by remember { mutableStateOf<Job?>(null) }
    LaunchedEffect(state.conversation.messages.size, state.isLoading, state.showingHistory, settingsVisible) {
        if (!state.showingHistory && !settingsVisible) {
            val lastVisibleItem = state.conversation.messages.size + if (state.isLoading) 1 else 0
            if (lastVisibleItem > 0) listState.animateScrollToItem(lastVisibleItem - 1)
        }
    }
    SideEffect {
        hostActivity?.window?.let { window ->
            if (settingsVisible) window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
            else window.clearFlags(WindowManager.LayoutParams.FLAG_SECURE)
        }
    }
    val keystoreProfile = remember(credentials, settingsVisible) {
        if (settingsVisible) runCatching { credentials?.securityProfile() }.getOrNull() ?: "未確認" else "未確認"
    }
    Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
        Column(modifier = Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text("Serea", style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.Bold,
                modifier = Modifier.semantics { heading() })
            Text("DEMO · Mock chat · No cloud AI", color = MaterialTheme.colorScheme.primary, style = MaterialTheme.typography.labelLarge)
            Text("クラウド候補: 未有効（データ送信契約のOwner承認待ち）", style = MaterialTheme.typography.bodySmall)
            Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                TextButton(onClick = chatViewModel::newConversation, enabled = !state.isRestoring) { Text("新しい会話") }
                TextButton(onClick = chatViewModel::showHistory, enabled = !state.isRestoring) { Text("履歴") }
                TextButton(onClick = { settingsVisible = !settingsVisible; secretText = "" }, enabled = !state.isRestoring) { Text("設定") }
                TextButton(onClick = chatViewModel::deleteCurrentConversation, enabled = !state.isRestoring) { Text("この会話を削除") }
                TextButton(onClick = chatViewModel::deleteAllHistory, enabled = !state.isRestoring) { Text("全履歴を削除") }
            }
            if (settingsVisible) {
                Text("Provider: Ollama Cloud（候補）", style = MaterialTheme.typography.titleMedium)
                Text("Model ID: gemma4:31b · 通常チャットのCloud送信: disabled",
                    style = MaterialTheme.typography.bodyMedium)
                Text("公開料金の表示: 入力 $0.14 / 出力 $0.40 per million tokens。最大出力512 tokens、料金とFree creditsはProvider側で変わります。",
                    style = MaterialTheme.typography.bodySmall)
                Text("API key: ${if (credentialReady) "設定済み" else "未設定"}", style = MaterialTheme.typography.bodyMedium)
                Text("保存先: Android Keystoreで暗号化 · $keystoreProfile",
                    style = MaterialTheme.typography.bodySmall)
                OutlinedTextField(value = secretText, onValueChange = { secretText = it },
                    modifier = Modifier.fillMaxWidth(), label = { Text("Ollama Cloud API key") },
                    visualTransformation = PasswordVisualTransformation(), keyboardOptions = KeyboardOptions(keyboardType = KeyboardType.Password, autoCorrectEnabled = false),
                    singleLine = true, supportingText = { Text("端末内に暗号化保存します。キーは会話・Saved State・ログへ保存しません。設定を開いている間は画面キャプチャを無効にします。") })
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(onClick = {
                        val secret = secretText
                        secretText = ""
                        scope.launch {
                            runCatching { credentials?.store(secret.toCharArray()) }
                                .onSuccess { credentialReady = credentials?.isConfigured() == true; settingsMessage = "API keyを暗号化保存しました。Cloud送信は無効のままです。" }
                                .onFailure { error -> settingsMessage = if (error is HardwareBackedKeyRequiredException)
                                    "Cloud API keyにはHardware-backed Android Keystoreが必要です。この端末ではキーを保存しません。"
                                else "API keyを安全に保存できませんでした。" }
                        }
                    }, enabled = secretText.isNotEmpty() && credentials != null) { Text("保存") }
                    TextButton(onClick = {
                        scope.launch {
                            runCatching { credentials?.currentHandle()?.let { credentials.delete(it) } }
                                .onSuccess { credentialReady = false; settingsMessage = "API keyを削除しました。" }
                                .onFailure { settingsMessage = "API keyを削除できませんでした。" }
                        }
                    }, enabled = credentialReady && credentials != null) { Text("削除") }
                }
                Row {
                    Checkbox(checked = cloudEnabled, onCheckedChange = { cloudEnabled = it; cloudOutput = "" },
                        enabled = credentialReady && !cloudWorking)
                    Text("Cloud候補を明示的に有効化（固定PUBLIC接続テストのみ）", modifier = Modifier.padding(top = 12.dp))
                }
                Button(onClick = {
                    val repo = credentials ?: return@Button
                    cloudWorking = true
                    cloudOutput = ""
                    cloudJob = scope.launch {
                        val streamedText = StringBuilder()
                        val renderer = launch {
                            while (isActive) {
                                delay(80)
                                cloudOutput = synchronized(streamedText) { streamedText.toString() }
                            }
                        }
                        try {
                            val probe = CloudCandidatePort(OllamaCloudAdapter(repo, CloudSettings(enabled = cloudEnabled)))
                            val result = probe.runFixedPublicContractProbe { delta -> synchronized(streamedText) { streamedText.append(delta) } }
                            renderer.cancelAndJoin()
                            cloudOutput = result.text + "\n\nUsage: ${result.usage?.let { "${it.inputTokens ?: "?"} in / ${it.outputTokens ?: "?"} out" } ?: "provider did not report usage"}"
                        } catch (cancelled: CancellationException) {
                            withContext(NonCancellable) {
                                renderer.cancelAndJoin()
                                cloudOutput = "接続テストをキャンセルしました。Provider側で処理が完了している可能性があります。"
                            }
                        } catch (error: Exception) {
                            renderer.cancelAndJoin()
                            cloudOutput = (error as? ProviderFailure)?.message ?: "Cloud connection failed."
                        } finally {
                            withContext(NonCancellable) { renderer.cancelAndJoin() }
                            cloudWorking = false
                            cloudJob = null
                        }
                    }
                }, enabled = cloudEnabled && credentialReady && !cloudWorking && credentials != null) {
                    Text(if (cloudWorking) "接続テスト中…" else "固定PUBLIC入力でCloud接続をテスト")
                }
                if (cloudWorking) TextButton(onClick = { cloudJob?.cancel() }) { Text("接続テストをキャンセル") }
                if (cloudOutput.isNotEmpty()) Text(cloudOutput, style = MaterialTheme.typography.bodyMedium)
                if (settingsMessage.isNotEmpty()) Text(settingsMessage, style = MaterialTheme.typography.bodySmall)
                Text("Freeにはstarter creditsとstarter modelsがあり無制限ではありません。通常会話のCloud送信はOwnerポリシー承認まで無効です。",
                    color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                Button(onClick = { cloudJob?.cancel(); settingsVisible = false; secretText = "" }) { Text("設定を閉じる") }
            } else if (state.showingHistory) {
                Text("保存された会話", style = MaterialTheme.typography.titleMedium)
                LazyColumn(modifier = Modifier.weight(1f).fillMaxWidth(), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(state.history, key = { it.id }) { conversation ->
                        TextButton(onClick = { chatViewModel.openConversation(conversation.id) }, modifier = Modifier.fillMaxWidth()) {
                            Text("${conversation.title} · ${conversation.messages.size}件")
                        }
                    }
                    if (state.history.isEmpty()) item { Text("履歴はありません。") }
                }
            } else {
                if (state.conversation.messages.isEmpty()) {
                    Text(if (state.isRestoring) "暗号化された履歴を復元しています…" else "端末内の暗号化履歴に保存されるDemo会話です。Cloud送信は無効です。",
                        style = MaterialTheme.typography.bodyLarge)
                }
                LazyColumn(modifier = Modifier.weight(1f).fillMaxWidth(), state = listState,
                    verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    items(state.conversation.messages, key = { it.id }) { message ->
                        Surface(color = if (message.role == MessageRole.USER) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surfaceVariant,
                            shape = MaterialTheme.shapes.medium) {
                            Column(Modifier.fillMaxWidth().padding(12.dp)) {
                                Text(if (message.role == MessageRole.USER) "You · ${message.status}" else "Demo assistant",
                                    style = MaterialTheme.typography.labelMedium, fontWeight = FontWeight.Bold)
                                Text(message.text, style = MaterialTheme.typography.bodyLarge)
                            }
                        }
                    }
                    if (state.isLoading) item { Row(horizontalArrangement = Arrangement.spacedBy(8.dp),
                        modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }) {
                        CircularProgressIndicator(modifier = Modifier.padding(4.dp), strokeWidth = 2.dp)
                        Text("Demo応答を準備しています…", modifier = Modifier.padding(top = 8.dp))
                    } }
                }
            }
            state.errorMessage?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Assertive }) }
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(value = state.input, onValueChange = chatViewModel::updateInput,
                    modifier = Modifier.weight(1f).testTag("chat-composer"), label = { Text("Message") },
                    enabled = !state.isLoading && !state.isRestoring && !state.showingHistory,
                    maxLines = 4, keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                    keyboardActions = KeyboardActions(onSend = { chatViewModel.sendMessage(); keyboard?.hide() }),
                    supportingText = { Text("Demo only. Cloudへは送信されません。") })
                if (state.isLoading) Button(onClick = chatViewModel::cancelCurrent, modifier = Modifier.padding(top = 8.dp)) { Text("キャンセル") }
                else Button(onClick = { chatViewModel.sendMessage(); keyboard?.hide() },
                    enabled = state.input.isNotBlank() && !state.isRestoring && !state.showingHistory,
                    modifier = Modifier.padding(top = 8.dp)) { Text("送信") }
            }
        }
    }
}

@Composable
fun SereaTheme(content: @Composable () -> Unit) {
    val colors = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()
    MaterialTheme(colorScheme = colors, content = content)
}
