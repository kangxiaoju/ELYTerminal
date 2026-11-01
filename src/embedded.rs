//! Embedded terminals keep the host's identity, theme and application lifecycle.

use std::borrow::Cow;

use gpui::{App, Global};

use crate::{settings::{Settings, SettingsStore}, term::{input, zoom}};

/// Presence selects the host's Ely palette and disables herdr discovery.
pub struct Host;
impl Global for Host {}

/// Registers only terminal fonts, settings and scoped keyboard bindings.
/// Does not read or write the standalone application's configuration.
/// Terminal words use their own catalogs and follow the host's `I18n` locale;
/// this never replaces or modifies the host's application messages.
pub fn init(cx: &mut App) -> anyhow::Result<()> {
    if cx.has_global::<Host>() {
        return Ok(());
    }
    cx.text_system().add_fonts(vec![
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-Italic.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMono-BoldItalic.ttf")),
        Cow::Borrowed(include_bytes!("../assets/fonts/JetBrainsMonoNerdFontMono-Regular.ttf")),
    ])?;
    SettingsStore::init_embedded(Settings {
        theme: "ely-dark".into(),
        background_opacity: 1.0,
        background_blur: false,
        check_updates: false,
        ..Settings::default()
    }, cx)?;
    cx.set_global(zoom::Enlarged::default());
    cx.set_global(Host);
    input::bind_keys(cx);
    Ok(())
}
