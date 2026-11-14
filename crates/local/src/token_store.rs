//! Where the API token lives: the macOS Keychain in normal use, a 0600 file when
//! `ALMAHRYA_TOKEN_FILE` is set (tests, and `scripts/dev.sh` for the development
//! profile). The daemon creates the token on its first start; the desktop app only reads
//! it. The token is never printed.

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
    /// The stored token, or `None` while none exists. Never creates one: the desktop app
    /// reads with this and waits for the daemon to create the token on its first start.
    fn get(&self) -> Result<Option<ApiToken>, LocalError>;

    /// Returns the stored token, generating and storing one on first use. Only the
    /// daemon calls this, while it holds the instance lock.
    fn get_or_create(&self) -> Result<ApiToken, LocalError>;
}

/// What an add-only write found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Added {
    Created,
    /// Another process stored a token first; the existing one was left alone.
    AlreadyExists,
}

/// Read, else create, on top of a backend whose `add` never overwrites. When another
/// process wins the race between the read and the add, its token is read and returned,
/// so every process ends up with the one stored value.
pub fn get_or_create_with(
    get: impl Fn() -> Result<Option<ApiToken>, LocalError>,
    add: impl Fn(&str) -> Result<Added, LocalError>,
) -> Result<ApiToken, LocalError> {
    if let Some(token) = get()? {
        return Ok(token);
    }
    let secret = generate()?;
    match add(secret.expose())? {
        Added::Created => to_token(secret.expose()),
        Added::AlreadyExists => get()?.ok_or_else(|| {
            LocalError::InvalidStoredToken(
                "another process created the token, but it could not be read".to_owned(),
            )
        }),
    }
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

/// Token in a file with mode 0600. For tests and `scripts/dev.sh`, which keeps the
/// development token in the development data directory so it never touches the Keychain.
pub struct FileTokenStore {
    pub path: PathBuf,
}

impl FileTokenStore {
    /// Writes the token to a private temporary file, then hard-links it into place. The
    /// link fails when the file exists, so a reader never sees a half-written token and
    /// a concurrent writer never replaces the first one.
    fn add(&self, secret: &str) -> Result<Added, LocalError> {
        let name = self.path.file_name().ok_or_else(|| {
            LocalError::io(
                format!("{} is not a file path", self.path.display()),
                std::io::Error::from(ErrorKind::InvalidInput),
            )
        })?;
        let mut tmp_name = std::ffi::OsString::from(".");
        tmp_name.push(name);
        tmp_name.push(format!(".{}.tmp", std::process::id()));
        let tmp = self.path.with_file_name(tmp_name);
        let written = write_new(&tmp, secret);
        let linked = written.and_then(|()| std::fs::hard_link(&tmp, &self.path));
        let _ = std::fs::remove_file(&tmp);
        match linked {
            Ok(()) => Ok(Added::Created),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(Added::AlreadyExists),
            Err(e) => Err(LocalError::io(
                format!("could not create {}", self.path.display()),
                e,
            )),
        }
    }
}

/// Creates `path` with mode 0600 and writes `secret` and a newline to it.
fn write_new(path: &std::path::Path, secret: &str) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(secret.as_bytes())?;
    file.write_all(b"\n")?;
    file.sync_all()
}

impl TokenStore for FileTokenStore {
    fn get(&self) -> Result<Option<ApiToken>, LocalError> {
        match std::fs::read_to_string(&self.path) {
            Ok(text) => to_token(&text).map(Some),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(None),
            Err(e) => Err(LocalError::io(
                format!("could not read {}", self.path.display()),
                e,
            )),
        }
    }

    fn get_or_create(&self) -> Result<ApiToken, LocalError> {
        get_or_create_with(|| self.get(), |secret| self.add(secret))
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

    /// `SecItemAdd` only. `set_generic_password` would update an existing item and so
    /// replace a token another process just stored; this reports the duplicate instead.
    fn add(&self, secret: &str) -> Result<Added, LocalError> {
        use core_foundation::data::CFData;
        use security_framework::item::{ItemAddOptions, ItemAddValue, ItemClass};
        /// `errSecDuplicateItem`.
        const DUPLICATE: i32 = -25299;

        let mut options = ItemAddOptions::new(ItemAddValue::Data {
            class: ItemClass::generic_password(),
            data: CFData::from_buffer(secret.as_bytes()),
        });
        options
            .set_service(self.service)
            .set_account_name(Self::ACCOUNT);
        match options.add() {
            Ok(()) => Ok(Added::Created),
            Err(e) if e.code() == DUPLICATE => Ok(Added::AlreadyExists),
            Err(e) => Err(LocalError::Keychain(e.to_string())),
        }
    }
}

#[cfg(target_os = "macos")]
impl TokenStore for KeychainTokenStore {
    fn get(&self) -> Result<Option<ApiToken>, LocalError> {
        use security_framework::passwords::get_generic_password;
        /// `errSecItemNotFound`.
        const NOT_FOUND: i32 = -25300;

        match get_generic_password(self.service, Self::ACCOUNT) {
            Ok(bytes) => {
                let text = String::from_utf8(bytes)
                    .map_err(|_| LocalError::InvalidStoredToken("not UTF-8".to_owned()))?;
                to_token(&text).map(Some)
            }
            Err(e) if e.code() == NOT_FOUND => Ok(None),
            Err(e) => Err(LocalError::Keychain(e.to_string())),
        }
    }

    fn get_or_create(&self) -> Result<ApiToken, LocalError> {
        get_or_create_with(|| self.get(), |secret| self.add(secret))
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
    fn get_never_creates_a_token() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        let store = FileTokenStore { path: path.clone() };
        assert!(store.get().unwrap().is_none());
        assert!(!path.exists());
        let created = store.get_or_create().unwrap();
        assert_eq!(store.get().unwrap().unwrap().expose(), created.expose());
    }

    #[test]
    fn file_store_keeps_the_token_another_process_stored_first() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("token");
        let store = FileTokenStore { path: path.clone() };
        assert_eq!(store.add("winner").unwrap(), Added::Created);
        assert_eq!(store.add("loser").unwrap(), Added::AlreadyExists);
        assert_eq!(store.get().unwrap().unwrap().expose(), "winner");
        assert_eq!(mode_of(&path), "600");
        // No temporary file is left next to the token.
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().collect();
        assert_eq!(names.len(), 1);
    }

    #[test]
    fn losing_the_create_race_returns_the_stored_token() {
        use std::cell::RefCell;
        // A fake backend that is empty at our first read; another process stores its
        // token before our add, so the add finds a duplicate and stores nothing.
        let stored: RefCell<Option<String>> = RefCell::new(None);
        let get = || Ok(stored.borrow().as_deref().map(|t| to_token(t).unwrap()));
        let add = |_: &str| {
            *stored.borrow_mut() = Some("winner".to_owned());
            Ok(Added::AlreadyExists)
        };
        let token = get_or_create_with(get, add).unwrap();
        assert_eq!(token.expose(), "winner");
    }

    #[test]
    fn winning_the_create_race_returns_the_new_token() {
        let token = get_or_create_with(|| Ok(None), |_| Ok(Added::Created)).unwrap();
        assert_eq!(token.expose().len(), 64);
        let existing =
            get_or_create_with(|| Ok(Some(to_token("kept").unwrap())), |_| unreachable!()).unwrap();
        assert_eq!(existing.expose(), "kept");
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
        assert!(matches!(
            store.get(),
            Err(LocalError::InvalidStoredToken(_))
        ));
    }
}
