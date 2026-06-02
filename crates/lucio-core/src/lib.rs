//! `lucio-core` — create new Vivaldi profiles based on existing ones, as
//! isolated settings/extensions *templates*.
//!
//! A clone inherits the source profile's Vivaldi settings (themes, keyboard
//! shortcuts, mouse gestures, search engines) and its installed extensions plus
//! their options, but is fully isolated from the source's personal data:
//! cookies, saved passwords, history, autofill, open sessions, bookmarks, site
//! storage and Vivaldi mail/calendar are never copied.
//!
//! The work splits into focused modules:
//! - [`paths`] resolves the per-platform user-data directory.
//! - [`local_state`] reads and edits the `Local State` profile registry.
//! - [`clone`] copies the allowlisted files and sanitizes `Preferences`.
//! - [`manifest`] defines exactly which files are copied.
//!
//! The high-level entry point is [`Vivaldi`].
//!
//! Only the source profile is read; it is never modified, so the file copy is
//! safe even while Vivaldi is open. A running Vivaldi keeps `Local State` in
//! memory, so a new profile is registered either by opening it in the running
//! instance ([`Vivaldi::open_profile_live`], no restart) or, when Vivaldi is
//! closed, by writing `Local State` directly ([`Vivaldi::commit_registration`]).
//! See also [`Vivaldi::stage_clone`].

pub mod clone;
pub mod error;
pub mod launch;
pub mod local_state;
pub mod manifest;
pub mod paths;
pub mod profile;
pub mod running;
mod util;

use camino::{Utf8Path, Utf8PathBuf};

pub use crate::clone::CloneReport;
pub use crate::error::{Error, Result};
pub use crate::profile::ProfileInfo;

use crate::local_state::{LocalState, NewProfile};

/// Options controlling a profile clone.
#[derive(Debug, Clone)]
pub struct CloneOptions {
    /// Source profile, by display name (case-insensitive) or directory name.
    pub source: String,
    /// Display name for the new profile.
    pub new_name: String,
    /// Category ids (see [`manifest::CATEGORIES`]) to carry over. Empty means the
    /// default isolated-template set ([`manifest::default_category_ids`]).
    pub categories: Vec<String>,
    /// Plan the copy and report it without writing anything.
    pub dry_run: bool,
}

/// The result of a clone.
#[derive(Debug)]
pub struct CloneOutcome {
    /// The new profile's on-disk directory name (e.g. `Profile 4`).
    pub new_dir: String,
    /// The new profile's display name.
    pub new_name: String,
    /// The unique metrics bucket index allocated to the new profile.
    pub metrics_bucket_index: i64,
    /// Whether this was a dry run (nothing written).
    pub dry_run: bool,
    /// What was (or, for a dry run, would be) copied.
    pub report: CloneReport,
}

/// A clone whose files have been copied but not yet registered in `Local State`.
///
/// Registration is deferred so it can be written while Vivaldi is closed (a
/// running Vivaldi would overwrite it). See [`Vivaldi::commit_registration`].
#[derive(Debug)]
pub struct StagedClone {
    /// The new profile's on-disk directory name (e.g. `Profile 4`).
    pub new_dir: String,
    /// The new profile's display name.
    pub new_name: String,
    /// What was copied.
    pub report: CloneReport,
    avatar_icon: Option<String>,
    is_using_default_avatar: bool,
}

/// A located Vivaldi installation, identified by its user-data directory.
#[derive(Debug)]
pub struct Vivaldi {
    user_data_dir: Utf8PathBuf,
    explicit_dir: bool,
}

impl Vivaldi {
    /// Locate the Vivaldi user-data directory, preferring an explicit override
    /// over the per-platform default.
    ///
    /// # Errors
    /// Propagates [`paths::resolve_user_data_dir`] errors (directory not found
    /// or missing).
    pub fn locate(explicit: Option<Utf8PathBuf>) -> Result<Self> {
        let explicit_dir = explicit.is_some();
        let user_data_dir = paths::resolve_user_data_dir(explicit)?;
        Ok(Self {
            user_data_dir,
            explicit_dir,
        })
    }

    /// The resolved user-data directory.
    #[must_use]
    pub fn user_data_dir(&self) -> &Utf8Path {
        &self.user_data_dir
    }

    /// List all registered profiles, in `info_cache` order.
    ///
    /// # Errors
    /// Propagates [`LocalState::load`] errors.
    pub fn list_profiles(&self) -> Result<Vec<ProfileInfo>> {
        Ok(LocalState::load(&self.user_data_dir)?.profiles())
    }

    /// Whether Vivaldi currently appears to be running against this user-data
    /// directory.
    #[must_use]
    pub fn is_running(&self) -> bool {
        running::is_running(&self.user_data_dir)
    }

    /// Open a staged profile in a *running* Vivaldi so it registers the profile
    /// live (no restart), by forwarding `--profile-directory` through Chromium's
    /// `ProcessSingleton`. Do **not** also call [`Vivaldi::commit_registration`] on
    /// this path — the running Vivaldi writes the `Local State` entry itself.
    ///
    /// # Errors
    /// [`Error::VivaldiNotFound`] or [`Error::LaunchFailed`] if Vivaldi cannot be
    /// launched.
    pub fn open_profile_live(&self, profile_dir: &str) -> Result<()> {
        let override_dir = self.explicit_dir.then_some(self.user_data_dir.as_path());
        launch::open_profile(profile_dir, override_dir)
    }

    /// Copy a source profile's settings/extensions into a new profile directory
    /// and sanitize its `Preferences`, **without** registering it in
    /// `Local State`. Safe to run while Vivaldi is open (only the source is read).
    ///
    /// Follow with [`Vivaldi::commit_registration`] once Vivaldi is closed.
    ///
    /// # Errors
    /// Propagates errors from loading `Local State`, resolving the source, and
    /// copying / sanitizing files.
    pub fn stage_clone(&self, opts: &CloneOptions) -> Result<StagedClone> {
        let state = LocalState::load(&self.user_data_dir)?;
        let source = state.resolve_profile(&opts.source)?;
        let src_dir = self.user_data_dir.join(&source.dir);
        if !src_dir.is_dir() {
            return Err(Error::SourceProfileMissing(src_dir));
        }

        let new_dir = state.next_profile_dir(&self.user_data_dir);
        if !opts.dry_run {
            // Reserve the number immediately so it is never reused, even if the
            // user deletes this profile later (which would lower the on-disk max).
            local_state::bump_high_water(&self.user_data_dir, &new_dir);
        }
        let dst_dir = self.user_data_dir.join(&new_dir);

        let entries = if opts.categories.is_empty() {
            manifest::default_entries()
        } else {
            manifest::entries_for(&opts.categories)
        };
        let report = clone::copy_template(&src_dir, &dst_dir, opts.dry_run, &entries)?;
        if !opts.dry_run {
            clone::sanitize_preferences(&dst_dir, &opts.new_name)?;
        }

        Ok(StagedClone {
            new_dir,
            new_name: opts.new_name.clone(),
            report,
            avatar_icon: source.avatar_icon.clone(),
            is_using_default_avatar: source.is_using_default_avatar,
        })
    }

    /// Register a [`StagedClone`] in `Local State` (`info_cache` + `profiles_order`),
    /// allocating a fresh unique metrics bucket, and write it atomically after a
    /// timestamped backup.
    ///
    /// Call this only when [`Vivaldi::is_running`] is `false`; otherwise Vivaldi
    /// overwrites the registration on its next flush/exit.
    ///
    /// # Errors
    /// Propagates errors from loading and writing `Local State`.
    pub fn commit_registration(&self, staged: StagedClone) -> Result<CloneOutcome> {
        let mut state = LocalState::load(&self.user_data_dir)?;
        let bucket = state.next_bucket_index();
        state.register(&NewProfile {
            dir: &staged.new_dir,
            name: &staged.new_name,
            avatar_icon: staged.avatar_icon.as_deref(),
            is_using_default_avatar: staged.is_using_default_avatar,
            metrics_bucket_index: bucket,
        })?;
        state.save()?;

        Ok(CloneOutcome {
            new_dir: staged.new_dir,
            new_name: staged.new_name,
            metrics_bucket_index: bucket,
            dry_run: false,
            report: staged.report,
        })
    }

    /// Convenience: [`stage_clone`](Self::stage_clone) followed immediately by
    /// [`commit_registration`](Self::commit_registration), with no waiting. The
    /// caller is responsible for ensuring Vivaldi is closed.
    ///
    /// # Errors
    /// Propagates staging and registration errors.
    pub fn clone_profile(&self, opts: &CloneOptions) -> Result<CloneOutcome> {
        let staged = self.stage_clone(opts)?;
        if opts.dry_run {
            return Ok(CloneOutcome {
                new_dir: staged.new_dir,
                new_name: staged.new_name,
                metrics_bucket_index: 0,
                dry_run: true,
                report: staged.report,
            });
        }
        self.commit_registration(staged)
    }
}
