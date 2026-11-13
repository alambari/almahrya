//! The running system: drives the sources, applies the rules, starts agent runs and
//! the Lark bot, and persists state.
//!
//! It runs on its own async runtime, separate from the GPUI executor, and talks to
//! the UI only through channels.
