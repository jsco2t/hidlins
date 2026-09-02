package app.hidlins.verifier;

import android.app.Application;
import app.hidlins.HidlinsNative;

/** Initializes the platform TLS verifier before any Activity or Flutter engine. */
public final class VerifierApplication extends Application {
    @Override
    public void onCreate() {
        super.onCreate();
        System.loadLibrary("hidlins_api");

        // Exercise the JNI failure boundary in the same release/R8 process.
        // A bad Context must become the stable Java exception, must not abort
        // the VM, and must leave verifier initialization retryable.
        try {
            HidlinsNative.initVerifier(null);
            throw new IllegalStateException("hidlins verifier accepted a null context");
        } catch (IllegalStateException expected) {
            if (!expected.getMessage().startsWith("hidlins: rustls platform verifier")) {
                throw expected;
            }
        }

        if (!HidlinsNative.initVerifier(this)) {
            throw new IllegalStateException("hidlins verifier initialization returned false");
        }
    }
}
