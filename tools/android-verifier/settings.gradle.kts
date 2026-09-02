pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositories {
        exclusiveContent {
            forRepository {
                maven {
                    url = uri(rootDir.resolve("../../vendor/rustls-platform-verifier-android/maven"))
                }
            }
            filter { includeGroup("rustls") }
        }
        google()
        mavenCentral()
    }
}

rootProject.name = "hidlins-android-verifier"
include(":app")
