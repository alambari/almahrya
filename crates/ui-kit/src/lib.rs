//! The app's own components, wrapped around GPUI and gpui-component, so a GPUI
//! upgrade is fixed here instead of in every screen.
//!
//! Nothing public here mentions a GPUI type: callers hand in plain data and callbacks.

mod fonts;
mod sidebar;
mod theme;
mod window;

pub use sidebar::{ConnectionStatus, SessionItem};
pub use theme::Theme;
pub use window::{SessionsUpdate, WindowConfig, run_sessions_window};
