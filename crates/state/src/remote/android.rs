//! Android media controls.
//!
//! This mirrors `mpris::Controls` so [`super::Remote`] is written once. The
//! implementation arrives with the platform-services phase: a
//! `MediaSessionCompat` held by a foreground service, which is also what makes
//! the hardware volume and media keys work. Until then every setter is a no-op.

use std::ffi::c_void;
use std::time::Duration;

use anyhow::Result;
use gpui::App;
use music::Track;
use tokio::sync::mpsc;

use super::Command;
use crate::{PlaybackState, Repeat};

pub struct Controls;

impl Controls {
    pub fn new(
        _hwnd: Option<*mut c_void>,
        _commands: mpsc::UnboundedSender<Command>,
        _cx: &mut App,
    ) -> Result<Self> {
        Ok(Self)
    }

    pub fn describe(&mut self, _track: Option<&Track>, _cover: Option<&str>, _cx: &App) {}

    pub fn set_playback(&mut self, _state: &PlaybackState, _at: Duration) {}

    pub fn seeked(&mut self, _at: Duration) {}

    pub fn set_volume(&mut self, _level: f64) {}

    pub fn set_shuffle(&mut self, _on: bool) {}

    pub fn set_repeat(&mut self, _repeat: Repeat) {}
}
