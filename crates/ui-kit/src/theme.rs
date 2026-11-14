//! Design tokens from DESIGN.md and their mapping onto gpui-component's theme.

use gpui::{App, px, rgb};
use gpui_component::{Theme as ComponentTheme, ThemeMode};

/// The colour tokens of DESIGN.md as `0xRRGGBB`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub bg: u32,
    pub surface: u32,
    pub sidebar: u32,
    pub hairline: u32,
    pub text: u32,
    pub text_muted: u32,
    pub text_faint: u32,
    pub agent: u32,
    pub waiting: u32,
}

impl Theme {
    /// The dark palette.
    pub const DARK: Theme = Theme {
        bg: 0x18181A,
        surface: 0x232326,
        sidebar: 0x1F1F22,
        hairline: 0x2C2C30,
        text: 0xECEAE4,
        text_muted: 0x97958E,
        text_faint: 0x6A6863,
        agent: 0x6F7CF2,
        waiting: 0xE8A93C,
    };

    /// Interface text.
    pub const UI_FONT: &'static str = "Plus Jakarta Sans";
    /// Code, paths and keys.
    pub const MONO_FONT: &'static str = "JetBrains Mono";
    /// Body and controls, in px.
    pub const BODY_SIZE: f32 = 13.;
    /// Captions, in px.
    pub const CAPTION_SIZE: f32 = 12.;
}

/// Label colour on `agent` buttons.
const ON_AGENT: u32 = 0xFFFFFF;
/// Link text on dark backgrounds (DESIGN.md, `agent` row).
const LINK_ON_DARK: u32 = 0x8E98F5;

/// Applies `theme` to every gpui-component slot the app uses.
///
/// gpui-component starts in light mode, and changing the mode reloads its colours, so
/// the mode is switched first and the tokens are written in a second update.
pub(crate) fn apply(theme: &Theme, cx: &mut App) {
    ComponentTheme::change(ThemeMode::Dark, None, cx);
    let t = *theme;
    ComponentTheme::update(cx, |c| {
        c.font_family = Theme::UI_FONT.into();
        c.mono_font_family = Theme::MONO_FONT.into();
        c.font_size = px(Theme::BODY_SIZE);
        c.radius = px(6.);

        let k = &mut c.colors;
        k.background = rgb(t.bg).into();
        k.foreground = rgb(t.text).into();
        k.border = rgb(t.hairline).into();
        k.input = rgb(t.hairline).into();
        k.ring = rgb(t.agent).into();
        k.muted = rgb(t.surface).into();
        k.muted_foreground = rgb(t.text_muted).into();
        k.accent = rgb(t.surface).into();
        k.accent_foreground = rgb(t.text).into();
        k.popover = rgb(t.surface).into();
        k.popover_foreground = rgb(t.text).into();
        k.link = rgb(LINK_ON_DARK).into();
        k.warning = rgb(t.waiting).into();

        k.primary = rgb(t.agent).into();
        k.primary_hover = rgb(t.agent).into();
        k.primary_active = rgb(t.agent).into();
        k.primary_foreground = rgb(ON_AGENT).into();
        // Buttons paint from their own slots, not from `primary`.
        k.button_primary = rgb(t.agent).into();
        k.button_primary_hover = rgb(t.agent).into();
        k.button_primary_active = rgb(t.agent).into();
        k.button_primary_foreground = rgb(ON_AGENT).into();
        k.secondary = rgb(t.surface).into();
        k.secondary_hover = rgb(t.hairline).into();
        k.secondary_active = rgb(t.hairline).into();
        k.secondary_foreground = rgb(t.text).into();

        k.sidebar = rgb(t.sidebar).into();
        k.sidebar_foreground = rgb(t.text).into();
        k.sidebar_border = rgb(t.hairline).into();
        k.list_hover = rgb(t.surface).into();
        k.title_bar = rgb(t.sidebar).into();
        k.title_bar_border = rgb(t.hairline).into();
        k.scrollbar_thumb = rgb(t.hairline).into();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_tokens_match_the_design_guide() {
        let t = Theme::DARK;
        assert_eq!(
            [t.bg, t.surface, t.sidebar, t.hairline],
            [0x18181A, 0x232326, 0x1F1F22, 0x2C2C30]
        );
        assert_eq!(
            [t.text, t.text_muted, t.text_faint],
            [0xECEAE4, 0x97958E, 0x6A6863]
        );
        assert_eq!([t.agent, t.waiting], [0x6F7CF2, 0xE8A93C]);
    }
}
