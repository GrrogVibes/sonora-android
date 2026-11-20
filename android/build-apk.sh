#!/usr/bin/env bash
# Build the installable arm64 APK.
#
#   . /root/buildenv.sh
#   android/build-apk.sh
#
# The Rust side is built with cargo-ndk into jniLibs, then Gradle packages it.
set -euo pipefail
cd "$(dirname "$0")/.."

export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-4}"

echo "== rust =="
cargo ndk -t arm64-v8a --platform 26 -o android/app/src/main/jniLibs \
    build --release -p sonora --lib

# cargo ndk copies every cdylib in the graph. gpui-mobile is statically linked
# into libsonora.so, so its own .so must not ship.
rm -f android/app/src/main/jniLibs/arm64-v8a/libgpui_mobile.so

# The Widevine CDM shim is C++, so libsonora.so has a DT_NEEDED on
# libc++_shared.so. Gradle adds it automatically for CMake builds; with
# cargo-ndk we have to copy it ourselves or the app dies at dlopen.
NDK_SYSROOT="$ANDROID_NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/sysroot"
cp "$NDK_SYSROOT/usr/lib/aarch64-linux-android/libc++_shared.so" \
    android/app/src/main/jniLibs/arm64-v8a/

echo "== gradle =="
gradle -p android --no-daemon assembleRelease

echo "== sign =="
KEYSTORE="${SONORA_KEYSTORE:-/root/.android/debug.keystore}"
if [ ! -f "$KEYSTORE" ]; then
    mkdir -p "$(dirname "$KEYSTORE")"
    keytool -genkeypair -keystore "$KEYSTORE" -storepass android \
        -alias androiddebugkey -keypass android -keyalg RSA -keysize 2048 \
        -validity 10000 -dname "CN=Android Debug,O=Android,C=US"
fi
"$ANDROID_HOME/build-tools/36.0.0/apksigner" sign \
    --ks "$KEYSTORE" --ks-pass pass:android --key-pass pass:android \
    --out android/app-release.apk \
    android/app/build/outputs/apk/release/app-release-unsigned.apk

ls -la android/app-release.apk
