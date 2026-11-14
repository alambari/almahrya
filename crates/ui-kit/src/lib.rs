//! The app's own components, wrapped around GPUI and gpui-component, so a GPUI
//! upgrade is fixed here instead of in every screen.
//!
//! Nothing public here mentions a GPUI type: callers hand in plain data and callbacks.
//! Components live in `components/` and render themselves inside the crate;
//! `run_gallery` shows them all.

mod components;
mod fonts;
mod gallery;
mod theme;
mod window;

pub use components::{
    Chip, ConnectionStatus, DecisionCard, DecisionOption, DevRibbon, Label, LabelStyle, ListCard,
    ListRow, SessionItem, StatusBar, StatusKind, Toggle,
};
pub use gallery::{
    Gallery, GalleryColumn, GalleryError, GalleryItem, GalleryLane, GallerySection, run_gallery,
};
pub use theme::Theme;
pub use window::{SessionsUpdate, WindowConfig, run_sessions_window};
