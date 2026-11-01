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
import java.util.concurrent.atomic.AtomicInteger;

/** Signature-permission-gated IPC bridge to the currently Android-bound IME session. */
public final class RealImeDiagnosticsProvider extends ContentProvider {
    private static final long IME_THREAD_TIMEOUT_SECONDS = 5;
    private static final String TARGET_EDITOR_PACKAGE = "app.serea.android.isolated";
    private static final String FIXTURE_DRAFT = "draft";
    private static final String FIXTURE_REPLACEMENT = "draft survives navigation";
    private static final String FIXTURE_INSERTION_GMAIL = "!";
    private static final String FIXTURE_INSERTION_CALENDAR = "?";

    private static final int TASK_PENDING = 0;
    private static final int TASK_READING_OR_VALIDATING = 1;
    private static final int TASK_CANCELLED = 2;
    private static final int TASK_COMPLETE = 3;
    private static final int TASK_OPERATION_STARTED = 4;

    private static final String REASON_SNAPSHOT = "SNAPSHOT";
    private static final String REASON_NOT_EVALUATED = "NOT_EVALUATED";
    private static final String REASON_NO_LIVE_SERVICE = "NO_LIVE_SERVICE";
    private static final String REASON_NO_LIVE_CONNECTION = "NO_LIVE_CONNECTION";
    private static final String REASON_MISSING_EXPECTED_PACKAGE = "MISSING_EXPECTED_PACKAGE";
    private static final String REASON_WRONG_EXPECTED_PACKAGE = "WRONG_EXPECTED_PACKAGE";
    private static final String REASON_UNAPPROVED_FIXTURE_TEXT = "UNAPPROVED_FIXTURE_TEXT";
    private static final String REASON_MISSING_EXPECTED_SERVICE = "MISSING_EXPECTED_SERVICE";
    private static final String REASON_STALE_SERVICE = "STALE_SERVICE";
    private static final String REASON_MISSING_EXPECTED_SESSION = "MISSING_EXPECTED_SESSION";
    private static final String REASON_INVALID_EXPECTED_SESSION = "INVALID_EXPECTED_SESSION";
    private static final String REASON_INVALID_CURRENT_SESSION = "INVALID_CURRENT_SESSION";
    private static final String REASON_STALE_SESSION = "STALE_SESSION";
    private static final String REASON_MISSING_EXPECTED_CONNECTION = "MISSING_EXPECTED_CONNECTION";
    private static final String REASON_INVALID_EXPECTED_CONNECTION = "INVALID_EXPECTED_CONNECTION";
    private static final String REASON_STALE_CONNECTION = "STALE_CONNECTION";
    private static final String REASON_MISSING_EXPECTED_INPUT_TYPE = "MISSING_EXPECTED_INPUT_TYPE";
    private static final String REASON_INVALID_EXPECTED_INPUT_TYPE = "INVALID_EXPECTED_INPUT_TYPE";
    private static final String REASON_INVALID_INPUT_TYPE = "INVALID_INPUT_TYPE";
    private static final String REASON_WRONG_INPUT_TYPE = "WRONG_INPUT_TYPE";
    private static final String REASON_CANCELLED_BEFORE_INVOCATION = "CANCELLED_BEFORE_INVOCATION";
    private static final String REASON_INVOKED_ACCEPTED = "INVOKED_ACCEPTED";
    private static final String REASON_INVOKED_RETURNED_FALSE = "INVOKED_RETURNED_FALSE";
    private static final String REASON_INVOKED_THROWN = "INVOKED_THROWN";

    @Override public boolean onCreate() { return true; }

    @Override public Bundle call(String method, String arg, Bundle extras) {
        if ("events".equals(method)) {
            Bundle result = new Bundle();
            result.putStringArrayList("events", new ArrayList<>(RealImeService.events));
            return result;
        }
        if ("snapshot".equals(method)) {
            OperationResult snapshot = runOnImeThread(method, null, null);
            Bundle result = new Bundle();
            putSnapshotResult(result, snapshot);
            result.putStringArrayList("events", new ArrayList<>(RealImeService.events));
            return result;
        }

        OperationResult operation;
        if ("setComposingText".equals(method)) {
            final String text = extras == null ? null : extras.getString("text");
            if (text == null) throw new IllegalArgumentException("Missing synthetic fixture text");
            if (!isSyntheticFixtureText(text)) {
                operation = rejectedFixtureText(extras);
            } else {
                final int cursor = extras.getInt("cursor", 1);
                operation = runOnImeThread(method, extras, new ImeOperation() {
                    @Override public boolean run(InputConnection current) {
                        return current.setComposingText(text, cursor);
                    }
                });
            }
        } else if ("finishComposingText".equals(method)) {
            operation = runOnImeThread(method, extras, new ImeOperation() {
                @Override public boolean run(InputConnection current) {
                    return current.finishComposingText();
                }
            });
        } else if ("setSelection".equals(method)) {
            final int start = extras == null ? -1 : extras.getInt("start", -1);
            final int end = extras == null ? -1 : extras.getInt("end", -1);
            operation = runOnImeThread(method, extras, new ImeOperation() {
                @Override public boolean run(InputConnection current) {
                    return current.setSelection(start, end);
                }
            });
        } else if ("commitText".equals(method)) {
            final String text = extras == null ? null : extras.getString("text");
            if (text == null) throw new IllegalArgumentException("Missing synthetic fixture text");
            if (!isSyntheticFixtureText(text)) {
                operation = rejectedFixtureText(extras);
            } else {
                final int cursor = extras.getInt("cursor", 1);
                operation = runOnImeThread(method, extras, new ImeOperation() {
                    @Override public boolean run(InputConnection current) {
                        return current.commitText(text, cursor);
                    }
                });
            }
        } else {
            throw new IllegalArgumentException("Unsupported diagnostic method");
        }

        recordOperation(method, operation);
        Bundle result = new Bundle();
        putOperationResult(result, operation);
        return result;
    }

    private static boolean isSyntheticFixtureText(String text) {
        return FIXTURE_DRAFT.equals(text)
                || FIXTURE_REPLACEMENT.equals(text)
                || FIXTURE_INSERTION_GMAIL.equals(text)
                || FIXTURE_INSERTION_CALENDAR.equals(text);
    }

    private OperationResult rejectedFixtureText(Bundle request) {
        OperationResult result = new OperationResult();
        result.reason = REASON_UNAPPROVED_FIXTURE_TEXT;
        if (request != null) {
            String expectedPackage = request.getString("expectedEditorPackage");
            String expectedService = request.getString("expectedService");
            String expectedConnection = request.getString("expectedConnection");
            result.expectedEditorPackage = expectedPackage == null ? "null" : expectedPackage;
            result.expectedServiceIdentity = expectedService == null ? "null" : expectedService;
            result.expectedSession = request.getLong("expectedSession", 0);
            result.expectedConnectionIdentity = expectedConnection == null ? "null" : expectedConnection;
            result.expectedInputType = request.getInt("expectedInputType", InputType.TYPE_NULL);
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
        int expectedInputType = InputType.TYPE_NULL;
        long session;
        String serviceIdentity = "null";
        String connectionIdentity = "null";
        String editorPackage = "none";
        int inputType = InputType.TYPE_NULL;
        int fieldId = -1;
    }

    private void putSnapshotResult(Bundle target, OperationResult snapshot) {
        target.putBoolean("serviceBound", !"null".equals(snapshot.serviceIdentity));
        target.putBoolean("connectionBound", !"null".equals(snapshot.connectionIdentity));
        target.putString("serviceIdentity", snapshot.serviceIdentity);
        target.putString("connectionIdentity", snapshot.connectionIdentity);
        target.putLong("session", snapshot.session);
        target.putString("editorPackage", snapshot.editorPackage);
        target.putInt("inputType", snapshot.inputType);
        target.putInt("fieldId", snapshot.fieldId);
    }

    private void putOperationResult(Bundle target, OperationResult operation) {
        target.putBoolean("operationInvoked", operation.operationInvoked);
        target.putBoolean("accepted", operation.accepted);
        target.putString("operationReason", operation.reason);
        target.putString("operationExpectedPackage", operation.expectedEditorPackage);
        target.putString("operationExpectedService", operation.expectedServiceIdentity);
        target.putLong("operationExpectedSession", operation.expectedSession);
        target.putString("operationExpectedConnection", operation.expectedConnectionIdentity);
        target.putInt("operationExpectedInputType", operation.expectedInputType);
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
                + " expectedInputType=" + operation.expectedInputType
                + " actualInputType=" + operation.inputType);
    }

    private interface ImeOperation { boolean run(InputConnection connection); }

    /** Snapshot validation and InputConnection use happen on the IME main thread. */
    private OperationResult runOnImeThread(
            final String operationName,
            final Bundle request,
            final ImeOperation operation) {
        final OperationResult[] outcome = {new OperationResult()};
        final AtomicInteger taskState = new AtomicInteger(TASK_PENDING);
        final boolean hasExpectedSession = request != null && request.containsKey("expectedSession");
        final boolean hasExpectedConnection = request != null && request.containsKey("expectedConnection");
        final boolean hasExpectedInputType = request != null && request.containsKey("expectedInputType");
        final String expectedEditorPackage = request == null ? null : request.getString("expectedEditorPackage");
        final String expectedServiceIdentity = request == null ? null : request.getString("expectedService");
        final long expectedSession = request == null ? 0 : request.getLong("expectedSession", 0);
        final String expectedConnectionIdentity = request == null ? null : request.getString("expectedConnection");
        final int expectedInputType = request == null ? InputType.TYPE_NULL
                : request.getInt("expectedInputType", InputType.TYPE_NULL);
        final Handler imeHandler = new Handler(Looper.getMainLooper());
        final CountDownLatch completed = new CountDownLatch(1);
        final Runnable task = new Runnable() {
            @Override public void run() {
                if (!taskState.compareAndSet(TASK_PENDING, TASK_READING_OR_VALIDATING)) {
                    completed.countDown();
                    return;
                }
                try {
                    RealImeService active = RealImeService.active;
                    InputConnection current = active == null ? null : active.getCurrentInputConnection();
                    captureCurrentState(outcome[0], active, current);
                    outcome[0].expectedEditorPackage = expectedEditorPackage == null ? "null" : expectedEditorPackage;
                    outcome[0].expectedServiceIdentity = expectedServiceIdentity == null
                            ? "null" : expectedServiceIdentity;
                    outcome[0].expectedSession = expectedSession;
                    outcome[0].expectedConnectionIdentity = expectedConnectionIdentity == null
                            ? "null" : expectedConnectionIdentity;
                    outcome[0].expectedInputType = expectedInputType;

                    if (operation == null) {
                        outcome[0].reason = REASON_SNAPSHOT;
                        return;
                    }

                    outcome[0].reason = rejectionReason(
                            active,
                            current,
                            expectedEditorPackage,
                            expectedServiceIdentity,
                            hasExpectedSession,
                            expectedSession,
                            hasExpectedConnection,
                            expectedConnectionIdentity,
                            hasExpectedInputType,
                            expectedInputType);
                    if (!REASON_NOT_EVALUATED.equals(outcome[0].reason)) return;

                    // A timeout can cancel a queued or validating operation. The atomic
                    // transition makes the operation invocation the point of no return.
                    if (!taskState.compareAndSet(TASK_READING_OR_VALIDATING, TASK_OPERATION_STARTED)) {
                        outcome[0].reason = REASON_CANCELLED_BEFORE_INVOCATION;
                        return;
                    }
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
                } finally {
                    taskState.compareAndSet(TASK_READING_OR_VALIDATING, TASK_COMPLETE);
                    taskState.compareAndSet(TASK_OPERATION_STARTED, TASK_COMPLETE);
                    completed.countDown();
                }
            }
        };

        if (!imeHandler.post(task)) throw new IllegalStateException("Could not post work to the IME main thread");
        try {
            if (!completed.await(IME_THREAD_TIMEOUT_SECONDS, TimeUnit.SECONDS)) {
                if (cancelBeforeInvocation(taskState)) {
                    imeHandler.removeCallbacks(task);
                    record("IME_TASK_CANCELLED name=" + operationName + " reason=TIMEOUT_BEFORE_INVOCATION");
                    throw new IllegalStateException("IME main-thread work timed out and was cancelled before invocation");
                }
                if (taskState.get() == TASK_COMPLETE) return outcome[0];
                if (taskState.get() == TASK_OPERATION_STARTED) {
                    Log.e(RealImeService.TAG, "IME_TASK_TIMEOUT_AFTER_START name=" + operationName
                            + " warning=do-not-retry-this-input-operation");
                    throw new IllegalStateException("IME operation timed out after invocation started; do not retry");
                }
                throw new IllegalStateException("IME main-thread work timed out");
            }
        } catch (InterruptedException error) {
            if (cancelBeforeInvocation(taskState)) {
                imeHandler.removeCallbacks(task);
            } else if (taskState.get() == TASK_OPERATION_STARTED) {
                Log.e(RealImeService.TAG, "IME_TASK_INTERRUPTED_AFTER_START name=" + operationName
                        + " warning=do-not-retry-this-input-operation");
            }
            Thread.currentThread().interrupt();
            throw new IllegalStateException("Interrupted while waiting for IME main-thread work", error);
        }
        return outcome[0];
    }

    private boolean cancelBeforeInvocation(AtomicInteger taskState) {
        return taskState.compareAndSet(TASK_PENDING, TASK_CANCELLED)
                || taskState.compareAndSet(TASK_READING_OR_VALIDATING, TASK_CANCELLED);
    }

    private void captureCurrentState(OperationResult target, RealImeService active, InputConnection current) {
        if (active == null) return;
        target.session = active.getInputSessionId();
        target.serviceIdentity = RealImeService.identity(active);
        target.editorPackage = active.currentEditorPackage;
        target.inputType = active.currentInputType;
        target.connectionIdentity = RealImeService.identity(current);
        target.fieldId = active.currentFieldId;
    }

    private String rejectionReason(
            RealImeService active,
            InputConnection current,
            String expectedEditorPackage,
            String expectedServiceIdentity,
            boolean hasExpectedSession,
            long expectedSession,
            boolean hasExpectedConnection,
            String expectedConnectionIdentity,
            boolean hasExpectedInputType,
            int expectedInputType) {
        if (active == null) return REASON_NO_LIVE_SERVICE;
        if (current == null) return REASON_NO_LIVE_CONNECTION;
        if (expectedEditorPackage == null || expectedEditorPackage.trim().isEmpty()) {
            return REASON_MISSING_EXPECTED_PACKAGE;
        }
        if (!TARGET_EDITOR_PACKAGE.equals(expectedEditorPackage)
                || !TARGET_EDITOR_PACKAGE.equals(active.currentEditorPackage)) {
            return REASON_WRONG_EXPECTED_PACKAGE;
        }
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
        if (!hasExpectedInputType) return REASON_MISSING_EXPECTED_INPUT_TYPE;
        if (expectedInputType == InputType.TYPE_NULL) return REASON_INVALID_EXPECTED_INPUT_TYPE;
        if (active.currentInputType == InputType.TYPE_NULL) return REASON_INVALID_INPUT_TYPE;
        if (expectedInputType != active.currentInputType) return REASON_WRONG_INPUT_TYPE;
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
