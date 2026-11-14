//! Where the API token lives: the macOS Keychain in normal use, a 0600 file when
//! `ALMAHRYA_TOKEN_FILE` is set (tests, development). The token is never printed.

use crate::error::DaemonError;
use almahrya_api::Token;
use std::io::{ErrorKind, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;

pub const TOKEN_FILE_ENV: &str = "ALMAHRYA_TOKEN_FILE";

/// A freshly generated token before it is stored. Debug output is masked.
pub struct Secret(String);

impl Secret {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(***)")
    }
}

/// 32 bytes from the OS RNG as 64 lowercase hex characters (URL-safe).
pub fn generate() -> Result<Secret, DaemonError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| DaemonError::Random(e.to_string()))?;
    let mut hex = String::with_capacity(64);
    for b in bytes {
        hex.push(char::from(HEX[usize::from(b >> 4)]));
        hex.push(char::from(HEX[usize::from(b & 0x0f)]));
    }
    Ok(Secret(hex))
}

const HEX: &[u8; 16] = b"0123456789abcdef";

fn to_token(value: &str) -> Result<Token, DaemonError> {
    Token::new(value).map_err(|e| DaemonError::InvalidStoredToken(e.to_string()))
}

pub trait TokenStore {
    /// Returns the stored token, generating and storing one on first use.
    fn get_or_create(&self) -> Result<Token, DaemonError>;
}

/// The file store when `ALMAHRYA_TOKEN_FILE` is set, else the Keychain.
pub fn select() -> Result<Box<dyn TokenStore>, DaemonError> {
    if let Some(path) = std::env::var_os(TOKEN_FILE_ENV).filter(|v| !v.is_empty()) {
        return Ok(Box::new(FileTokenStore {
            path: PathBuf::from(path),
        }));
    }
    keychain()
}

#[cfg(target_os = "macos")]
fn keychain() -> Result<Box<dyn TokenStore>, DaemonError> {
    Ok(Box::new(KeychainTokenStore))
}

#[cfg(not(target_os = "macos"))]
fn keychain() -> Result<Box<dyn TokenStore>, DaemonError> {
    Err(DaemonError::NoTokenStore)
}

/// Token in a file created with mode 0600. For tests and development only.
pub struct FileTokenStore {
    pub path: PathBuf,
}

impl TokenStore for FileTokenStore {
    fn get_or_create(&self) -> Result<Token, DaemonError> {
        let created = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&self.path);
        match created {
            Ok(mut file) => {
                let secret = generate()?;
                file.write_all(secret.expose().as_bytes())
                    .and_then(|()| file.write_all(b"\n"))
                    .map_err(|e| {
                        DaemonError::io(format!("could not write {}", self.path.display()), e)
                    })?;
                to_token(secret.expose())
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let text = std::fs::read_to_string(&self.path).map_err(|e| {
                    DaemonError::io(format!("could not read {}", self.path.display()), e)
                })?;
                to_token(&text)
            }
            Err(e) => Err(DaemonError::io(
                format!("could not create {}", self.path.display()),
                e,
            )),
        }
    }
}

/// Token in the login Keychain as a generic password.
#[cfg(target_os = "macos")]
pub struct KeychainTokenStore;

#[cfg(target_os = "macos")]
impl KeychainTokenStore {
    pub const SERVICE: &'static str = "almahrya";
    pub const ACCOUNT: &'static str = "api-token";
}

#[cfg(target_os = "macos")]
impl TokenStore for KeychainTokenStore {
    fn get_or_create(&self) -> Result<Token, DaemonError> {
        use security_framework::passwords::{get_generic_password, set_generic_password};
        /// `errSecItemNotFound`.
        const NOT_FOUND: i32 = -25300;

        match get_generic_password(Self::SERVICE, Self::ACCOUNT) {
            Ok(bytes) => {
                let text = String::from_utf8(bytes)
                    .map_err(|_| DaemonError::InvalidStoredToken("not UTF-8".to_owned()))?;
                to_token(&text)
            }
            Err(e) if e.code() == NOT_FOUND => {
                let secret = generate()?;
                set_generic_password(Self::SERVICE, Self::ACCOUNT, secret.expose().as_bytes())
                    .map_err(|e| DaemonError::Keychain(e.to_string()))?;
                to_token(secret.expose())
            }
            Err(e) => Err(DaemonError::Keychain(e.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::path::Path;

    /// Permission bits as `stat` prints them, e.g. "600".
    fn mode_of(path: &Path) -> String {
        let out = std::process::Command::new("stat")
            .args(["-f", "%Lp"])
            .arg(path)
            .output()
            .unwrap();
        assert!(out.status.success());
        String::from_utf8(out.stdout).unwrap().trim().to_owned()
    }

    #[test]
    fn generated_token_is_64_lowercase_hex_and_unique() {
        let a = generate().unwrap();
        let b = generate().unwrap();
        for s in [&a, &b] {
            assert_eq!(s.expose().len(), 64);
            assert!(
                s.expose()
                    .bytes()
                    .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            );
        }
        assert_ne!(a.expose(), b.expose());
        assert_eq!(format!("{a:?}"), "Secret(***)");
    }

    #[test]
    fn file_store_creates_0600_file_and_reuses_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        let store = FileTokenStore { path: path.clone() };
        let first = store.get_or_create().unwrap();
        assert_eq!(mode_of(&path), "600");
        let stored = std::fs::read_to_string(&path).unwrap();
        assert!(first.matches(stored.trim()));
        let second = store.get_or_create().unwrap();
        assert!(second.matches(stored.trim()));
        assert_eq!(format!("{first:?}"), "Token(***)");
    }

    #[test]
    fn empty_token_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        std::fs::write(&path, "  \n").unwrap();
        let store = FileTokenStore { path };
        assert!(matches!(
            store.get_or_create(),
            Err(DaemonError::InvalidStoredToken(_))
        ));
    }
}
