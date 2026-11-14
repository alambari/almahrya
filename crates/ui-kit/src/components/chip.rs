//! The chip: a small rounded label for related items such as tickets, merge requests and tools.

use super::label::{LabelStyle, styled_text};
use crate::theme::Theme;
use gpui::{IntoElement, Styled, px, rgb};

/// A chip. `mono` sets the text in JetBrains Mono, for keys, paths and tool names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Chip {
    text: String,
    mono: bool,
}

/// Chip height, in px.
const HEIGHT: f32 = 22.;

impl Chip {
    /// A chip showing `text`.
    pub fn new(text: &str, mono: bool) -> Self {
        Chip {
            text: text.to_owned(),
            mono,
        }
    }

    /// Whether the text is set in the mono font.
    pub fn is_mono(&self) -> bool {
        self.mono
    }

    /// What assistive technology reads: the chip text.
    pub fn label(&self) -> String {
        self.text.trim().to_owned()
    }

    pub(crate) fn render(&self, theme: &Theme) -> impl IntoElement + use<> {
        let style = if self.mono {
            LabelStyle::Mono
        } else {
            LabelStyle::Muted
        };
        styled_text(self.text.clone(), style, theme)
            .h(px(HEIGHT))
            .px(px(8.))
            .flex()
            .flex_none()
            .items_center()
            .rounded(px(Theme::RADIUS_CONTROL))
            .bg(rgb(theme.surface))
            .border_1()
            .border_color(rgb(theme.hairline))
            // Chips are captions whichever font they use.
            .text_size(px(Theme::CAPTION_SIZE))
            .text_color(rgb(theme.text_muted))
    }
}
