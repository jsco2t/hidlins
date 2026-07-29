plugins {
    id("com.android.application")
    // Required for MainActivity.kt and the kotlin {} block below:
    // gradle.properties sets android.builtInKotlin=false, so AGP compiles no
    // Kotlin on its own, and settings.gradle.kts only *declares* this plugin
    // (apply false) — the app module must apply it.
    id("org.jetbrains.kotlin.android")
    // The Flutter Gradle Plugin must be applied after the Android and Kotlin Gradle plugins.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "app.hidlins"
    compileSdk = flutter.compileSdkVersion
    ndkVersion = flutter.ndkVersion

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        // Design D-10: the identifier scheme is app.hidlins (Android),
        // app.hidlins.ios, app.hidlins.macos. `flutter create --org app.hidlins`
        // with project name "app" produced app.hidlins.app; corrected here.
        applicationId = "app.hidlins"
        // Design D-10 floor: API 29. NOT flutter.minSdkVersion (24) — the
        // alpha's clipboard-sensitivity and storage behavior are specified
        // against 29+, and 24-28 lack the platform mitigations assumed by the
        // mobile hardening tasks.
        minSdk = 29
        targetSdk = flutter.targetSdkVersion
        versionCode = flutter.versionCode
        versionName = flutter.versionName
    }

    buildTypes {
        release {
            // Deliberately NO signingConfig. The Flutter template defaults to
            // signing release builds with the debug keystore, whose private
            // key ships with every Android SDK install — for a secrets manager
            // that would mean any attacker can forge a same-origin update and
            // every signature-derived protection is void. Leaving this unset
            // makes an unsigned release build fail loudly instead.
            //
            // Real release signing is tracked as an Android (D3) task; wire it
            // from a gitignored key.properties when that lands.
            isMinifyEnabled = false
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

flutter {
    source = "../.."
}
