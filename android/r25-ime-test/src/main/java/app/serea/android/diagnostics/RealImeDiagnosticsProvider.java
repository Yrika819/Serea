package app.serea.android.diagnostics;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.text.InputType;
import android.util.Log;
import android.view.inputmethod.InputConnection;

import java.util.ArrayList;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/** Signature-permission-gated IPC bridge for the test APK to the active IME session. */
public final class RealImeDiagnosticsProvider extends ContentProvider {
    @Override public boolean onCreate() { return true; }

    @Override public Bundle call(String method, String arg, Bundle extras) {
        RealImeService service = RealImeService.active;
        InputConnection connection = service == null ? null : service.getCurrentInputConnection();
        Bundle result = new Bundle();
        result.putBoolean("serviceBound", service != null);
        result.putBoolean("connectionBound", connection != null);
        result.putString("serviceIdentity", RealImeService.identity(service));
        result.putString("connectionIdentity", RealImeService.identity(connection));
        result.putLong("session", service == null ? 0 : service.getInputSessionId());
        result.putString("editorPackage", service == null ? "none" : service.currentEditorPackage);
        result.putInt("inputType", service == null ? InputType.TYPE_NULL : service.currentInputType);
        result.putInt("fieldId", service == null ? -1 : service.currentFieldId);
        result.putString("fieldName", service == null ? "null" : service.currentFieldName);
        result.putStringArrayList("events", new ArrayList<>(RealImeService.events));
        if ("setComposingText".equals(method)) {
            final String text = extras == null ? null : extras.getString("text");
            if (text == null) throw new IllegalArgumentException("Missing synthetic fixture text");
            final int cursor = extras.getInt("cursor", 1);
            final long expectedSession = extras.getLong("expectedSession", -1);
            final String expectedConnection = extras.getString("expectedConnection");
            boolean accepted = runOnImeThread(new ImeOperation() {
                @Override public boolean run() {
                    RealImeService active = RealImeService.active;
                    InputConnection current = active == null ? null : active.getCurrentInputConnection();
                    return active != null && active.getInputSessionId() == expectedSession
                            && RealImeService.identity(current).equals(expectedConnection)
                            && current != null && current.setComposingText(text, cursor);
                }
            });
            record("setComposingText text=" + text + " cursor=" + cursor + " accepted=" + accepted);
            result.putBoolean("accepted", accepted);
        } else if ("finishComposingText".equals(method)) {
            final long expectedSession = extras == null ? -1 : extras.getLong("expectedSession", -1);
            final String expectedConnection = extras == null ? null : extras.getString("expectedConnection");
            boolean accepted = runOnImeThread(new ImeOperation() {
                @Override public boolean run() {
                    RealImeService active = RealImeService.active;
                    InputConnection current = active == null ? null : active.getCurrentInputConnection();
                    return active != null && active.getInputSessionId() == expectedSession
                            && RealImeService.identity(current).equals(expectedConnection)
                            && current != null && current.finishComposingText();
                }
            });
            record("finishComposingText accepted=" + accepted);
            result.putBoolean("accepted", accepted);
        }
        return result;
    }

    private interface ImeOperation { boolean run(); }

    private boolean runOnImeThread(final ImeOperation operation) {
        final boolean[] accepted = {false};
        final CountDownLatch completed = new CountDownLatch(1);
        new Handler(Looper.getMainLooper()).post(new Runnable() {
            @Override public void run() {
                try { accepted[0] = operation.run(); }
                finally { completed.countDown(); }
            }
        });
        try {
            if (!completed.await(5, TimeUnit.SECONDS)) throw new IllegalStateException("IME main thread timed out");
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException("Interrupted waiting for IME main thread", error);
        }
        return accepted[0];
    }

    private void record(String event) {
        Log.i(RealImeService.TAG, "uptimeNanos=" + SystemClock.elapsedRealtimeNanos() + " " + event);
    }

    @Override public Cursor query(Uri uri, String[] projection, String selection, String[] selectionArgs, String sortOrder) { return null; }
    @Override public String getType(Uri uri) { return null; }
    @Override public Uri insert(Uri uri, ContentValues values) { return null; }
    @Override public int delete(Uri uri, String selection, String[] selectionArgs) { return 0; }
    @Override public int update(Uri uri, ContentValues values, String selection, String[] selectionArgs) { return 0; }
}
