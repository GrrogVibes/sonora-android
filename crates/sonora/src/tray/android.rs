//! Android has no tray icon. The one type the shared `dock` module names is
//! declared here so it keeps compiling.

use gpui::App;

/// What the tray would show. Unused on Android; `dock` only names the type.
pub struct Shown;

/// Installs nothing and reports that the app must not refuse to quit on close.
pub fn install(
    _show: impl Fn(&mut App) + 'static,
    _toggle: impl Fn(&mut App) + 'static,
    _cx: &mut App,
) -> bool {
    false
}
