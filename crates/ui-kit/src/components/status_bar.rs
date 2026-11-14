//! The session status bar: what is happening now and the one action that fits.

use super::label::{LabelStyle, join_label, styled_text};
use crate::theme::Theme;
use gpui::{FontWeight, IntoElement, ParentElement, Styled, div, px, rgb};

/// Who the bar is about. DESIGN.md keeps `waiting` for the owner and `agent` for the agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusKind {
    /// Something waits for the owner: drawn in `waiting` with the 3 px left edge.
    Waiting,
    /// Nothing waits for the owner; the agent is working: drawn in `agent`.
    Agent,
}

/// The session status bar. Text and action label come from the caller in its language.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusBar {
    kind: StatusKind,
    text: String,
    action: String,
}

/// Bar height, in px.
const HEIGHT: f32 = 40.;
/// Diameter of the status dot, in px.
const DOT: f32 = 8.;

impl StatusBar {
    /// A bar that tells the owner something waits for them.
    pub fn waiting(text: &str, action: &str) -> Self {
        Self::new(StatusKind::Waiting, text, action)
    }

    /// A bar that says what the agent is doing.
    pub fn agent(text: &str, action: &str) -> Self {
        Self::new(StatusKind::Agent, text, action)
    }

    fn new(kind: StatusKind, text: &str, action: &str) -> Self {
        StatusBar {
            kind,
            text: text.to_owned(),
            action: action.to_owned(),
        }
    }

    /// Which variant this is.
    pub fn kind(&self) -> StatusKind {
        self.kind
    }

    /// What assistive technology reads: the status, then the action.
    pub fn label(&self) -> String {
        join_label([self.text.as_str(), self.action.as_str()])
    }

    pub(crate) fn render(&self, theme: &Theme) -> impl IntoElement + use<> {
        let t = *theme;
        let (accent, edge) = match self.kind {
            StatusKind::Waiting => (t.waiting, px(Theme::WAITING_EDGE)),
            StatusKind::Agent => (t.agent, px(0.)),
        };
        let action_color = match self.kind {
            StatusKind::Waiting => t.waiting,
            StatusKind::Agent => t.link(),
        };
        div()
            .h(px(HEIGHT))
            .w_full()
            .flex()
            .flex_row()
            .items_center()
            .gap(px(10.))
            .pl(px(14.))
            .pr(px(8.))
            .rounded(px(Theme::RADIUS_CARD))
            .bg(rgb(t.surface))
            .border_l(edge)
            .border_color(rgb(t.waiting))
            .child(div().size(px(DOT)).rounded_full().bg(rgb(accent)))
            .child(
                styled_text(self.text.clone(), LabelStyle::Body, &t)
                    .flex_1()
                    .min_w_0()
                    .overflow_hidden()
                    .whitespace_nowrap()
                    .text_ellipsis(),
            )
            .child(
                styled_text(self.action.clone(), LabelStyle::Body, &t)
                    .flex_none()
                    .h(px(28.))
                    .px(px(10.))
                    .flex()
                    .items_center()
                    .rounded(px(Theme::RADIUS_CONTROL))
                    .text_color(rgb(action_color))
                    .font_weight(FontWeight::SEMIBOLD),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constructors_pick_the_variant() {
        assert_eq!(StatusBar::waiting("a", "b").kind(), StatusKind::Waiting);
        assert_eq!(StatusBar::agent("a", "b").kind(), StatusKind::Agent);
    }
}
