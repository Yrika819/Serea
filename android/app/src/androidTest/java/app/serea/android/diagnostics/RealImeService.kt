package app.serea.android.diagnostics

import android.inputmethodservice.InputMethodService
import android.os.SystemClock
import android.util.Log
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import java.util.concurrent.CopyOnWriteArrayList

/** Test APK only. The system binds this service as an IME on a disposable emulator. */
class RealImeService : InputMethodService() {
    private var session = 0L
    val inputSessionId: Long get() = session

    override fun onStartInput(attribute: EditorInfo?, restarting: Boolean) {
        super.onStartInput(attribute, restarting)
        session += 1
        record("onStartInput restarting=$restarting editor=${attribute?.fieldId} session=$session")
    }

    override fun onFinishInput() {
        record("onFinishInput session=$session")
        super.onFinishInput()
    }

    override fun onStartInputView(info: EditorInfo?, restarting: Boolean) {
        super.onStartInputView(info, restarting)
        record("onStartInputView restarting=$restarting session=$session")
    }

    override fun onFinishInputView(finishingInput: Boolean) {
        record("onFinishInputView finishingInput=$finishingInput session=$session")
        super.onFinishInputView(finishingInput)
    }

    private fun record(event: String) {
        val detail = "uptimeNanos=${SystemClock.elapsedRealtimeNanos()} service=${identity(this)} $event"
        events += detail
        Log.i(TAG, detail)
    }

    companion object {
        const val TAG = "SereaRealIme"
        @Volatile var active: RealImeService? = null
            private set
        val events = CopyOnWriteArrayList<String>()

        fun identity(value: Any?): String = value?.let {
            "${it.javaClass.name}@${System.identityHashCode(it)}"
        } ?: "null"
    }

    override fun onCreate() {
        super.onCreate()
        active = this
        record("onCreate")
    }

    override fun onDestroy() {
        record("onDestroy")
        if (active === this) active = null
        super.onDestroy()
    }
}
