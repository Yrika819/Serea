package app.serea.android.tasks

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import app.serea.android.SereaTheme
import java.time.Instant

class TasksActivity : ComponentActivity() {
    private val database by lazy { TasksDatabase.open(this) }
    private val taskClock: () -> Instant = Instant::now
    private val repository by lazy { EncryptedTaskRepository(database.taskDao(), KeystoreTaskCipher(), taskClock) }
    private val taskViewModel: TaskViewModel by viewModels {
        TaskViewModel.factory(repository, taskClock)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { SereaTheme { TaskScreen(taskViewModel) } }
    }
}
