//! Notification sources behind one `Source` trait, each turning its own feed into
//! `almahrya_core` events.
//!
//! First sources: Lark (the `lark-cli event consume` NDJSON stream) and GitLab
//! (polling, since a desktop app cannot receive webhooks).
