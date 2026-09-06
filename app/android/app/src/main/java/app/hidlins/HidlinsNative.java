package app.hidlins;

import android.content.Context;

/** Exact Java ABI for the Rust TLS verifier initializer. */
public final class HidlinsNative {
    private HidlinsNative() {}

    /** Receives one mutable secret buffer and must wipe it before returning. */
    public interface ClipboardReceiver {
        boolean receive(byte[] value);
    }

    public static native boolean initVerifier(Context context);
    public static native int consumeClipboard(String transferId, ClipboardReceiver receiver);
}
