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
    pub success: u32,
    pub danger: u32,
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
        success: 0x7DBB8E,
        danger: 0xD98A7A,
    };

    /// The light palette.
    pub const LIGHT: Theme = Theme {
        bg: 0xF4F5F2,
        surface: 0xE8E9E4,
        sidebar: 0xECEDE8,
        hairline: 0xDADBD5,
        text: 0x1D1D1F,
        text_muted: 0x6E6C66,
        text_faint: 0x8E8C86,
        agent: 0x4A57D6,
        waiting: 0xB97A12,
        success: 0x3E8A57,
        danger: 0xB2503D,
    };

    /// The dark palette (Arang).
    pub const fn dark() -> Theme {
        Theme::DARK
    }

    /// The light palette (Kertas).
    pub const fn light() -> Theme {
        Theme::LIGHT
    }

    /// Whether the background is dark, judged by its perceived lightness, so a palette
    /// built by hand still gets the right component mode.
    pub fn is_dark(&self) -> bool {
        let [_, r, g, b] = self.bg.to_be_bytes();
        let luma = 299 * u32::from(r) + 587 * u32::from(g) + 114 * u32::from(b);
        luma < 128 * 1000
    }

    /// Link text: lighter than `agent` on dark backgrounds (DESIGN.md, `agent` row).
    pub fn link(&self) -> u32 {
        if self.is_dark() {
            LINK_ON_DARK
        } else {
            self.agent
        }
    }

    /// Interface text.
    pub const UI_FONT: &'static str = "Plus Jakarta Sans";
    /// Code, paths and keys.
    pub const MONO_FONT: &'static str = "JetBrains Mono";
    /// Body and controls, in px.
    pub const BODY_SIZE: f32 = 13.;
    /// Captions, in px.
    pub const CAPTION_SIZE: f32 = 12.;
    /// Reading text in sessions and card titles, in px.
    pub const READING_SIZE: f32 = 15.;
    /// Corner radius of controls (buttons, inputs, chips), in px.
    pub const RADIUS_CONTROL: f32 = 6.;
    /// Corner radius of cards, in px (DESIGN.md allows 8 to 10).
    pub const RADIUS_CARD: f32 = 10.;
    /// Corner radius of popups and drawers, in px.
    pub const RADIUS_POPUP: f32 = 12.;
    /// Width of the `waiting` left edge on items that wait for the owner, in px.
    pub const WAITING_EDGE: f32 = 3.;
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
    let mode = if theme.is_dark() {
        ThemeMode::Dark
    } else {
        ThemeMode::Light
    };
    ComponentTheme::change(mode, None, cx);
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
        k.link = rgb(t.link()).into();
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
        assert_eq!([t.success, t.danger], [0x7DBB8E, 0xD98A7A]);
        assert_eq!(Theme::dark(), Theme::DARK);
    }

    #[test]
    fn light_tokens_match_the_design_guide() {
        let t = Theme::light();
        assert_eq!(
            [t.bg, t.surface, t.sidebar, t.hairline],
            [0xF4F5F2, 0xE8E9E4, 0xECEDE8, 0xDADBD5]
        );
        assert_eq!(
            [t.text, t.text_muted, t.text_faint],
            [0x1D1D1F, 0x6E6C66, 0x8E8C86]
        );
        assert_eq!([t.agent, t.waiting], [0x4A57D6, 0xB97A12]);
        assert_eq!([t.success, t.danger], [0x3E8A57, 0xB2503D]);
    }

    #[test]
    fn dark_and_light_differ_where_it_shows() {
        let (d, l) = (Theme::dark(), Theme::light());
        assert_ne!(d.bg, l.bg);
        assert_ne!(d.text, l.text);
        assert!(d.is_dark());
        assert!(!l.is_dark());
        assert_eq!(d.link(), LINK_ON_DARK);
        assert_eq!(l.link(), l.agent);
    }
}
