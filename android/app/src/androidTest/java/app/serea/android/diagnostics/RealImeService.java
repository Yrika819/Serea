package app.serea.android.diagnostics;

import android.inputmethodservice.InputMethodService;
import android.os.SystemClock;
import android.util.Log;
import android.view.inputmethod.EditorInfo;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/** Test APK only. The system binds this service as an IME on a disposable emulator. */
public final class RealImeService extends InputMethodService {
    public static final String TAG = "SereaRealIme";
    public static final List<String> events = new CopyOnWriteArrayList<>();
    public static volatile RealImeService active;

    private long session;

    public long getInputSessionId() {
        return session;
    }

    public static String identity(Object value) {
        return value == null ? "null" : value.getClass().getName() + "@" + System.identityHashCode(value);
    }

    private void record(String event) {
        String detail = "uptimeNanos=" + SystemClock.elapsedRealtimeNanos()
                + " service=" + identity(this) + " " + event;
        events.add(detail);
        Log.i(TAG, detail);
    }

    @Override
    public void onCreate() {
        super.onCreate();
        active = this;
        record("onCreate");
    }

    @Override
    public void onStartInput(EditorInfo attribute, boolean restarting) {
        super.onStartInput(attribute, restarting);
        session++;
        record("onStartInput restarting=" + restarting + " editor="
                + (attribute == null ? "null" : attribute.fieldId) + " session=" + session);
    }

    @Override
    public void onFinishInput() {
        record("onFinishInput session=" + session);
        super.onFinishInput();
    }

    @Override
    public void onStartInputView(EditorInfo info, boolean restarting) {
        super.onStartInputView(info, restarting);
        record("onStartInputView restarting=" + restarting + " session=" + session);
    }

    @Override
    public void onFinishInputView(boolean finishingInput) {
        record("onFinishInputView finishingInput=" + finishingInput + " session=" + session);
        super.onFinishInputView(finishingInput);
    }

    @Override
    public void onDestroy() {
        record("onDestroy");
        if (active == this) active = null;
        super.onDestroy();
    }
}
