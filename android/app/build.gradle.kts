// App module: packages the pre-built Rust library into an APK hosted by
// NativeActivity. No CMake/ndk-build — cargo-ndk produces the .so.

plugins {
    id("com.android.application")
}

android {
    namespace = "org.sonora"
    compileSdk = 36

    defaultConfig {
        applicationId = "org.sonora"
        minSdk = 26          // Vulkan 1.0 is mandatory from API 26+
        targetSdk = 36
        versionCode = 1
        versionName = "0.50.3"

        ndk {
            abiFilters += listOf("arm64-v8a")
        }

        manifestPlaceholders["nativeLibraryName"] = "sonora"
    }

    buildTypes {
        release {
            isMinifyEnabled = false
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro")
        }
        debug {
            isDebuggable = true
            isJniDebuggable = true
        }
    }

    // The native library is compiled externally; Gradle must not look for a
    // CMakeLists.txt or Android.mk.
    externalNativeBuild {
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }

    sourceSets {
        getByName("main") {
            jniLibs.srcDirs("src/main/jniLibs")
        }
    }

    packaging {
        jniLibs {
            keepDebugSymbols += listOf("*/arm64-v8a/libsonora.so")
        }
    }

    lint {
        abortOnError = false
        checkReleaseBuilds = false
    }
}

dependencies {
    implementation("androidx.core:core:1.13.1")
    // MediaSessionCompat: the always-on notification and the hardware media keys.
    implementation("androidx.media:media:1.7.1")
    // reqwest 0.13 verifies TLS through rustls-platform-verifier. The version
    // must match `rustls-platform-verifier-android` in Cargo.lock (0.2.0).
    implementation("org.rustls:rustls-platform-verifier:0.2.0")
}
