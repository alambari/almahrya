//! The window's left column: the new-session button and the session list.

use crate::theme::Theme;
use gpui::{
    App, IntoElement, ParentElement, RenderOnce, SharedString, Styled, Window, div, prelude::*, px,
    rgb,
};
use gpui_component::button::{Button, ButtonVariants};
use std::rc::Rc;

/// One row of the session list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionItem {
    pub id: String,
    pub title: String,
}

/// Whether the app is talking to the daemon. Shown under the list unless connected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConnectionStatus {
    Connecting,
    Connected,
    Reconnecting,
    /// Starting or reaching the daemon failed; the text says why.
    Failed(String),
}

impl ConnectionStatus {
    fn label(&self) -> Option<SharedString> {
        match self {
            ConnectionStatus::Connected => None,
            ConnectionStatus::Connecting => Some("Connecting to almahryad".into()),
            ConnectionStatus::Reconnecting => Some("Reconnecting to almahryad".into()),
            ConnectionStatus::Failed(why) => Some(format!("Not connected: {why}").into()),
        }
    }
}

/// Sidebar width from DESIGN.md.
pub(crate) const WIDTH: f32 = 244.;
/// Room for the traffic lights above the content (the title bar is transparent).
const TITLE_BAR_HEIGHT: f32 = 38.;

#[derive(IntoElement)]
pub(crate) struct Sidebar {
    pub theme: Theme,
    pub sessions: Vec<SessionItem>,
    pub status: ConnectionStatus,
    pub on_new_session: Rc<dyn Fn()>,
}

impl RenderOnce for Sidebar {
    fn render(self, _window: &mut Window, _cx: &mut App) -> impl IntoElement {
        let t = self.theme;
        let on_new = self.on_new_session.clone();
        let is_empty = self.sessions.is_empty();
        let rows = self.sessions.into_iter().map(move |s| {
            div()
                .id(SharedString::from(s.id))
                .h(px(30.))
                .px(px(8.))
                .flex()
                .items_center()
                .rounded(px(6.))
                .text_color(rgb(t.text))
                .hover(move |style| style.bg(rgb(t.surface)))
                .child(
                    div()
                        .w_full()
                        .overflow_hidden()
                        .whitespace_nowrap()
                        .text_ellipsis()
                        .child(s.title),
                )
        });
        let list = div()
            .id("sessions")
            .flex()
            .flex_col()
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .children(rows);
        let empty = div()
            .flex_1()
            .px(px(8.))
            .text_color(rgb(t.text_faint))
            .child("No sessions yet");

        div()
            .w(px(WIDTH))
            .h_full()
            .flex()
            .flex_col()
            .flex_none()
            .bg(rgb(t.sidebar))
            .border_r_1()
            .border_color(rgb(t.hairline))
            .pt(px(TITLE_BAR_HEIGHT))
            .px(px(12.))
            .pb(px(12.))
            .child(
                Button::new("new-chat-session")
                    .primary()
                    .w_full()
                    .label("New chat session")
                    .on_click(move |_, _, _| on_new()),
            )
            .child(
                div()
                    .mt(px(20.))
                    .mb(px(4.))
                    .px(px(8.))
                    .text_size(px(Theme::CAPTION_SIZE))
                    .text_color(rgb(t.text_faint))
                    .child("Session"),
            )
            .map(|el| {
                if is_empty {
                    el.child(empty)
                } else {
                    el.child(list)
                }
            })
            .children(self.status.label().map(|label| {
                div()
                    .pt(px(8.))
                    .px(px(8.))
                    .text_size(px(Theme::CAPTION_SIZE))
                    .text_color(rgb(t.text_muted))
                    .child(label)
            }))
    }
}
