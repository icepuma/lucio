//! The allowlist of files copied into a cloned template profile.

/// Top-level entries copied from the source profile into a new template profile.
///
/// Anything *not* listed here is intentionally left behind, so the clone cannot
/// see the source profile's personal data: cookies, saved passwords, history,
/// autofill, open sessions, bookmarks, site storage
/// (`Local Storage`/`IndexedDB`/`Service Worker`/…), Vivaldi mail & calendar,
/// and sync identity.
///
/// Each entry may be a file or a directory and is copied only if it exists in
/// the source profile.
///
/// Rationale for an allowlist (rather than copy-everything-then-delete): the
/// worst case of a missing entry is a *missing setting* (safe), whereas the
/// worst case of a forgotten denylist entry is a *leaked personal-data file*
/// (unacceptable), which also gets riskier as Vivaldi adds new state files.
pub const COPY_ALLOWLIST: &[&str] = &[
    // Browser + Vivaldi settings: themes, keyboard shortcuts, mouse gestures,
    // search engines, panels, appearance, etc. (sanitized after copy).
    "Preferences",
    // Tamper-protected settings, including the extension registration
    // (`extensions.settings`). Copied verbatim so its HMACs stay valid on the
    // same machine and the extensions remain installed.
    "Secure Preferences",
    // Installed extensions and their runtime state / rules / scripts.
    "Extensions",
    "Extension State",
    "Extension Rules",
    "Extension Scripts",
    "DNR Extension Rules",
    // Extension options: chrome.storage `local` / `sync` / `managed`.
    "Local Extension Settings",
    "Sync Extension Settings",
    "Managed Extension Settings",
];
