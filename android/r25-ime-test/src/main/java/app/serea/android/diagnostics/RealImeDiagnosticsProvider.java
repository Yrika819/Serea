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
    private static final String REASON_NOT_EVALUATED = "NOT_EVALUATED";
    private static final String REASON_NO_LIVE_SERVICE = "NO_LIVE_SERVICE";
    private static final String REASON_NO_LIVE_CONNECTION = "NO_LIVE_CONNECTION";
    private static final String REASON_MISSING_EXPECTED_PACKAGE = "MISSING_EXPECTED_PACKAGE";
    private static final String REASON_WRONG_EXPECTED_PACKAGE = "WRONG_EXPECTED_PACKAGE";
    private static final String REASON_MISSING_EXPECTED_SERVICE = "MISSING_EXPECTED_SERVICE";
    private static final String REASON_STALE_SERVICE = "STALE_SERVICE";
    private static final String REASON_MISSING_EXPECTED_SESSION = "MISSING_EXPECTED_SESSION";
    private static final String REASON_INVALID_EXPECTED_SESSION = "INVALID_EXPECTED_SESSION";
    private static final String REASON_INVALID_CURRENT_SESSION = "INVALID_CURRENT_SESSION";
    private static final String REASON_STALE_SESSION = "STALE_SESSION";
    private static final String REASON_MISSING_EXPECTED_CONNECTION = "MISSING_EXPECTED_CONNECTION";
    private static final String REASON_INVALID_EXPECTED_CONNECTION = "INVALID_EXPECTED_CONNECTION";
    private static final String REASON_STALE_CONNECTION = "STALE_CONNECTION";
    private static final String REASON_INVALID_INPUT_TYPE = "INVALID_INPUT_TYPE";
    private static final String REASON_INVOKED_ACCEPTED = "INVOKED_ACCEPTED";
    private static final String REASON_INVOKED_RETURNED_FALSE = "INVOKED_RETURNED_FALSE";
    private static final String REASON_INVOKED_THROWN = "INVOKED_THROWN";

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
            OperationResult operation = runOnImeThread(method, extras, new ImeOperation() {
                @Override public boolean run(InputConnection current) {
                    return current.setComposingText(text, cursor);
                }
            });
            recordOperation(method, operation);
            putOperationResult(result, operation);
        } else if ("finishComposingText".equals(method)) {
            OperationResult operation = runOnImeThread(method, extras, new ImeOperation() {
                @Override public boolean run(InputConnection current) {
                    return current.finishComposingText();
                }
            });
            recordOperation(method, operation);
            putOperationResult(result, operation);
        } else if ("setSelection".equals(method)) {
            final int start = extras == null ? -1 : extras.getInt("start", -1);
            final int end = extras == null ? -1 : extras.getInt("end", -1);
            OperationResult operation = runOnImeThread(method, extras, new ImeOperation() {
                @Override public boolean run(InputConnection current) {
                    return current.setSelection(start, end);
                }
            });
            recordOperation(method, operation);
            putOperationResult(result, operation);
        } else if ("commitText".equals(method)) {
            final String text = extras == null ? null : extras.getString("text");
            if (text == null) throw new IllegalArgumentException("Missing synthetic fixture text");
            final int cursor = extras.getInt("cursor", 1);
            OperationResult operation = runOnImeThread(method, extras, new ImeOperation() {
                @Override public boolean run(InputConnection current) {
                    return current.commitText(text, cursor);
                }
            });
            recordOperation(method, operation);
            putOperationResult(result, operation);
        }
        return result;
    }

    private static final class OperationResult {
        boolean operationInvoked;
        boolean accepted;
        String reason = REASON_NOT_EVALUATED;
        String expectedEditorPackage = "null";
        String expectedServiceIdentity = "null";
        long expectedSession;
        String expectedConnectionIdentity = "null";
        long session;
        String serviceIdentity = "null";
        String connectionIdentity = "null";
        String editorPackage = "none";
        int inputType = InputType.TYPE_NULL;
    }

    private void putOperationResult(Bundle target, OperationResult operation) {
        target.putBoolean("operationInvoked", operation.operationInvoked);
        target.putBoolean("accepted", operation.accepted);
        target.putString("operationReason", operation.reason);
        target.putString("operationExpectedPackage", operation.expectedEditorPackage);
        target.putString("operationExpectedService", operation.expectedServiceIdentity);
        target.putLong("operationExpectedSession", operation.expectedSession);
        target.putString("operationExpectedConnection", operation.expectedConnectionIdentity);
        target.putLong("operationSession", operation.session);
        target.putString("operationService", operation.serviceIdentity);
        target.putString("operationConnection", operation.connectionIdentity);
        target.putString("operationEditorPackage", operation.editorPackage);
        target.putInt("operationInputType", operation.inputType);
    }

    private void recordOperation(String operationName, OperationResult operation) {
        record("IME_OPERATION name=" + operationName
                + " operationInvoked=" + operation.operationInvoked
                + " accepted=" + operation.accepted
                + " reason=" + operation.reason
                + " expectedPackage=" + operation.expectedEditorPackage
                + " actualPackage=" + operation.editorPackage
                + " expectedService=" + operation.expectedServiceIdentity
                + " actualService=" + operation.serviceIdentity
                + " expectedSession=" + operation.expectedSession
                + " actualSession=" + operation.session
                + " expectedConnection=" + operation.expectedConnectionIdentity
                + " actualConnection=" + operation.connectionIdentity
                + " inputType=" + operation.inputType);
    }

    private interface ImeOperation { boolean run(InputConnection connection); }

    /** Resolve and use the live Android-bound connection atomically on the IME main thread. */
    private OperationResult runOnImeThread(
            final String operationName,
            final Bundle request,
            final ImeOperation operation) {
        final OperationResult[] outcome = {new OperationResult()};
        final boolean hasExpectedSession = request != null && request.containsKey("expectedSession");
        final boolean hasExpectedConnection = request != null && request.containsKey("expectedConnection");
        final String expectedEditorPackage = request == null ? null : request.getString("expectedEditorPackage");
        final String expectedServiceIdentity = request == null ? null : request.getString("expectedService");
        final long expectedSession = request == null ? 0 : request.getLong("expectedSession", 0);
        final String expectedConnectionIdentity = request == null ? null : request.getString("expectedConnection");
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
                    outcome[0].expectedEditorPackage = expectedEditorPackage == null ? "null" : expectedEditorPackage;
                    outcome[0].expectedServiceIdentity = expectedServiceIdentity == null
                            ? "null" : expectedServiceIdentity;
                    outcome[0].expectedSession = expectedSession;
                    outcome[0].expectedConnectionIdentity = expectedConnectionIdentity == null
                            ? "null" : expectedConnectionIdentity;
                    outcome[0].reason = rejectionReason(
                            active,
                            current,
                            expectedEditorPackage,
                            expectedServiceIdentity,
                            hasExpectedSession,
                            expectedSession,
                            hasExpectedConnection,
                            expectedConnectionIdentity);
                    if (REASON_NOT_EVALUATED.equals(outcome[0].reason)) {
                        outcome[0].operationInvoked = true;
                        try {
                            outcome[0].accepted = operation.run(current);
                            outcome[0].reason = outcome[0].accepted
                                    ? REASON_INVOKED_ACCEPTED : REASON_INVOKED_RETURNED_FALSE;
                        } catch (RuntimeException error) {
                            outcome[0].reason = REASON_INVOKED_THROWN;
                            Log.e(RealImeService.TAG, "IME_OPERATION_THROWN name=" + operationName
                                    + " exception=" + error.getClass().getSimpleName());
                        }
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

    private String rejectionReason(
            RealImeService active,
            InputConnection current,
            String expectedEditorPackage,
            String expectedServiceIdentity,
            boolean hasExpectedSession,
            long expectedSession,
            boolean hasExpectedConnection,
            String expectedConnectionIdentity) {
        if (active == null) return REASON_NO_LIVE_SERVICE;
        if (current == null) return REASON_NO_LIVE_CONNECTION;
        if (expectedEditorPackage == null || expectedEditorPackage.trim().isEmpty()) {
            return REASON_MISSING_EXPECTED_PACKAGE;
        }
        if (!expectedEditorPackage.equals(active.currentEditorPackage)) return REASON_WRONG_EXPECTED_PACKAGE;
        if (expectedServiceIdentity == null || expectedServiceIdentity.trim().isEmpty()
                || "null".equals(expectedServiceIdentity)) {
            return REASON_MISSING_EXPECTED_SERVICE;
        }
        if (!expectedServiceIdentity.equals(RealImeService.identity(active))) return REASON_STALE_SERVICE;
        if (!hasExpectedSession) return REASON_MISSING_EXPECTED_SESSION;
        if (expectedSession <= 0) return REASON_INVALID_EXPECTED_SESSION;
        if (active.getInputSessionId() <= 0) return REASON_INVALID_CURRENT_SESSION;
        if (expectedSession != active.getInputSessionId()) return REASON_STALE_SESSION;
        if (!hasExpectedConnection) return REASON_MISSING_EXPECTED_CONNECTION;
        if (expectedConnectionIdentity == null || expectedConnectionIdentity.trim().isEmpty()
                || "null".equals(expectedConnectionIdentity)) {
            return REASON_INVALID_EXPECTED_CONNECTION;
        }
        if (!expectedConnectionIdentity.equals(RealImeService.identity(current))) return REASON_STALE_CONNECTION;
        if (active.currentInputType == InputType.TYPE_NULL) return REASON_INVALID_INPUT_TYPE;
        return REASON_NOT_EVALUATED;
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
