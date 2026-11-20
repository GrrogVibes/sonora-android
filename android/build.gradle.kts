// Root build for the Sonora Android shell.
//
// The Rust side is compiled externally with cargo-ndk into
// app/src/main/jniLibs/<abi>/libsonora.so; this project only packages it.

plugins {
    id("com.android.application") version "8.9.1" apply false
}
