//! Which touches should become drags.
//!
//! A finger drag is not a mouse drag, so GPUI only drives one as a mouse drag
//! when the touch starts inside a control that asked for it. The controls
//! register their area here and GPUI asks [`touch_drag_hit`] before synthesizing
//! anything, which is what keeps a plain pan scrolling.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use gpui::{Bounds, Pixels, Point};

static AREAS: Mutex<Vec<(u64, Bounds<Pixels>)>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Whether a touch starting here should be driven as a drag.
pub fn touch_drag_hit(position: Point<Pixels>) -> bool {
    let Ok(areas) = AREAS.lock() else {
        return false;
    };
    areas.iter().any(|(_, bounds)| bounds.contains(&position))
}

/// A control's draggable area, forgotten when the control goes away.
pub(crate) struct TouchArea(u64);

impl TouchArea {
    pub(crate) fn new() -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut areas) = AREAS.lock() {
            areas.push((id, Bounds::default()));
        }
        Self(id)
    }

    pub(crate) fn set(&self, bounds: Bounds<Pixels>) {
        if let Ok(mut areas) = AREAS.lock()
            && let Some(area) = areas.iter_mut().find(|(held, _)| *held == self.0)
        {
            area.1 = bounds;
        }
    }
}

impl Drop for TouchArea {
    fn drop(&mut self) {
        if let Ok(mut areas) = AREAS.lock() {
            areas.retain(|(held, _)| *held != self.0);
        }
    }
}
