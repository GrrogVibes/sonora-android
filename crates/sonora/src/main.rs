#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! The desktop binary. Everything platform-specific lives in the library; this
//! is only the entry point Windows, macOS and Linux call.
//!
//! Android loads the library through NativeActivity, so there the binary is a
//! no-op — it exists only so the package still checks and builds as a whole.

#[cfg(not(target_os = "android"))]
fn main() -> std::process::ExitCode {
    sonora::desktop::main()
}

#[cfg(target_os = "android")]
fn main() -> std::process::ExitCode {
    std::process::ExitCode::SUCCESS
}
