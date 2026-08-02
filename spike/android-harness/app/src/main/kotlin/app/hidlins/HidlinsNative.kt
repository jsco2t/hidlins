package app.hidlins

import android.content.Context

/**
 * Binding for the production JNI export in `crates/hidlins-api/src/android.rs`
 * (`Java_app_hidlins_HidlinsNative_initVerifier`). The package + object name
 * here ARE the ABI — they must match the Rust symbol. This exact class is
 * what the real app's `Application` subclass will use in T8.2.
 */
object HidlinsNative {
    @JvmStatic
    external fun initVerifier(context: Context): Boolean
}
