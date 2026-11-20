//! Android media controls.
//!
//! A foreground service (`SonoraMediaService`) owns a `MediaSessionCompat` and
//! posts the always-on notification; this side pushes track metadata at it and
//! receives the media buttons back as JNI calls. The same session is what makes
//! the hardware volume and media keys do something.

use std::ffi::c_void;
use std::sync::OnceLock;
use std::time::Duration;

use anyhow::Result;
use gpui::App;
use jni::{JValue, jni_sig, jni_str};
use music::Track;
use tokio::sync::mpsc;

use super::Command;
use crate::{PlaybackState, Repeat};

const SERVICE: &str = "org.sonora.SonoraMediaService";
const ACTION_START: &str = "org.sonora.media.START";
const ACTION_STOP: &str = "org.sonora.media.STOP";

/// The codes `SonoraMediaService` sends back; keep in step with the Java constants.
const MEDIA_PLAY: i32 = 1;
const MEDIA_PAUSE: i32 = 2;
const MEDIA_TOGGLE: i32 = 3;
const MEDIA_NEXT: i32 = 4;
const MEDIA_PREVIOUS: i32 = 5;

static VM: OnceLock<jni::JavaVM> = OnceLock::new();
static COMMANDS: OnceLock<mpsc::UnboundedSender<Command>> = OnceLock::new();

fn vm() -> Result<&'static jni::JavaVM> {
    if let Some(vm) = VM.get() {
        return Ok(vm);
    }
    let context = ndk_context::android_context();
    let pointer = context.vm();
    if pointer.is_null() {
        anyhow::bail!("the Java VM is not available");
    }
    Ok(VM.get_or_init(|| {
        // SAFETY: `ndk_context` owns the pointer for the process lifetime.
        unsafe { jni::JavaVM::from_raw(pointer as *mut jni::sys::JavaVM) }
    }))
}

fn with_env<T>(f: impl FnOnce(&mut jni::Env) -> Result<T>) -> Result<T> {
    let vm = vm()?;
    let mut done: Option<Result<T>> = None;
    vm.attach_current_thread(|env: &mut jni::Env| -> jni::errors::Result<()> {
        done = Some(f(env));
        Ok(())
    })?;
    done.expect("the closure ran")
}

/// The app context, which is what `startForegroundService` wants.
fn context<'local>(env: &jni::Env<'local>) -> Result<jni::objects::JObject<'local>> {
    let pointer = ndk_context::android_context().context();
    if pointer.is_null() {
        anyhow::bail!("the app context is not available");
    }
    // SAFETY: a global reference owned by `ndk_context`.
    Ok(unsafe { jni::objects::JObject::from_raw(env, pointer as jni::sys::jobject) })
}

fn class<'local>(env: &mut jni::Env<'local>) -> Result<jni::objects::JClass<'local>> {
    let context = context(env)?;
    let loader = env
        .call_method(&context, jni_str!("getClassLoader"), jni_sig!("()Ljava/lang/ClassLoader;"), &[])
        .and_then(|value| value.l())
        .map_err(|error| {
            env.exception_clear();
            error
        })?;
    let name = env.new_string(SERVICE)?;
    let loaded = env
        .call_method(
            &loader,
            jni_str!("loadClass"),
            jni_sig!("(Ljava/lang/String;)Ljava/lang/Class;"),
            &[JValue::Object(&name)],
        )
        .and_then(|value| value.l())
        .map_err(|error| {
            env.exception_clear();
            error
        })?;
    // SAFETY: `loadClass` returned a `java.lang.Class`.
    Ok(unsafe { jni::objects::JClass::from_raw(env, loaded.as_raw()) })
}

/// Delivers `action` to the service with the metadata extras.
fn start(action: &str, playing: bool, meta: &Metadata) {
    let _ = with_env(|env| {
        let context = context(env)?;
        let class = class(env)?;
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Landroid/content/Context;Ljava/lang/Class;)V"),
            &[JValue::Object(&context), JValue::Object(&class)],
        )?;
        put_string(env, &intent, "action", action)?;
        put_string(env, &intent, "title", &meta.title)?;
        put_string(env, &intent, "artist", &meta.artist)?;
        put_string(env, &intent, "album", &meta.album)?;
        put_string(env, &intent, "cover", &meta.cover)?;
        put_bool(env, &intent, "playing", playing)?;
        put_long(env, &intent, "position", meta.position as i64)?;
        put_long(env, &intent, "duration", meta.duration as i64)?;
        env.call_method(
            &context,
            jni_str!("startForegroundService"),
            jni_sig!("(Landroid/content/Intent;)Landroid/content/ComponentName;"),
            &[JValue::Object(&intent)],
        )
        .map_err(|error| {
            env.exception_describe();
            env.exception_clear();
            error
        })?;
        Ok(())
    });
}

fn stop() {
    let _ = with_env(|env| {
        let context = context(env)?;
        let class = class(env)?;
        let intent = env.new_object(
            jni_str!("android/content/Intent"),
            jni_sig!("(Landroid/content/Context;Ljava/lang/Class;)V"),
            &[JValue::Object(&context), JValue::Object(&class)],
        )?;
        put_string(env, &intent, "action", ACTION_STOP)?;
        env.call_method(
            &context,
            jni_str!("startService"),
            jni_sig!("(Landroid/content/Intent;)Landroid/content/ComponentName;"),
            &[JValue::Object(&intent)],
        )
        .map_err(|error| {
            env.exception_describe();
            env.exception_clear();
            error
        })?;
        Ok(())
    });
}

fn put_string(env: &mut jni::Env<'_>, intent: &jni::objects::JObject<'_>, key: &str, value: &str) -> Result<()> {
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

fn put_bool(env: &mut jni::Env<'_>, intent: &jni::objects::JObject<'_>, key: &str, value: bool) -> Result<()> {
    let key = env.new_string(key)?;
    env.call_method(
        intent,
        jni_str!("putExtra"),
        jni_sig!("(Ljava/lang/String;Z)Landroid/content/Intent;"),
        &[JValue::Object(&key), JValue::Bool(value)],
    )?;
    Ok(())
}

fn put_long(env: &mut jni::Env<'_>, intent: &jni::objects::JObject<'_>, key: &str, value: i64) -> Result<()> {
    let key = env.new_string(key)?;
    env.call_method(
        intent,
        jni_str!("putExtra"),
        jni_sig!("(Ljava/lang/String;J)Landroid/content/Intent;"),
        &[JValue::Object(&key), JValue::Long(value)],
    )?;
    Ok(())
}

/// The track metadata the service needs, in the shape the JNI extras take.
struct Metadata {
    title: String,
    artist: String,
    album: String,
    cover: String,
    position: u64,
    duration: u64,
}

pub struct Controls {
    commands: mpsc::UnboundedSender<Command>,
    title: String,
    artist: String,
    album: String,
    cover: String,
    duration: u64,
    position: u64,
    playing: bool,
    active: bool,
    published: bool,
}

impl Controls {
    pub fn new(
        _hwnd: Option<*mut c_void>,
        commands: mpsc::UnboundedSender<Command>,
        _cx: &mut App,
    ) -> Result<Self> {
        let _ = COMMANDS.set(commands.clone());
        Ok(Self {
            commands,
            title: String::new(),
            artist: String::new(),
            album: String::new(),
            cover: String::new(),
            duration: 0,
            position: 0,
            playing: false,
            active: false,
            published: false,
        })
    }

    pub fn describe(&mut self, track: Option<&Track>, cover: Option<&str>, _cx: &App) {
        match track {
            Some(track) => {
                self.title = track.name.clone();
                self.artist = track.artists.clone();
                self.album = track.album.clone();
                self.duration = track.duration.as_millis() as u64;
            }
            None => {
                self.title.clear();
                self.artist.clear();
                self.album.clear();
                self.duration = 0;
            }
        }
        self.cover = cover.unwrap_or_default().to_string();
        self.publish();
    }

    pub fn set_playback(&mut self, state: &PlaybackState, at: Duration) {
        self.playing = matches!(state, PlaybackState::Playing | PlaybackState::Loading);
        self.active = !matches!(state, PlaybackState::Idle); 
        self.position = at.as_millis() as u64;
        self.publish();
    }

    pub fn seeked(&mut self, at: Duration) {
        self.position = at.as_millis() as u64;
    }

    /// Volume is the system's on Android; the UI keeps its own slider.
    pub fn set_volume(&mut self, _level: f64) {}

    pub fn set_shuffle(&mut self, _on: bool) {}

    pub fn set_repeat(&mut self, _repeat: Repeat) {}

    fn metadata(&self) -> Metadata {
        Metadata {
            title: self.title.clone(),
            artist: self.artist.clone(),
            album: self.album.clone(),
            cover: self.cover.clone(),
            position: self.position,
            duration: self.duration,
        }
    }

    fn publish(&mut self) {
        let meta = self.metadata();
        if self.playing {
            start(ACTION_START, true, &meta);
            self.published = true;
        } else if self.active && self.published {
            start(ACTION_START, false, &meta);
        } else if !self.active && self.published {
            stop();
            self.published = false;
        }
    }
}

/// A media button the service reports. Runs on the Android main thread.
///
/// # Safety
/// Called by JNI with a valid call frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_org_sonora_SonoraMediaService_nativeMediaAction(
    _env: *mut c_void,
    _class: *mut c_void,
    action: i32,
) {
    let Some(commands) = COMMANDS.get() else {
        return;
    };
    let command = match action {
        MEDIA_PLAY => Command::Play,
        MEDIA_PAUSE => Command::Pause,
        MEDIA_TOGGLE => Command::Toggle,
        MEDIA_NEXT => Command::Next,
        MEDIA_PREVIOUS => Command::Previous,
        _ => return,
    };
    commands.send(command).ok();
}

/// A seek the service reports, in milliseconds.
///
/// # Safety
/// Called by JNI with a valid call frame.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn Java_org_sonora_SonoraMediaService_nativeMediaSeek(
    _env: *mut c_void,
    _class: *mut c_void,
    position: i64,
) {
    let Some(commands) = COMMANDS.get() else {
        return;
    };
    commands
        .send(Command::Seek(Duration::from_millis(position.max(0) as u64)))
        .ok();
}
