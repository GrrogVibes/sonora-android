//! The Android entry point.
//!
//! `android-activity` loads the `.so` and calls [`android_main`] on a dedicated
//! native thread. The platform comes from `gpui_mobile`; the startup that
//! follows is the one [`crate::launch`] shares with the desktop.

use std::sync::Arc;

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

    // A finger drag only becomes a mouse drag inside a control that asked for it
    // (the sliders); everywhere else the pan keeps scrolling.
    gpui::set_touch_drag_hit_test(Box::new(|position| ui::touch_drag_hit(position)));

    // reqwest 0.13 verifies TLS through `rustls-platform-verifier`, which on
    // Android must be given the JVM before any request runs; without it the
    // first HTTPS call panics with "Expect rustls-platform-verifier to be
    // initialized" and every cover silently fails.
    init_platform_verifier();

    // reqwest is built with `rustls-no-provider`, so rustls cannot pick a crypto
    // provider by itself. Without this every HTTPS request made through
    // `http::Client` — every cover and remote image — fails silently, which is
    // exactly how a phone ended up with no artwork at all.
    rustls::crypto::ring::default_provider()
        .install_default()
        .ok();

    let io = match state::Io::new() {
        Ok(io) => io,
        Err(error) => {
            log::error!("sonora: cannot start runtime: {error:#}");
            return;
        }
    };

    // Android has neither an asset source nor an HTTP client unless we give it
    // one: without them every icon, cover and remote image resolves to nothing.
    let app = Application::with_platform(shared.into_rc())
        .with_assets(crate::assets::Assets)
        .with_http_client(Arc::new(crate::http::Client::new(io.handle())));

    launch::start(
        app,
        Launch {
            io,
            args: Vec::new(),
        },
    );
}

/// Gives `rustls-platform-verifier` the JVM handles it needs on Android.
///
/// Must run once, before any networking. The Kotlin half of the verifier ships
/// in the `rustls-platform-verifier` Gradle artifact.
fn init_platform_verifier() {
    let context = ndk_context::android_context();
    let (vm, activity) = (context.vm(), context.context());
    if vm.is_null() || activity.is_null() {
        log::warn!("verifier: no Java VM or context to initialise with");
        return;
    }
    // SAFETY: `ndk_context` owns both pointers for the life of the process.
    // The `JavaVM` wrapper is forgotten afterwards so its `Drop` cannot destroy
    // the real VM.
    let vm = unsafe { jni::JavaVM::from_raw(vm as *mut jni::sys::JavaVM) };
    let result = vm.attach_current_thread(|env: &mut jni::Env| -> jni::errors::Result<()> {
        // SAFETY: the context is a global reference to the app's own context.
        let context = unsafe { jni::objects::JObject::from_raw(env, activity as jni::sys::jobject) };
        rustls_platform_verifier::android::init_with_env(env, context)
    });
    std::mem::forget(vm);
    if let Err(error) = result {
        log::warn!("verifier: cannot initialise rustls-platform-verifier: {error}");
    }
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
