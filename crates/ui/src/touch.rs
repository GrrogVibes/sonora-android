//! Which touches should become drags.
//!
//! A finger drag is not a mouse drag, so GPUI only drives one as a mouse drag
//! when the touch starts inside a control that asked for it. A control mounts a
//! [`TouchDrag`] element, which publishes its area here; GPUI asks
//! [`touch_drag_hit`] before synthesizing anything, so a plain pan keeps
//! scrolling. Publishing from the element's own layout means the area follows
//! the control when the page scrolls.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

use gpui::{
    App, Bounds, DispatchPhase, Element, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Point, Refineable as _, Style, StyleRefinement, Styled, TouchDragEvent, Window,
};

static AREAS: Mutex<Vec<(u64, Bounds<Pixels>)>> = Mutex::new(Vec::new());
static NEXT: AtomicU64 = AtomicU64::new(0);

/// Whether a touch starting here should be driven as a drag.
pub fn touch_drag_hit(position: Point<Pixels>) -> bool {
    let Ok(areas) = AREAS.lock() else {
        return false;
    };
    areas.iter().any(|(_, bounds)| bounds.contains(&position))
}

/// Publishes a control's draggable area; GPUI asks it before turning a touch
/// into a mouse drag.
pub struct TouchDrag {
    id: u64,
    bounds: Rc<Cell<Bounds<Pixels>>>,
    handler: Option<Rc<RefCell<dyn FnMut(&TouchDragEvent, &mut Window, &mut App)>>>,
    style: StyleRefinement,
}

impl TouchDrag {
    /// An area that reports its bounds into a cell the control already keeps.
    pub fn area_in(bounds: Rc<Cell<Bounds<Pixels>>>) -> Self {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut areas) = AREAS.lock() {
            areas.push((id, Bounds::default()));
        }
        Self {
            id,
            bounds,
            handler: None,
            style: StyleRefinement::default(),
        }
    }

    /// An area that only reports its bounds.
    pub fn area() -> (Self, Rc<Cell<Bounds<Pixels>>>) {
        let bounds = Rc::new(Cell::new(Bounds::default()));
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        if let Ok(mut areas) = AREAS.lock() {
            areas.push((id, Bounds::default()));
        }
        let element = Self {
            id,
            bounds: Rc::clone(&bounds),
            handler: None,
            style: StyleRefinement::default(),
        };
        (element, bounds)
    }

    /// An area that also hears the drags, for a control that drives itself.
    pub fn new<H>(bounds: Rc<Cell<Bounds<Pixels>>>, handler: H) -> Self
    where
        H: FnMut(&TouchDragEvent, &mut Window, &mut App) + 'static,
    {
        let id = NEXT.fetch_add(1, Ordering::Relaxed);
        Self {
            id,
            bounds,
            handler: Some(Rc::new(RefCell::new(handler))),
            style: StyleRefinement::default(),
        }
    }

    fn publish(&self, bounds: Bounds<Pixels>) {
        self.bounds.set(bounds);
        if let Ok(mut areas) = AREAS.lock()
            && let Some(area) = areas.iter_mut().find(|(held, _)| *held == self.id)
        {
            area.1 = bounds;
        }
    }
}

impl Drop for TouchDrag {
    fn drop(&mut self) {
        if let Ok(mut areas) = AREAS.lock() {
            areas.retain(|(held, _)| *held != self.id);
        }
    }
}

impl IntoElement for TouchDrag {
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl Styled for TouchDrag {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl Element for TouchDrag {
    type RequestLayoutState = Style;
    type PrepaintState = ();

    fn id(&self) -> Option<gpui::ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        let id = window.request_layout(style.clone(), [], cx);
        (id, style)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        self.publish(bounds);
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _layout: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        cx: &mut App,
    ) {
        if let Some(handler) = &self.handler {
            let handler = Rc::clone(handler);
            window.on_mouse_event(move |event: &TouchDragEvent, phase, window, cx| {
                if phase == DispatchPhase::Bubble {
                    (handler.borrow_mut())(event, window, cx);
                }
            });
        }
    }
}
