//! The font subset the app ships with, so it renders the same where the fonts are not
//! installed. Licences: `assets/fonts/README.md`.

use gpui::App;
use std::borrow::Cow;

const FONTS: [&[u8]; 5] = [
    include_bytes!("../assets/fonts/PlusJakartaSans-Regular.ttf"),
    include_bytes!("../assets/fonts/PlusJakartaSans-Medium.ttf"),
    include_bytes!("../assets/fonts/PlusJakartaSans-SemiBold.ttf"),
    include_bytes!("../assets/fonts/PlusJakartaSans-Bold.ttf"),
    include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
];

/// Registers the embedded fonts. On failure the installed copies, if any, still work,
/// so the error is reported and startup continues.
pub(crate) fn register(cx: &App) {
    let fonts = FONTS.iter().map(|bytes| Cow::Borrowed(*bytes)).collect();
    if let Err(e) = cx.text_system().add_fonts(fonts) {
        eprintln!("could not register the embedded fonts: {e}");
    }
}
