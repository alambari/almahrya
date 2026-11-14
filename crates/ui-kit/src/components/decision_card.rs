//! The decision card: a question with options, a recommendation, an own answer and the answer button.

use super::label::{LabelStyle, join_label, styled_text};
use crate::theme::Theme;
use gpui::{FontWeight, IntoElement, ParentElement, Styled, div, prelude::*, px, rgb, rgba};

/// One answer the owner can pick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionOption {
    pub title: String,
    /// What happens if this option is picked.
    pub consequence: String,
    /// The agent's recommendation; at most one option should carry it.
    pub recommended: bool,
}

/// A decision card. Every string comes from the caller; optional rows are left out until
/// their text is set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecisionCard {
    question: String,
    options: Vec<DecisionOption>,
    recommended_tag: String,
    own_answer: String,
    note: String,
    answer: String,
}

/// Label colour on the `waiting` button: dark in both palettes, because white on the
/// light `waiting` tone is too faint to read.
const ON_WAITING: u32 = 0x18181A;
/// Diameter of the option marker, in px.
const MARKER: f32 = 14.;

impl DecisionCard {
    /// A card asking `question` with `options` in the order given.
    pub fn new(question: &str, options: Vec<DecisionOption>) -> Self {
        DecisionCard {
            question: question.to_owned(),
            options,
            recommended_tag: String::new(),
            own_answer: String::new(),
            note: String::new(),
            answer: String::new(),
        }
    }

    /// The tag shown on the recommended option, for example "Recommended".
    pub fn recommended_tag(mut self, tag: &str) -> Self {
        self.recommended_tag = tag.to_owned();
        self
    }

    /// Adds the own-answer row with `placeholder` as its hint.
    pub fn own_answer(mut self, placeholder: &str) -> Self {
        self.own_answer = placeholder.to_owned();
        self
    }

    /// A note under the options, for example on the default and its timeout.
    pub fn note(mut self, note: &str) -> Self {
        self.note = note.to_owned();
        self
    }

    /// Adds the answer button with `label`.
    pub fn answer_button(mut self, label: &str) -> Self {
        self.answer = label.to_owned();
        self
    }

    /// The options in display order.
    pub fn options(&self) -> &[DecisionOption] {
        &self.options
    }

    /// What assistive technology reads: the question, then each option.
    pub fn label(&self) -> String {
        join_label(
            std::iter::once(self.question.as_str())
                .chain(self.options.iter().map(|o| o.title.as_str())),
        )
    }

    pub(crate) fn render(&self, theme: &Theme) -> impl IntoElement + use<> {
        let t = *theme;
        let options = self
            .options
            .iter()
            .map(|o| render_option(o.clone(), self.recommended_tag.clone(), t))
            .collect::<Vec<_>>();

        div()
            .w_full()
            .flex()
            .flex_col()
            .gap(px(12.))
            .p(px(16.))
            .rounded(px(Theme::RADIUS_CARD))
            .bg(rgb(t.surface))
            .border_l(px(Theme::WAITING_EDGE))
            .border_color(rgb(t.waiting))
            .child(styled_text(self.question.clone(), LabelStyle::Title, &t))
            .child(div().flex().flex_col().gap(px(6.)).children(options))
            .when(!self.own_answer.is_empty(), |el| {
                el.child(
                    styled_text(self.own_answer.clone(), LabelStyle::Faint, &t)
                        .h(px(32.))
                        .px(px(10.))
                        .flex()
                        .items_center()
                        .rounded(px(Theme::RADIUS_CONTROL))
                        .bg(rgb(t.bg))
                        .border_1()
                        .border_color(rgb(t.hairline))
                        .text_size(px(Theme::BODY_SIZE)),
                )
            })
            .child(
                div()
                    .flex()
                    .flex_row()
                    .items_center()
                    .gap(px(12.))
                    .child(
                        styled_text(self.note.clone(), LabelStyle::Muted, &t)
                            .flex_1()
                            .min_w_0(),
                    )
                    .when(!self.answer.is_empty(), |el| {
                        el.child(
                            styled_text(self.answer.clone(), LabelStyle::Body, &t)
                                .flex_none()
                                .h(px(30.))
                                .px(px(14.))
                                .flex()
                                .items_center()
                                .rounded(px(Theme::RADIUS_CONTROL))
                                .bg(rgb(t.waiting))
                                .text_color(rgb(ON_WAITING))
                                .font_weight(FontWeight::SEMIBOLD),
                        )
                    }),
            )
    }
}

fn render_option(option: DecisionOption, tag: String, t: Theme) -> impl IntoElement {
    let marker_color = if option.recommended {
        t.waiting
    } else {
        t.text_faint
    };
    div()
        .flex()
        .flex_row()
        .items_start()
        .gap(px(10.))
        .px(px(12.))
        .py(px(10.))
        .rounded(px(Theme::RADIUS_CARD - 2.))
        .border_1()
        .map(|el| {
            if option.recommended {
                // A light wash of `waiting` (about 10 %) so the recommended row reads as
                // selected without competing with the answer button.
                el.bg(rgba((t.waiting << 8) | 0x1A))
                    .border_color(rgb(t.waiting))
            } else {
                el.border_color(rgb(t.hairline))
            }
        })
        .child(
            div()
                .mt(px(2.))
                .size(px(MARKER))
                .flex_none()
                .rounded_full()
                .border_1()
                .border_color(rgb(marker_color))
                .when(option.recommended, |el| {
                    el.flex().items_center().justify_center().child(
                        div()
                            .size(px(MARKER / 2.))
                            .rounded_full()
                            .bg(rgb(t.waiting)),
                    )
                }),
        )
        .child(
            // `min_w_0` lets the text column shrink below its content so long text wraps
            // inside the card instead of pushing past it.
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(2.))
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_start()
                        .gap(px(8.))
                        .child(
                            styled_text(option.title, LabelStyle::Body, &t)
                                .flex_1()
                                .min_w_0()
                                .font_weight(FontWeight::MEDIUM),
                        )
                        .when(option.recommended && !tag.is_empty(), |el| {
                            el.child(
                                styled_text(tag, LabelStyle::Muted, &t)
                                    .flex_none()
                                    .px(px(6.))
                                    .rounded(px(Theme::RADIUS_CONTROL))
                                    .border_1()
                                    .border_color(rgb(t.waiting))
                                    .text_color(rgb(t.waiting)),
                            )
                        }),
                )
                .child(styled_text(option.consequence, LabelStyle::Muted, &t)),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn optional_text_does_not_enter_the_label() {
        let card = DecisionCard::new("Pick?", Vec::new()).note("Default in 10 min");
        assert_eq!(card.label(), "Pick?");
    }
}
