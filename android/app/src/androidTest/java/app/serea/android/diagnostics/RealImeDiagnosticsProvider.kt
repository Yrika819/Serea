package app.serea.android.diagnostics

import android.content.ContentProvider
import android.content.ContentValues
import android.database.Cursor
import android.net.Uri
import android.os.Bundle
import android.os.SystemClock
import android.util.Log

/** Intra-test-APK bridge for instrumentation running in the target app's separate process. */
class RealImeDiagnosticsProvider : ContentProvider() {
    override fun onCreate(): Boolean = true

    @Suppress("DEPRECATION")
    override fun call(method: String, arg: String, authority: String?, extras: Bundle?): Bundle? = callInternal(method, extras)

    private fun callInternal(method: String, extras: Bundle?): Bundle {
        val service = RealImeService.active
        val connection = service?.currentInputConnection
        val result = Bundle().apply {
            putBoolean("serviceBound", service != null)
            putBoolean("connectionBound", connection != null)
            putString("serviceIdentity", RealImeService.identity(service))
            putString("connectionIdentity", RealImeService.identity(connection))
            putLong("session", service?.inputSessionId ?: 0L)
            putStringArrayList("events", ArrayList(RealImeService.events))
        }
        when (method) {
            "setComposingText" -> {
                val text = requireNotNull(extras?.getString("text"))
                val cursor = extras.getInt("cursor", 1)
                val accepted = connection?.setComposingText(text, cursor) ?: false
                record("setComposingText text=$text cursor=$cursor accepted=$accepted session=${service?.inputSessionId}")
                result.putBoolean("accepted", accepted)
            }
            "finishComposingText" -> {
                val accepted = connection?.finishComposingText() ?: false
                record("finishComposingText accepted=$accepted session=${service?.inputSessionId}")
                result.putBoolean("accepted", accepted)
            }
        }
        return result
    }

    private fun record(event: String) {
        Log.i(RealImeService.TAG, "uptimeNanos=${SystemClock.elapsedRealtimeNanos()} $event")
    }

    override fun query(uri: Uri, projection: Array<out String>?, selection: String?, selectionArgs: Array<out String>?, sortOrder: String?): Cursor? = null
    override fun getType(uri: Uri): String? = null
    override fun insert(uri: Uri, values: ContentValues?): Uri? = null
    override fun delete(uri: Uri, selection: String?, selectionArgs: Array<out String>?): Int = 0
    override fun update(uri: Uri, values: ContentValues?, selection: String?, selectionArgs: Array<out String>?): Int = 0
}
