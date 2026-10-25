package app.serea.android.diagnostics;

import android.inputmethodservice.InputMethodService;
import android.os.SystemClock;
import android.util.Log;
import android.view.View;
import android.view.inputmethod.EditorInfo;
import android.widget.TextView;

import java.util.List;
import java.util.concurrent.CopyOnWriteArrayList;

/** Standalone test-only IME APK; the system owns and binds its input session. */
public final class RealImeService extends InputMethodService {
    public static final String TAG = "SereaRealIme";
    public static final List<String> events = new CopyOnWriteArrayList<>();
    public static volatile RealImeService active;
    public volatile String currentEditorPackage = "none";
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

    @Override public void onCreate() {
        super.onCreate();
        active = this;
        record("onCreate");
    }

    @Override public View onCreateInputView() {
        TextView view = new TextView(this);
        view.setText("Serea diagnostic IME");
        view.setTextSize(16);
        view.setPadding(24, 20, 24, 20);
        view.setFocusable(false);
        record("onCreateInputView");
        return view;
    }

    @Override public void onStartInput(EditorInfo attribute, boolean restarting) {
        super.onStartInput(attribute, restarting);
        session++;
        currentEditorPackage = attribute == null ? "null" : String.valueOf(attribute.packageName);
        record("onStartInput restarting=" + restarting + " session=" + session
                + " package=" + (attribute == null ? "null" : attribute.packageName)
                + " fieldId=" + (attribute == null ? -1 : attribute.fieldId)
                + " fieldName=" + (attribute == null ? "null" : attribute.fieldName)
                + " inputType=" + (attribute == null ? -1 : attribute.inputType)
                + " imeOptions=" + (attribute == null ? -1 : attribute.imeOptions)
                + " privateImeOptions=" + (attribute == null ? "null" : attribute.privateImeOptions));
    }

    @Override public void onFinishInput() {
        record("onFinishInput session=" + session);
        currentEditorPackage = "none";
        super.onFinishInput();
    }

    @Override public void onStartInputView(EditorInfo info, boolean restarting) {
        super.onStartInputView(info, restarting);
        record("onStartInputView restarting=" + restarting + " session=" + session);
    }

    @Override public void onFinishInputView(boolean finishingInput) {
        record("onFinishInputView finishingInput=" + finishingInput + " session=" + session);
        super.onFinishInputView(finishingInput);
    }

    @Override public void onDestroy() {
        record("onDestroy");
        if (active == this) active = null;
        super.onDestroy();
    }
}
