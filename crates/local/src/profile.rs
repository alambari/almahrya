//! Which copy of Almahrya this process belongs to. The development profile runs beside
//! production on one Mac with its own data directory, TCP port and Keychain item, so a
//! rebuild loop never touches the data the owner works with.

use std::ffi::OsStr;

/// Selects the profile; the value `dev` means [`Profile::Dev`], anything else production.
pub const PROFILE_ENV: &str = "ALMAHRYA_PROFILE";

/// The one-line warning for an `ALMAHRYA_PROFILE` value that is neither `dev` nor
/// `production` (a typo such as `Dev` silently selects production otherwise), or `None`.
pub fn unknown_profile_warning(value: Option<&OsStr>) -> Option<String> {
    let value = value?;
    if value.is_empty() || value == "dev" || value == "production" {
        return None;
    }
    Some(format!(
        "warning: {PROFILE_ENV}={} is neither \"dev\" nor \"production\"; using the production profile",
        value.to_string_lossy()
    ))
}

/// Prints [`unknown_profile_warning`] for this process to stderr. The daemon and the app
/// call it once at start; [`Profile::from_env`] stays silent so tests can call it freely.
pub fn warn_unknown_profile() {
    if let Some(warning) = unknown_profile_warning(std::env::var_os(PROFILE_ENV).as_deref()) {
        eprintln!("{warning}");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    Production,
    Dev,
}

impl Profile {
    /// The profile named by `ALMAHRYA_PROFILE`. The daemon and the app both call this,
    /// so they always agree.
    pub fn from_env() -> Profile {
        Profile::from_value(std::env::var_os(PROFILE_ENV).as_deref())
    }

    /// `dev` selects the development profile; any other value, or none, is production.
    pub fn from_value(value: Option<&OsStr>) -> Profile {
        match value {
            Some(v) if v == "dev" => Profile::Dev,
            _ => Profile::Production,
        }
    }

    /// The `ALMAHRYA_PROFILE` value that selects this profile, for child processes.
    pub fn env_value(self) -> &'static str {
        match self {
            Profile::Production => "production",
            Profile::Dev => "dev",
        }
    }

    /// The data directory's name under `~/Library/Application Support`.
    pub fn data_dir_name(self) -> &'static str {
        match self {
            Profile::Production => "Almahrya",
            Profile::Dev => "Almahrya-Dev",
        }
    }

    /// The Keychain service that holds this profile's API token.
    pub fn keychain_service(self) -> &'static str {
        match self {
            Profile::Production => "almahrya",
            Profile::Dev => "almahrya-dev",
        }
    }

    /// The TCP port: `base` in production; in development `dev_override` (`[dev] api_port`)
    /// or else `base + 1`. `None` when `base + 1` does not fit in a port number.
    pub fn api_port(self, base: u16, dev_override: Option<u16>) -> Option<u16> {
        match self {
            Profile::Production => Some(base),
            Profile::Dev => dev_override.or_else(|| base.checked_add(1)),
        }
    }
}
