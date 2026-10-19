package app.serea.android

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.util.Log
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.runBlocking

/** Debug-only ADB harness endpoint. It is absent from release builds. */
class ProcessDeathHarnessReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val pending = goAsync()
        Thread {
            try {
                val database = SereaHistoryDatabase.open(context)
                val repository = ConversationRepository(database.historyDao(), KeystoreAesGcm("serea-history-v1"))
                runBlocking(Dispatchers.IO) {
                    when (intent.action) {
                        ACTION -> when (intent.getStringExtra("phase")) {
                            "prepare" -> prepare(repository)
                            "verify" -> verify(repository)
                            "delete-all" -> repository.deleteAll()
                            "verify-empty" -> check(listOf("process-death-complete", "process-death-pending", "process-death-deleted")
                                .all { repository.get(it) == null })
                            else -> error("Unknown harness phase")
                        }
                    }
                }
                Log.i(TAG, "PASS:${intent.getStringExtra("phase")}")
            } catch (error: Throwable) {
                Log.e(TAG, "FAIL:${intent.getStringExtra("phase")}:${error.javaClass.simpleName}")
            } finally {
                pending.finish()
            }
        }.start()
    }

    private suspend fun prepare(repository: ConversationRepository) {
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
        // A database read completes before the external force-stop boundary.
        check(repository.get("process-death-complete")?.messages?.size == 2)
    }

    private suspend fun verify(repository: ConversationRepository) {
        val completed = checkNotNull(repository.get("process-death-complete"))
        check(completed.messages.map { it.id } == listOf("pd-user", "pd-assistant"))
        check(completed.messages.last().text == "completed answer")
        val pending = checkNotNull(repository.get("process-death-pending"))
        check(pending.messages.count { it.id == "pd-pending" } == 1)
        // MainActivity startup must have converted the orphan and never retried it.
        check(pending.messages.single().status == "OUTCOME_UNKNOWN")
        check(repository.get("process-death-deleted") == null)
    }

    companion object {
        const val ACTION = "app.serea.android.action.PROCESS_DEATH_HARNESS"
        private const val TAG = "SereaProcessDeath"
    }
}
