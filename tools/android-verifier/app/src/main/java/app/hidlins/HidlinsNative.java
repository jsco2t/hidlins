package app.hidlins;

import android.content.Context;

/** Exact Java ABI for the single Rust JNI verifier initializer. */
public final class HidlinsNative {
    private HidlinsNative() {}

    public static native boolean initVerifier(Context context);
}
