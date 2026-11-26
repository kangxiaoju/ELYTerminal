//! Host-owned terminal setup without standalone configuration or app lifecycle.

use std::borrow::Cow;
use gpui::{App, Global};
use crate::{settings::{Settings, SettingsStore}, term::zoom};

/// Marks a host-owned terminal and selects its Ely theme.
pub struct Host;
impl Global for Host {}

/// Loads terminal fonts, in-memory settings and scoped keyboard bindings.
///
/// # Errors
/// Returns an error when font registration or settings validation fails.
pub fn init(cx: &mut App) -> anyhow::Result<()> {
    if cx.has_global::<Host>() { return Ok(()); }
    #[cfg(windows)]
    crate::term::keep_wheel()?;
    cx.text_system().add_fonts(vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-BoldItalic.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf")),
    ])?;
    SettingsStore::init_embedded(Settings {
        theme: "ely-dark".into(), background_opacity: 1.0,
        background_blur: false, check_updates: false,
        ..Settings::default()
    }, cx)?;
    cx.set_global(zoom::Enlarged::default());
    cx.set_global(Host);
    crate::keymap::init(cx);
    Ok(())
}
