# lucio

Create new [Vivaldi](https://vivaldi.com) profiles based on existing ones — as
**isolated settings/extensions templates**.

A clone inherits the source profile's Vivaldi configuration (themes, keyboard
shortcuts, mouse gestures, search engines) and its **installed extensions and
their options**, but is fully isolated from the source's personal data: cookies,
saved passwords, history, autofill, open sessions, bookmarks, site storage and
Vivaldi mail/calendar are **never** copied.

The new profile is a real in-app profile — it shows up in Vivaldi's profile
switcher, not as a separate `--user-data-dir` instance.

## Install

### Homebrew

```sh
brew tap icepuma/lucio https://github.com/icepuma/lucio
brew install lucio
```

### From source

```sh
cargo install --path crates/lucio
```

Pre-built binaries for Linux and macOS are also attached to each
[release](https://github.com/icepuma/lucio/releases).

## Usage

```sh
# List the profiles Vivaldi knows about
lucio list

# Clone the "Privat" profile into a new isolated profile called "Work"
lucio clone "Privat" --name "Work"

# Preview what would be copied, without writing anything
lucio clone "Privat" --name "Work" --dry-run
```

The source can be given by display name (case-insensitive) or by directory name
(`Default`, `Profile 1`, …).

### Options

| Flag | Description |
| --- | --- |
| `--user-data-dir <PATH>` | Use a specific Vivaldi user-data directory instead of the platform default. |
| `--dry-run` | (clone) Show the copy plan without writing anything. |
| `-v`, `-vv`, `-vvv` | Increase log verbosity (also honours `RUST_LOG`). |

### Default user-data locations

| OS | Path |
| --- | --- |
| macOS | `~/Library/Application Support/Vivaldi` |
| Linux | `~/.config/vivaldi` |
| Windows | `%LOCALAPPDATA%\Vivaldi\User Data` |

## Shell completions

`lucio` generates completion scripts for your shell:

```sh
lucio completions bash   # also: zsh, fish, elvish, powershell
```

Installed via Homebrew, completions are set up automatically. To install them
manually:

```sh
# bash
lucio completions bash | sudo tee /etc/bash_completion.d/lucio > /dev/null

# zsh — into a directory on your $fpath
lucio completions zsh > ~/.zfunc/_lucio

# fish
lucio completions fish > ~/.config/fish/completions/lucio.fish
```

## How it works

1. Reads the user-data-root `Local State` registry to find the source profile.
2. Picks the next free `Profile N` directory and a fresh, unique
   `metrics_bucket_index`.
3. Copies only an [allowlist](crates/lucio-core/src/manifest.rs) of
   settings/extension files (never personal data).
4. Sanitizes the clone's `Preferences` (sets the display name, clears sign-in /
   account identity).
5. Registers the new profile — by opening it in a running Vivaldi (which adds it
   to the switcher live, no restart) or, when Vivaldi is closed, by writing
   `Local State` directly (atomic write + timestamped backup).

### Running Vivaldi & safety

The clone only ever **reads** the source profile, so it is safe to run while
Vivaldi is open — the source can't be corrupted.

- `Preferences` / `Secure Preferences` are written atomically by Chromium, so a
  whole-file copy always sees a consistent version. Copying `Secure Preferences`
  verbatim is what keeps the extensions registered (its tamper-protection HMACs
  remain valid on the same machine).
- Extension LevelDB stores are copied with their volatile `LOCK`/`LOG` files
  skipped and `CURRENT` written last; files that vanish mid-copy are skipped.
  Any inconsistency only affects the disposable clone, never the source.

A running Vivaldi keeps `Local State` in memory and rewrites it on exit, so an
external edit to the profile list would be discarded. `lucio` works around this
**without needing a restart**:

- **Vivaldi running:** after copying, `lucio` opens the new profile in the
  running Vivaldi (`--profile-directory`, forwarded via Chromium's
  ProcessSingleton). Vivaldi loads it and **registers it itself**, so it appears
  in the switcher live — and keeps the name from the copied `Preferences`. A
  window for the new profile opens (that is what triggers the registration). If
  the launch can't be performed, `lucio` falls back to waiting for you to quit
  Vivaldi, then writes `Local State` directly.
- **Vivaldi closed:** `lucio` writes the registration to `Local State` directly
  (no window). A timestamped backup is written on every change.

> Note: "extension options" (chrome.storage) are carried over. A small number of
> extensions store account/session state there, which would come along too.

## Development

```sh
just verify   # fmt, clippy (pedantic/nursery/cargo), tests, docs, cargo-deny
```

## License

[MIT](LICENSE)
