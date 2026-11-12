package app.serea.android.googleauth;

import android.app.Activity;
import android.content.Intent;
import android.os.Bundle;
import android.os.Looper;
import android.os.Process;
import android.util.Log;
import android.widget.TextView;

import java.util.UUID;

/** External synthetic consent UI hosted by the separate instrumentation APK process. */
public final class SyntheticConsentSenderActivity extends Activity {
    static final String TAG = "SereaSyntheticConsent";
    static final String EXTRA_TOKEN = "token";
    static final String EXTRA_PAYLOAD = "synthetic-consent-payload";

    private final String instanceNonce = UUID.randomUUID().toString();
    private String runToken = "";
    private boolean completed;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        runToken = getIntent().getStringExtra(EXTRA_TOKEN);
        if (runToken == null || !runToken.matches("[A-Za-z0-9-]{1,120}")) {
            throw new IllegalStateException("invalid synthetic sender run token");
        }
        SyntheticConsentSenderRegistry.register(this, runToken, instanceNonce);
        TextView view = new TextView(this);
        view.setText("Synthetic Consent A is waiting");
        setContentView(view);
        Log.i(TAG, "SENDER_CREATED:" + runToken + ":pid=" + Process.myPid() + ":instance=" + instanceNonce);
        Log.i(TAG, "SENDER_IN_FLIGHT:" + runToken + ":pid=" + Process.myPid() + ":instance=" + instanceNonce);
    }

    @Override
    protected void onResume() {
        super.onResume();
        Log.i(TAG, "SENDER_RESUMED:" + runToken + ":pid=" + Process.myPid() + ":instance=" + instanceNonce);
    }

    @Override
    protected void onNewIntent(Intent intent) {
        super.onNewIntent(intent);
        setIntent(intent);
        Log.w(TAG, "SENDER_UNEXPECTED_NEW_INTENT:" + runToken + ":pid=" + Process.myPid() + ":instance=" + instanceNonce);
    }

    boolean completeFromTestHost(String requestToken, int targetPid, String expectedInstanceNonce) {
        if (Looper.myLooper() != Looper.getMainLooper()) {
            throw new IllegalStateException("synthetic Consent completion must run on the Sender main thread");
        }
        if (completed || !requestToken.equals(runToken) || targetPid != Process.myPid()
                || !expectedInstanceNonce.equals(instanceNonce)
                || SyntheticConsentSenderRegistry.current() != this) {
            return false;
        }
        completed = true;
        Log.i(TAG, "SENDER_COMPLETION_REQUESTED:" + runToken + ":pid=" + Process.myPid() + ":instance=" + instanceNonce);
        setResult(Activity.RESULT_OK, new Intent().putExtra(EXTRA_PAYLOAD, "synthetic-consent-result-A"));
        Log.i(TAG, "SENDER_COMPLETED:" + runToken + ":pid=" + Process.myPid() + ":instance=" + instanceNonce
                + ":code=" + Activity.RESULT_OK + ":payload=synthetic-consent-result-A");
        finish();
        return true;
    }

    @Override
    protected void onDestroy() {
        SyntheticConsentSenderRegistry.unregister(this);
        super.onDestroy();
    }

}

final class SyntheticConsentSenderRegistry {
    private static volatile SyntheticConsentSenderActivity current;
    private static volatile String registeredToken;
    private static volatile String registeredNonce;

    static synchronized void register(SyntheticConsentSenderActivity activity, String token, String nonce) {
        current = activity;
        registeredToken = token;
        registeredNonce = nonce;
    }

    static synchronized SyntheticConsentSenderActivity current() {
        return current;
    }

    static synchronized boolean complete(String token, int pid, String nonce) {
        SyntheticConsentSenderActivity activity = current;
        if (activity == null || !token.equals(registeredToken) || !nonce.equals(registeredNonce)
                || activity.isFinishing()) {
            return false;
        }
        return activity.completeFromTestHost(token, pid, nonce);
    }

    static synchronized void unregister(SyntheticConsentSenderActivity activity) {
        if (current == activity) {
            current = null;
            registeredToken = null;
            registeredNonce = null;
        }
    }
}
