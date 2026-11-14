//! Where the API token lives: the macOS Keychain in normal use, a 0600 file when
//! `ALMAHRYA_TOKEN_FILE` is set (tests, development). The token is never printed.

use crate::LocalError;
use crate::profile::Profile;
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
pub fn generate() -> Result<Secret, LocalError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|e| LocalError::Random(e.to_string()))?;
    let mut hex = String::with_capacity(64);
    for b in bytes {
        hex.push(char::from(HEX[usize::from(b >> 4)]));
        hex.push(char::from(HEX[usize::from(b & 0x0f)]));
    }
    Ok(Secret(hex))
}

const HEX: &[u8; 16] = b"0123456789abcdef";

/// The stored API token: trimmed and never empty. Debug output is masked.
#[derive(Clone)]
pub struct ApiToken(String);

impl ApiToken {
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for ApiToken {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("ApiToken(***)")
    }
}

fn to_token(value: &str) -> Result<ApiToken, LocalError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(LocalError::InvalidStoredToken(
            "token must not be empty".to_owned(),
        ));
    }
    Ok(ApiToken(trimmed.to_owned()))
}

pub trait TokenStore {
    /// Returns the stored token, generating and storing one on first use.
    fn get_or_create(&self) -> Result<ApiToken, LocalError>;
}

/// The file store when `ALMAHRYA_TOKEN_FILE` is set, else the Keychain item of `profile`.
pub fn select(profile: Profile) -> Result<Box<dyn TokenStore>, LocalError> {
    if let Some(path) = std::env::var_os(TOKEN_FILE_ENV).filter(|v| !v.is_empty()) {
        return Ok(Box::new(FileTokenStore {
            path: PathBuf::from(path),
        }));
    }
    keychain(profile)
}

#[cfg(target_os = "macos")]
fn keychain(profile: Profile) -> Result<Box<dyn TokenStore>, LocalError> {
    Ok(Box::new(KeychainTokenStore::for_profile(profile)))
}

#[cfg(not(target_os = "macos"))]
fn keychain(_profile: Profile) -> Result<Box<dyn TokenStore>, LocalError> {
    Err(LocalError::NoTokenStore)
}

/// Token in a file created with mode 0600. For tests and development only.
pub struct FileTokenStore {
    pub path: PathBuf,
}

impl TokenStore for FileTokenStore {
    fn get_or_create(&self) -> Result<ApiToken, LocalError> {
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
                        LocalError::io(format!("could not write {}", self.path.display()), e)
                    })?;
                to_token(secret.expose())
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                let text = std::fs::read_to_string(&self.path).map_err(|e| {
                    LocalError::io(format!("could not read {}", self.path.display()), e)
                })?;
                to_token(&text)
            }
            Err(e) => Err(LocalError::io(
                format!("could not create {}", self.path.display()),
                e,
            )),
        }
    }
}

/// Token in the login Keychain as a generic password, one service per profile.
#[cfg(target_os = "macos")]
pub struct KeychainTokenStore {
    service: &'static str,
}

#[cfg(target_os = "macos")]
impl KeychainTokenStore {
    pub const ACCOUNT: &'static str = "api-token";

    /// The item of `profile`: service `almahrya`, or `almahrya-dev` for development.
    pub fn for_profile(profile: Profile) -> KeychainTokenStore {
        KeychainTokenStore {
            service: profile.keychain_service(),
        }
    }

    pub fn service(&self) -> &'static str {
        self.service
    }
}

#[cfg(target_os = "macos")]
impl TokenStore for KeychainTokenStore {
    fn get_or_create(&self) -> Result<ApiToken, LocalError> {
        use security_framework::passwords::{get_generic_password, set_generic_password};
        /// `errSecItemNotFound`.
        const NOT_FOUND: i32 = -25300;

        match get_generic_password(self.service, Self::ACCOUNT) {
            Ok(bytes) => {
                let text = String::from_utf8(bytes)
                    .map_err(|_| LocalError::InvalidStoredToken("not UTF-8".to_owned()))?;
                to_token(&text)
            }
            Err(e) if e.code() == NOT_FOUND => {
                let secret = generate()?;
                set_generic_password(self.service, Self::ACCOUNT, secret.expose().as_bytes())
                    .map_err(|e| LocalError::Keychain(e.to_string()))?;
                to_token(secret.expose())
            }
            Err(e) => Err(LocalError::Keychain(e.to_string())),
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
        assert_eq!(first.expose(), stored.trim());
        let second = store.get_or_create().unwrap();
        assert_eq!(second.expose(), stored.trim());
        assert_eq!(format!("{first:?}"), "ApiToken(***)");
    }

    #[test]
    fn empty_token_file_is_rejected() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        std::fs::write(&path, "  \n").unwrap();
        let store = FileTokenStore { path };
        assert!(matches!(
            store.get_or_create(),
            Err(LocalError::InvalidStoredToken(_))
        ));
    }
}
