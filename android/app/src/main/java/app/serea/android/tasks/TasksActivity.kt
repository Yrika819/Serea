package app.serea.android.tasks

import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.activity.viewModels
import app.serea.android.SereaTheme

class TasksActivity : ComponentActivity() {
    private val database by lazy { TasksDatabase.open(this) }
    private val taskViewModel: TaskViewModel by viewModels {
        TaskViewModel.factory(EncryptedTaskRepository(database.taskDao(), KeystoreTaskCipher()))
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContent { SereaTheme { TaskScreen(taskViewModel) } }
    }
}
