//! Reading and editing Vivaldi's `Local State` profile registry.
//!
//! `Local State` is a JSON file at the user-data root. The profile switcher is
//! driven by `profile.info_cache` (keyed by directory name), ordered by
//! `profile.profiles_order`, with `profile.profiles_created` acting as the
//! monotonic directory-number counter and `profile.metrics.next_bucket_index`
//! allocating unique per-profile metrics buckets.
//!
//! We parse into a [`serde_json::Value`] (with the `preserve_order` feature) and
//! mutate only the keys we care about, so untouched keys keep their order and
//! the file is written back compact, matching how Chromium serializes it.

use std::collections::HashSet;

use camino::{Utf8Path, Utf8PathBuf};
use serde_json::{Map, Value};

use crate::error::{Error, Result};
use crate::profile::{self, ProfileInfo};
use crate::util;

/// File name of the user-data-root registry.
const LOCAL_STATE_FILE: &str = "Local State";

/// An in-memory, editable view of `Local State`.
#[derive(Debug)]
pub struct LocalState {
    path: Utf8PathBuf,
    root: Value,
}

/// The fields needed to register a freshly cloned profile in `info_cache`.
#[derive(Debug)]
pub struct NewProfile<'a> {
    /// On-disk directory name, e.g. `Profile 4`.
    pub dir: &'a str,
    /// Display name shown in the profile switcher.
    pub name: &'a str,
    /// Avatar icon URL to inherit/assign, if any.
    pub avatar_icon: Option<&'a str>,
    /// Whether the profile uses a stock (non-GAIA) avatar.
    pub is_using_default_avatar: bool,
    /// A freshly allocated, unique metrics bucket index.
    pub metrics_bucket_index: i64,
}

impl LocalState {
    /// Load `Local State` from a user-data directory.
    ///
    /// # Errors
    /// [`Error::LocalStateMissing`] if the file is absent, [`Error::Io`] on a
    /// read failure, or [`Error::Json`] if the contents are not valid JSON.
    pub fn load(user_data_dir: &Utf8Path) -> Result<Self> {
        let path = user_data_dir.join(LOCAL_STATE_FILE);
        if !path.is_file() {
            return Err(Error::LocalStateMissing(path));
        }
        let bytes = std::fs::read(path.as_std_path()).map_err(|e| Error::io(&path, e))?;
        let root: Value = serde_json::from_slice(&bytes).map_err(|e| Error::json(&path, e))?;
        Ok(Self { path, root })
    }

    /// The path this state was loaded from.
    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        &self.path
    }

    /// All registered profiles, in `info_cache` order. Empty if the registry
    /// has no `info_cache` yet.
    #[must_use]
    pub fn profiles(&self) -> Vec<ProfileInfo> {
        let Some(info_cache) = self.info_cache() else {
            return Vec::new();
        };
        info_cache
            .iter()
            .map(|(dir, value)| ProfileInfo {
                dir: dir.clone(),
                name: value
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned(),
                avatar_icon: value
                    .get("avatar_icon")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned),
                is_using_default_avatar: value
                    .get("is_using_default_avatar")
                    .and_then(Value::as_bool)
                    .unwrap_or(true),
            })
            .collect()
    }

    /// Resolve a single profile from a name-or-directory `query`.
    ///
    /// # Errors
    /// [`Error::ProfileNotFound`] or [`Error::AmbiguousProfile`] per
    /// [`profile::resolve`].
    pub fn resolve_profile(&self, query: &str) -> Result<ProfileInfo> {
        let profiles = self.profiles();
        profile::resolve(&profiles, query).cloned()
    }

    /// Compute the next free `Profile N` directory name.
    ///
    /// Follows Chromium's `profile.profiles_created` counter, but guards against
    /// collisions with directories already present on disk or in `info_cache`
    /// (deletions leave gaps, so the counter alone is not collision-proof).
    #[must_use]
    pub fn next_profile_dir(&self, user_data_dir: &Utf8Path) -> String {
        let counter = self
            .profile_obj()
            .and_then(|p| p.get("profiles_created"))
            .and_then(Value::as_u64)
            .unwrap_or(1)
            .max(1);

        let existing: HashSet<String> = self.profiles().into_iter().map(|p| p.dir).collect();

        let mut n = counter;
        loop {
            let dir = format!("Profile {n}");
            if !existing.contains(&dir) && !user_data_dir.join(&dir).exists() {
                return dir;
            }
            n += 1;
        }
    }

    /// Allocate a fresh, unique `metrics_bucket_index`.
    ///
    /// Bucket `0` is reserved for the Guest profile, so this never returns less
    /// than `1`, and always exceeds every existing bucket.
    #[must_use]
    pub fn next_bucket_index(&self) -> i64 {
        let metrics_next = self
            .profile_obj()
            .and_then(|p| p.get("metrics"))
            .and_then(|m| m.get("next_bucket_index"))
            .and_then(Value::as_i64)
            .unwrap_or(1);

        let max_existing = self.info_cache().map_or(0, |ic| {
            ic.values()
                .filter_map(|v| v.get("metrics_bucket_index").and_then(Value::as_i64))
                .max()
                .unwrap_or(0)
        });

        metrics_next.max(max_existing + 1).max(1)
    }

    /// Register a freshly cloned profile: insert its `info_cache` entry, append
    /// it to `profiles_order`, advance the directory counter, and bump
    /// `metrics.next_bucket_index`. Account/sign-in fields are intentionally
    /// omitted so the clone is not tied to the source account.
    ///
    /// # Errors
    /// [`Error::MalformedLocalState`] if `Local State` is not a JSON object.
    pub fn register(&mut self, new: &NewProfile<'_>) -> Result<()> {
        let profile = ensure_object(self.root_obj_mut()?, "profile")?;

        let mut entry = Map::new();
        entry.insert("name".to_owned(), Value::String(new.name.to_owned()));
        entry.insert("is_using_default_name".to_owned(), Value::Bool(false));
        entry.insert(
            "is_using_default_avatar".to_owned(),
            Value::Bool(new.is_using_default_avatar),
        );
        if let Some(icon) = new.avatar_icon {
            entry.insert("avatar_icon".to_owned(), Value::String(icon.to_owned()));
        }
        entry.insert(
            "metrics_bucket_index".to_owned(),
            Value::from(new.metrics_bucket_index),
        );
        entry.insert("is_ephemeral".to_owned(), Value::Bool(false));

        ensure_object(profile, "info_cache")?.insert(new.dir.to_owned(), Value::Object(entry));

        let order = ensure_array(profile, "profiles_order")?;
        if !order.iter().any(|v| v.as_str() == Some(new.dir)) {
            order.push(Value::String(new.dir.to_owned()));
        }

        let used = dir_number(new.dir).unwrap_or(0);
        let old_created = profile
            .get("profiles_created")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        profile.insert(
            "profiles_created".to_owned(),
            Value::from(old_created.max(used + 1)),
        );

        ensure_object(profile, "metrics")?.insert(
            "next_bucket_index".to_owned(),
            Value::from(new.metrics_bucket_index + 1),
        );

        Ok(())
    }

    /// Persist the state back to disk, compact, after taking a timestamped
    /// backup of the previous file.
    ///
    /// # Errors
    /// [`Error::Json`] on serialization failure or [`Error::Io`] on any
    /// filesystem error.
    pub fn save(&self) -> Result<()> {
        let serialized = serde_json::to_vec(&self.root).map_err(|e| Error::json(&self.path, e))?;

        if self.path.exists() {
            let backup = self.path.with_file_name(format!(
                "{LOCAL_STATE_FILE}.lucio-backup-{}",
                util::unix_millis()
            ));
            std::fs::copy(self.path.as_std_path(), backup.as_std_path())
                .map_err(|e| Error::io(&backup, e))?;
        }

        util::write_atomic(&self.path, &serialized)
    }

    // --- internal accessors -------------------------------------------------

    fn profile_obj(&self) -> Option<&Map<String, Value>> {
        self.root.get("profile").and_then(Value::as_object)
    }

    fn info_cache(&self) -> Option<&Map<String, Value>> {
        self.profile_obj()
            .and_then(|p| p.get("info_cache"))
            .and_then(Value::as_object)
    }

    fn root_obj_mut(&mut self) -> Result<&mut Map<String, Value>> {
        self.root
            .as_object_mut()
            .ok_or_else(|| Error::MalformedLocalState("top level is not a JSON object".to_owned()))
    }
}

/// Parse the trailing integer of a `Profile N` directory name.
fn dir_number(dir: &str) -> Option<u64> {
    dir.strip_prefix("Profile ").and_then(|n| n.parse().ok())
}

/// Get a child object by `key`, creating an empty one if absent.
fn ensure_object<'a>(
    parent: &'a mut Map<String, Value>,
    key: &str,
) -> Result<&'a mut Map<String, Value>> {
    parent
        .entry(key.to_owned())
        .or_insert_with(|| Value::Object(Map::new()))
        .as_object_mut()
        .ok_or_else(|| Error::MalformedLocalState(format!("`{key}` is not an object")))
}

/// Get a child array by `key`, creating an empty one if absent.
fn ensure_array<'a>(parent: &'a mut Map<String, Value>, key: &str) -> Result<&'a mut Vec<Value>> {
    parent
        .entry(key.to_owned())
        .or_insert_with(|| Value::Array(Vec::new()))
        .as_array_mut()
        .ok_or_else(|| Error::MalformedLocalState(format!("`{key}` is not an array")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Value {
        serde_json::json!({
            "profile": {
                "info_cache": {
                    "Default":   { "name": "Privat",     "metrics_bucket_index": 1 },
                    "Profile 1": { "name": "Projekt P",  "metrics_bucket_index": 2 },
                    "Profile 3": { "name": "Vattenfall", "metrics_bucket_index": 4 }
                },
                "profiles_order": ["Default", "Profile 1", "Profile 3"],
                "profiles_created": 4,
                "metrics": { "next_bucket_index": 5 }
            }
        })
    }

    fn state(root: Value) -> LocalState {
        LocalState {
            path: Utf8PathBuf::from("/tmp/Local State"),
            root,
        }
    }

    #[test]
    fn lists_profiles() {
        let st = state(sample());
        let names: Vec<_> = st.profiles().into_iter().map(|p| p.name).collect();
        assert_eq!(names, ["Privat", "Projekt P", "Vattenfall"]);
    }

    #[test]
    fn next_dir_skips_the_deleted_gap_via_counter() {
        let st = state(sample());
        // profiles_created == 4 and "Profile 4" is free → "Profile 4".
        assert_eq!(
            st.next_profile_dir(Utf8Path::new("/nonexistent")),
            "Profile 4"
        );
    }

    #[test]
    fn next_bucket_is_unique_and_monotonic() {
        let st = state(sample());
        assert_eq!(st.next_bucket_index(), 5);
    }

    #[test]
    fn register_inserts_entry_and_advances_counters() {
        let mut st = state(sample());
        let bucket = st.next_bucket_index();
        let dir = st.next_profile_dir(Utf8Path::new("/nonexistent"));
        st.register(&NewProfile {
            dir: &dir,
            name: "Work",
            avatar_icon: Some("chrome://theme/IDR_PROFILE_AVATAR_0"),
            is_using_default_avatar: true,
            metrics_bucket_index: bucket,
        })
        .unwrap();

        let entry = &st.root["profile"]["info_cache"]["Profile 4"];
        assert_eq!(entry["name"], "Work");
        assert_eq!(entry["metrics_bucket_index"], 5);
        assert_eq!(entry["is_using_default_name"], false);
        // No account identity leaks into the clone.
        assert!(entry.get("gaia_id").is_none());
        assert!(entry.get("user_name").is_none());

        assert_eq!(st.root["profile"]["profiles_created"], 5);
        assert_eq!(st.root["profile"]["metrics"]["next_bucket_index"], 6);
        let order = st.root["profile"]["profiles_order"].as_array().unwrap();
        assert_eq!(order.last().unwrap(), "Profile 4");
    }

    #[test]
    fn register_into_empty_local_state() {
        let mut st = state(serde_json::json!({}));
        let dir = st.next_profile_dir(Utf8Path::new("/nonexistent"));
        assert_eq!(dir, "Profile 1");
        st.register(&NewProfile {
            dir: &dir,
            name: "Fresh",
            avatar_icon: None,
            is_using_default_avatar: true,
            metrics_bucket_index: st.next_bucket_index(),
        })
        .unwrap();
        assert_eq!(
            st.root["profile"]["info_cache"]["Profile 1"]["name"],
            "Fresh"
        );
    }
}
