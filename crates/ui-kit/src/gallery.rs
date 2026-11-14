//! A window that shows every component in each palette side by side, so a GPUI upgrade
//! can be checked by comparing screenshots taken before and after it.

use crate::components::sidebar::Sidebar;
use crate::components::{
    Chip, ConnectionStatus, DecisionCard, DevRibbon, Label, LabelStyle, ListCard, SessionItem,
    StatusBar, Toggle,
};
use crate::fonts;
use crate::theme::{self, Theme};
use gpui::{
    AnyElement, App, AppContext, Bounds, Context, IntoElement, ParentElement, Render, Styled,
    TitlebarOptions, Window, WindowBounds, WindowOptions, div, point, px, rgb, size,
};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Duration;

/// Everything the gallery shows. All text, headings included, comes from the caller.
#[derive(Debug, Clone, PartialEq)]
pub struct Gallery {
    pub title: String,
    pub width: f32,
    pub height: f32,
    /// One column per palette, left to right. The first palette also styles the
    /// gpui-component widgets (the sidebar button), which take one app-wide theme.
    pub columns: Vec<GalleryColumn>,
    /// Lanes side by side inside every column; each column draws the same lanes.
    pub lanes: Vec<GalleryLane>,
}

/// A column drawn in one palette.
#[derive(Debug, Clone, PartialEq)]
pub struct GalleryColumn {
    pub heading: String,
    pub theme: Theme,
}

/// A vertical stack of sections inside a column.
#[derive(Debug, Clone, PartialEq)]
pub struct GalleryLane {
    /// Fixed width in px; `None` shares the remaining width.
    pub width: Option<f32>,
    pub sections: Vec<GallerySection>,
}

/// A titled group of components.
#[derive(Debug, Clone, PartialEq)]
pub struct GallerySection {
    pub heading: String,
    pub items: Vec<GalleryItem>,
}

/// One gallery entry. Labels, toggles and chips come in groups, since one alone says little.
#[derive(Debug, Clone, PartialEq)]
pub enum GalleryItem {
    Labels(Vec<Label>),
    StatusBar(StatusBar),
    DecisionCard(DecisionCard),
    Toggles(Vec<Toggle>),
    Chips(Vec<Chip>),
    ListCard(ListCard),
    DevRibbon(DevRibbon),
    /// The sessions sidebar, drawn `height` px tall.
    Sidebar {
        sessions: Vec<SessionItem>,
        status: ConnectionStatus,
        height: f32,
    },
}

impl GalleryItem {
    /// The accessible label of the entry, for checks that every component describes itself.
    pub fn label(&self) -> String {
        fn join<T>(row: &[T], label: impl Fn(&T) -> String) -> String {
            row.iter().map(label).collect::<Vec<_>>().join(", ")
        }
        match self {
            GalleryItem::Labels(row) => join(row, Label::label),
            GalleryItem::StatusBar(c) => c.label(),
            GalleryItem::DecisionCard(c) => c.label(),
            GalleryItem::ListCard(c) => c.label(),
            GalleryItem::DevRibbon(c) => c.label(),
            GalleryItem::Toggles(row) => join(row, Toggle::label),
            GalleryItem::Chips(row) => join(row, Chip::label),
            GalleryItem::Sidebar { sessions, .. } => join(sessions, |s| s.title.clone()),
        }
    }
}

/// Why a gallery screenshot was not written.
#[derive(Debug, thiserror::Error)]
pub enum GalleryError {
    #[error("this build cannot capture the window; build with the `screenshot` feature")]
    ScreenshotUnsupported,
    #[error("could not open the gallery window: {0}")]
    Window(String),
    #[error("could not render the gallery to an image: {0}")]
    Render(String),
    #[error("could not write {path}: {reason}")]
    Save { path: PathBuf, reason: String },
}

/// How long the window gets to load fonts and lay out before it is captured.
const SETTLE: Duration = Duration::from_millis(800);
/// Room for the traffic lights above the content (the title bar is transparent).
const TITLE_BAR_HEIGHT: f32 = 38.;

struct GalleryView {
    gallery: Gallery,
}

impl Render for GalleryView {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let columns = self
            .gallery
            .columns
            .iter()
            .map(|column| render_column(column, &self.gallery.lanes))
            .collect::<Vec<_>>();
        div()
            .size_full()
            .flex()
            .flex_row()
            .font_family(Theme::UI_FONT)
            .text_size(px(Theme::BODY_SIZE))
            .children(columns)
    }
}

fn render_column(column: &GalleryColumn, lanes: &[GalleryLane]) -> AnyElement {
    let t = column.theme;
    let lanes = lanes.iter().map(|lane| {
        let sections = lane.sections.iter().map(|section| {
            div()
                .flex()
                .flex_col()
                .gap(px(8.))
                .child(Label::new(&section.heading, LabelStyle::Muted).render(&t))
                .children(section.items.iter().map(|item| render_item(item, &t)))
        });
        let el = div().flex().flex_col().gap(px(20.)).min_w_0();
        match lane.width {
            Some(w) => el.w(px(w)).flex_none(),
            None => el.flex_1(),
        }
        .children(sections)
    });
    div()
        .flex_1()
        .min_w_0()
        .h_full()
        .flex()
        .flex_col()
        .gap(px(16.))
        .pt(px(TITLE_BAR_HEIGHT + 4.))
        .px(px(28.))
        .pb(px(24.))
        .bg(rgb(t.bg))
        .text_color(rgb(t.text))
        .child(Label::new(&column.heading, LabelStyle::Title).render(&t))
        .child(div().flex().flex_row().gap(px(24.)).children(lanes))
        .into_any_element()
}

fn render_item(item: &GalleryItem, t: &Theme) -> AnyElement {
    let row = || div().flex().flex_row().items_center();
    match item {
        GalleryItem::Labels(labels) => div()
            .flex()
            .flex_col()
            .gap(px(6.))
            .children(labels.iter().map(|c| c.render(t)))
            .into_any_element(),
        GalleryItem::StatusBar(c) => c.render(t).into_any_element(),
        GalleryItem::DecisionCard(c) => c.render(t).into_any_element(),
        GalleryItem::ListCard(c) => c.render(t).into_any_element(),
        GalleryItem::DevRibbon(c) => row().child(c.render()).into_any_element(),
        GalleryItem::Toggles(toggles) => row()
            .gap(px(12.))
            .children(toggles.iter().map(|c| c.render(t)))
            .into_any_element(),
        GalleryItem::Chips(chips) => row()
            .flex_wrap()
            .gap(px(6.))
            .children(chips.iter().map(|c| c.render(t)))
            .into_any_element(),
        GalleryItem::Sidebar {
            sessions,
            status,
            height,
        } => div()
            .h(px(*height))
            .flex()
            .rounded(px(Theme::RADIUS_CARD))
            .border_1()
            .border_color(rgb(t.hairline))
            .overflow_hidden()
            .child(Sidebar {
                theme: *t,
                sessions: sessions.clone(),
                status: status.clone(),
                on_new_session: Rc::new(|| {}),
            })
            .into_any_element(),
    }
}

/// Opens the gallery and runs until the window closes.
///
/// With `screenshot`, the window is rendered to that PNG once it has settled,
/// `on_screenshot` receives the outcome, and the app quits. The platform ends the
/// process when the app quits, so a caller that wants a failing exit status sets it in
/// `on_screenshot`.
pub fn run_gallery(
    gallery: Gallery,
    screenshot: Option<PathBuf>,
    on_screenshot: impl FnOnce(Result<PathBuf, GalleryError>) + 'static,
) {
    gpui_platform::application().run(move |cx: &mut App| {
        gpui_component::init(cx);
        fonts::register(cx);
        if let Some(first) = gallery.columns.first() {
            theme::apply(&first.theme, cx);
        }
        cx.on_window_closed(|cx, _| cx.quit()).detach();

        let bounds = Bounds::centered(None, size(px(gallery.width), px(gallery.height)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            titlebar: Some(TitlebarOptions {
                title: Some(gallery.title.clone().into()),
                appears_transparent: true,
                traffic_light_position: Some(point(px(12.), px(12.))),
            }),
            ..Default::default()
        };
        let opened = cx.open_window(options, move |_, cx| cx.new(|_| GalleryView { gallery }));
        let handle = match opened {
            Ok(handle) => handle,
            Err(e) => {
                if screenshot.is_some() {
                    on_screenshot(Err(GalleryError::Window(e.to_string())));
                } else {
                    eprintln!("could not open the gallery window: {e}");
                }
                cx.quit();
                return;
            }
        };
        cx.activate(true);

        if let Some(path) = screenshot {
            cx.spawn(async move |cx| {
                cx.background_executor().timer(SETTLE).await;
                // `update_window` lends the window without leasing the root view, so
                // the frame can be drawn inside it.
                let outcome = cx
                    .update_window(handle.into(), |_, window, cx| capture(window, cx, &path))
                    .map_err(|e| GalleryError::Window(e.to_string()))
                    .and_then(|r| r)
                    .map(|()| path);
                on_screenshot(outcome);
                cx.update(|cx| cx.quit());
            })
            .detach();
        }
    });
}

/// Draws a fresh frame (so a hidden or occluded window still has one) and writes it to
/// `path` as a PNG at the display's pixel density.
#[cfg(feature = "screenshot")]
fn capture(window: &mut Window, cx: &mut App, path: &std::path::Path) -> Result<(), GalleryError> {
    let drawn = window.draw(cx);
    let image = window.render_to_image();
    drawn.clear(cx);
    let image = image.map_err(|e| GalleryError::Render(e.to_string()))?;
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir).map_err(|e| GalleryError::Save {
            path: path.to_owned(),
            reason: e.to_string(),
        })?;
    }
    image.save(path).map_err(|e| GalleryError::Save {
        path: path.to_owned(),
        reason: e.to_string(),
    })
}

#[cfg(not(feature = "screenshot"))]
fn capture(
    _window: &mut Window,
    _cx: &mut App,
    _path: &std::path::Path,
) -> Result<(), GalleryError> {
    Err(GalleryError::ScreenshotUnsupported)
}
