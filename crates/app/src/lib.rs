//! The desktop app's logic without any GPUI: finding or starting the daemon and keeping
//! the session list in step with the API. `main.rs` hands the results to `ui-kit`.

pub mod daemon_link;
pub mod sessions_view;
