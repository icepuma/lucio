//! End-to-end CLI tests driving the real `lucio` binary against a synthetic
//! Vivaldi user-data directory in a tempdir.

use std::fs;
use std::path::Path;

use assert_cmd::Command;
use serde_json::{Value, json};
use tempfile::TempDir;

/// Write `contents` to `path`, creating parent directories as needed.
fn write(path: &Path, contents: &[u8]) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, contents).unwrap();
}

/// Read and parse a JSON file.
fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

/// Build a user-data directory with a single `Default` profile that has
/// settings, an extension, extension options, and personal data.
fn fixture() -> TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let default = root.join("Default");

    write(
        &default.join("Preferences"),
        br#"{"profile":{"name":"Privat"},"account_info":[{"email":"x@example.com"}]}"#,
    );
    write(&default.join("Secure Preferences"), b"{\"protection\":{}}");
    write(&default.join("Extensions/ext1/manifest.json"), b"{}");
    write(
        &default.join("Local Extension Settings/000001.log"),
        b"opts",
    );

    // Personal data that must never be copied into a clone.
    write(&default.join("Cookies"), b"secret");
    write(&default.join("History"), b"history");
    write(&default.join("Bookmarks"), b"bookmarks");
    write(&default.join("Login Data"), b"passwords");

    let local_state = json!({
        "profile": {
            "info_cache": { "Default": { "name": "Privat", "metrics_bucket_index": 1 } },
            "profiles_order": ["Default"],
            "profiles_created": 1,
            "metrics": { "next_bucket_index": 2 }
        }
    });
    write(
        &root.join("Local State"),
        &serde_json::to_vec(&local_state).unwrap(),
    );

    tmp
}

fn lucio(root: &Path) -> Command {
    let mut cmd = Command::cargo_bin("lucio").unwrap();
    cmd.arg("--user-data-dir").arg(root);
    cmd
}

#[test]
fn clone_creates_isolated_profile_and_registers_it() {
    let tmp = fixture();
    let root = tmp.path();

    lucio(root)
        .args(["clone", "Privat", "Work", "--execute"])
        .assert()
        .success();

    let new = root.join("Profile 1");

    // Settings + extensions + options were copied.
    assert!(new.join("Preferences").exists());
    assert!(new.join("Secure Preferences").exists());
    assert!(new.join("Extensions/ext1/manifest.json").exists());
    assert!(new.join("Local Extension Settings/000001.log").exists());

    // Personal data was NOT copied — the clone is isolated.
    assert!(!new.join("Cookies").exists());
    assert!(!new.join("History").exists());
    assert!(!new.join("Bookmarks").exists());
    assert!(!new.join("Login Data").exists());

    // The clone's Preferences were renamed and stripped of account identity.
    let prefs = read_json(&new.join("Preferences"));
    assert_eq!(prefs["profile"]["name"], "Work");
    assert!(prefs.get("account_info").is_none());

    // The clone is registered in Local State with a fresh, unique bucket and no
    // account fields.
    let local_state = read_json(&root.join("Local State"));
    let entry = &local_state["profile"]["info_cache"]["Profile 1"];
    assert_eq!(entry["name"], "Work");
    assert_eq!(entry["metrics_bucket_index"], 2);
    assert_eq!(entry["is_using_default_name"], false);
    assert!(entry.get("gaia_id").is_none());
    assert!(entry.get("user_name").is_none());

    let order = local_state["profile"]["profiles_order"].as_array().unwrap();
    assert!(order.iter().any(|v| v == "Profile 1"));
    assert_eq!(local_state["profile"]["profiles_created"], 2);
    assert_eq!(local_state["profile"]["metrics"]["next_bucket_index"], 3);
}

#[test]
fn default_is_dry_run_and_writes_nothing() {
    let tmp = fixture();
    let root = tmp.path();
    let before = fs::read(root.join("Local State")).unwrap();

    lucio(root)
        .args(["clone", "Privat", "Work"])
        .assert()
        .success()
        .stdout(predicates::str::contains("Dry run"));

    assert!(!root.join("Profile 1").exists());
    assert_eq!(fs::read(root.join("Local State")).unwrap(), before);
}

#[test]
fn list_shows_registered_profiles() {
    let tmp = fixture();
    lucio(tmp.path())
        .arg("list")
        .assert()
        .success()
        .stdout(predicates::str::contains("Default"))
        .stdout(predicates::str::contains("Privat"));
}

#[test]
fn clone_unknown_source_fails() {
    let tmp = fixture();
    lucio(tmp.path())
        .args(["clone", "Nonexistent", "Work"])
        .assert()
        .failure();
}
