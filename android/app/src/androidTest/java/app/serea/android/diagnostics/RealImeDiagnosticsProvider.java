package app.serea.android.diagnostics;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.os.SystemClock;
import android.util.Log;
import android.view.inputmethod.InputConnection;

import java.util.ArrayList;
import java.util.concurrent.CountDownLatch;
import java.util.concurrent.TimeUnit;

/** Intra-test-APK bridge for instrumentation in the target app's separate process. */
public final class RealImeDiagnosticsProvider extends ContentProvider {
    @Override
    public boolean onCreate() {
        return true;
    }

    @Override
    public Bundle call(String method, String arg, Bundle extras) {
        RealImeService service = RealImeService.active;
        InputConnection connection = service == null ? null : service.getCurrentInputConnection();
        Bundle result = new Bundle();
        result.putBoolean("serviceBound", service != null);
        result.putBoolean("connectionBound", connection != null);
        result.putString("serviceIdentity", RealImeService.identity(service));
        result.putString("connectionIdentity", RealImeService.identity(connection));
        result.putLong("session", service == null ? 0L : service.getInputSessionId());
        result.putStringArrayList("events", new ArrayList<>(RealImeService.events));

        if ("setComposingText".equals(method)) {
            final String text = extras == null ? null : extras.getString("text");
            if (text == null) throw new IllegalArgumentException("Missing composing text");
            final int cursor = extras.getInt("cursor", 1);
            boolean accepted = runOnImeThread(new ImeOperation() {
                @Override
                public boolean run() {
                    RealImeService active = RealImeService.active;
                    InputConnection current = active == null ? null : active.getCurrentInputConnection();
                    return current != null && current.setComposingText(text, cursor);
                }
            });
            record("setComposingText text=" + text + " cursor=" + cursor + " accepted=" + accepted
                    + " session=" + (RealImeService.active == null ? 0 : RealImeService.active.getInputSessionId()));
            result.putBoolean("accepted", accepted);
        } else if ("finishComposingText".equals(method)) {
            boolean accepted = runOnImeThread(new ImeOperation() {
                @Override
                public boolean run() {
                    RealImeService active = RealImeService.active;
                    InputConnection current = active == null ? null : active.getCurrentInputConnection();
                    return current != null && current.finishComposingText();
                }
            });
            record("finishComposingText accepted=" + accepted + " session="
                    + (RealImeService.active == null ? 0 : RealImeService.active.getInputSessionId()));
            result.putBoolean("accepted", accepted);
        }
        return result;
    }

    private interface ImeOperation {
        boolean run();
    }

    private boolean runOnImeThread(final ImeOperation operation) {
        final boolean[] accepted = {false};
        final CountDownLatch completed = new CountDownLatch(1);
        new Handler(Looper.getMainLooper()).post(new Runnable() {
            @Override
            public void run() {
                try {
                    accepted[0] = operation.run();
                } finally {
                    completed.countDown();
                }
            }
        });
        try {
            if (!completed.await(5, TimeUnit.SECONDS)) {
                throw new IllegalStateException("Timed out waiting for the IME main thread.");
            }
        } catch (InterruptedException error) {
            Thread.currentThread().interrupt();
            throw new IllegalStateException("Interrupted while waiting for the IME main thread.", error);
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
