package app.hidlins.verifier;

import android.app.Activity;
import android.os.Bundle;
import android.util.Log;
import android.widget.TextView;

/** Emits a stable, payload-free marker after Application initialization succeeds. */
public final class VerifierActivity extends Activity {
    private static final String TAG = "HidlinsVerifier";

    @Override
    protected void onCreate(Bundle state) {
        super.onCreate(state);
        Log.i(TAG, "HIDLINS_VERIFIER_INIT_OK");
        TextView view = new TextView(this);
        view.setText("HIDLINS_VERIFIER_INIT_OK");
        setContentView(view);
    }
}
