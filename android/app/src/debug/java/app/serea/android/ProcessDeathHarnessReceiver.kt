package app.serea.android

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.os.Build
import android.util.Log
import app.serea.android.KeystoreAesGcm
import app.serea.android.tasks.EncryptedTaskRepository
import app.serea.android.tasks.TaskDraft
import app.serea.android.tasks.TaskPriority
import app.serea.android.tasks.TaskStatus
import app.serea.android.tasks.TasksDatabase
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking
import java.io.File

/** Debug-only ADB harness endpoint. It is absent from release builds. */
class ProcessDeathHarnessReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val pending = goAsync()
        Thread {
            val token = intent.getStringExtra("token") ?: "missing-token"
            val phase = intent.getStringExtra("phase") ?: "missing-phase"
            val requestId = intent.getStringExtra("requestId") ?: "none"
            try {
                check(token != "missing-token")
                check(phase != "missing-phase")
                check(requestId.matches(Regex("[A-Za-z0-9-]{1,120}")))
                check(intent.action == ACTION) { "Unsupported harness action" }
                validateTargetPackage(context, phase)
                validateDisposableDevice()
                if (phase == "campaign-seed") {
                    seedInstrumentationMarker(context, token)
                    Log.i(TAG, "PASS:$token:$phase:$requestId")
                    return@Thread
                }
                if (phase == "process-death-seed") {
                    seedInstrumentationMarker(context, token)
                    claimInstrumentationMarker(context, token)
                    Log.i(TAG, "PASS:$token:$phase:$requestId")
                    return@Thread
                }
                if (phase == "campaign-consume") {
                    promoteInstrumentationMarker(context, token)
                    Log.i(TAG, "PASS:$token:$phase:$requestId")
                    return@Thread
                }
                if (phase == "seed-sentinel") {
                    assertFreshDebugDataNamespace(context)
                }
                if (phase == "verify-sentinel") {
                    assertDebugSentinelAuthorization(context, token)
                }
                val updateCompatibilityPhase = context.packageName == UPDATE_COMPAT_PACKAGE &&
                    phase in UPDATE_COMPAT_PHASES
                if (phase in DESTRUCTIVE_PHASES && !updateCompatibilityPhase) {
                    verifyProcessDeathAuthorization(context, token)
                }
                val database = SereaHistoryDatabase.open(context)
                val repository = ConversationRepository(database.historyDao(), KeystoreAesGcm("serea-history-v1"))
                runBlocking(Dispatchers.IO) {
                    when (phase) {
                            "seed-sentinel" -> seedDebugSentinel(context, repository, token)
                            "verify-sentinel" -> verifyDebugSentinel(context, repository, token)
                            "prepare" -> { prepare(context, repository, token); prepareTasks(context) }
                            "verify" -> { verify(context, repository, token); verifyTasks(context) }
                            "delete-all" -> {
                                repository.create("process-death-unrelated", "Unrelated history before Delete All")
                                repository.deleteAll()
                            }
                            "verify-empty" -> {
                                check(repository.list().isEmpty()) {
                                    "Delete All left one or more history conversations behind"
                                }
                                verifyTasks(context)
                            }
                            "task-prepare" -> prepareTasks(context)
                            "task-verify" -> verifyTasks(context)
                            else -> error("Unknown harness phase")
                    }
                }
                if (phase == "verify-empty") consumeProcessDeathAuthorization(context, token)
                Log.i(TAG, "PASS:$token:$phase:$requestId")
            } catch (error: Throwable) {
                Log.e(TAG, "FAIL:$token:$phase:$requestId:${error.javaClass.simpleName}")
            } finally {
                pending.finish()
            }
        }.start()
    }

    /**
     * The normal demo package can contain a person's history and preferences. It may only
     * receive the non-destructive sentinel phases. Process-death mutation phases are tied to
     * the isolated application ID, whose app-private DB, preferences and Keystore namespace are
     * separate from app.serea.android.demo. A conditional Beta test overlay permits only campaign
     * marker and fixture prepare/verify phases in the disposable identity-validation build.
     */
    private fun validateTargetPackage(context: Context, phase: String) {
        val betaValidationPhase = context.packageName == PERSONAL_BETA_PACKAGE && phase in PERSONAL_BETA_PHASES
        val updateCompatibilityPhase = context.packageName == UPDATE_COMPAT_PACKAGE &&
            phase in UPDATE_COMPAT_PHASES
        val expectedPackage = when (phase) {
            "seed-sentinel", "verify-sentinel" -> DEBUG_DEMO_PACKAGE
            "campaign-seed", "process-death-seed", "campaign-consume", "prepare", "verify", "delete-all", "verify-empty", "task-prepare", "task-verify" -> ISOLATED_PACKAGE
            else -> error("Unknown harness phase")
        }
        check(betaValidationPhase || updateCompatibilityPhase || context.packageName == expectedPackage) {
            "Phase $phase is not allowed for package ${context.packageName}"
        }
    }

    private fun validateDisposableDevice() {
        check(Build.VERSION.SDK_INT == 26 || Build.VERSION.SDK_INT == 35) {
            "Unsupported Android API for destructive harness"
        }
        val hardware = Build.HARDWARE.lowercase()
        val fingerprint = Build.FINGERPRINT.lowercase()
        check(hardware in setOf("ranchu", "goldfish") &&
            (fingerprint.contains("generic") || fingerprint.contains("sdk_gphone"))) {
            "Refusing harness operations on a physical or unrecognized Android device"
        }
    }

    private fun assertFreshDebugDataNamespace(context: Context) {
        val databaseDirectory = File(context.applicationInfo.dataDir, "databases")
        val historyDatabase = context.getDatabasePath("serea-history.db")
        check(context.databaseList().isEmpty()) {
            "Refusing to seed the debug sentinel in an app that already has databases"
        }
        check(isAbsentOrEmptyDirectory(databaseDirectory) && !historyDatabase.exists()) {
            "Refusing to seed the debug sentinel when the database namespace is present or unreadable"
        }
        val preferenceDirectory = File(context.applicationInfo.dataDir, "shared_prefs")
        val persistentDirectories = listOf(
            preferenceDirectory,
            context.filesDir,
            context.noBackupFilesDir,
            context.cacheDir,
            context.codeCacheDir,
        )
        check(persistentDirectories.all(::isAbsentOrEmptyDirectory)) {
            "Refusing to seed the debug sentinel in an app that already has private files or preferences"
        }
        check(context.getSharedPreferences("model_selection", Context.MODE_PRIVATE).all.isEmpty()) {
            "Refusing to seed existing model_selection preferences"
        }
    }

    private fun assertDebugSentinelAuthorization(context: Context, token: String) {
        val database = context.getDatabasePath("serea-history.db")
        check(database.isFile && database.length() > 0L) {
            "Refusing to open Room before a debug sentinel database is proven to exist"
        }
        val marker = File(context.noBackupFilesDir, DEBUG_SENTINEL_AUTHORIZATION)
        check(marker.isFile && marker.readText() == token) {
            "Refusing sentinel verification without its matching seed authorization"
        }
        val selection = ModelSelectionSettings(
            context.getSharedPreferences("model_selection", Context.MODE_PRIVATE),
        ).load()
        check(selection.mode == ModelSelectionMode.MANUAL && selection.manualModelId == "sentinel/$token") {
            "Refusing sentinel verification without matching model_selection data"
        }
    }

    private fun isAbsentOrEmptyDirectory(directory: File): Boolean {
        if (!directory.exists()) {
            val parent = directory.parentFile ?: return false
            if (!parent.isDirectory) return false
            val siblings = parent.listFiles() ?: return false
            return siblings.none { it.name == directory.name }
        }
        return directory.isDirectory && directory.listFiles()?.isEmpty() == true
    }

    private fun seedInstrumentationMarker(context: Context, token: String) {
        check(token.matches(Regex("[A-Za-z0-9-]{1,120}")))
        check(context.databaseList().isEmpty()) { "Refusing instrumentation target with existing databases" }
        val databaseDirectory = File(context.applicationInfo.dataDir, "databases")
        check(isAbsentOrEmptyDirectory(databaseDirectory) && !context.getDatabasePath("serea-history.db").exists()) {
            "Refusing instrumentation target with an existing or unreadable database namespace"
        }
        val preferences = File(context.applicationInfo.dataDir, "shared_prefs")
        check(isAbsentOrEmptyDirectory(preferences)) {
            "Refusing instrumentation target with existing preferences"
        }
        check(isAbsentOrEmptyDirectory(context.filesDir)) { "Refusing instrumentation target with existing files" }
        check(isAbsentOrEmptyDirectory(context.noBackupFilesDir)) {
            "Refusing instrumentation target with existing no-backup data"
        }
        val marker = File(context.noBackupFilesDir, INSTRUMENTATION_MARKER)
        check(!marker.exists()) { "Refusing to replace an existing instrumentation marker" }
        check(marker.createNewFile()) { "Unable to create instrumentation marker" }
        marker.writeText(token)
    }

    private fun promoteInstrumentationMarker(context: Context, token: String) {
        val marker = File(context.noBackupFilesDir, CLAIMED_INSTRUMENTATION_MARKER)
        val authorization = File(context.noBackupFilesDir, PROCESS_DEATH_AUTHORIZATION)
        check(marker.isFile && marker.readText() == token) {
            "Missing or mismatched disposable instrumentation marker"
        }
        check(!authorization.exists()) { "Process Death authorization was already used" }
        check(marker.renameTo(authorization)) { "Unable to consume instrumentation marker" }
    }

    private fun claimInstrumentationMarker(context: Context, token: String) {
        val marker = File(context.noBackupFilesDir, INSTRUMENTATION_MARKER)
        val claimedMarker = File(context.noBackupFilesDir, CLAIMED_INSTRUMENTATION_MARKER)
        check(marker.isFile && marker.readText() == token) {
            "Missing or mismatched fresh Process Death marker"
        }
        check(!claimedMarker.exists()) { "Process Death marker was already claimed" }
        check(marker.renameTo(claimedMarker)) { "Unable to claim fresh Process Death marker" }
    }

    private fun verifyProcessDeathAuthorization(context: Context, token: String) {
        val authorization = File(context.noBackupFilesDir, PROCESS_DEATH_AUTHORIZATION)
        check(authorization.isFile && authorization.readText() == token) {
            "Missing or mismatched one-run Process Death authorization"
        }
    }

    private fun consumeProcessDeathAuthorization(context: Context, token: String) {
        verifyProcessDeathAuthorization(context, token)
        val authorization = File(context.noBackupFilesDir, PROCESS_DEATH_AUTHORIZATION)
        check(authorization.delete()) { "Unable to consume Process Death authorization" }
    }

    private suspend fun seedDebugSentinel(
        context: Context,
        repository: ConversationRepository,
        token: String,
    ) {
        val preferences = context.getSharedPreferences("model_selection", Context.MODE_PRIVATE)
        check(preferences.all.isEmpty()) { "Refusing to overwrite existing model_selection preferences" }
        check(repository.list().isEmpty()) { "Refusing to seed a non-empty demo history database" }
        val conversationId = "data-sentinel-$token"
        val timestamp = System.currentTimeMillis()
        repository.create(conversationId, "CI data sentinel $token", now = timestamp)
        repository.addMessage(
            StoredMessage(
                id = "sentinel-message-$token",
                conversationId = conversationId,
                role = MessageRole.USER,
                text = "preserve-demo-data-$token",
                classification = DataClassification.UNKNOWN,
                createdAt = timestamp + 1,
                status = "COMPLETED",
                origin = DataOrigin.USER_AUTHORED,
            ),
        )
        ModelSelectionSettings(preferences).save(
            ModelRoutingPolicy(
                ModelSelectionMode.MANUAL,
                AutoRoutingPolicy.COST_PRIORITY,
                "sentinel/$token",
                maxEstimatedCost = 0.37,
                budgetCurrencyCode = "USD",
            ),
        )
        verifyDebugSentinel(context, repository, token)
        val marker = File(context.noBackupFilesDir, DEBUG_SENTINEL_AUTHORIZATION)
        check(marker.createNewFile()) { "Refusing to replace an existing debug sentinel authorization" }
        marker.writeText(token)
    }

    private suspend fun verifyDebugSentinel(
        context: Context,
        repository: ConversationRepository,
        token: String,
    ) {
        val conversation = checkNotNull(repository.get("data-sentinel-$token"))
        check(conversation.title == "CI data sentinel $token")
        check(conversation.messages.single().let {
            it.id == "sentinel-message-$token" && it.text == "preserve-demo-data-$token" &&
                it.status == "COMPLETED" && it.origin == DataOrigin.USER_AUTHORED
        })
        val selection = ModelSelectionSettings(
            context.getSharedPreferences("model_selection", Context.MODE_PRIVATE),
        ).load()
        check(selection.mode == ModelSelectionMode.MANUAL)
        check(selection.autoPolicy == AutoRoutingPolicy.COST_PRIORITY)
        check(selection.manualModelId == "sentinel/$token")
        check(selection.maxEstimatedCost == 0.37 && selection.budgetCurrencyCode == "USD")
    }

    private suspend fun prepare(context: Context, repository: ConversationRepository, token: String) {
        val base = System.currentTimeMillis()
        repository.delete("process-death-complete")
        repository.delete("process-death-pending")
        repository.delete("process-death-deleted")
        repository.create("process-death-complete", "Completed recovery", now = base)
        repository.addMessage(StoredMessage("pd-user", "process-death-complete", MessageRole.USER,
            "completed request", DataClassification.UNKNOWN, base + 1, "COMPLETED", DataOrigin.USER_AUTHORED))
        repository.addMessage(StoredMessage("pd-assistant", "process-death-complete", MessageRole.ASSISTANT,
            "completed answer", DataClassification.UNKNOWN, base + 2, "COMPLETED", DataOrigin.MODEL_OUTPUT))
        repository.create("process-death-pending", "Pending recovery", now = base + 10)
        repository.addMessage(StoredMessage("pd-pending", "process-death-pending", MessageRole.USER,
            "interrupted request", DataClassification.UNKNOWN, base + 11, "PENDING", DataOrigin.USER_AUTHORED))
        repository.create("process-death-deleted", "Delete recovery", now = base + 20)
        repository.addMessage(StoredMessage("pd-deleted", "process-death-deleted", MessageRole.USER,
            "delete this", DataClassification.UNKNOWN, base + 21, "COMPLETED", DataOrigin.USER_AUTHORED))
        repository.delete("process-death-deleted", now = base + 30)
        ModelSelectionSettings(context.getSharedPreferences("model_selection", Context.MODE_PRIVATE)).save(
            ModelRoutingPolicy(ModelSelectionMode.MANUAL, AutoRoutingPolicy.COST_PRIORITY, "fixture/deleted-after-upgrade",
                maxEstimatedCost = 0.25, budgetCurrencyCode = "USD"),
        )
        val database = SereaHistoryDatabase.open(context)
        UsageRepository(database.usageDao()) { base }.reserve("process-death-usage-$token", 30)
        // A database read completes before the external force-stop boundary.
        check(repository.get("process-death-complete")?.messages?.size == 2)
    }

    private suspend fun verify(context: Context, repository: ConversationRepository, token: String) {
        val completed = checkNotNull(repository.get("process-death-complete"))
        check(completed.messages.map { it.id } == listOf("pd-user", "pd-assistant"))
        check(completed.messages.last().text == "completed answer")
        val pending = checkNotNull(repository.get("process-death-pending"))
        check(pending.messages.count { it.id == "pd-pending" } == 1)
        // MainActivity startup must have converted the orphan and never retried it.
        check(pending.messages.single().status == "OUTCOME_UNKNOWN")
        check(repository.get("process-death-deleted") == null)
        val selection = ModelSelectionSettings(context.getSharedPreferences("model_selection", Context.MODE_PRIVATE)).load()
        check(selection.mode == ModelSelectionMode.MANUAL)
        check(selection.autoPolicy == AutoRoutingPolicy.COST_PRIORITY)
        check(selection.manualModelId == "fixture/deleted-after-upgrade")
        check(selection.maxEstimatedCost == 0.25 && selection.budgetCurrencyCode == "USD")
        check(AdaptiveModelRouter(LocalModelCatalog()).select(RoutingRequest("hello"), selection).model == null)
        val usage = UsageRepository(SereaHistoryDatabase.open(context).usageDao())
        check(usage.snapshot().uncertainRequests >= 1)
        check(SereaHistoryDatabase.open(context).usageDao().reservation("process-death-usage-$token")?.state == "OUTCOME_UNKNOWN")
    }


    private suspend fun prepareTasks(context: Context) {
        val database = tasksDatabase(context)
        val dao = database.taskDao()
        val ids = listOf(TASK_COMPLETED_ID, TASK_TODO_ID, TASK_DELETED_ID)
        ids.forEach { dao.deleteById(it) }
        var next = 0
        val repository = EncryptedTaskRepository(dao, ProcessDeathTaskCipher(context), { java.time.Instant.parse("2026-10-20T00:00:00Z") }) {
            ids[next++]
        }
        val completed = repository.create(TaskDraft("Process recovery completed", "Persisted encrypted task", TaskPriority.HIGH))
        repository.complete(completed.id, completed.revision)
        repository.create(TaskDraft("Process recovery todo", "Must remain incomplete", TaskPriority.NORMAL))
        val deleted = repository.create(TaskDraft("Deleted task"))
        repository.delete(deleted.id, deleted.revision)
        check(repository.get(TASK_COMPLETED_ID)?.status == TaskStatus.COMPLETED)
    }

    private suspend fun verifyTasks(context: Context) {
        val repository = EncryptedTaskRepository(tasksDatabase(context).taskDao(), ProcessDeathTaskCipher(context))
        val complete = checkNotNull(repository.get(TASK_COMPLETED_ID))
        check(complete.status == TaskStatus.COMPLETED && complete.completedAt != null)
        check(complete.title == "Process recovery completed" && complete.description == "Persisted encrypted task")
        check(repository.get(TASK_TODO_ID)?.status == TaskStatus.TODO)
        check(repository.get(TASK_DELETED_ID) == null)
    }

    private fun tasksDatabase(context: Context): TasksDatabase =
        androidx.room.Room.databaseBuilder(context, TasksDatabase::class.java, "serea-tasks-process-death-harness.db").build()

    private class ProcessDeathTaskCipher(context: Context) : app.serea.android.tasks.TaskCipher {
        private val ALIAS = "serea-tasks-process-death-test-v1"
        private val aes = KeystoreAesGcm(ALIAS)
        override fun encrypt(id: String, plaintext: ByteArray) = aes.encrypt(plaintext, aad(id))
        override fun decrypt(id: String, ciphertext: String): ByteArray {
            check(java.security.KeyStore.getInstance("AndroidKeyStore").apply { load(null) }.containsAlias(ALIAS))
            return aes.decrypt(ciphertext, aad(id))
        }
        private fun aad(id: String) = "process-death-task:$id".toByteArray()
    }

    companion object {
        private const val DEBUG_DEMO_PACKAGE = "app.serea.android.demo"
        private const val ISOLATED_PACKAGE = "app.serea.android.isolated"
        private const val PERSONAL_BETA_PACKAGE = "app.serea.android.beta"
        private const val UPDATE_COMPAT_PACKAGE = "app.serea.android.updatecompat"
        private val PERSONAL_BETA_PHASES = setOf("campaign-seed", "campaign-consume", "prepare", "verify")
        private val UPDATE_COMPAT_PHASES = setOf("prepare", "verify")
        private const val INSTRUMENTATION_MARKER = "serea-instrumentation-token"
        private const val CLAIMED_INSTRUMENTATION_MARKER = "serea-instrumentation-claimed"
        private const val PROCESS_DEATH_AUTHORIZATION = "serea-process-death-authorization"
        private const val DEBUG_SENTINEL_AUTHORIZATION = "serea-debug-sentinel-authorization"
        private val DESTRUCTIVE_PHASES = setOf("prepare", "verify", "delete-all", "verify-empty", "task-prepare", "task-verify")
        private const val TASK_COMPLETED_ID = "00000000-0000-0000-0000-000000000026"
        private const val TASK_TODO_ID = "00000000-0000-0000-0000-000000000035"
        private const val TASK_DELETED_ID = "00000000-0000-0000-0000-000000000099"
        const val ACTION = "app.serea.android.action.PROCESS_DEATH_HARNESS"
        private const val TAG = "SereaProcessDeath"
    }
}
