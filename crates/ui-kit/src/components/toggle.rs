//! The toggle: an on/off switch, 38 x 22 px.

use super::label::join_label;
use crate::theme::Theme;
use gpui::{IntoElement, ParentElement, Styled, div, prelude::*, px, rgb};

/// A toggle. Every word it reads out comes from the caller: the name of what it switches
/// and, optionally, the words for its two states. Without them its label is empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toggle {
    on: bool,
    label: String,
    on_word: String,
    off_word: String,
}

/// Track size from DESIGN.md, in px.
pub(crate) const WIDTH: f32 = 38.;
pub(crate) const HEIGHT: f32 = 22.;
/// Gap between the knob and the track edge, in px.
const INSET: f32 = 2.;
/// Track colour when off; DESIGN.md gives one value for both palettes.
const OFF: u32 = 0x3A3A40;
/// Knob colour in both states.
const KNOB: u32 = 0xFFFFFF;

impl Toggle {
    /// A toggle in the given state.
    pub fn new(on: bool) -> Self {
        Toggle {
            on,
            label: String::new(),
            on_word: String::new(),
            off_word: String::new(),
        }
    }

    /// Names what the toggle switches, in the caller's language.
    pub fn labelled(mut self, label: &str) -> Self {
        self.label = label.to_owned();
        self
    }

    /// The words for the on and off states, in the caller's language; the current one
    /// follows the name in the label.
    pub fn states(mut self, on: &str, off: &str) -> Self {
        self.on_word = on.to_owned();
        self.off_word = off.to_owned();
        self
    }

    /// Whether the toggle is on.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// What assistive technology reads: the caller's name, then the caller's word for
    /// the current state. Empty when the caller gave neither.
    pub fn label(&self) -> String {
        let state = if self.on {
            &self.on_word
        } else {
            &self.off_word
        };
        join_label([self.label.as_str(), state.as_str()])
    }

    pub(crate) fn render(&self, theme: &Theme) -> impl IntoElement + use<> {
        let knob = HEIGHT - 2. * INSET;
        div()
            .w(px(WIDTH))
            .h(px(HEIGHT))
            .flex_none()
            .flex()
            .items_center()
            .px(px(INSET))
            .rounded_full()
            .bg(rgb(if self.on { theme.agent } else { OFF }))
            // The knob sits at the right edge when on.
            .map(|el| {
                if self.on {
                    el.justify_end()
                } else {
                    el.justify_start()
                }
            })
            .child(div().size(px(knob)).rounded_full().bg(rgb(KNOB)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn label_uses_only_caller_words() {
        assert_eq!(Toggle::new(true).label(), "");
        assert_eq!(Toggle::new(false).labelled("Notify").label(), "Notify");
        assert_eq!(Toggle::new(true).states("Aktif", "Mati").label(), "Aktif");
    }

    #[test]
    fn size_matches_the_design_guide() {
        assert_eq!((WIDTH, HEIGHT), (38., 22.));
    }
}
