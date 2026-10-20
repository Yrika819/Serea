package app.serea.android

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import app.serea.android.KeystoreAesGcm
import app.serea.android.tasks.EncryptedTaskRepository
import app.serea.android.tasks.TaskDraft
import app.serea.android.tasks.TaskPriority
import app.serea.android.tasks.TaskStatus
import app.serea.android.tasks.TasksDatabase
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking

/** Debug-only ADB harness endpoint. It is absent from release builds. */
class ProcessDeathHarnessReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val pending = goAsync()
        Thread {
            val token = intent.getStringExtra("token") ?: "missing-token"
            val phase = intent.getStringExtra("phase") ?: "missing-phase"
            try {
                check(token != "missing-token")
                check(phase != "missing-phase")
                val database = SereaHistoryDatabase.open(context)
                val repository = ConversationRepository(database.historyDao(), KeystoreAesGcm("serea-history-v1"))
                runBlocking(Dispatchers.IO) {
                    when (intent.action) {
                        ACTION -> when (phase) {
                            "prepare" -> { prepare(context, repository, token); prepareTasks(context) }
                            "verify" -> { verify(context, repository, token); verifyTasks(context) }
                            "delete-all" -> repository.deleteAll()
                            "verify-empty" -> {
                                check(listOf("process-death-complete", "process-death-pending", "process-death-deleted")
                                    .all { repository.get(it) == null })
                                verifyTasks(context)
                            }
                            "task-prepare" -> prepareTasks(context)
                            "task-verify" -> verifyTasks(context)
                            else -> error("Unknown harness phase")
                        }
                    }
                }
                Log.i(TAG, "PASS:$token:$phase")
            } catch (error: Throwable) {
                Log.e(TAG, "FAIL:$token:$phase:${error.javaClass.simpleName}")
            } finally {
                pending.finish()
            }
        }.start()
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
        private const val TASK_COMPLETED_ID = "00000000-0000-0000-0000-000000000026"
        private const val TASK_TODO_ID = "00000000-0000-0000-0000-000000000035"
        private const val TASK_DELETED_ID = "00000000-0000-0000-0000-000000000099"
        const val ACTION = "app.serea.android.action.PROCESS_DEATH_HARNESS"
        private const val TAG = "SereaProcessDeath"
    }
}
