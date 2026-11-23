# Sonora on Android

A native Sonora client for Android. The desktop GPUI UI is unchanged — only the GPUI **platform
backend** is swapped for a vendored [`gpui-mobile`](vendor/gpui-mobile), and the app ships as a
`NativeActivity` APK. Nothing about the desktop build is forked: the Android-only code is behind
`#[cfg(target_os = "android")]`.

> **⚠️ This project is 110% vibe-coded via [Hermes Agent](https://hermes-agent.nousresearch.com).**
> **It works on my phone; I have no idea what happens on yours — use it at your own risk.**

- **Upstream:** [sonorahq/sonora](https://github.com/sonorahq/sonora) (v0.50.3, `1cb2c2c`)
- **Vendored backend:** `vendor/gpui-mobile` (from `itsbalamurali/gpui-mobile`, patched)
- **GPUI fork:** `vendor/gpui` (`sonorahq/gpui`)
- **APK:** `SonoraDroid0.50.3.1.apk`, package `org.sonora`, arm64-v8a, minSdk 26

## Download

The APK is attached to the [releases](../../releases) page. Grab `SonoraDroid0.50.3.1.apk` and
install it:

```sh
adb install -r SonoraDroid0.50.3.1.apk
```

It is debug-signed (no release keystore), so you may need to allow installs from unknown sources.
A 32-bit or x86_64 device will not install it — it is arm64-v8a only.

## What is ported

| Area | State |
| --- | --- |
| UI (the whole Sonora interface) | ported, touch-adapted (single tap plays, hold is right-click, per-control drag) |
| YouTube Music sign-in | a real browser window (`WebViewActivity`) |
| Sessions | joined by code, real system clipboard |
| Cover art | loads (TLS verifier initialised, bundled roots) |
| Media notification | foreground service, media buttons, background playback, Bluetooth keys |
| Hardware volume keys | move the system media stream |
| Downloads / offline | downloads kept in the encrypted vault; offline mode and the Downloaded filter list them |

## Build

The environment is recorded in [`TOOLCHAIN.md`](./TOOLCHAIN.md). The short version:

```sh
. /root/buildenv.sh          # the whole environment in one file
android/build-apk.sh         # cargo ndk → jniLibs → Gradle → debug sign
```

The script produces `android/SonoraDroid0.50.3.1.apk`. Two things it does that a naive build
misses: it deletes the copied `libgpui_mobile.so` (it is statically linked into `libsonora.so`),
and it copies `libc++_shared.so` from the NDK sysroot (the Widevine CDM shim is C++ and would
otherwise fail to `dlopen`).

Build gates:

```sh
cargo check --workspace
cargo check -p sonora --target aarch64-linux-android
```

## Layout

| What | Path |
| --- | --- |
| Android entry (Rust) | `crates/sonora/src/android.rs` |
| Android entry (Java) | `android/app/src/main/java/org/sonora/` |
| Gradle shell | `android/` |
| Build script | `android/build-apk.sh` |
| Media service | `android/app/src/main/java/org/sonora/SonoraMediaService.java` |
| Touch-drag element | `crates/ui/src/touch.rs` |
| Media controls (Rust) | `crates/state/src/remote/android.rs` |

## Licences

The Sonora code is GPL-3.0-or-later (`COPYING`); the third-party inventory is
[`THIRD-PARTY.md`](./THIRD-PARTY.md). The vendored backends carry their own licences, kept beside
their sources:

- `vendor/gpui` — Apache-2.0 (`vendor/gpui/LICENSE`)
- `vendor/gpui-mobile` — see `vendor/gpui-mobile/LICENSE-*` (AGPL-3.0 among them)

Combining them may make the resulting binary AGPL-3.0. Before publishing or distributing a build,
take advice on what that means for you. The upstream Sonora project is not affiliated with this
port.

## Known gaps

- arm64 only (a 32-bit or x86_64 device will not install).
- The Artists and Playlists library pages do not yet list what a download of a single track
  implies; the Songs and Albums pages do.
- The full device smoke test is done by hand on a phone; there is no emulator in the build
  environment.
