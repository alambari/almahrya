//! The toggle: an on/off switch, 38 x 22 px.

use crate::theme::Theme;
use gpui::{IntoElement, ParentElement, Styled, div, prelude::*, px, rgb};

/// A toggle. The caller may name what it switches; without a name the label falls back
/// to the state, so assistive technology always has something to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Toggle {
    on: bool,
    label: String,
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
        }
    }

    /// Names what the toggle switches, in the caller's language.
    pub fn labelled(mut self, label: &str) -> Self {
        self.label = label.to_owned();
        self
    }

    /// Whether the toggle is on.
    pub fn is_on(&self) -> bool {
        self.on
    }

    /// The caller's name for the toggle, or its state when it has none.
    pub fn label(&self) -> String {
        if self.label.trim().is_empty() {
            if self.on { "on" } else { "off" }.to_owned()
        } else {
            self.label.clone()
        }
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
    fn label_falls_back_to_the_state() {
        assert_eq!(Toggle::new(true).label(), "on");
        assert_eq!(Toggle::new(false).label(), "off");
        assert_eq!(Toggle::new(false).labelled("Notify").label(), "Notify");
    }

    #[test]
    fn size_matches_the_design_guide() {
        assert_eq!((WIDTH, HEIGHT), (38., 22.));
    }
}
