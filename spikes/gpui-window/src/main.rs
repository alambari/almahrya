//! Throwaway GPUI spike: one window with the dark design tokens from DESIGN.md.

use gpui::{
    App, Bounds, Context, IntoElement, ParentElement, Render, Styled, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size,
};
use gpui_component::{
    Root, Theme, ThemeMode,
    button::{Button, ButtonVariants},
};

const BG: u32 = 0x18181A;
const SIDEBAR: u32 = 0x1F1F22;
const TEXT: u32 = 0xECEAE4;
/// `agent` token: primary buttons.
const AGENT: u32 = 0x6F7CF2;

struct Spike;

impl Render for Spike {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(rgb(BG))
            .text_color(rgb(TEXT))
            .child(
                div()
                    .w(px(244.))
                    .h_full()
                    .bg(rgb(SIDEBAR))
                    .p(px(16.))
                    .font_family("Plus Jakarta Sans")
                    .text_size(px(13.))
                    .child("Sidebar"),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_1()
                    .h_full()
                    .p(px(24.))
                    .gap(px(12.))
                    .child(
                        div()
                            .font_family("Plus Jakarta Sans")
                            .text_size(px(13.))
                            .child("Plus Jakarta Sans 13 px: The quick brown fox jumps over the lazy dog"),
                    )
                    .child(
                        div()
                            .font_family("JetBrains Mono")
                            .text_size(px(13.))
                            .child("JetBrains Mono 13 px: fn main() { let x = 0x18181A; } // il1| O0"),
                    )
                    .child(
                        div().w(px(200.)).child(
                            Button::new("primary")
                                .primary()
                                .label("Open project")
                                .on_click(|_, _, _| println!("clicked")),
                        ),
                    ),
            )
    }
}

fn main() {
    gpui_platform::application().run(|cx: &mut App| {
        gpui_component::init(cx);
        // Dark mode first (it loads the registered dark theme), then map the
        // design tokens onto the component theme in a second update.
        Theme::change(ThemeMode::Dark, None, cx);
        Theme::update(cx, |theme| {
            theme.font_family = "Plus Jakarta Sans".into();
            theme.font_size = px(13.);
            theme.colors.background = rgb(BG).into();
            theme.colors.foreground = rgb(TEXT).into();
            theme.colors.primary = rgb(AGENT).into();
            theme.colors.primary_hover = rgb(AGENT).into();
            theme.colors.primary_active = rgb(AGENT).into();
            theme.colors.primary_foreground = rgb(0xFFFFFF).into();
            // Buttons read their own `button_primary*` slots, not `primary`.
            theme.colors.button_primary = rgb(AGENT).into();
            theme.colors.button_primary_hover = rgb(AGENT).into();
            theme.colors.button_primary_active = rgb(AGENT).into();
            theme.colors.button_primary_foreground = rgb(0xFFFFFF).into();
        });
        let bounds = Bounds::centered(None, size(px(1440.), px(900.)), cx);
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|_| Spike);
                cx.new(|cx| Root::new(view, window, cx))
            },
        )
        .expect("open window");
        cx.activate(true);
    });
}
