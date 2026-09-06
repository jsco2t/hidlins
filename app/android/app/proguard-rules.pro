# Rust resolves the verifier class by exact name through JNI. The application
# classes are also JNI/lifecycle entry points, not ordinary Java call sites.
-keep class org.rustls.platformverifier.** { *; }
-keep class app.hidlins.HidlinsNative { *; }
-keep class app.hidlins.HidlinsApplication { *; }
