//! Opening a profile in a *running* Vivaldi so it registers the profile live.
//!
//! Launching the Vivaldi binary with `--profile-directory=<dir>` forwards the
//! request to the already-running instance via Chromium's `ProcessSingleton`
//! ("Opening in existing browser session"). The running instance then loads the
//! on-disk profile and registers it in `Local State` itself — so the new profile
//! appears in the switcher with no restart, and Chromium keeps our pre-set
//! `profile.name` (its default-name code only runs when `profile.name` is unset).
//!
//! Note: on macOS this must invoke the binary directly, not `open -a`. `open`
//! only forwards `--args` when it *starts* the app; if Vivaldi is already
//! running, `open` just activates it and drops the arguments.

use std::process::{Command, Stdio};

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::{Error, Result};

/// Open `profile_dir` in the running Vivaldi via `--profile-directory`.
///
/// `user_data_dir_override` is passed only when the caller used a non-default
/// user-data directory (it must match the running instance, since the
/// `ProcessSingleton` key *is* the user-data directory).
///
/// The process is spawned detached: when it forwards to a running instance it
/// exits immediately; we do not wait so a stale-lock false positive can never
/// hang the caller.
///
/// # Errors
/// [`Error::VivaldiNotFound`] if the Vivaldi binary cannot be located, or
/// [`Error::LaunchFailed`] if spawning it fails.
pub fn open_profile(profile_dir: &str, user_data_dir_override: Option<&Utf8Path>) -> Result<()> {
    let mut command = base_command()?;
    command.arg(format!("--profile-directory={profile_dir}"));
    if let Some(dir) = user_data_dir_override {
        command.arg(format!("--user-data-dir={dir}"));
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|source| Error::LaunchFailed { source })?;
    Ok(())
}

/// Build the `Command` that runs the Vivaldi binary.
#[cfg(target_os = "macos")]
fn base_command() -> Result<Command> {
    let mut app_dirs = vec![Utf8PathBuf::from("/Applications/Vivaldi.app")];
    if let Some(home) = dirs::home_dir().and_then(|h| Utf8PathBuf::from_path_buf(h).ok()) {
        app_dirs.push(home.join("Applications").join("Vivaldi.app"));
    }

    let exe = app_dirs
        .iter()
        .filter(|dir| dir.exists())
        .find_map(|dir| macos_executable(dir))
        .ok_or(Error::VivaldiNotFound)?;
    Ok(Command::new(exe))
}

/// The main executable inside a macOS `.app` bundle (`Contents/MacOS/<exe>`).
#[cfg(target_os = "macos")]
fn macos_executable(app_dir: &Utf8Path) -> Option<Utf8PathBuf> {
    let macos = app_dir.join("Contents/MacOS");
    std::fs::read_dir(macos.as_std_path())
        .ok()?
        .filter_map(std::result::Result::ok)
        .find_map(|entry| {
            let path = entry.path();
            if path.is_file() {
                Utf8PathBuf::from_path_buf(path).ok()
            } else {
                None
            }
        })
}

#[cfg(target_os = "linux")]
fn base_command() -> Result<Command> {
    // Resolved via `PATH`; if absent, `spawn` fails and the caller falls back.
    Ok(Command::new("vivaldi"))
}

#[cfg(target_os = "windows")]
fn base_command() -> Result<Command> {
    let base = dirs::data_local_dir().ok_or(Error::VivaldiNotFound)?;
    let base = Utf8PathBuf::from_path_buf(base).map_err(Error::NonUtf8Path)?;
    Ok(Command::new(
        base.join("Vivaldi").join("Application").join("vivaldi.exe"),
    ))
}

#[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
fn base_command() -> Result<Command> {
    Err(Error::VivaldiNotFound)
}
