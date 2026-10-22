package app.serea.android

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.text.KeyboardActions
import androidx.compose.foundation.text.KeyboardOptions
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.foundation.lazy.rememberLazyListState
import androidx.compose.material3.Button
import androidx.compose.material3.CircularProgressIndicator
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.OutlinedTextField
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.collectAsState
import androidx.compose.runtime.getValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.liveRegion
import androidx.compose.ui.semantics.LiveRegionMode
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.text.input.ImeAction
import androidx.compose.ui.unit.dp
import androidx.lifecycle.ViewModel
import androidx.lifecycle.ViewModelProvider
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.lifecycle.viewModelScope
import kotlinx.coroutines.delay
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.TimeoutCancellationException
import kotlinx.coroutines.flow.MutableStateFlow
import kotlinx.coroutines.flow.StateFlow
import kotlinx.coroutines.flow.asStateFlow
import kotlinx.coroutines.flow.update
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeout
import java.time.Instant
import java.util.UUID

class MainActivity : ComponentActivity() {
    private val chatViewModel: ChatViewModel by viewModels {
        ChatViewModel.factory(MockModelChatPort())
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { SereaTheme { ChatScreen(chatViewModel) } }
    }
}

data class ChatMessage(
    val id: String,
    val role: MessageRole,
    val text: String,
    val createdAt: Instant,
    val classification: DataClassification = DataClassification.UNKNOWN,
    val deletedAt: Instant? = null,
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
    val errorMessage: String? = null,
)

data class ModelChatRequest(
    val conversationId: String,
    val message: String,
    val timeoutMillis: Long = 15_000,
    val classification: DataClassification = DataClassification.UNKNOWN,
    val preparedPersonal: PreparedPersonalData? = null,
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
        delay(responseDelayMillis)
        return ModelChatResponse("Demo response: I received “${request.message}”. No cloud AI was contacted.")
    }
}

class ChatViewModel(private val modelChat: ModelChatPort) : ViewModel() {
    private val mutableState = MutableStateFlow(ChatUiState())
    val state: StateFlow<ChatUiState> = mutableState.asStateFlow()

    fun updateInput(value: String) = mutableState.update { it.copy(input = value, errorMessage = null) }

    fun sendMessage() {
        val snapshot = mutableState.value
        val messageText = snapshot.input.trim()
        if (messageText.isEmpty() || snapshot.isLoading) return
        val conversationId = snapshot.conversation.id
        val userMessage = ChatMessage(UUID.randomUUID().toString(), MessageRole.USER, messageText, Instant.now())
        mutableState.update {
            it.copy(
                conversation = it.conversation.copy(messages = it.conversation.messages + userMessage),
                input = "",
                isLoading = true,
                errorMessage = null,
            )
        }
        viewModelScope.launch {
            try {
                val request = ModelChatRequest(conversationId, messageText)
                val reply = withTimeout(request.timeoutMillis) { modelChat.send(request) }
                mutableState.update { current ->
                    current.copy(
                        conversation = current.conversation.copy(
                            messages = current.conversation.messages + ChatMessage(
                                UUID.randomUUID().toString(), MessageRole.ASSISTANT, reply.text, Instant.now(),
                                DataClassification.PUBLIC,
                            ),
                        ),
                        isLoading = false,
                    )
                }
            } catch (_: TimeoutCancellationException) {
                mutableState.update { it.copy(isLoading = false, errorMessage = "Demo response timed out. Retry explicitly.") }
            } catch (cancelled: CancellationException) {
                throw cancelled
            } catch (_: ModelChatFailure) {
                mutableState.update { it.copy(isLoading = false, errorMessage = "Demo response failed. Please try again.") }
            } catch (_: Exception) {
                mutableState.update { it.copy(isLoading = false, errorMessage = "Demo response failed. Please try again.") }
            }
        }
    }

    companion object {
        fun factory(port: ModelChatPort): ViewModelProvider.Factory = object : ViewModelProvider.Factory {
            @Suppress("UNCHECKED_CAST")
            override fun <T : ViewModel> create(modelClass: Class<T>): T = ChatViewModel(port) as T
        }
    }
}

@Composable
fun ChatScreen(chatViewModel: ChatViewModel = viewModel()) {
    val state by chatViewModel.state.collectAsState()
    val keyboard = LocalSoftwareKeyboardController.current
    val listState = rememberLazyListState()
    Surface(modifier = Modifier.fillMaxSize(), color = MaterialTheme.colorScheme.background) {
        Column(modifier = Modifier.fillMaxSize().padding(16.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            Text("Serea", style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.Bold,
                modifier = Modifier.semantics { heading() })
            Text("DEMO · Mock chat · No cloud AI", color = MaterialTheme.colorScheme.primary,
                style = MaterialTheme.typography.labelLarge)
            if (state.conversation.messages.isEmpty()) {
                Text("Start a demo conversation. Messages stay in memory and are cleared when the app process ends.",
                    style = MaterialTheme.typography.bodyLarge)
            }
            LazyColumn(
                modifier = Modifier.weight(1f).fillMaxWidth(),
                state = listState,
                verticalArrangement = Arrangement.spacedBy(8.dp),
            ) {
                items(state.conversation.messages, key = { it.id }) { message ->
                    Surface(color = if (message.role == MessageRole.USER) MaterialTheme.colorScheme.secondaryContainer else MaterialTheme.colorScheme.surfaceVariant,
                        shape = MaterialTheme.shapes.medium) {
                        Column(Modifier.fillMaxWidth().padding(12.dp)) {
                            Text(if (message.role == MessageRole.USER) "You" else "Demo assistant",
                                style = MaterialTheme.typography.labelMedium, fontWeight = FontWeight.Bold)
                            Text(message.text, style = MaterialTheme.typography.bodyLarge)
                        }
                    }
                }
                if (state.isLoading) item { Row(horizontalArrangement = Arrangement.spacedBy(8.dp),
                    modifier = Modifier.semantics { liveRegion = LiveRegionMode.Polite }) {
                    CircularProgressIndicator(modifier = Modifier.padding(4.dp), strokeWidth = 2.dp)
                    Text("Preparing demo response…", modifier = Modifier.padding(top = 8.dp))
                } }
            }
            state.errorMessage?.let { Text(it, color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodyMedium,
                modifier = Modifier.semantics { liveRegion = LiveRegionMode.Assertive }) }
            Row(modifier = Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                OutlinedTextField(
                    value = state.input,
                    onValueChange = chatViewModel::updateInput,
                    modifier = Modifier.weight(1f),
                    label = { Text("Message") },
                    enabled = !state.isLoading,
                    maxLines = 4,
                    keyboardOptions = KeyboardOptions(imeAction = ImeAction.Send),
                    keyboardActions = KeyboardActions(onSend = { chatViewModel.sendMessage(); keyboard?.hide() }),
                    supportingText = { Text("Demo only. Do not enter private information.") },
                )
                Button(onClick = { chatViewModel.sendMessage(); keyboard?.hide() },
                    enabled = state.input.isNotBlank() && !state.isLoading,
                    modifier = Modifier.padding(top = 8.dp)) { Text("Send") }
            }
        }
    }
}

@Composable
fun SereaTheme(content: @Composable () -> Unit) {
    val colors = if (isSystemInDarkTheme()) darkColorScheme() else lightColorScheme()
    MaterialTheme(colorScheme = colors, content = content)
}
