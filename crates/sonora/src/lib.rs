//! Sonora — a native, familiar music streaming client.
//!
//! One crate, two entry points. [`desktop`] builds its window through
//! `gpui_platform`; [`android`] builds it through `gpui_mobile`. Both then call
//! the same [`launch::start`], which is the only startup body.
//!
//! Desktop-only modules are gated out on Android so the mobile build never
//! links a desktop dependency.

pub mod actions;
pub mod assets;
pub mod dock;
pub mod http;
pub mod launch;
pub mod memory;
pub mod tray;

#[cfg(not(target_os = "android"))]
pub mod console;
#[cfg(not(target_os = "android"))]
pub mod desktop;
#[cfg(not(target_os = "android"))]
pub mod logging;
#[cfg(not(target_os = "android"))]
pub mod single;

#[cfg(target_os = "android")]
pub mod android;

#[cfg(windows)]
pub mod app_icon;
#[cfg(target_os = "macos")]
pub mod gestures;
#[cfg(windows)]
pub mod thumbbar;
#[cfg(windows)]
pub mod window_title;
#[cfg(target_os = "linux")]
pub mod x11_icon;
