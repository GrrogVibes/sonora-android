//! The newest media-button event, kept so a silent failure can be seen on screen.
//!
//! A headset whose play/pause goes nowhere is otherwise impossible to split
//! into "the key never left the headset" and "the key arrived and was dropped";
//! this records the arrival either way.

use std::sync::Mutex;

static LAST: Mutex<Option<String>> = Mutex::new(None);

/// Records what the platform reported.
pub fn record(event: impl Into<String>) {
    if let Ok(mut held) = LAST.lock() {
        *held = Some(event.into());
    }
}

/// The newest media-button event, if any. Diagnostics only.
pub fn last() -> Option<String> {
    LAST.lock().ok().and_then(|held| held.clone())
}
