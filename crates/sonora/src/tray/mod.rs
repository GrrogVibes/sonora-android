//! The tray icon. Android has none — transport lives in the media notification
//! the platform-services phase adds — so that build gets a stub with the same
//! shape instead of the desktop implementations.

#[cfg(not(target_os = "android"))]
mod desktop;
#[cfg(not(target_os = "android"))]
pub use desktop::*;

#[cfg(target_os = "android")]
mod android;
#[cfg(target_os = "android")]
pub use android::*;
