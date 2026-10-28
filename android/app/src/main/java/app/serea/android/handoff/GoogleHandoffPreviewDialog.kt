package app.serea.android.handoff

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.heightIn
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.AlertDialog
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp

@Composable
fun GoogleHandoffPreviewDialog(
    preview: GoogleHandoffPreview,
    errorMessage: String? = null,
    confirming: Boolean = false,
    onCancel: () -> Unit,
    onConfirm: () -> Unit,
) {
    AlertDialog(
        onDismissRequest = { if (!confirming) onCancel() },
        title = { Text("Chatへ渡す内容を確認") },
        text = {
            Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text("対象: ${preview.displayTitle}", fontWeight = FontWeight.SemiBold)
                Text("情報源: ${preview.service.label} · ${preview.syntheticAccountIdentifier}")
                Text("利用目的: ${preview.purpose}")
                Text("送信先: この端末内のローカルChat")
                Text("Google由来の内容は未信頼データとして扱います。Cloud AIには送信しません。")
                Text(
                    preview.selectedContent,
                    modifier = Modifier.fillMaxWidth().heightIn(max = 220.dp).verticalScroll(rememberScrollState())
                        .padding(8.dp),
                    style = MaterialTheme.typography.bodySmall,
                )
                Text("確認すると、この端末内の処理だけに同意します。Cloud送信への同意にはなりません。")
                if (errorMessage != null) {
                    Text(errorMessage, color = MaterialTheme.colorScheme.error)
                }
            }
        },
        confirmButton = {
            TextButton(onClick = onConfirm, enabled = !confirming) {
                Text(if (confirming) "確認中…" else "端末内で処理してChatへ渡す")
            }
        },
        dismissButton = {
            TextButton(onClick = onCancel, enabled = !confirming) { Text("キャンセル") }
        },
    )
}

private val GoogleHandoffService.label: String
    get() = when (this) {
        GoogleHandoffService.GMAIL -> "Fake Gmail"
        GoogleHandoffService.CALENDAR -> "Fake Calendar"
    }
