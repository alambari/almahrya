//! The list card: a rounded container of label and value rows aligned in columns.

use super::label::{LabelStyle, join_label, styled_text};
use crate::theme::Theme;
use gpui::{IntoElement, ParentElement, Styled, div, prelude::*, px, rgb};

/// One row: a label in the fixed column and its value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListRow {
    pub label: String,
    pub value: String,
}

/// A list card. Labels share one fixed-width column, so values line up without spaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListCard {
    rows: Vec<ListRow>,
}

/// Minimum row height from DESIGN.md, in px.
pub(crate) const ROW_HEIGHT: f32 = 44.;
/// Width of the label column, in px.
pub(crate) const LABEL_WIDTH: f32 = 160.;

impl ListCard {
    /// A card with `rows` in the order given.
    pub fn new(rows: Vec<ListRow>) -> Self {
        ListCard { rows }
    }

    /// The rows in display order.
    pub fn rows(&self) -> &[ListRow] {
        &self.rows
    }

    /// What assistive technology reads: each label followed by its value.
    pub fn label(&self) -> String {
        join_label(
            self.rows
                .iter()
                .flat_map(|r| [r.label.as_str(), r.value.as_str()]),
        )
    }

    pub(crate) fn render(&self, theme: &Theme) -> impl IntoElement + use<> {
        let t = *theme;
        let rows = self
            .rows
            .clone()
            .into_iter()
            .enumerate()
            .map(move |(i, row)| {
                div()
                    .min_h(px(ROW_HEIGHT))
                    .px(px(14.))
                    .flex()
                    .flex_row()
                    .items_center()
                    .when(i > 0, |el| el.border_t_1().border_color(rgb(t.hairline)))
                    .child(
                        styled_text(row.label, LabelStyle::Muted, &t)
                            .w(px(LABEL_WIDTH))
                            .flex_none(),
                    )
                    .child(
                        styled_text(row.value, LabelStyle::Body, &t)
                            .flex_1()
                            .min_w_0(),
                    )
            });
        div()
            .w_full()
            .flex()
            .flex_col()
            .rounded(px(Theme::RADIUS_CARD))
            .bg(rgb(t.surface))
            .border_1()
            .border_color(rgb(t.hairline))
            .children(rows)
    }
}
