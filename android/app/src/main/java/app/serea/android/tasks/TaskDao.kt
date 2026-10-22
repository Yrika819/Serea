package app.serea.android.tasks

import androidx.room.Dao
import androidx.room.Entity
import androidx.room.Insert
import androidx.room.OnConflictStrategy
import androidx.room.PrimaryKey
import androidx.room.Query
import androidx.room.Transaction

@Entity(tableName = "tasks")
data class TaskRow(
    @PrimaryKey val id: String,
    val revision: Long,
    val encryptedRecord: String,
)

@Entity(tableName = "proposal_receipts")
data class ProposalReceiptRow(@PrimaryKey val proposalId: String, val taskId: String)

@Dao
interface TaskDao {
    @Query("SELECT * FROM tasks") suspend fun allRows(): List<TaskRow>
    @Query("SELECT * FROM tasks WHERE id = :id") suspend fun row(id: String): TaskRow?
    @Insert(onConflict = OnConflictStrategy.ABORT) suspend fun insert(row: TaskRow)
    @Insert(onConflict = OnConflictStrategy.ABORT) suspend fun insertReceipt(row: ProposalReceiptRow)
    @Query("SELECT * FROM proposal_receipts WHERE proposalId = :proposalId") suspend fun receipt(proposalId: String): ProposalReceiptRow?
    @Transaction
    suspend fun insertTaskForProposal(task: TaskRow, receipt: ProposalReceiptRow) {
        insert(task)
        insertReceipt(receipt)
    }
    @Transaction
    suspend fun compareAndSetTaskForProposal(
        id: String,
        expectedRevision: Long,
        nextRevision: Long,
        encrypted: String,
        receipt: ProposalReceiptRow,
    ) {
        if (receipt(receipt.proposalId) != null) throw TaskConflictException()
        if (compareAndSet(id, expectedRevision, nextRevision, encrypted) != 1) throw TaskConflictException()
        insertReceipt(receipt)
    }
    @Transaction
    suspend fun deleteTaskForProposal(id: String, expectedRevision: Long, receipt: ProposalReceiptRow) {
        if (receipt(receipt.proposalId) != null) throw TaskConflictException()
        if (deleteIfRevisionMatches(id, expectedRevision) != 1) throw TaskConflictException()
        insertReceipt(receipt)
    }
    @Query("UPDATE tasks SET revision = :nextRevision, encryptedRecord = :encrypted WHERE id = :id AND revision = :expectedRevision")
    suspend fun compareAndSet(id: String, expectedRevision: Long, nextRevision: Long, encrypted: String): Int
    @Query("DELETE FROM tasks WHERE id = :id AND revision = :expectedRevision") suspend fun deleteIfRevisionMatches(id: String, expectedRevision: Long): Int
    @Query("DELETE FROM tasks WHERE id = :id") suspend fun deleteById(id: String): Int
}
