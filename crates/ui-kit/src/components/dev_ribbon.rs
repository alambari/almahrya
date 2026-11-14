//! The development ribbon: marks a window that belongs to the development profile, so it
//! is never mistaken for the production app running beside it.

use crate::theme::Theme;
use gpui::{FontWeight, IntoElement, ParentElement, Styled, div, px, rgb};

/// A small ribbon showing the caller's text (the app passes its profile name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DevRibbon {
    text: String,
}

/// Ribbon height, in px.
const HEIGHT: f32 = 18.;
/// The ribbon is `waiting`-orange on dark text in both palettes, so it reads the same
/// whichever theme the window uses.
const FILL: u32 = Theme::DARK.waiting;
const INK: u32 = Theme::DARK.bg;

impl DevRibbon {
    /// A ribbon reading `text`.
    pub fn new(text: &str) -> Self {
        DevRibbon {
            text: text.to_owned(),
        }
    }

    /// The fill colour as `0xRRGGBB`.
    pub fn fill(&self) -> u32 {
        FILL
    }

    /// The text colour as `0xRRGGBB`.
    pub fn ink(&self) -> u32 {
        INK
    }

    /// What assistive technology reads: the ribbon text.
    pub fn label(&self) -> String {
        self.text.trim().to_owned()
    }

    pub(crate) fn render(&self) -> impl IntoElement + use<> {
        div()
            .h(px(HEIGHT))
            .px(px(8.))
            .flex()
            .flex_none()
            .items_center()
            .rounded(px(Theme::RADIUS_CONTROL))
            .bg(rgb(FILL))
            .text_color(rgb(INK))
            .text_size(px(Theme::CAPTION_SIZE))
            .font_weight(FontWeight::SEMIBOLD)
            .child(self.text.clone())
    }
}
