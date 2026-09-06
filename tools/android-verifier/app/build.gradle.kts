plugins {
    id("com.android.application")
}

android {
    namespace = "app.hidlins.verifier"
    compileSdk = 36

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    defaultConfig {
        applicationId = "app.hidlins.verifier"
        minSdk = 29
        targetSdk = 36
        versionCode = 1
        versionName = "1"
        ndk {
            abiFilters += listOf("arm64-v8a", "x86_64")
        }
    }

    buildTypes {
        release {
            isMinifyEnabled = true
            isShrinkResources = true
            proguardFiles(
                getDefaultProguardFile("proguard-android-optimize.txt"),
                "proguard-rules.pro",
            )
            // This repository-only verifier is never distributed. Debug-key
            // signing exists solely so the release/R8 artifact can run on the
            // controlled emulator; the product app remains unsigned here.
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    sourceSets {
        getByName("main") {
            jniLibs.directories.add(
                rootDir.resolve("../../app/android/native-artifacts/jniLibs").absolutePath,
            )
        }
    }

    packaging {
        jniLibs.keepDebugSymbols += "**/libhidlins_api.so"
    }
}

dependencyLocking {
    lockAllConfigurations()
}

val verifyNativeArtifacts by tasks.registering(Exec::class) {
    workingDir(rootDir.resolve("../.."))
    commandLine("python3", "tools/android-native/artifacts.py", "check")
}

tasks.named("preBuild") {
    dependsOn(verifyNativeArtifacts)
}

dependencies {
    implementation("rustls:rustls-platform-verifier:0.1.1")
}

tasks.withType<org.gradle.api.tasks.compile.JavaCompile>().configureEach {
    options.compilerArgs.addAll(listOf("-Xlint:all", "-Werror"))
}
