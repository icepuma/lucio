//! The catalogue of data categories a clone can carry over.
//!
//! A clone copies the files of the *selected* categories. The categories that
//! are `default_on` (settings + extensions + extension options) are the
//! isolated-template defaults — selecting only those copies no personal data.
//! The rest (bookmarks, history, cookies, passwords, …) are opt-in; choosing
//! them relaxes isolation by the user's explicit choice.
//!
//! Each entry is a file or directory name in the profile directory and is copied
//! only if it exists; SQLite-backed categories also list the `-journal`/`-wal`
//! siblings so a copy stays consistent.

/// A user-facing group of profile files that can be carried over.
#[derive(Debug, Clone, Copy)]
pub struct Category {
    /// Stable identifier (kebab-case), used in `CloneOptions::categories`.
    pub id: &'static str,
    /// Human-readable label shown in the selector.
    pub label: &'static str,
    /// Profile file/dir names this category copies.
    pub entries: &'static [&'static str],
    /// Whether this category is preselected (the isolated-template default set).
    pub default_on: bool,
    /// Whether this category carries personal data (flagged in the selector).
    pub sensitive: bool,
}

/// All carry-over categories, in display order.
pub const CATEGORIES: &[Category] = &[
    Category {
        id: "settings",
        label: "Settings & appearance",
        entries: &["Preferences", "Secure Preferences"],
        default_on: true,
        sensitive: false,
    },
    Category {
        id: "extensions",
        label: "Extensions",
        entries: &[
            "Extensions",
            "Extension State",
            "Extension Rules",
            "Extension Scripts",
            "DNR Extension Rules",
        ],
        default_on: true,
        sensitive: false,
    },
    Category {
        id: "extension-options",
        label: "Extension options",
        entries: &[
            "Local Extension Settings",
            "Sync Extension Settings",
            "Managed Extension Settings",
        ],
        default_on: true,
        sensitive: false,
    },
    Category {
        id: "bookmarks",
        label: "Bookmarks",
        entries: &["Bookmarks", "Bookmarks.bak"],
        default_on: false,
        sensitive: false,
    },
    Category {
        id: "notes",
        label: "Notes",
        entries: &["Notes"],
        default_on: false,
        sensitive: false,
    },
    Category {
        id: "history",
        label: "History",
        entries: &[
            "History",
            "History-journal",
            "Top Sites",
            "Top Sites-journal",
            "Visited Links",
            "Shortcuts",
            "Shortcuts-journal",
            "Favicons",
            "Favicons-journal",
        ],
        default_on: false,
        sensitive: true,
    },
    Category {
        id: "cookies",
        label: "Cookies",
        entries: &[
            "Cookies",
            "Cookies-journal",
            "Safe Browsing Cookies",
            "Safe Browsing Cookies-journal",
        ],
        default_on: false,
        sensitive: true,
    },
    Category {
        id: "autofill",
        label: "Autofill & cards",
        entries: &[
            "Web Data",
            "Web Data-journal",
            "Account Web Data",
            "Account Web Data-journal",
        ],
        default_on: false,
        sensitive: true,
    },
    Category {
        id: "passwords",
        label: "Saved passwords",
        entries: &[
            "Login Data",
            "Login Data-journal",
            "Login Data For Account",
            "Login Data For Account-journal",
        ],
        default_on: false,
        sensitive: true,
    },
    Category {
        id: "sessions",
        label: "Open tabs & sessions",
        entries: &[
            "Sessions",
            "Session Storage",
            "Current Session",
            "Current Tabs",
            "Last Session",
            "Last Tabs",
        ],
        default_on: false,
        sensitive: true,
    },
    Category {
        id: "site-storage",
        label: "Site storage",
        entries: &[
            "Local Storage",
            "IndexedDB",
            "Service Worker",
            "File System",
            "Storage",
            "WebStorage",
            "Shared Dictionary",
        ],
        default_on: false,
        sensitive: true,
    },
    Category {
        id: "mail",
        label: "Mail, calendar & contacts",
        entries: &[
            "Calendar",
            "Calendar-journal",
            "Contacts",
            "Contacts-journal",
            "MailSearchDB",
            "MailSearchDB-journal",
        ],
        default_on: false,
        sensitive: true,
    },
];

/// Look up a category by `id`.
#[must_use]
pub fn category(id: &str) -> Option<&'static Category> {
    CATEGORIES.iter().find(|cat| cat.id == id)
}

/// The ids of the preselected (default) categories.
#[must_use]
pub fn default_category_ids() -> Vec<&'static str> {
    CATEGORIES
        .iter()
        .filter(|cat| cat.default_on)
        .map(|cat| cat.id)
        .collect()
}

/// The deduplicated, order-preserving union of profile entries for the given
/// category ids. Unknown ids are ignored.
#[must_use]
pub fn entries_for(ids: &[String]) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for id in ids {
        let Some(cat) = category(id) else { continue };
        for &entry in cat.entries {
            if !out.contains(&entry) {
                out.push(entry);
            }
        }
    }
    out
}

/// The entries for the default (isolated-template) category set.
#[must_use]
pub fn default_entries() -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for cat in CATEGORIES.iter().filter(|cat| cat.default_on) {
        for &entry in cat.entries {
            if !out.contains(&entry) {
                out.push(entry);
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_settings_and_extensions() {
        assert_eq!(
            default_category_ids(),
            ["settings", "extensions", "extension-options"]
        );
    }

    #[test]
    fn default_entries_match_the_template_allowlist() {
        // The historical fixed allowlist, preserved as the default set.
        assert_eq!(
            default_entries(),
            [
                "Preferences",
                "Secure Preferences",
                "Extensions",
                "Extension State",
                "Extension Rules",
                "Extension Scripts",
                "DNR Extension Rules",
                "Local Extension Settings",
                "Sync Extension Settings",
                "Managed Extension Settings",
            ]
        );
    }

    #[test]
    fn entries_for_unions_and_dedups_in_order() {
        let ids = vec!["settings".to_owned(), "bookmarks".to_owned()];
        assert_eq!(
            entries_for(&ids),
            [
                "Preferences",
                "Secure Preferences",
                "Bookmarks",
                "Bookmarks.bak"
            ]
        );
    }

    #[test]
    fn entries_for_ignores_unknown_ids() {
        assert!(entries_for(&["nope".to_owned()]).is_empty());
    }

    #[test]
    fn every_category_id_is_unique() {
        let mut ids: Vec<&str> = CATEGORIES.iter().map(|c| c.id).collect();
        ids.sort_unstable();
        let len = ids.len();
        ids.dedup();
        assert_eq!(ids.len(), len);
    }
}
