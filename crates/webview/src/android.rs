//! The Android backend: a throwaway `WebViewActivity`.
//!
//! The activity owns a `WebView` over a data store that dies with it, and Rust
//! polls the static state it publishes. `CookieManager` answers with a header
//! rather than individual cookies, so every cookie read here is stamped with the
//! target's domain — the caller asked for exactly that url.

use std::sync::OnceLock;

use anyhow::{Context as _, Result, bail};
use jni::{JValue, jni_sig, jni_str};

use crate::{Cookie, Target};

const ACTIVITY: &str = "org.sonora.WebViewActivity";

/// `Intent.FLAG_ACTIVITY_NEW_TASK`. `ndk_context` hands us the *Application*,
/// not an Activity, and Android refuses to start an activity from an
/// application context without this flag.
const FLAG_ACTIVITY_NEW_TASK: i32 = 0x1000_0000;

static VM: OnceLock<jni::JavaVM> = OnceLock::new();

/// The process `JavaVM`, taken from the context android-activity published.
fn vm() -> Result<&'static jni::JavaVM> {
    if let Some(vm) = VM.get() {
        return Ok(vm);
    }
    let context = ndk_context::android_context();
    let pointer = context.vm();
    if pointer.is_null() {
        bail!("the Java VM is not available");
    }
    Ok(VM.get_or_init(|| {
        // SAFETY: the pointer comes from `ndk_context`, which android-activity
        // set before `android_main` ran, and it lives for the whole process.
        unsafe { jni::JavaVM::from_raw(pointer as *mut jni::sys::JavaVM) }
    }))
}

/// Runs `f` with a `jni::Env` for the current thread.
fn with_env<T>(f: impl FnOnce(&mut jni::Env) -> Result<T>) -> Result<T> {
    let vm = vm()?;
    let mut done: Option<Result<T>> = None;
    vm.attach_current_thread(|env: &mut jni::Env| -> jni::errors::Result<()> {
        done = Some(f(env));
        Ok(())
    })?;
    done.expect("the closure ran")
}

/// The activity as a JNI object.
fn activity<'local>(env: &jni::Env<'local>) -> Result<jni::objects::JObject<'local>> {
    let pointer = ndk_context::android_context().context();
    if pointer.is_null() {
        bail!("the activity is not available");
    }
    // SAFETY: `ndk_context` holds a global reference to the activity for the
    // process; `from_raw` only borrows the local-reference frame.
    Ok(unsafe { jni::objects::JObject::from_raw(env, pointer as jni::sys::jobject) })
}

/// The app's own class, through the activity's class loader: `FindClass` from a
/// native thread only sees the framework.
fn class<'local>(env: &mut jni::Env<'local>) -> Result<jni::objects::JClass<'local>> {
    let activity = activity(env)?;
    let loader = env
        .call_method(
            &activity,
            jni_str!("getClassLoader"),
            jni_sig!("()Ljava/lang/ClassLoader;"),
            &[],
        )
        .and_then(|value| value.l())
        .map_err(|error| {
            env.exception_describe();
            env.exception_clear();
            error
        })?;
    let name = env.new_string(ACTIVITY)?;
    let loaded = env
        .call_method(
            &loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&name)],
        )
        .and_then(|value| value.l())
        .map_err(|error| {
            env.exception_describe();
            env.exception_clear();
            error
        })?;
    // SAFETY: `loadClass` returned a `java.lang.Class` object.
    Ok(unsafe { jni::objects::JClass::from_raw(env, loaded.as_raw()) })
}

/// One of the activity's `String` accessors.
fn static_string(env: &mut jni::Env<'_>, name: &'static str) -> Result<String> {
    let class = class(env)?;
    let name = jni::strings::JNIString::from(name);
    let value = env
        .call_static_method(&class, &name, jni_sig!("()Ljava/lang/String;"), &[])
        .map_err(|error| {
            env.exception_clear();
            error
        })?;
    let object = value.l()?;
    // SAFETY: the activity returns `java.lang.String`.
    Ok(unsafe { jni::objects::JString::from_raw(env, object.as_raw()) }.to_string())
}

/// One of the activity's `boolean` accessors.
fn static_bool(env: &mut jni::Env<'_>, name: &'static str) -> Result<bool> {
    let class = class(env)?;
    let name = jni::strings::JNIString::from(name);
    Ok(env
        .call_static_method(&class, &name, jni_sig!("()Z"), &[])?
        .z()?)
}

/// `webViewLoad(String)`
fn static_load(env: &mut jni::Env<'_>, url: &str) -> Result<()> {
    let class = class(env)?;
    let url = env.new_string(url)?;
    env.call_static_method(
        &class,
        jni_str!("webViewLoad"),
        jni_sig!("(Ljava/lang/String;)V"),
        &[JValue::Object(&url)],
    )?
    .v()?;
    Ok(())
}

/// `webViewRun(String)`
fn static_run(env: &mut jni::Env<'_>, script: &str) -> Result<()> {
    let class = class(env)?;
    let script = env.new_string(script)?;
    env.call_static_method(
        &class,
        jni_str!("webViewRun"),
        jni_sig!("(Ljava/lang/String;)V"),
        &[JValue::Object(&script)],
    )?
    .v()?;
    Ok(())
}

/// `webViewClose()`
fn static_close(env: &mut jni::Env<'_>) -> Result<()> {
    let class = class(env)?;
    env.call_static_method(&class, jni_str!("webViewClose"), jni_sig!("()V"), &[])?
        .v()?;
    Ok(())
}

pub(crate) struct Window {
    domain: String,
}

impl Window {
    pub(crate) fn open(target: &Target) -> Result<Self> {
        with_env(|env| {
            let activity = activity(env)?;
            let class = class(env)?;
            let intent = env.new_object(
                jni_str!("android/content/Intent"),
                jni_sig!("(Landroid/content/Context;Ljava/lang/Class;)V"),
                &[JValue::Object(&activity), JValue::Object(&class)],
            )?;
            put(env, &intent, "org.sonora.URL", &target.url)?;
            put(env, &intent, "org.sonora.DOMAIN", &target.domain)?;
            put(env, &intent, "org.sonora.TITLE", &target.title)?;
            if let Some(agent) = &target.agent {
                put(env, &intent, "org.sonora.AGENT", agent)?;
            }
            if let Some(script) = &target.script {
                put(env, &intent, "org.sonora.SCRIPT", script)?;
            }
            env.call_method(
                &intent,
                jni_str!("addFlags"),
                jni_sig!("(I)Landroid/content/Intent;"),
                &[JValue::Int(FLAG_ACTIVITY_NEW_TASK)],
            )
            .map_err(|error| {
                env.exception_describe();
                env.exception_clear();
                error
            })?;
            env.call_method(
                &activity,
                jni_str!("startActivity"),
                jni_sig!("(Landroid/content/Intent;)V"),
                &[JValue::Object(&intent)],
            )
            .map_err(|error| {
                // Print the Java stack trace before clearing: the jni error
                // string alone is only "java exception was thrown".
                env.exception_describe();
                env.exception_clear();
                error
            })?;
            Ok(())
        })
        .context("cannot start the sign-in window")?;
        Ok(Self {
            domain: target.domain.clone(),
        })
    }

    pub(crate) fn closed(&self) -> bool {
        with_env(|env| static_bool(env, "webViewClosed")).unwrap_or(true)
    }

    pub(crate) fn host(&self) -> Option<String> {
        let host = with_env(|env| static_string(env, "webViewHost")).unwrap_or_default();
        (!host.is_empty()).then_some(host)
    }

    pub(crate) fn fetch(&mut self) -> Option<Vec<Cookie>> {
        let header = with_env(|env| static_string(env, "webViewCookies")).unwrap_or_default();
        (!header.is_empty()).then(|| {
            header
                .split(';')
                .filter_map(|pair| pair.trim().split_once('='))
                .map(|(name, value)| Cookie {
                    name: name.to_string(),
                    value: value.to_string(),
                    domain: self.domain.clone(),
                })
                .collect()
        })
    }

    pub(crate) fn load(&self, url: &str) {
        let _ = with_env(|env| static_load(env, url));
    }

    pub(crate) fn run(&self, script: &str) {
        let _ = with_env(|env| static_run(env, script));
    }

    pub(crate) fn close(&self) {
        let _ = with_env(|env| static_close(env));
    }
}

/// Adds a string extra to an intent.
fn put(
    env: &mut jni::Env<'_>,
    intent: &jni::objects::JObject<'_>,
    key: &str,
    value: &str,
) -> Result<()> {
    let key = env.new_string(key)?;
    let value = env.new_string(value)?;
    env.call_method(
        intent,
        jni_str!("putExtra"),
        jni_sig!("(Ljava/lang/String;Ljava/lang/String;)Landroid/content/Intent;"),
        &[JValue::Object(&key), JValue::Object(&value)],
    )?;
    Ok(())
}

pub(crate) fn supported() -> bool {
    true
}
