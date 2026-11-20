pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // The Kotlin half of rustls-platform-verifier, which reqwest 0.13 uses
        // to verify TLS on Android. Served from the project's maven-archive
        // branch; the artifact version matches rustls-platform-verifier-android
        // in Cargo.lock.
        maven {
            url = uri("https://raw.githubusercontent.com/rustls/rustls-platform-verifier/maven-archive/android-release-support/maven/")
        }
    }
}

rootProject.name = "Sonora"
include(":app")
