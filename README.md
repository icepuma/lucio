# lucio

Create a new [Vivaldi](https://vivaldi.com) profile from an existing one, as an
**isolated settings/extensions template**.

The clone inherits the source profile's Vivaldi settings and its installed
extensions (with their options), but **never** its cookies, passwords, history,
autofill, sessions or bookmarks. It's a real in-app profile and — when Vivaldi is
running — appears in the profile switcher live, with no restart.

## Install

```sh
# Homebrew
brew tap icepuma/lucio https://github.com/icepuma/lucio
brew install lucio

# or from source
cargo install --path crates/lucio
```

Pre-built Linux/macOS binaries are attached to each
[release](https://github.com/icepuma/lucio/releases).

## Usage

```sh
lucio list                                   # show existing profiles
lucio clone "Privat" --name "Work"           # clone by name (or directory, e.g. "Profile 1")
lucio clone "Privat" --name "Work" --dry-run # preview without writing
```

Add `--user-data-dir <PATH>` to target a non-default Vivaldi directory. Shell
completions: `lucio completions <bash|zsh|fish|elvish|powershell>` (Homebrew
installs them automatically).

## How it works

Only the source profile is read, so cloning is safe while Vivaldi is open. lucio
copies an [allowlist](crates/lucio-core/src/manifest.rs) of settings/extension
files (never personal data), then registers the new profile — by opening it in a
running Vivaldi (which registers it live via `--profile-directory`), or by writing
`Local State` directly when Vivaldi is closed.

> Extension options (`chrome.storage`) are carried over; a few extensions keep
> account/session state there, which comes along too.

## Development

```sh
just verify   # fmt, clippy, tests, docs, cargo-deny
```

## License

[MIT](LICENSE)
