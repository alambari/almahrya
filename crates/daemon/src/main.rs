//! `almahryad`: the background daemon. With no arguments it serves the API on a Unix
//! socket and on localhost; `install-agent` writes its LaunchAgent plist.
//! `ALMAHRYA_PROFILE=dev` runs the development profile beside production.

mod config;
mod error;
mod launchd;
mod lock;
mod serve;

use crate::config::Config;
use crate::error::DaemonError;
use crate::lock::InstanceLock;
use almahrya_engine::Engine;
use almahrya_local::paths::{self, Paths};
use almahrya_local::profile::{self, Profile};
use almahrya_local::{LocalError, token_store};
use almahrya_store::Store;
use std::ffi::OsString;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

#[derive(Debug, PartialEq, Eq)]
enum Command {
    Serve,
    InstallAgent { plist_dir: Option<PathBuf> },
}

fn parse(args: impl IntoIterator<Item = OsString>) -> Result<Command, DaemonError> {
    let mut args = args.into_iter();
    let Some(first) = args.next() else {
        return Ok(Command::Serve);
    };
    if first != "install-agent" {
        return Err(DaemonError::Usage(format!(
            "unknown argument: {}",
            first.to_string_lossy()
        )));
    }
    let mut plist_dir = None;
    while let Some(arg) = args.next() {
        if arg != "--plist-dir" {
            return Err(DaemonError::Usage(format!(
                "unknown argument: {}",
                arg.to_string_lossy()
            )));
        }
        let dir = args
            .next()
            .ok_or_else(|| DaemonError::Usage("--plist-dir needs a directory".to_owned()))?;
        plist_dir = Some(PathBuf::from(dir));
    }
    Ok(Command::InstallAgent { plist_dir })
}

fn serve(paths: &Paths) -> Result<(), DaemonError> {
    let config = Config::load(paths)?;
    let socket = paths::socket(&config.data_dir);
    serve::check_socket_path(&socket)?;
    paths::ensure_dir(&config.data_dir)?;

    let _lock = InstanceLock::acquire(&paths::lock(&config.data_dir))?;
    // Only now is a leftover socket known to be stale.
    paths::remove_stale_socket(&socket)?;

    let stored = token_store::select(paths.profile)?.get_or_create()?;
    let token = almahrya_api::Token::new(stored.expose())
        .map_err(|e| LocalError::InvalidStoredToken(e.to_string()))?;
    let store = Store::open(&paths::database(&config.data_dir))?;

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(|e| DaemonError::io("could not start the async runtime", e))?;
    runtime.block_on(async {
        let engine = Arc::new(Engine::new(store, config.event_retention));
        let router = almahrya_api::router(engine, token);
        let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, config.api_port));
        serve::run(router, socket, addr).await
    })
}

fn install_agent(paths: &Paths, plist_dir: Option<PathBuf>) -> Result<(), DaemonError> {
    // One agent label exists, and it belongs to production; dev.sh runs the dev daemon.
    if paths.profile == Profile::Dev {
        return Err(DaemonError::NoDevAgent);
    }
    let plist = launchd::install(plist_dir, &paths.data_dir)?;
    println!("Wrote {}", plist.display());
    println!("Load it with:");
    println!("  {}", launchd::bootstrap_command(&plist));
    Ok(())
}

fn main() -> ExitCode {
    profile::warn_unknown_profile();
    let result = parse(std::env::args_os().skip(1)).and_then(|command| {
        let paths = Paths::for_profile(Profile::from_env())?;
        match command {
            Command::Serve => serve(&paths),
            Command::InstallAgent { plist_dir } => install_agent(&paths, plist_dir),
        }
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(e.exit_code())
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;

    fn args(list: &[&str]) -> Vec<OsString> {
        list.iter().map(OsString::from).collect()
    }

    #[test]
    fn parses_commands() {
        assert_eq!(parse(args(&[])).unwrap(), Command::Serve);
        assert_eq!(
            parse(args(&["install-agent"])).unwrap(),
            Command::InstallAgent { plist_dir: None }
        );
        assert_eq!(
            parse(args(&["install-agent", "--plist-dir", "/x"])).unwrap(),
            Command::InstallAgent {
                plist_dir: Some(PathBuf::from("/x"))
            }
        );
        let err = parse(args(&["nope"])).unwrap_err();
        assert_eq!(err.exit_code(), 2);
        assert!(parse(args(&["install-agent", "--plist-dir"])).is_err());
    }
}
