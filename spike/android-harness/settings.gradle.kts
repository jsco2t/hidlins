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
        // The verifier's Kotlin/AAR component, vendored IN-TREE by the
        // rustls-platform-verifier-android crates.io package. The
        // exclusiveContent wrapper is what actually ENFORCES "no network
        // fetch for the security-critical component": without it Gradle
        // consults google()/mavenCentral() first, and a same-coordinate
        // artifact appearing upstream (or a typosquat under the rustls
        // group) would silently win over the vendored bytes.
        exclusiveContent {
            forRepository {
                maven { url = uri(rootDir.resolve("../../vendor/rustls-platform-verifier-android/maven")) }
            }
            filter { includeGroup("rustls") }
        }
        google()
        mavenCentral()
    }
}

rootProject.name = "hidlins-android-spike"
include(":app")
