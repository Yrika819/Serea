package app.serea.android.googleauth;

import android.app.Activity;
import android.content.BroadcastReceiver;
import android.content.Context;
import android.content.Intent;
import android.os.Looper;
import android.util.Log;

/** Explicit completion endpoint packaged only in the AndroidTest APK. */
public final class SyntheticConsentSenderCompletionReceiver extends BroadcastReceiver {
    static final String ACTION_COMPLETE = "app.serea.android.test.action.COMPLETE_SYNTHETIC_CONSENT";
    static final String EXTRA_INSTANCE_NONCE = "instanceNonce";
    static final String EXTRA_TARGET_PID = "targetPid";

    @Override
    public void onReceive(Context context, Intent intent) {
        if (Looper.myLooper() != Looper.getMainLooper()) {
            throw new IllegalStateException("synthetic Consent completion receiver must run on the app main thread");
        }
        String token = intent.getStringExtra(SyntheticConsentSenderActivity.EXTRA_TOKEN);
        String nonce = intent.getStringExtra(EXTRA_INSTANCE_NONCE);
        int targetPid = intent.getIntExtra(EXTRA_TARGET_PID, -1);
        boolean completed = ACTION_COMPLETE.equals(intent.getAction()) && token != null
                && token.matches("[A-Za-z0-9-]{1,120}") && nonce != null && !nonce.isEmpty()
                && targetPid > 0 && SyntheticConsentSenderRegistry.complete(token, targetPid, nonce);
        String event = completed ? "COMPLETION_BROADCAST_ACCEPTED" : "SENDER_COMPLETION_REJECTED";
        Log.i(SyntheticConsentSenderActivity.TAG, event + ":" + (token == null ? "" : token) + ":pid="
                + targetPid + ":instance=" + (nonce == null ? "" : nonce));
        setResultCode(completed ? Activity.RESULT_OK : Activity.RESULT_CANCELED);
    }
}
