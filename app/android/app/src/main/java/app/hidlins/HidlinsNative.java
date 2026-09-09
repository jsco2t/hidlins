package app.hidlins;

/** Exact Java ABI for the Rust clipboard boundary. */
public final class HidlinsNative {
    private HidlinsNative() {}

    /** Receives one mutable secret buffer and must wipe it before returning. */
    public interface ClipboardReceiver {
        boolean receive(byte[] value);
    }

    public static native int consumeClipboard(String transferId, ClipboardReceiver receiver);
}
