/// Failures reading local paths or the token store. Messages never contain the token.
#[derive(Debug, thiserror::Error)]
pub enum LocalError {
    #[error("HOME is not set; set ALMAHRYA_CONFIG and ALMAHRYA_DATA_DIR or HOME")]
    NoHome,
    #[error("{context}: {source}")]
    Io {
        context: String,
        #[source]
        source: std::io::Error,
    },
    #[error("token store does not hold a valid token: {0}")]
    InvalidStoredToken(String),
    #[error("could not generate a token: {0}")]
    Random(String),
    #[error("keychain error: {0}")]
    Keychain(String),
    #[cfg(not(target_os = "macos"))]
    #[error("no token store on this platform; set ALMAHRYA_TOKEN_FILE")]
    NoTokenStore,
}

impl LocalError {
    pub fn io(context: impl Into<String>, source: std::io::Error) -> LocalError {
        LocalError::Io {
            context: context.into(),
            source,
        }
    }
}
