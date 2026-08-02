plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
}

android {
    namespace = "app.hidlins.spike"
    compileSdk = 36

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "app.hidlins.spike"
        minSdk = 29 // design D-10 floor — same as the product
        targetSdk = 36
        versionCode = 1
        versionName = "0.1-spike"
    }

    buildTypes {
        release {
            // The release leg exists ONLY to prove the ProGuard/R8 keep
            // rules: a minified build must not strip or rename the
            // verifier's Kotlin classes (Rust resolves them by name via
            // JNI). Debug-keystore signing is fine HERE — this harness is
            // scratch tooling, unlike app/android where release signing is
            // deliberately unset (see its build.gradle.kts comment).
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    // The Rust cdylib is copied out of the workspace target dir at build
    // time (never committed — it is a 110 MB debug artifact). Run
    // `make build-android` at the worktree root first.
    sourceSets {
        getByName("main") {
            // Static string path (module-relative): AGP 9 rejects Provider
            // instances here; the copyRustLib -> preBuild dependency below
            // still orders the population correctly.
            jniLibs.srcDirs("build/rustJniLibs")
        }
    }
}

kotlin {
    compilerOptions {
        jvmTarget = org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17
    }
}

val copyRustLib by tasks.registering(Copy::class) {
    val workspaceTarget = rootDir.resolve("../../target")
    from(workspaceTarget.resolve("aarch64-linux-android/debug/libhidlins_api.so"))
    into(layout.buildDirectory.dir("rustJniLibs/arm64-v8a"))
}

tasks.named("preBuild") {
    dependsOn(copyRustLib)
}

dependencies {
    // The verifier's Kotlin component, resolved from the vendored in-tree
    // Maven repo (see settings.gradle.kts). Version pinned to the artifact
    // the rustls-platform-verifier-android 0.1.1 crate ships.
    implementation("rustls:rustls-platform-verifier:0.1.1")
}
