//! The profile-clone engine: allowlist copy + `Preferences` sanitization.
//!
//! The copy is designed to be safe even while Vivaldi is running, because it
//! only ever *reads* the source profile (the source is never modified):
//!
//! - `Preferences` / `Secure Preferences` are written atomically by Chromium
//!   (temp file + rename), so a whole-file copy always sees a consistent
//!   version.
//! - `LevelDB` stores (extension settings/state) are copied with the volatile
//!   `LOCK`/`LOG` files skipped and `CURRENT` written last (so it never points
//!   at a manifest we haven't copied yet); files that vanish mid-copy (e.g. a
//!   compaction) are skipped rather than treated as errors. `LevelDB` recovers on
//!   open, and any inconsistency only affects the disposable clone.

use camino::{Utf8Path, Utf8PathBuf};
use indicatif::{ProgressBar, ProgressStyle};
use serde_json::{Map, Value};
use walkdir::WalkDir;

use crate::error::{Error, Result};
use crate::manifest::COPY_ALLOWLIST;
use crate::util;

/// Per-directory files that are never copied: the `LevelDB` process lock and the
/// human-readable logs. They are unnecessary and copying a held `LOCK` is
/// pointless.
const SKIP_BASENAMES: &[&str] = &["LOCK", "LOG", "LOG.old"];

/// The `LevelDB` pointer file, copied last so it never references a manifest that
/// has not been copied yet.
const CURRENT_FILE: &str = "CURRENT";

/// What a clone copied (or, for a dry run, what it *would* copy).
#[derive(Debug, Default)]
pub struct CloneReport {
    /// Top-level allowlist entries that were present in the source profile.
    pub items: Vec<String>,
    /// Number of files copied.
    pub files: usize,
    /// Number of files skipped because they vanished mid-copy (live source).
    pub skipped: usize,
    /// Total bytes copied.
    pub bytes: u64,
}

/// Copy the allowlisted settings/extension files from `src_dir` to `dst_dir`.
///
/// Only entries in [`COPY_ALLOWLIST`] are copied, so personal data in the source
/// profile is never duplicated. When `dry_run` is set, the plan is computed and
/// returned but nothing is written.
///
/// # Errors
/// [`Error::SourceProfileMissing`] if `src_dir` is not a directory,
/// [`Error::ProfileDirExists`] if `dst_dir` already exists, or [`Error::Io`] /
/// [`Error::NonUtf8Path`] on filesystem problems.
pub fn copy_template(src_dir: &Utf8Path, dst_dir: &Utf8Path, dry_run: bool) -> Result<CloneReport> {
    if !src_dir.is_dir() {
        return Err(Error::SourceProfileMissing(src_dir.to_owned()));
    }
    if dst_dir.exists() {
        return Err(Error::ProfileDirExists(dst_dir.to_owned()));
    }

    let mut report = CloneReport::default();
    let mut plan = build_plan(src_dir, dst_dir, &mut report)?;

    // Copy CURRENT pointer files last for snapshot-consistent LevelDB copies.
    plan.sort_by_key(|(src, _)| src.file_name() == Some(CURRENT_FILE));

    if dry_run {
        report.files = plan.len();
        return Ok(report);
    }

    let bar = ProgressBar::new(u64::try_from(plan.len()).unwrap_or(u64::MAX));
    bar.set_style(
        ProgressStyle::with_template("{spinner:.green} copied {pos}/{len} files")
            .unwrap_or_else(|_| ProgressStyle::default_bar()),
    );

    std::fs::create_dir_all(dst_dir.as_std_path()).map_err(|e| Error::io(dst_dir, e))?;
    for (src_file, dst_file) in &plan {
        if let Some(parent) = dst_file.parent() {
            std::fs::create_dir_all(parent.as_std_path()).map_err(|e| Error::io(parent, e))?;
        }
        match std::fs::copy(src_file.as_std_path(), dst_file.as_std_path()) {
            Ok(bytes) => {
                report.bytes += bytes;
                report.files += 1;
            }
            // The file vanished between planning and copying (e.g. a LevelDB
            // compaction in a live source). Skip it rather than aborting.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                report.skipped += 1;
                tracing::debug!(file = %src_file, "source file vanished mid-copy; skipping");
            }
            Err(e) => return Err(Error::io(src_file, e)),
        }
        bar.inc(1);
    }
    bar.finish_and_clear();

    Ok(report)
}

/// Walk the allowlist and build the `(src_file, dst_file)` copy plan.
fn build_plan(
    src_dir: &Utf8Path,
    dst_dir: &Utf8Path,
    report: &mut CloneReport,
) -> Result<Vec<(Utf8PathBuf, Utf8PathBuf)>> {
    let mut plan = Vec::new();
    for entry in COPY_ALLOWLIST {
        let src = src_dir.join(entry);
        if !src.exists() {
            continue;
        }
        report.items.push((*entry).to_owned());

        if !src.is_dir() {
            plan.push((src.clone(), dst_dir.join(entry)));
            continue;
        }

        for walked in WalkDir::new(src.as_std_path()) {
            let walked = walked.map_err(|e| walk_error(&src, e))?;
            if !walked.file_type().is_file() {
                continue;
            }
            let abs = Utf8Path::from_path(walked.path())
                .ok_or_else(|| Error::NonUtf8Path(walked.path().to_path_buf()))?;
            if abs.file_name().is_some_and(|n| SKIP_BASENAMES.contains(&n)) {
                continue;
            }
            let rel = abs
                .strip_prefix(src_dir)
                .map_err(|_| Error::io(abs, std::io::Error::other("walked path escaped source")))?;
            plan.push((abs.to_owned(), dst_dir.join(rel)));
        }
    }
    Ok(plan)
}

/// Set the clone's display name and strip sign-in/account identity from its
/// copied `Preferences`, so it starts signed out. A no-op if `Preferences` is
/// absent.
///
/// # Errors
/// [`Error::Io`] or [`Error::Json`] on read/parse/write failure.
pub fn sanitize_preferences(profile_dir: &Utf8Path, new_name: &str) -> Result<()> {
    edit_preferences(profile_dir, |obj| {
        obj.remove("account_info");
        if let Some(signin) = obj.get_mut("signin").and_then(Value::as_object_mut) {
            signin.clear();
        }
        set_profile_name(obj, new_name);
    })
}

/// Read `<profile_dir>/Preferences`, apply `edit`, and atomically write it back.
fn edit_preferences(
    profile_dir: &Utf8Path,
    edit: impl FnOnce(&mut Map<String, Value>),
) -> Result<()> {
    let path = profile_dir.join("Preferences");
    if !path.is_file() {
        return Ok(());
    }
    let bytes = std::fs::read(path.as_std_path()).map_err(|e| Error::io(&path, e))?;
    let mut root: Value = serde_json::from_slice(&bytes).map_err(|e| Error::json(&path, e))?;
    if let Some(obj) = root.as_object_mut() {
        edit(obj);
    }
    let serialized = serde_json::to_vec(&root).map_err(|e| Error::json(&path, e))?;
    util::write_atomic(&path, &serialized)
}

/// Set `profile.name` and clear the default-name flag in a `Preferences` object.
fn set_profile_name(obj: &mut Map<String, Value>, new_name: &str) {
    let profile = obj
        .entry("profile".to_owned())
        .or_insert_with(|| Value::Object(Map::new()));
    if let Some(profile) = profile.as_object_mut() {
        profile.insert("name".to_owned(), Value::String(new_name.to_owned()));
        profile.insert("is_using_default_name".to_owned(), Value::Bool(false));
    }
}

/// Convert a [`walkdir::Error`] into our path-annotated I/O error.
fn walk_error(path: &Utf8Path, err: walkdir::Error) -> Error {
    let io = err
        .into_io_error()
        .unwrap_or_else(|| std::io::Error::other("directory walk failed"));
    Error::io(path, io)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn copies_only_allowlisted_entries_and_skips_personal_data() {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(tmp.path()).unwrap();
        let src = root.join("Default");
        std::fs::create_dir_all(src.as_std_path()).unwrap();

        // Allowlisted settings/extension data.
        std::fs::write(src.join("Preferences").as_std_path(), b"{}").unwrap();
        std::fs::write(src.join("Secure Preferences").as_std_path(), b"{}").unwrap();
        std::fs::create_dir_all(src.join("Extensions/abc").as_std_path()).unwrap();
        std::fs::write(
            src.join("Extensions/abc/manifest.json").as_std_path(),
            b"{}",
        )
        .unwrap();
        std::fs::create_dir_all(src.join("Local Extension Settings").as_std_path()).unwrap();
        std::fs::write(
            src.join("Local Extension Settings/000003.log")
                .as_std_path(),
            b"opts",
        )
        .unwrap();
        // A LevelDB lock that must be skipped.
        std::fs::write(src.join("Local Extension Settings/LOCK").as_std_path(), b"").unwrap();

        // Personal data that must NOT be copied.
        std::fs::write(src.join("Cookies").as_std_path(), b"secret").unwrap();
        std::fs::write(src.join("History").as_std_path(), b"history").unwrap();
        std::fs::write(src.join("Bookmarks").as_std_path(), b"bm").unwrap();
        std::fs::write(src.join("Login Data").as_std_path(), b"pw").unwrap();

        let dst = root.join("Profile 1");
        let report = copy_template(&src, &dst, false).unwrap();

        assert!(dst.join("Preferences").exists());
        assert!(dst.join("Secure Preferences").exists());
        assert!(dst.join("Extensions/abc/manifest.json").exists());
        assert!(dst.join("Local Extension Settings/000003.log").exists());
        // The LevelDB LOCK was skipped.
        assert!(!dst.join("Local Extension Settings/LOCK").exists());

        assert!(!dst.join("Cookies").exists());
        assert!(!dst.join("History").exists());
        assert!(!dst.join("Bookmarks").exists());
        assert!(!dst.join("Login Data").exists());

        assert!(report.items.contains(&"Preferences".to_owned()));
        assert!(report.files >= 4);
    }

    #[test]
    fn refuses_existing_destination() {
        let tmp = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(tmp.path()).unwrap();
        let src = root.join("Default");
        let dst = root.join("Profile 1");
        std::fs::create_dir_all(src.as_std_path()).unwrap();
        std::fs::create_dir_all(dst.as_std_path()).unwrap();
        assert!(matches!(
            copy_template(&src, &dst, false).unwrap_err(),
            Error::ProfileDirExists(_)
        ));
    }

    #[test]
    fn sanitizes_name_and_clears_account() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = Utf8Path::from_path(tmp.path()).unwrap();
        std::fs::write(
            dir.join("Preferences").as_std_path(),
            br#"{"profile":{"name":"Old"},"account_info":[{"email":"x@y.z"}]}"#,
        )
        .unwrap();

        sanitize_preferences(dir, "Work").unwrap();

        let bytes = std::fs::read(dir.join("Preferences").as_std_path()).unwrap();
        let root: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(root["profile"]["name"], "Work");
        assert_eq!(root["profile"]["is_using_default_name"], false);
        assert!(root.get("account_info").is_none());
    }
}
