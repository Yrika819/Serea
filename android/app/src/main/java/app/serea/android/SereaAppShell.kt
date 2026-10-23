package app.serea.android

import androidx.activity.compose.BackHandler
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.FilterChip
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.saveable.rememberSaveableStateHolder
import androidx.compose.runtime.saveable.rememberSaveable
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalFocusManager
import androidx.compose.ui.platform.LocalSoftwareKeyboardController
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.semantics.heading
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.viewmodel.compose.viewModel
import app.serea.android.assistant.taskproposal.TaskProposalDemoScreen
import app.serea.android.calendar.FakeCalendarProvider
import app.serea.android.calendar.ui.CalendarAgendaScreen
import app.serea.android.gmail.FakeGmailProvider
import app.serea.android.gmail.ui.GmailScreen
import app.serea.android.gmail.ui.GmailViewModel
import app.serea.android.tasks.TaskScreen
import app.serea.android.tasks.TaskViewModel
import java.time.ZoneId

@Composable
internal fun SereaAppShell(
    chatViewModel: ChatViewModel,
    credentials: CredentialRepository,
    usage: UsageRepository,
    taskViewModel: TaskViewModel,
    taskProposalDemoContent: @Composable (
        Modifier,
        (() -> Unit)?,
        (Boolean) -> Unit,
    ) -> Unit = { modifier, onClose, onPendingChanged ->
        TaskProposalDemoScreen(
            modifier = modifier,
            onClose = onClose,
            onActionPendingChanged = onPendingChanged,
        )
    },
) {
    var destinationName by rememberSaveable { mutableStateOf(SereaDestination.CHAT.name) }
    var backStackNames by rememberSaveable { mutableStateOf(emptyList<String>()) }
    var focusClearRequest by remember { mutableIntStateOf(0) }
    val destination = SereaNavigationState.restore(destinationName, backStackNames).current
    val canGoBack = backStackNames.isNotEmpty()
    var taskProposalActionPending by androidx.compose.runtime.remember { mutableStateOf(false) }
    val saveableStateHolder = rememberSaveableStateHolder()
    val keyboardController = LocalSoftwareKeyboardController.current
    val focusManager = LocalFocusManager.current
    // Keep the bottom navigation to two rows on compact and large-text displays.
    // Three columns remain wide enough for the two-line Japanese labels while
    // giving the current destination more vertical room.
    val navigationColumns = 3

    fun navigate(target: SereaDestination) {
        keyboardController?.hide()
        focusManager.clearFocus(force = true)
        focusClearRequest += 1
        val state = SereaNavigationState.restore(destinationName, backStackNames).navigate(target)
        destinationName = state.current.name
        backStackNames = state.backStack.takeLast(SereaNavigationState.MAX_BACK_STACK).map { it.name }
    }
    fun goBack() {
        keyboardController?.hide()
        focusManager.clearFocus(force = true)
        focusClearRequest += 1
        val state = SereaNavigationState.restore(destinationName, backStackNames).back()
        destinationName = state.current.name
        backStackNames = state.backStack.map { it.name }
    }

    // Hide the IME and clear the outgoing destination's focus synchronously. ChatScreen
    // receives focusClearRequest so it can clear a restored composer after reattachment.
    LaunchedEffect(destination) {
        keyboardController?.hide()
        focusManager.clearFocus(force = true)
    }

    BackHandler(enabled = taskProposalActionPending || canGoBack) {
        if (!taskProposalActionPending) goBack()
    }

    Scaffold(
        bottomBar = {
            Surface(
                modifier = Modifier.testTag("serea_navigation_bar"),
                color = MaterialTheme.colorScheme.surface,
                tonalElevation = 3.dp,
            ) {
                Column(
                    Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp),
                    verticalArrangement = Arrangement.spacedBy(2.dp),
                ) {
                    DestinationContract.all.chunked(navigationColumns).forEach { rowDestinations ->
                        Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.spacedBy(4.dp)) {
                            rowDestinations.forEach { contract ->
                                FilterChip(
                                    selected = destination == contract.destination,
                                    onClick = { navigate(contract.destination) },
                                    enabled = !taskProposalActionPending,
                                    label = { Text(contract.title, maxLines = 2) },
                                    modifier = Modifier.weight(1f)
                                        .testTag("destination_${contract.destination.name.lowercase()}"),
                                )
                            }
                        }
                    }
                }
            }
        },
    ) { padding ->
        Column(Modifier.fillMaxSize().padding(padding)) {
            if (destination == SereaDestination.TASKS || destination == SereaDestination.SETTINGS) {
                Row(
                    Modifier.fillMaxWidth().padding(horizontal = 8.dp, vertical = 4.dp),
                    horizontalArrangement = Arrangement.spacedBy(4.dp),
                ) {
                    androidx.compose.material3.TextButton(
                        onClick = ::goBack,
                        enabled = canGoBack && !taskProposalActionPending,
                        modifier = Modifier.testTag("destination_back"),
                    ) { Text("戻る") }
                    if (destination == SereaDestination.SETTINGS) {
                        Text(
                            "設定",
                            style = MaterialTheme.typography.titleMedium,
                            modifier = Modifier.padding(8.dp).semantics { heading() }.testTag("destination_title"),
                        )
                    }
                }
            }
            Box(Modifier.weight(1f).fillMaxWidth()) {
                saveableStateHolder.SaveableStateProvider(destination.name) {
                    when (destination) {
                        SereaDestination.CHAT -> ChatScreen(
                            chatViewModel = chatViewModel,
                            credentials = credentials,
                            usage = usage,
                            onOpenTasks = { navigate(SereaDestination.TASKS) },
                            onOpenSettings = { navigate(SereaDestination.SETTINGS) },
                            focusClearRequest = focusClearRequest,
                        )
                        SereaDestination.TASKS -> TaskScreen(taskViewModel)
                        SereaDestination.GMAIL -> GmailDestination(onBack = ::goBack)
                        SereaDestination.CALENDAR -> CalendarDestination(onBack = ::goBack)
                        SereaDestination.SETTINGS -> ChatScreen(
                            chatViewModel = chatViewModel,
                            credentials = credentials,
                            usage = usage,
                            settingsDestination = true,
                            onSettingsClosed = ::goBack,
                            focusClearRequest = focusClearRequest,
                        )
                        SereaDestination.TASK_PROPOSAL_DEMO -> taskProposalDemoContent(
                            Modifier.fillMaxSize().testTag("task_proposal_demo"),
                            ::goBack,
                            { taskProposalActionPending = it },
                        )
                    }
                }
            }
        }
    }
}

@Composable
private fun GmailDestination(onBack: () -> Unit) {
    val provider = androidx.compose.runtime.remember { FakeGmailProvider() }
    val vm: GmailViewModel = viewModel(factory = GmailViewModel.factory(provider, provider))
    GmailScreen(viewModel = vm, onBack = onBack, modifier = Modifier.fillMaxSize().testTag("gmail_destination"))
}

@Composable
private fun CalendarDestination(onBack: () -> Unit) {
    val provider = androidx.compose.runtime.remember {
        FakeCalendarProvider(events = FakeCalendarProvider.Fixtures.appShellEvents, pageSize = 50)
    }
    val clock = provider.clock
    val displayZone = androidx.compose.runtime.remember { ZoneId.systemDefault() }
    CalendarAgendaScreen(
        onBack = onBack,
        provider = provider,
        clock = clock,
        displayZone = displayZone,
        modifier = Modifier.fillMaxSize().testTag("calendar_destination"),
    )
}
