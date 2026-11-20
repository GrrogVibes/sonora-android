//! The Android entry point.
//!
//! `android-activity` loads the `.so` and calls [`android_main`] on a dedicated
//! native thread. The platform comes from `gpui_mobile`; the startup that
//! follows is the one [`crate::launch`] shares with the desktop.

use gpui::Application;

use crate::launch::{self, Launch};

#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    // Logcat first, so everything after this is visible.
    android_logger::init_once(
        android_logger::Config::default()
            .with_max_level(log::LevelFilter::Info)
            .with_tag("sonora"),
    );
    gpui_mobile::android::jni::install_panic_hook();

    let _platform = gpui_mobile::android::jni::init_platform(&app);
    let shared = match gpui_mobile::android::jni::shared_platform() {
        Some(shared) => shared,
        None => {
            log::error!("android_main: shared_platform() returned None — aborting");
            return;
        }
    };

    publish_android_dirs();

    let io = match state::Io::new() {
        Ok(io) => io,
        Err(error) => {
            log::error!("sonora: cannot start runtime: {error:#}");
            return;
        }
    };

    launch::start(
        Application::with_platform(shared.into_rc()),
        Launch {
            io,
            args: Vec::new(),
        },
    );
}

/// Android has no `$HOME` and no XDG variables, so `dirs` resolves every path to
/// `.` — the read-only filesystem root. Point home at the app's private storage
/// before anything derives a path from it.
fn publish_android_dirs() {
    let Some(app) = gpui_mobile::android::jni::android_app() else {
        log::warn!("android: no AndroidApp handle; leaving HOME unset");
        return;
    };
    let Some(files) = app.internal_data_path() else {
        log::warn!("android: no internal data path; leaving HOME unset");
        return;
    };
    let _ = std::fs::create_dir_all(files.join("home"));
    // `set_var` is unsafe in edition 2024. This runs once on the native thread
    // before any other thread can read the environment.
    unsafe {
        std::env::set_var("HOME", files.join("home"));
        std::env::set_var("XDG_CONFIG_HOME", files.join("config"));
        std::env::set_var("XDG_DATA_HOME", files.join("data"));
        std::env::set_var("XDG_CACHE_HOME", files.join("cache"));
    }
    log::info!("android: HOME → {}", files.display());
}
