//! The application entry point and the sessions window.

use crate::fonts;
use crate::sidebar::{ConnectionStatus, SessionItem, Sidebar};
use crate::theme::{self, Theme};
use futures_util::{Stream, StreamExt};
use gpui::{
    App, AppContext, Bounds, Context, IntoElement, KeyBinding, Menu, MenuItem, ParentElement,
    Render, Styled, Task, TitlebarOptions, Window, WindowBounds, WindowOptions, actions, div,
    point, px, rgb, size,
};
use gpui_component::Root;
use std::rc::Rc;

actions!(almahrya, [Quit]);

/// What the window shows first.
#[derive(Debug, Clone, PartialEq)]
pub struct WindowConfig {
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub theme: Theme,
}

impl Default for WindowConfig {
    fn default() -> Self {
        WindowConfig {
            title: "Almahrya".to_owned(),
            width: 1200.,
            height: 780.,
            theme: Theme::DARK,
        }
    }
}

/// A change pushed into the window from outside the UI thread.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionsUpdate {
    /// The full list, newest first.
    Sessions(Vec<SessionItem>),
    Status(ConnectionStatus),
}

struct SessionsWindow {
    theme: Theme,
    sessions: Vec<SessionItem>,
    status: ConnectionStatus,
    on_new_session: Rc<dyn Fn()>,
    _updates: Task<()>,
}

impl SessionsWindow {
    fn apply(&mut self, update: SessionsUpdate) {
        match update {
            SessionsUpdate::Sessions(sessions) => self.sessions = sessions,
            SessionsUpdate::Status(status) => self.status = status,
        }
    }
}

impl Render for SessionsWindow {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let t = self.theme;
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(t.bg))
            .text_color(rgb(t.text))
            .font_family(Theme::UI_FONT)
            .text_size(px(Theme::BODY_SIZE))
            .child(Sidebar {
                theme: t,
                sessions: self.sessions.clone(),
                status: self.status.clone(),
                on_new_session: self.on_new_session.clone(),
            })
            .child(
                div()
                    .flex()
                    .flex_1()
                    .h_full()
                    .items_center()
                    .justify_center()
                    .text_color(rgb(t.text_muted))
                    .child("Choose a session or start a new chat"),
            )
    }
}

/// Runs the app with one sessions window until the user quits. `updates` is polled on
/// the UI thread; `on_new_session` is called there when the button is clicked, so it
/// should only hand the request to another thread.
pub fn run_sessions_window<S>(config: WindowConfig, updates: S, on_new_session: impl Fn() + 'static)
where
    S: Stream<Item = SessionsUpdate> + Unpin + 'static,
{
    let on_new_session: Rc<dyn Fn()> = Rc::new(on_new_session);
    gpui_platform::application().run(move |cx: &mut App| {
        gpui_component::init(cx);
        fonts::register(cx);
        theme::apply(&config.theme, cx);

        cx.on_action(|_: &Quit, cx| cx.quit());
        cx.bind_keys([KeyBinding::new("cmd-q", Quit, None)]);
        cx.set_menus([Menu::new(config.title.clone())
            .items([MenuItem::action(format!("Quit {}", config.title), Quit)])]);
        cx.on_window_closed(|cx, _| cx.quit()).detach();

        let bounds = Bounds::centered(None, size(px(config.width), px(config.height)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(config.title.clone().into()),
                // The native bar follows the system appearance, which can be light above
                // a dark app; the sidebar runs under the traffic lights instead.
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.), px(12.))),
            }),
            ..Default::default()
        };
        let theme = config.theme;
        let on_new_session = on_new_session.clone();
        let opened = cx.open_window(options, move |window, cx| {
            let view = cx.new(|cx: &mut Context<SessionsWindow>| {
                let task = cx.spawn(async move |this, cx| {
                    let mut updates = updates;
                    while let Some(update) = updates.next().await {
                        let applied = this.update(cx, |view, cx| {
                            view.apply(update);
                            cx.notify();
                        });
                        if applied.is_err() {
                            break;
                        }
                    }
                });
                SessionsWindow {
                    theme,
                    sessions: Vec::new(),
                    status: ConnectionStatus::Connecting,
                    on_new_session,
                    _updates: task,
                }
            });
            cx.new(|cx| Root::new(view, window, cx))
        });
        if let Err(e) = opened {
            eprintln!("could not open the window: {e}");
            cx.quit();
            return;
        }
        cx.activate(true);
    });
}
