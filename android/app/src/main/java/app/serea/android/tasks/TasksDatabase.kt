package app.serea.android.tasks

import android.content.Context
import androidx.room.Database
import androidx.room.Room
import androidx.room.RoomDatabase

@Database(entities = [TaskRow::class, ProposalReceiptRow::class], version = 1, exportSchema = true)
abstract class TasksDatabase : RoomDatabase() {
    abstract fun taskDao(): TaskDao

    companion object {
        @Volatile private var instance: TasksDatabase? = null
        fun open(context: Context): TasksDatabase = instance ?: synchronized(this) {
            instance ?: Room.databaseBuilder(context.applicationContext, TasksDatabase::class.java, "serea-tasks.db")
                .build().also { instance = it }
        }
    }
}
