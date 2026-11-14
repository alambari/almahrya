//! Every request, response and event type of the API, with serde; TypeScript types are generated from this crate.

mod error;
mod event;
mod health;
mod session;

pub use error::ApiError;
pub use event::{EventDto, EventEnvelope};
pub use health::HealthDto;
pub use session::{CreateSessionRequest, SessionDto, SessionKindDto};

/// Writes the TypeScript bindings of every exported type into `dir`.
pub fn export_all_to(dir: &std::path::Path) -> Result<(), ts_rs::ExportError> {
    use ts_rs::TS;
    let cfg = ts_rs::Config::default().with_out_dir(dir);
    SessionDto::export_all(&cfg)?;
    SessionKindDto::export_all(&cfg)?;
    CreateSessionRequest::export_all(&cfg)?;
    EventEnvelope::export_all(&cfg)?;
    EventDto::export_all(&cfg)?;
    ApiError::export_all(&cfg)?;
    HealthDto::export_all(&cfg)?;
    Ok(())
}
