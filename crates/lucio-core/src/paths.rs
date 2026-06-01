//! Locating the Vivaldi user-data directory across platforms.

use camino::Utf8PathBuf;

use crate::error::{Error, Result};

/// Convert a [`std::path::PathBuf`] (from the `dirs` crate) into a UTF-8 path.
fn utf8(path: std::path::PathBuf) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path).map_err(Error::NonUtf8Path)
}

/// Compute the platform default Vivaldi user-data directory.
///
/// - macOS: `~/Library/Application Support/Vivaldi`
/// - Linux: `~/.config/vivaldi` (honours `$XDG_CONFIG_HOME`)
/// - Windows: `%LOCALAPPDATA%\Vivaldi\User Data`
///
/// # Errors
/// Returns [`Error::UserDataDirNotFound`] if the platform base directory cannot
/// be determined (or the target OS is unsupported), or [`Error::NonUtf8Path`]
/// if that directory is not valid UTF-8.
pub fn default_user_data_dir() -> Result<Utf8PathBuf> {
    #[cfg(target_os = "macos")]
    {
        let base = utf8(dirs::config_dir().ok_or(Error::UserDataDirNotFound)?)?;
        Ok(base.join("Vivaldi"))
    }

    #[cfg(target_os = "linux")]
    {
        let base = utf8(dirs::config_dir().ok_or(Error::UserDataDirNotFound)?)?;
        Ok(base.join("vivaldi"))
    }

    #[cfg(target_os = "windows")]
    {
        let base = utf8(dirs::data_local_dir().ok_or(Error::UserDataDirNotFound)?)?;
        Ok(base.join("Vivaldi").join("User Data"))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    {
        Err(Error::UserDataDirNotFound)
    }
}

/// Resolve the user-data directory, preferring an explicit override over the
/// platform default, and verifying it exists.
///
/// # Errors
/// Propagates [`default_user_data_dir`] errors, and returns
/// [`Error::UserDataDirMissing`] if the resolved directory does not exist.
pub fn resolve_user_data_dir(explicit: Option<Utf8PathBuf>) -> Result<Utf8PathBuf> {
    let dir = match explicit {
        Some(dir) => dir,
        None => default_user_data_dir()?,
    };

    if dir.is_dir() {
        Ok(dir)
    } else {
        Err(Error::UserDataDirMissing(dir))
    }
}
