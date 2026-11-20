# Toolchain — recorded 2026-11-20 (fresh sandbox)

Everything below was installed from scratch on a clean Debian 13 (trixie) image.

| Component | Version | Path |
| --- | --- | --- |
| Rust | 1.98.1 (`48a229cea`) | `/root/.cargo` (rustup, `--no-modify-path`) |
| cargo-ndk | 4.1.2 | `/root/.cargo/bin/cargo-ndk` |
| JDK | Temurin 17.0.21.1+2 | `/opt/jdk/jdk-17.0.21.1+2` |
| Android SDK | platform-tools 37.0.3, `platforms;android-36`, `build-tools;36.0.0` | `/opt/android` |
| Android NDK | **r28c = 28.2.13676358** | `/opt/android/ndk/28.2.13676358` |
| Gradle | 8.11.1 | `/opt/gradle/gradle-8.11.1` |

**NDK note:** the plan's `28.2.13970746` does not exist in Google's repository. `r28b` is
`28.1.13356709`; `r28c` is `28.2.13676358`. We use **r28c**.

## Host build deps (apt)

`mold cmake g++ nasm perl pkg-config libclang-dev llvm-19-dev clang-19 libasound2-dev
libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev libfontconfig-dev libfreetype-dev
libgl1-mesa-dev curl unzip zip`

Debian trixie ships OpenJDK 21, not 17 — the JDK comes from Adoptium instead.

## Environment

```sh
. /root/buildenv.sh
```

Sets `CARGO_TARGET_DIR`, `ANDROID_HOME`, `ANDROID_NDK_HOME`, `JAVA_HOME`, `LIBCLANG_PATH`,
`BINDGEN_EXTRA_CLANG_ARGS`, `GRADLE_USER_ROOT`, `TMPDIR`, and `PATH` (Gradle, JDK, platform-tools,
NDK clang bin, cargo).

## Sandbox quirks

- **`/tmp` is mounted `noexec`** — `cargo install` and anything that executes from `/tmp` fails with
  `Permission denied`. `TMPDIR=/root/tmp` is exported to avoid it.
- `mold` is the host linker (`-fuse-ld=mold` in `.cargo/config.toml`).
- NDK clang symlinks added for the unprefixed names (`aarch64-linux-android-clang` → `…26-clang`,
  same for `-clang++`, and `-ar` → `llvm-ar`).
- `.cargo/config.toml` gained `[target.aarch64-linux-android]` and `[target.x86_64-linux-android]`
  linker entries pointing at the API-26 clang.
- cgroup cap: 16 GiB RAM + 16 GiB swap. A `signal: 9` with no compiler error is the OOM killer, not a
  code fault.
