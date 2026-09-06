import org.gradle.api.tasks.compile.JavaCompile

plugins {
    id("com.android.application")
    // AGP 9 supplies built-in Kotlin; applying the separate Kotlin Gradle
    // plugin here would mix both compilation models.
    id("dev.flutter.flutter-gradle-plugin")
}

android {
    namespace = "app.hidlins"
    compileSdk = flutter.compileSdkVersion
    // Exact Flutter 3.47.2 default. The Rust cross-build contract reads the
    // same version; changing either side alone fails `make check-android`.
    ndkVersion = "28.2.13676358"

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
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            // Deliberately NO signingConfig. The Flutter template defaults to
            // signing release builds with the debug keystore, whose private
            // key ships with every Android SDK install — for a secrets manager
            // that would mean any attacker can forge a same-origin update and
            // every signature-derived protection is void. Leaving this unset
            // produces a deliberately unsigned release artifact.
            //
            // Real release signing is tracked as an Android (D3) task; wire it
            // from a gitignored key.properties when that lands.
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
        }
    }

    packaging {
        jniLibs.keepDebugSymbols += "**/libhidlins_api.so"
    }
}

kotlin {
    compilerOptions {
        allWarningsAsErrors.set(true)
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

// This is the first-party application module. Dependency/plugin modules keep
// their own warning policy, while Java compiled as part of Hidlins enables the
// standard lint set and rejects every emitted warning.
tasks.withType<JavaCompile>().configureEach {
    options.compilerArgs.addAll(listOf("-Xlint:all", "-Werror"))
}

val verifyHidlinsNativeArtifacts by tasks.registering(Exec::class) {
    workingDir(rootDir.resolve("../.."))
    commandLine("python3", "tools/android-native/artifacts.py", "check")
}

tasks.named("preBuild") {
    dependsOn(verifyHidlinsNativeArtifacts)
}

dependencies {
    implementation("rustls:rustls-platform-verifier:0.1.1")
}

flutter {
    source = "../.."
}
