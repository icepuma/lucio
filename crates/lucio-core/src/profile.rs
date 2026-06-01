//! The profile model and source-profile resolution.

/// Metadata about a single Vivaldi profile, as registered in `Local State`
/// under `profile.info_cache.<dir>`.
#[derive(Debug, Clone)]
pub struct ProfileInfo {
    /// On-disk directory name, e.g. `Default` or `Profile 1`.
    pub dir: String,
    /// Display name shown in the profile switcher, e.g. `Privat`.
    pub name: String,
    /// Avatar icon URL, e.g. `chrome://theme/IDR_PROFILE_VIVALDI_AVATAR_0`.
    pub avatar_icon: Option<String>,
    /// Whether the profile uses a stock (non-GAIA) avatar.
    pub is_using_default_avatar: bool,
}

impl ProfileInfo {
    /// Returns `true` if `query` matches this profile by directory name
    /// (case-sensitive) or display name (case-insensitive).
    #[must_use]
    pub fn matches(&self, query: &str) -> bool {
        self.dir == query || self.name.eq_ignore_ascii_case(query)
    }
}

/// Resolve a single profile from `profiles` given a user `query`.
///
/// Matching precedence:
/// 1. An exact directory-name match wins immediately (unambiguous).
/// 2. Otherwise, case-insensitive display-name matches are considered.
///
/// # Errors
/// Returns [`crate::Error::ProfileNotFound`] when nothing matches and
/// [`crate::Error::AmbiguousProfile`] when more than one display name matches.
pub fn resolve<'a>(profiles: &'a [ProfileInfo], query: &str) -> crate::Result<&'a ProfileInfo> {
    if let Some(exact) = profiles.iter().find(|p| p.dir == query) {
        return Ok(exact);
    }

    let mut name_matches = profiles
        .iter()
        .filter(|p| p.name.eq_ignore_ascii_case(query));
    let first = name_matches.next();
    match (first, name_matches.next()) {
        (Some(only), None) => Ok(only),
        (Some(_), Some(_)) => {
            let matches = profiles
                .iter()
                .filter(|p| p.matches(query))
                .map(|p| p.dir.clone())
                .collect();
            Err(crate::Error::AmbiguousProfile {
                query: query.to_owned(),
                matches,
            })
        }
        (None, _) => Err(crate::Error::ProfileNotFound(query.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(dir: &str, name: &str) -> ProfileInfo {
        ProfileInfo {
            dir: dir.to_owned(),
            name: name.to_owned(),
            avatar_icon: None,
            is_using_default_avatar: true,
        }
    }

    #[test]
    fn resolves_by_display_name_case_insensitively() {
        let profiles = vec![profile("Default", "Privat"), profile("Profile 1", "Work")];
        assert_eq!(resolve(&profiles, "privat").unwrap().dir, "Default");
        assert_eq!(resolve(&profiles, "WORK").unwrap().dir, "Profile 1");
    }

    #[test]
    fn resolves_by_directory_name() {
        let profiles = vec![profile("Default", "Privat"), profile("Profile 1", "Work")];
        assert_eq!(resolve(&profiles, "Profile 1").unwrap().name, "Work");
    }

    #[test]
    fn directory_match_beats_name_collision() {
        // A profile literally named "Profile 1" must not shadow the dir lookup.
        let profiles = vec![
            profile("Default", "Profile 1"),
            profile("Profile 1", "Work"),
        ];
        assert_eq!(resolve(&profiles, "Profile 1").unwrap().dir, "Profile 1");
    }

    #[test]
    fn ambiguous_name_errors() {
        let profiles = vec![profile("Default", "Dup"), profile("Profile 1", "dup")];
        let err = resolve(&profiles, "dup").unwrap_err();
        assert!(matches!(err, crate::Error::AmbiguousProfile { .. }));
    }

    #[test]
    fn missing_errors() {
        let profiles = vec![profile("Default", "Privat")];
        assert!(matches!(
            resolve(&profiles, "nope").unwrap_err(),
            crate::Error::ProfileNotFound(_)
        ));
    }
}
