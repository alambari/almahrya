//! Domain model and decisions: the normalized `Event` every source produces, the rules
//! that route an event, and the policy that says what an agent run may do.
//!
//! Pure logic only: no IO, no async runtime, no GPUI. Everything here is testable
//! without a network, a database or a window.

mod session;

pub use session::{Session, SessionId, SessionKind};
