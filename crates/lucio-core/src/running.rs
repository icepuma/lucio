//! A lightweight "is Vivaldi running?" check, used to time the `Local State`
//! write.
//!
//! A running Vivaldi keeps `Local State` in memory and rewrites it on flush/exit,
//! discarding any external edits — so a new profile must be registered while
//! Vivaldi is closed. We only need a yes/no signal, derived from Chromium's
//! process-singleton lock at the user-data root (no process inspection needed).

use camino::Utf8Path;

/// Whether Vivaldi appears to be running against `user_data_dir`.
///
/// On Unix this checks Chromium's `SingletonLock` symlink (created on launch,
/// removed on clean exit). On Windows it checks the `lockfile`. A stale lock left
/// by a crash reads as "running" until it is removed or Vivaldi is relaunched and
/// quit cleanly.
#[must_use]
pub fn is_running(user_data_dir: &Utf8Path) -> bool {
    #[cfg(unix)]
    {
        // `SingletonLock` is a symlink whose target intentionally does not exist,
        // so use `symlink_metadata` (which does not follow it) to test presence.
        std::fs::symlink_metadata(user_data_dir.join("SingletonLock").as_std_path()).is_ok()
    }
    #[cfg(windows)]
    {
        user_data_dir.join("lockfile").exists()
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = user_data_dir;
        false
    }
}
