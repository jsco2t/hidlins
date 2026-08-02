# rustls-platform-verifier: Rust resolves these classes BY NAME through
# JNI (FindClass on org.rustls.platformverifier.CertificateVerifier), so
# no Java/Kotlin code references them and R8 would otherwise strip the
# whole package from a minified build. Renaming is equally fatal — the
# JNI lookup is by exact name. This is the keep rule T8.2 must carry.
-keep class org.rustls.platformverifier.** { *; }

# Our own JNI-bound singletons: `external fun` declarations are native
# methods, so AGP's default proguard-android-optimize.txt already keeps
# their names (-keepclasseswithmembernames ... native <methods>), but the
# CLASS name must survive too for the Java_app_hidlins_* symbol lookup.
-keep class app.hidlins.HidlinsNative { *; }
-keep class app.hidlins.spike.SpikeNative { *; }
