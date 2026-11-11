# Rust resolves this application class by exact name through JNI, so it is not
# an ordinary Java-only call site.
-keep class app.hidlins.HidlinsNative { *; }

# Rust invokes receive([B)Z by name; R8 cannot see that Java call edge.
-keepclassmembers interface app.hidlins.HidlinsNative$ClipboardReceiver {
    public boolean receive(byte[]);
}
-keepclassmembers class app.hidlins.ClipboardController {
    public boolean receive(byte[]);
}
