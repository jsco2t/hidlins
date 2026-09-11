# Rust resolves this application class by exact name through JNI, so it is not
# an ordinary Java-only call site.
-keep class app.hidlins.HidlinsNative { *; }
