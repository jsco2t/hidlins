// SPIKE-ONLY (T6.2, branch spike/android-verifier): minimal Android harness
// that proves the rustls-platform-verifier JNI init + a real TLS request
// through hidlins-sync's production HTTP stack. Never shipped; never merged.
pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        google()
        mavenCentral()
        // The verifier's Kotlin/AAR component, vendored IN-TREE by the
        // rustls-platform-verifier-android crates.io package — no Maven
        // Central fetch for the security-critical component itself.
        maven { url = uri(rootDir.resolve("../../vendor/rustls-platform-verifier-android/maven")) }
    }
}

rootProject.name = "hidlins-android-spike"
include(":app")
