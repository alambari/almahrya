//! The label: text in one of the token text styles, so typography is defined once.

use crate::theme::Theme;
use gpui::{Div, FontWeight, ParentElement, SharedString, Styled, div, px, rgb};

/// The text styles of DESIGN.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelStyle {
    /// Card and section titles: 15 px, semibold, `text`.
    Title,
    /// Body text and controls: 13 px, `text`.
    Body,
    /// Secondary text: 12 px, `text-muted`.
    Muted,
    /// Hints and captions: 12 px, `text-faint`.
    Faint,
    /// Code, paths and keys: JetBrains Mono 13 px, `text`.
    Mono,
}

/// A text label. The text comes from the caller in its language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Label {
    text: String,
    style: LabelStyle,
}

impl Label {
    /// `text` in `style`.
    pub fn new(text: &str, style: LabelStyle) -> Self {
        Label {
            text: text.to_owned(),
            style,
        }
    }

    /// The text as given.
    pub fn text(&self) -> &str {
        &self.text
    }

    /// The style.
    pub fn style(&self) -> LabelStyle {
        self.style
    }

    /// What assistive technology reads: the text.
    pub fn label(&self) -> String {
        self.text.trim().to_owned()
    }

    pub(crate) fn render(&self, theme: &Theme) -> Div {
        styled_text(self.text.clone(), self.style, theme)
    }
}

/// Text in `style`, for components that do not keep a `Label` around. Callers may
/// restyle the returned element, for example to colour an action.
pub(crate) fn styled_text(text: impl Into<SharedString>, style: LabelStyle, t: &Theme) -> Div {
    let el = div().font_family(Theme::UI_FONT);
    let el = match style {
        LabelStyle::Title => el
            .text_size(px(Theme::READING_SIZE))
            .font_weight(FontWeight::SEMIBOLD)
            .text_color(rgb(t.text)),
        LabelStyle::Body => el.text_size(px(Theme::BODY_SIZE)).text_color(rgb(t.text)),
        LabelStyle::Muted => el
            .text_size(px(Theme::CAPTION_SIZE))
            .text_color(rgb(t.text_muted)),
        LabelStyle::Faint => el
            .text_size(px(Theme::CAPTION_SIZE))
            .text_color(rgb(t.text_faint)),
        LabelStyle::Mono => el
            .font_family(Theme::MONO_FONT)
            .text_size(px(Theme::BODY_SIZE))
            .text_color(rgb(t.text)),
    };
    el.child(text.into())
}

/// Joins the visible strings of a component into one accessible label, skipping empty
/// parts so a missing optional string leaves no stray separator. Only caller strings go
/// in, so the label follows the caller's language.
pub(crate) fn join_label<'a>(parts: impl IntoIterator<Item = &'a str>) -> String {
    parts
        .into_iter()
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect::<Vec<_>>()
        .join(". ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_skips_empty_parts() {
        assert_eq!(join_label(["Waiting", "", " Answer "]), "Waiting. Answer");
    }

    #[test]
    fn label_is_the_trimmed_text() {
        let l = Label::new(" Settings ", LabelStyle::Title);
        assert_eq!(l.label(), "Settings");
        assert_eq!(l.text(), " Settings ");
        assert_eq!(l.style(), LabelStyle::Title);
    }
}
