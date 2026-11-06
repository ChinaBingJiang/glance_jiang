//! The panel: how it is laid out and what it shows, on any platform, and how
//! each platform draws it (on Windows, Direct2D and DirectWrite on a
//! DirectComposition surface of the panel's own window).

/// The about-this-PC card: a small window of its own, on a thread of its own,
/// opened from the tray (see `crate::show_about`).
#[cfg(windows)]
pub mod about;
pub mod arrange;
#[cfg(windows)]
pub(crate) mod ask;
#[cfg(windows)]
pub mod backdrop;
pub mod beside;
pub mod buttons;
pub mod canvas;
pub mod dock;
pub mod editor;
pub mod forms;
#[cfg(windows)]
pub mod genie;
pub mod gfx;
#[cfg(windows)]
pub mod glass;
pub mod icons;
#[cfg(windows)]
pub mod keys;
/// The launchpad's own shape: its grid, its pages, its search (see `crate::launchpad`).
pub mod launchpad;
pub mod life;
#[cfg(windows)]
pub mod menu;
pub mod morph;
pub mod motion;
#[cfg(windows)]
pub mod music;
pub mod note;
pub mod overlay;
pub mod phone_art;
pub(crate) mod picture;
pub mod prefs;
#[cfg(windows)]
pub mod render;
pub mod scene;
/// How long each app has kept the screen, gathered by a thread of this
/// program's own for the 屏幕使用时间 page (see `settings_window`).
#[cfg(windows)]
pub(crate) mod screen_time;
pub mod seen;
#[cfg(windows)]
pub mod settings_window;
#[cfg(windows)]
pub mod skins;
pub mod smoke;
/// The helpers the settings window's PC's own pages use to read and write
/// Windows directly (registry, battery, input, theme, privacy, wallpapers);
/// the system settings' window that used to live here is gone (see
/// `settings_window`).
#[cfg(windows)]
pub mod system_settings;
pub mod text;
pub mod theme;
pub mod thumb;
#[cfg(windows)]
pub mod tip;
pub mod typing;
pub mod view;
#[cfg(windows)]
pub mod wallpaper;
/// The wireless networks, read from Windows' own WLAN service for the Wi-Fi
/// page (see `settings_window`).
#[cfg(windows)]
mod wifi;
#[cfg(windows)]
pub mod window;
