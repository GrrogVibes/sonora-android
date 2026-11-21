//! Touch drags for controls that want them.
//!
//! A finger drag is not a mouse drag, so GPUI leaves it to the pan. A control
//! that wants to be dragged (a slider) mounts a [`TouchDrag`] element: it hears
//! the raw touch, claims the gesture only when the touch starts inside its own
//! area, and drives its owner. Everything else keeps scrolling.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use gpui::{
    App, Bounds, DispatchPhase, Element, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Refineable as _, Style, StyleRefinement, Styled, TouchDragEvent, TouchPhase,
    Window,
};

/// Forwards touch drags to a control, which decides by the area it was given.
pub struct TouchDrag<F> {
    bounds: Rc<Cell<Bounds<Pixels>>>,
    handler: Rc<RefCell<F>>,
    style: StyleRefinement,
}

impl<F> TouchDrag<F> {
    pub fn new(bounds: Rc<Cell<Bounds<Pixels>>>, handler: F) -> Self {
        Self {
            bounds,
            handler: Rc::new(RefCell::new(handler)),
            style: StyleRefinement::default(),
        }
    }
}

impl<F> IntoElement for TouchDrag<F>
where
    F: FnMut(&TouchDragEvent, Bounds<Pixels>, &mut Window, &mut App) + 'static,
{
    type Element = Self;

    fn into_element(self) -> Self {
        self
    }
}

impl<F> Styled for TouchDrag<F> {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl<F> Element for TouchDrag<F>
where
    F: FnMut(&TouchDragEvent, Bounds<Pixels>, &mut Window, &mut App) + 'static,
{
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
        self.bounds.set(bounds);
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
        let bounds = Rc::clone(&self.bounds);
        let handler = Rc::clone(&self.handler);
        window.on_mouse_event(move |event: &TouchDragEvent, phase, window, cx| {
            if phase == DispatchPhase::Bubble {
                (handler.borrow_mut())(event, bounds.get(), window, cx);
            }
        });
    }
}
