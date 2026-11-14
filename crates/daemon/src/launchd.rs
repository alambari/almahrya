//! `almahryad install-agent`: writes the per-user LaunchAgent plist and prints the
//! command that loads it. It never runs `launchctl` itself.

use crate::error::DaemonError;
use almahrya_local::paths;
use std::path::{Path, PathBuf};

pub const LABEL: &str = "dev.almahrya.daemon";

fn xml_escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

fn string(path: &Path) -> String {
    xml_escape(&path.to_string_lossy())
}

/// The plist: start at login, restart when it exits, logs in the data dir.
pub fn plist(exe: &Path, data_dir: &Path) -> String {
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
	<key>Label</key>
	<string>{LABEL}</string>
	<key>ProgramArguments</key>
	<array>
		<string>{exe}</string>
	</array>
	<key>RunAtLoad</key>
	<true/>
	<key>KeepAlive</key>
	<true/>
	<key>StandardOutPath</key>
	<string>{out}</string>
	<key>StandardErrorPath</key>
	<string>{err}</string>
</dict>
</plist>
"#,
        exe = string(exe),
        out = string(&paths::stdout_log(data_dir)),
        err = string(&paths::stderr_log(data_dir)),
    )
}

/// Writes `<plist_dir>/dev.almahrya.daemon.plist` (default `~/Library/LaunchAgents`)
/// and returns its path.
pub fn install(plist_dir: Option<PathBuf>, data_dir: &Path) -> Result<PathBuf, DaemonError> {
    let plist_dir = match plist_dir {
        Some(dir) => dir,
        None => paths::launch_agents_dir()?,
    };
    let exe = std::env::current_exe()
        .and_then(std::fs::canonicalize)
        .map_err(|e| DaemonError::io("could not locate the almahryad executable", e))?;
    // launchd does not create the log directory.
    paths::ensure_dir(data_dir)?;
    std::fs::create_dir_all(&plist_dir)
        .map_err(|e| DaemonError::io(format!("could not create {}", plist_dir.display()), e))?;
    let path = plist_dir.join(format!("{LABEL}.plist"));
    std::fs::write(&path, plist(&exe, data_dir))
        .map_err(|e| DaemonError::io(format!("could not write {}", path.display()), e))?;
    Ok(path)
}

/// The command the user runs to load the agent.
pub fn bootstrap_command(plist: &Path) -> String {
    format!("launchctl bootstrap gui/$(id -u) {}", plist.display())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plist_escapes_paths() {
        let text = plist(Path::new("/a&b/almahryad"), Path::new("/d<x>"));
        assert!(text.contains("<string>/a&amp;b/almahryad</string>"));
        assert!(text.contains("<string>/d&lt;x&gt;/almahryad.err.log</string>"));
    }
}
