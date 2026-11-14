#![allow(clippy::unwrap_used)]

//! The development profile keeps its data, port and Keychain item apart from production.

use almahrya_local::paths::Paths;
use almahrya_local::profile::Profile;
use std::collections::HashMap;
use std::ffi::OsString;
use std::path::PathBuf;

/// An environment holding only `vars`.
fn env(vars: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
    let map: HashMap<String, OsString> = vars
        .iter()
        .map(|(k, v)| ((*k).to_owned(), OsString::from(v)))
        .collect();
    move |key| map.get(key).cloned()
}

#[test]
fn dev_profile_uses_separate_data_dir_port_and_keychain_service() {
    let home = env(&[("HOME", "/Users/someone")]);
    let prod = Paths::for_profile_in(Profile::Production, &home).unwrap();
    let dev = Paths::for_profile_in(Profile::Dev, &home).unwrap();

    assert_eq!(
        prod.data_dir,
        PathBuf::from("/Users/someone/Library/Application Support/Almahrya")
    );
    assert_eq!(
        dev.data_dir,
        PathBuf::from("/Users/someone/Library/Application Support/Almahrya-Dev")
    );
    assert_ne!(prod.socket(), dev.socket());
    // One config file; the `[dev]` table inside it holds the differences.
    assert_eq!(prod.config_file, dev.config_file);
    assert_eq!(prod.profile, Profile::Production);
    assert_eq!(dev.profile, Profile::Dev);

    assert_eq!(Profile::Production.keychain_service(), "almahrya");
    assert_eq!(Profile::Dev.keychain_service(), "almahrya-dev");

    assert_eq!(Profile::Production.api_port(47821, None), Some(47821));
    assert_eq!(Profile::Dev.api_port(47821, None), Some(47822));
}

#[test]
fn dev_api_port_override_wins_and_production_ignores_it() {
    assert_eq!(Profile::Dev.api_port(47821, Some(50000)), Some(50000));
    assert_eq!(
        Profile::Production.api_port(47821, Some(50000)),
        Some(47821)
    );
    assert_eq!(Profile::Dev.api_port(u16::MAX, None), None);
}

#[test]
fn data_dir_override_applies_to_both_profiles() {
    let vars = env(&[
        ("HOME", "/Users/someone"),
        ("ALMAHRYA_DATA_DIR", "/x/data"),
        ("ALMAHRYA_CONFIG", "/x/config.toml"),
    ]);
    for profile in [Profile::Production, Profile::Dev] {
        let paths = Paths::for_profile_in(profile, &vars).unwrap();
        assert_eq!(paths.data_dir, PathBuf::from("/x/data"));
        assert_eq!(paths.config_file, PathBuf::from("/x/config.toml"));
    }
}

#[test]
fn profile_env_other_than_dev_is_production() {
    let of = |value: Option<&str>| Profile::from_value(value.map(OsString::from).as_deref());
    assert_eq!(of(Some("dev")), Profile::Dev);
    assert_eq!(of(Some("staging")), Profile::Production);
    assert_eq!(of(Some("production")), Profile::Production);
    assert_eq!(of(Some("DEV")), Profile::Production);
    assert_eq!(of(Some("")), Profile::Production);
    assert_eq!(of(None), Profile::Production);
    assert_eq!(Profile::Dev.env_value(), "dev");
    assert_eq!(
        of(Some(Profile::Production.env_value())),
        Profile::Production
    );
}

#[test]
fn unknown_profile_values_get_a_warning() {
    use almahrya_local::profile::unknown_profile_warning;
    use std::ffi::OsStr;
    for quiet in [None, Some(""), Some("dev"), Some("production")] {
        assert_eq!(
            unknown_profile_warning(quiet.map(OsStr::new)),
            None,
            "{quiet:?}"
        );
    }
    for typo in ["Dev", "dev ", "staging"] {
        let warning = unknown_profile_warning(Some(OsStr::new(typo))).unwrap();
        assert!(
            warning.contains(&format!("ALMAHRYA_PROFILE={typo}")),
            "{warning}"
        );
        assert!(warning.contains("production profile"), "{warning}");
        assert!(!warning.contains('\n'));
        // The value still selects production; the warning changes nothing.
        assert_eq!(
            Profile::from_value(Some(OsStr::new(typo))),
            Profile::Production
        );
    }
}
