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
            final String expectedEditorPackage = extras.getString("expectedEditorPackage");
            OperationResult operation = runOnImeThread(expectedEditorPackage, new ImeOperation() {
                @Override public boolean run(InputConnection current) {
                    return current.setComposingText(text, cursor);
                }
            });
            record("setComposingText text=" + text + " cursor=" + cursor + " accepted=" + operation.accepted
                    + " session=" + operation.session + " connection=" + operation.connectionIdentity);
            putOperationResult(result, operation);
        } else if ("finishComposingText".equals(method)) {
            final String expectedEditorPackage = extras == null ? null : extras.getString("expectedEditorPackage");
            OperationResult operation = runOnImeThread(expectedEditorPackage, new ImeOperation() {
                @Override public boolean run(InputConnection current) {
                    return current.finishComposingText();
                }
            });
            record("finishComposingText accepted=" + operation.accepted
                    + " session=" + operation.session + " connection=" + operation.connectionIdentity);
            putOperationResult(result, operation);
        } else if ("setSelection".equals(method)) {
            final int start = extras == null ? -1 : extras.getInt("start", -1);
            final int end = extras == null ? -1 : extras.getInt("end", -1);
            final String expectedEditorPackage = extras == null ? null : extras.getString("expectedEditorPackage");
            OperationResult operation = runOnImeThread(expectedEditorPackage, new ImeOperation() {
                @Override public boolean run(InputConnection current) {
                    return start >= 0 && end >= 0 && current.setSelection(start, end);
                }
            });
            record("setSelection start=" + start + " end=" + end + " accepted=" + operation.accepted
                    + " session=" + operation.session + " connection=" + operation.connectionIdentity);
            putOperationResult(result, operation);
        } else if ("commitText".equals(method)) {
            final String text = extras == null ? null : extras.getString("text");
            if (text == null) throw new IllegalArgumentException("Missing synthetic fixture text");
            final int cursor = extras.getInt("cursor", 1);
            final String expectedEditorPackage = extras.getString("expectedEditorPackage");
            OperationResult operation = runOnImeThread(expectedEditorPackage, new ImeOperation() {
                @Override public boolean run(InputConnection current) {
                    return current.commitText(text, cursor);
                }
            });
            record("commitText text=" + text + " cursor=" + cursor + " accepted=" + operation.accepted
                    + " session=" + operation.session + " connection=" + operation.connectionIdentity);
            putOperationResult(result, operation);
        }
        return result;
    }

    private static final class OperationResult {
        boolean accepted;
        long session;
        String serviceIdentity = "null";
        String connectionIdentity = "null";
        String editorPackage = "none";
        int inputType = InputType.TYPE_NULL;
    }

    private void putOperationResult(Bundle target, OperationResult operation) {
        target.putBoolean("accepted", operation.accepted);
        target.putLong("operationSession", operation.session);
        target.putString("operationService", operation.serviceIdentity);
        target.putString("operationConnection", operation.connectionIdentity);
        target.putString("operationEditorPackage", operation.editorPackage);
        target.putInt("operationInputType", operation.inputType);
    }

    private interface ImeOperation { boolean run(InputConnection connection); }

    /** Resolve and use the live Android-bound connection atomically on the IME main thread. */
    private OperationResult runOnImeThread(
            final String expectedEditorPackage,
            final ImeOperation operation) {
        final OperationResult[] outcome = {new OperationResult()};
        final CountDownLatch completed = new CountDownLatch(1);
        new Handler(Looper.getMainLooper()).post(new Runnable() {
            @Override public void run() {
                try {
                    RealImeService active = RealImeService.active;
                    InputConnection current = active == null ? null : active.getCurrentInputConnection();
                    if (active != null) {
                        outcome[0].session = active.getInputSessionId();
                        outcome[0].serviceIdentity = RealImeService.identity(active);
                        outcome[0].editorPackage = active.currentEditorPackage;
                        outcome[0].inputType = active.currentInputType;
                        outcome[0].connectionIdentity = RealImeService.identity(current);
                    }
                    if (active != null && current != null
                            && expectedEditorPackage != null
                            && expectedEditorPackage.equals(active.currentEditorPackage)
                            && active.currentInputType != InputType.TYPE_NULL
                            && active.getInputSessionId() > 0) {
                        outcome[0].accepted = operation.run(current);
                    }
                }
                finally { completed.countDown(); }
            }
        });
        try {
            if (!completed.await(5, TimeUnit.SECONDS)) throw new IllegalStateException("IME main thread timed out");
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException("Interrupted waiting for IME main thread", error);
        }
        return outcome[0];
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
