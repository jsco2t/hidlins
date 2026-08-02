package app.hidlins.spike

/**
 * Binding for the SPIKE-ONLY probe export
 * (`Java_app_hidlins_spike_SpikeNative_probeTls` in
 * `crates/hidlins-api/src/android_spike.rs`, spike branch only). Drives a
 * hidlins-sync HEAD through the production TLS stack and reports a
 * one-line outcome string.
 */
object SpikeNative {
    @JvmStatic
    external fun probeTls(url: String): String

    @JvmStatic
    external fun probeInitFailure(): Boolean
}
