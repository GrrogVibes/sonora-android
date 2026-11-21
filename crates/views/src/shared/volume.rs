use gpui::accesskit::Role;
use gpui::prelude::*;
use gpui::{
    Context, Entity, MouseUpEvent, Pixels, ScrollWheelEvent, Window, deferred, div, px, relative,
};
use i18n::t;
use state::Playback;
use ui::{
    ActiveTheme as _, Button, Fading as _, GLASS_BLUR, GLASS_FILL, Rising as _, Scrubber,
    ScrubberState,
};

use crate::shared::transport::{NOTCH, percent, volume_icon};

/// How tall the slider stands once it has risen.
pub(crate) const VOLUME_RISE: f32 = 132.;
/// How far the level bubble floats from the slider.
pub(crate) const VOLUME_BUBBLE_LIFT: Pixels = px(18.);
/// How far around the risen panel the pointer may stray before it closes.
const ZONE: f32 = 14.;
/// The space between the button and the panel over it.
const GAP: f32 = 2.;

/// What a view keeps for a volume button whose slider rises over it while the pointer is on
/// the button, the slider or the space between them.
pub(crate) struct RisingVolume {
    over_button: bool,
    over_zone: bool,
    over_panel: bool,
    /// Whether the slider is being dragged, which keeps it up wherever the pointer goes.
    pub(crate) held: bool,
    /// A phone has no hover: the button itself carries the drag, and this keeps
    /// its anchor across a re-render. The flag says whether it became a drag, so
    /// a plain tap still opens the panel.
    pub(crate) drag: std::rc::Rc<std::cell::Cell<Option<(Pixels, f32, bool)>>>,
    slider: ScrubberState,
}

/// A phone has no hover, so the panel is pinned by hand; one flag covers the bar
/// and the fullscreen player, and lets the window close it on a tap outside.
static PINNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Whether a volume panel is pinned open.
pub(crate) fn pinned() -> bool {
    PINNED.load(std::sync::atomic::Ordering::Relaxed)
}

/// Pins or unpins the volume panel.
pub(crate) fn set_pinned(value: bool) {
    PINNED.store(value, std::sync::atomic::Ordering::Relaxed);
}

impl RisingVolume {
    pub(crate) fn new(id: &'static str) -> Self {
        Self {
            over_button: false,
            over_zone: false,
            over_panel: false,
            held: false,
            drag: std::rc::Rc::new(std::cell::Cell::new(None)),
            slider: ScrubberState::new(id),
        }
    }

    /// Whether the slider is up.
    pub(crate) fn open(&self) -> bool {
        #[cfg(target_os = "android")]
        {
            pinned() || self.held
        }
        #[cfg(not(target_os = "android"))]
        {
            self.parked() || self.held
        }
    }

    /// Whether the pointer rests on any part of the control.
    pub(crate) fn parked(&self) -> bool {
        self.over_button || self.over_zone || self.over_panel
    }

    /// Forgets where the pointer was, for a view that is shown again and hears hover afresh.
    pub(crate) fn unpark(&mut self) {
        self.over_button = false;
        self.over_zone = false;
        self.over_panel = false;
    }
}

/// A view that draws a [`rising_volume`].
pub(crate) trait VolumeHost: Sized + 'static {
    fn volume(&mut self) -> &mut RisingVolume;
    fn playback(&self) -> &Entity<Playback>;
}

/// Turns the volume by one notch for each step of the wheel, and keeps the wheel from the page.
pub(crate) fn turn_volume<V: VolumeHost>(
    host: &mut V,
    event: &ScrollWheelEvent,
    window: &mut Window,
    cx: &mut Context<V>,
) {
    let delta = event.delta.pixel_delta(window.line_height()).y;
    if delta == Pixels::ZERO {
        return;
    }
    cx.stop_propagation();

    let notch = match delta > Pixels::ZERO {
        true => NOTCH,
        false => -NOTCH,
    };
    let level = (host.playback().read(cx).shown_volume(cx) + notch).clamp(0., 1.);
    host.playback()
        .update(cx, |playback, cx| playback.set_volume(level, cx));
}

/// The mute button with a vertical slider that rises over it on hover, for a place with no room
/// for a slider beside the button. `volume` and `playback` are the host's own. A `frosted` button and
/// panel float over artwork and blur what is under them.
pub(crate) fn rising_volume<V: VolumeHost>(
    volume: &RisingVolume,
    playback: &Entity<Playback>,
    frosted: bool,
    cx: &mut Context<V>,
) -> impl IntoElement + use<V> {
    let theme = *cx.theme();
    let hazy = frosted && ui::blurring(cx);
    let zone = px(ZONE);
    let level = playback.read(cx).shown_volume(cx);
    let muted = playback.read(cx).muted(cx);
    let empty = theme.muted_foreground.opacity(0.3);
    let open = volume.open();
    let span = theme.metrics.control_small + zone * 2.;
    let reach = Scrubber::bubble_reach(VOLUME_BUBBLE_LIFT, cx);
    // A phone has no hover, so the button itself carries the drag: up turns it
    // up, down turns it down. The anchor lives on the host so a re-render mid-drag
    // does not drop it.
    let button_area = std::rc::Rc::new(std::cell::Cell::new(gpui::Bounds::default()));
    let drag = volume.drag.clone();
    let dragged_playback = playback.clone();
    let host = cx.entity_id();
    // The level shows for as long as the pointer is anywhere over the control, not only on
    // the track, and goes with it.
    let bubble = open.then(|| (level, percent(level)));

    div()
        .relative()
        .flex()
        .flex_none()
        .on_scroll_wheel(cx.listener(turn_volume))
        .child(
            div()
                .id("volume-hover")
                .on_hover(cx.listener(|this: &mut V, hovering: &bool, _, cx| {
                    this.volume().over_button = *hovering;
                    cx.notify();
                }))
                .when(cfg!(target_os = "android"), |this| {
                    // Under the button, so the button keeps its own taps; a drag
                    // starting here still turns the volume up going down.
                    this.child(
                        ui::TouchDrag::new(button_area.clone(), move |event: &gpui::TouchDragEvent,
                              window: &mut Window,
                              cx: &mut gpui::App| {
                            let area = button_area.get();
                            match event.phase {
                                gpui::TouchPhase::Started => {
                                    if area.contains(&event.position) {
                                        // Claim it, so the drag is delivered; a tap
                                        // is told apart from a drag by how far it
                                        // travels, and answered here on release.
                                        window.prevent_default();
                                        drag.set(Some((event.position.y, level, false)));
                                    }
                                }
                                gpui::TouchPhase::Moved => {
                                    if let Some((start_y, start_level, moved)) = drag.get() {
                                        let delta = event.position.y - start_y;
                                        if !moved && delta.abs() < px(6.) {
                                            return;
                                        }
                                        // Up turns it up, down turns it down.
                                        let next = (start_level - delta / px(120.)).clamp(0., 1.);
                                        dragged_playback.update(
                                            cx,
                                            |playback, cx| playback.slide_volume(next, cx),
                                        );
                                        cx.notify(dragged_playback.entity_id());
                                        drag.set(Some((start_y, start_level, true)));
                                    }
                                }
                                gpui::TouchPhase::Ended
                                | gpui::TouchPhase::Cancelled => {
                                    if let Some((_, _, moved)) = drag.get() {
                                        if moved {
                                            dragged_playback.update(
                                                cx,
                                                |playback, cx| playback.end_volume_slide(cx),
                                            );
                                        } else {
                                            set_pinned(!pinned());
                                            cx.notify(host);
                                        }
                                    }
                                    drag.set(None);
                                }
                            }
                        })
                        .absolute()
                        .size_full(),
                    )
                })
                .child(
                    Button::new("volume")
                        .ghost()
                        .when(frosted, Button::frosted)
                        .small()
                        .icon(volume_icon(level))
                        // No tooltip: it would sit over the slider rising from the same spot,
                        // so the button carries its name for screen readers alone.
                        .name(match muted {
                            true => t!("player-unmute"),
                            false => t!("player-mute"),
                        })
                        .tint(match open {
                            true => theme.foreground,
                            false => theme.muted_foreground,
                        })
                        .on_click(cx.listener(move |this: &mut V, _, _, cx| {
                            #[cfg(target_os = "android")]
                            {
                                // No hover on a phone: the button opens and closes the
                                // panel instead of muting, so the slider is reachable.
                                let next = !pinned();
                                set_pinned(next);
                                cx.notify();
                            }
                            #[cfg(not(target_os = "android"))]
                            this.playback()
                                .update(cx, |playback, cx| playback.toggle_muted(cx));
                        })),
                ),
        )
        .when(open, |this| {
            // The panel rises the way a menu does. A paint filter drops any backdrop
            // inside it, so the frost is a sibling underneath that fades in on the same
            // curve rather than arriving only once the rise is over.
            let panel = div()
                .id("volume-panel")
                .role(Role::Group)
                .aria_label(t!("equalizer-status-volume"))
                .occlude()
                .flex()
                .justify_center()
                .p_1()
                .py_2()
                .rounded(theme.radius)
                .border_1()
                .border_color(theme.border)
                .map(|this| match hazy {
                    true => this.bg(theme.popover.opacity(GLASS_FILL)),
                    false => this.bg(theme.popover),
                })
                .on_scroll_wheel(cx.listener(turn_volume))
                .on_hover(cx.listener(|this: &mut V, hovering: &bool, _, cx| {
                    this.volume().over_panel = *hovering;
                    cx.notify();
                }))
                .child(
                    div().h(px(VOLUME_RISE)).flex().child(
                        Scrubber::new(&volume.slider, level)
                            .vertical()
                            .tap_only()
                            .name(t!("equalizer-status-volume"))
                            .value(percent(level))
                            .colors(theme.progress_bar, empty, theme.foreground)
                            .lift(VOLUME_BUBBLE_LIFT)
                            .when_some(bubble, |this, (at, text)| this.bubble(at, text))
                            .on_move(cx.listener(|this: &mut V, fraction: &f32, _, cx| {
                                let level = *fraction;
                                this.volume().held = true;
                                this.playback()
                                    .update(cx, |playback, cx| playback.slide_volume(level, cx));
                                cx.notify();
                            }))
                            .on_release(cx.listener(|this: &mut V, _: &MouseUpEvent, _, cx| {
                                this.volume().held = false;
                                this.playback()
                                    .update(cx, |playback, cx| playback.end_volume_slide(cx));
                                cx.notify();
                            })),
                    ),
                );

            this.child(deferred(
                div()
                    .id("volume-zone")
                    .absolute()
                    .bottom_0()
                    .left(relative(0.5))
                    .ml(Pixels::ZERO - span / 2.)
                    .w(span)
                    .pt(zone)
                    .pb(theme.metrics.control_small + px(GAP) * 2.)
                    .flex()
                    .justify_center()
                    .on_mouse_down_out(cx.listener(|this: &mut V, _, _, cx| {
                        // A tap away from the control puts the slider away; a drag
                        // that is still going keeps it.
                        if !this.volume().held {
                            set_pinned(false);
                            cx.notify();
                        }
                    }))
                    .on_scroll_wheel(cx.listener(turn_volume))
                    .on_hover(cx.listener(|this: &mut V, hovering: &bool, _, cx| {
                        this.volume().over_zone = *hovering;
                        cx.notify();
                    }))
                    .child(
                        div()
                            .relative()
                            .when(hazy, |this| {
                                this.child(
                                    div()
                                        .absolute()
                                        .inset_0()
                                        .rounded(theme.radius)
                                        .backdrop_blur(GLASS_BLUR)
                                        .fading("volume-frost"),
                                )
                            })
                            // The rise composites what it carries as one layer the size of
                            // its bounds, so the wrapper reaches out as far as the level
                            // bubble can, and gives the room back with its margins.
                            .child(
                                div()
                                    .px(reach)
                                    .mx(Pixels::ZERO - reach)
                                    .child(panel)
                                    .rising("volume-rise"),
                            ),
                    ),
            ))
        })
}
