//! Small filesystem helpers shared across the crate.

use camino::Utf8Path;

use crate::error::{Error, Result};

/// Atomically write `bytes` to `path` by writing a sibling temporary file and
/// renaming it into place (a rename is atomic on the same filesystem).
pub fn write_atomic(path: &Utf8Path, bytes: &[u8]) -> Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| Error::io(path, std::io::Error::other("path has no parent directory")))?;
    let file_name = path.file_name().unwrap_or("state");
    let tmp = parent.join(format!(".{file_name}.lucio-tmp-{}", std::process::id()));

    std::fs::write(tmp.as_std_path(), bytes).map_err(|e| Error::io(&tmp, e))?;
    std::fs::rename(tmp.as_std_path(), path.as_std_path()).map_err(|e| Error::io(path, e))?;
    Ok(())
}

/// Milliseconds since the Unix epoch, saturating to `0` before 1970.
pub fn unix_millis() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis())
}
