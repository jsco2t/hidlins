allprojects {
    repositories {
        exclusiveContent {
            forRepository {
                maven {
                    url = uri(rootProject.file("../../vendor/rustls-platform-verifier-android/maven"))
                }
            }
            filter { includeGroup("rustls") }
        }
        google()
        mavenCentral()
    }
    dependencyLocking {
        lockAllConfigurations()
    }
}

val newBuildDir: Directory =
    rootProject.layout.buildDirectory
        .dir("../../build")
        .get()
rootProject.layout.buildDirectory.value(newBuildDir)

subprojects {
    val newSubprojectBuildDir: Directory = newBuildDir.dir(project.name)
    project.layout.buildDirectory.value(newSubprojectBuildDir)
}
subprojects {
    project.evaluationDependsOn(":app")
}

tasks.register<Delete>("clean") {
    delete(rootProject.layout.buildDirectory)
}
