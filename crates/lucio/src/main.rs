//! `lucio` — create new Vivaldi profiles based on existing ones as isolated
//! settings/extensions templates.

use std::time::Duration;

use anyhow::{Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use clap::{ArgAction, CommandFactory, Parser, Subcommand};
use clap_complete::Shell;
use lucio_core::{CloneOptions, CloneOutcome, CloneReport, StagedClone, Vivaldi};

/// How often to poll while waiting for Vivaldi to quit (wait-for-quit path).
const POLL_INTERVAL: Duration = Duration::from_millis(750);
/// Poll cadence/attempts while confirming a live registration after launching.
const CONFIRM_INTERVAL: Duration = Duration::from_millis(500);
const CONFIRM_ATTEMPTS: usize = 24;

/// Command-line interface definition.
#[derive(Debug, Parser)]
#[command(
    name = "lucio",
    version,
    about = "Create new Vivaldi profiles based on existing ones (isolated settings/extensions templates)."
)]
struct Cli {
    /// Override the Vivaldi user-data directory (otherwise the platform default
    /// is used).
    #[arg(long, global = true, value_name = "PATH")]
    user_data_dir: Option<Utf8PathBuf>,

    /// Increase logging verbosity (`-v` info, `-vv` debug, `-vvv` trace).
    #[arg(short, long, global = true, action = ArgAction::Count)]
    verbose: u8,

    #[command(subcommand)]
    command: Command,
}

/// Available subcommands.
#[derive(Debug, Subcommand)]
enum Command {
    /// List the profiles registered in Vivaldi.
    List,

    /// Create a new isolated profile from an existing one.
    ///
    /// Copies the source's Vivaldi settings, installed extensions and extension
    /// options, but none of its cookies, passwords, history, autofill, sessions
    /// or bookmarks. If Vivaldi is running, the new profile is opened so the
    /// running instance registers it live (no restart).
    ///
    /// Previews by default (a dry run); pass `--execute` to actually create it.
    Clone {
        /// Source profile, by display name (case-insensitive) or directory name.
        source: String,

        /// Display name for the new profile.
        target: String,

        /// Actually create the profile. Without this, lucio only previews what
        /// would be copied (a dry run).
        #[arg(long)]
        execute: bool,
    },

    /// Print a shell completion script to stdout (bash, zsh, fish, …).
    Completions {
        /// Shell to generate completions for.
        shell: Shell,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    init_tracing(cli.verbose);

    // Completions need no Vivaldi installation, so handle them before locating it.
    if let Command::Completions { shell } = &cli.command {
        print_completions(*shell);
        return Ok(());
    }

    let vivaldi = Vivaldi::locate(cli.user_data_dir.clone())
        .context("could not locate the Vivaldi user-data directory")?;

    match cli.command {
        Command::List => list(&vivaldi),
        Command::Clone {
            source,
            target,
            execute,
        } => clone(&vivaldi, &source, &target, execute),
        Command::Completions { .. } => unreachable!("handled before locating Vivaldi"),
    }
}

/// Write a shell completion script for `shell` to stdout.
fn print_completions(shell: Shell) {
    let mut command = Cli::command();
    clap_complete::generate(shell, &mut command, "lucio", &mut std::io::stdout());
}

/// Initialise tracing, honouring `RUST_LOG` and falling back to a verbosity-based
/// default. Logs go to stderr so they never pollute command output.
fn init_tracing(verbosity: u8) {
    use tracing_subscriber::{EnvFilter, fmt};

    let fallback = match verbosity {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(fallback));

    let _ = fmt()
        .with_env_filter(filter)
        .with_target(false)
        .without_time()
        .with_writer(std::io::stderr)
        .try_init();
}

/// `lucio list`
fn list(vivaldi: &Vivaldi) -> Result<()> {
    let profiles = vivaldi.list_profiles().context("failed to read profiles")?;
    if profiles.is_empty() {
        println!("No profiles found in {}", vivaldi.user_data_dir());
        return Ok(());
    }

    let width = profiles
        .iter()
        .map(|p| p.dir.len())
        .max()
        .unwrap_or(0)
        .max(9);
    println!("{:<width$}  NAME", "DIRECTORY");
    for profile in &profiles {
        println!("{:<width$}  {}", profile.dir, profile.name);
    }
    Ok(())
}

/// `lucio clone` — copy first (safe while Vivaldi runs), then register: open the
/// profile in a running Vivaldi (live, no restart), or write `Local State`
/// directly when Vivaldi is closed.
fn clone(vivaldi: &Vivaldi, source: &str, name: &str, execute: bool) -> Result<()> {
    let opts = CloneOptions {
        source: source.to_owned(),
        new_name: name.to_owned(),
        dry_run: !execute,
    };

    let staged = vivaldi
        .stage_clone(&opts)
        .with_context(|| format!("failed to copy profile {source:?}"))?;

    if !execute {
        println!(
            "Dry run — nothing written. Would create {} ({:?}) from {source:?} by copying {} files.",
            staged.new_dir, staged.new_name, staged.report.files,
        );
        println!(
            "Settings/extension entries: {}",
            staged.report.items.join(", ")
        );
        println!("Re-run with --execute to create it.");
        return Ok(());
    }

    // Vivaldi closed: write Local State directly — deterministic, no window.
    if !vivaldi.is_running() {
        let outcome = vivaldi
            .commit_registration(staged)
            .context("failed to register the clone")?;
        print_committed(vivaldi.user_data_dir(), &outcome);
        return Ok(());
    }

    // Vivaldi running: open the profile so the live instance registers it.
    match vivaldi.open_profile_live(&staged.new_dir) {
        Ok(()) => {
            let live = confirm_registered(vivaldi, &staged.new_dir);
            print_copy_summary(
                vivaldi.user_data_dir(),
                &staged.new_name,
                &staged.new_dir,
                &staged.report,
            );
            if live {
                println!(
                    "Registered live — {:?} is in Vivaldi's profile switcher now (no restart).",
                    staged.new_name,
                );
            } else {
                println!(
                    "Opened in Vivaldi — {:?} should appear in the switcher momentarily.",
                    staged.new_name,
                );
            }
            Ok(())
        }
        Err(err) => {
            eprintln!(
                "Could not launch Vivaldi to register the profile ({err}); waiting for you to \
                 quit Vivaldi instead."
            );
            let outcome = wait_then_commit(vivaldi, staged)?;
            print_committed(vivaldi.user_data_dir(), &outcome);
            Ok(())
        }
    }
}

/// Wait for Vivaldi to quit, then write the registration to `Local State`.
fn wait_then_commit(vivaldi: &Vivaldi, staged: StagedClone) -> Result<CloneOutcome> {
    eprintln!(
        "Vivaldi is running — quit it to finish registering {:?} (Ctrl-C to cancel; the copy \
         is kept).",
        staged.new_name,
    );
    eprintln!("Waiting for Vivaldi to quit…");
    while vivaldi.is_running() {
        std::thread::sleep(POLL_INTERVAL);
    }
    vivaldi
        .commit_registration(staged)
        .context("failed to register the clone")
}

/// Poll `Local State` for a short while to confirm the live registration landed.
fn confirm_registered(vivaldi: &Vivaldi, dir: &str) -> bool {
    for _ in 0..CONFIRM_ATTEMPTS {
        if vivaldi
            .list_profiles()
            .is_ok_and(|profiles| profiles.iter().any(|p| p.dir == dir))
        {
            return true;
        }
        std::thread::sleep(CONFIRM_INTERVAL);
    }
    false
}

/// Print the "Created … files copied" summary line(s).
fn print_copy_summary(user_data_dir: &Utf8Path, name: &str, dir: &str, report: &CloneReport) {
    println!(
        "Created {name:?} as {user_data_dir}/{dir} ({} files, {} copied).",
        report.files,
        human_size(report.bytes),
    );
    if report.skipped > 0 {
        println!(
            "  ({} volatile file(s) skipped during a live copy — harmless)",
            report.skipped,
        );
    }
}

/// Print the summary plus the "open Vivaldi" hint for a committed (non-live) clone.
fn print_committed(user_data_dir: &Utf8Path, outcome: &CloneOutcome) {
    print_copy_summary(
        user_data_dir,
        &outcome.new_name,
        &outcome.new_dir,
        &outcome.report,
    );
    println!(
        "Open Vivaldi to see {:?} in the profile switcher.",
        outcome.new_name
    );
}

/// Format a byte count as a human-readable size, using integer arithmetic so no
/// precision is lost.
fn human_size(bytes: u64) -> String {
    const KIB: u64 = 1024;
    const MIB: u64 = 1024 * KIB;
    const GIB: u64 = 1024 * MIB;

    let (value, unit) = match bytes {
        b if b >= GIB => (GIB, "GiB"),
        b if b >= MIB => (MIB, "MiB"),
        b if b >= KIB => (KIB, "KiB"),
        _ => return format!("{bytes} B"),
    };
    let whole = bytes / value;
    let tenths = (bytes % value) * 10 / value;
    format!("{whole}.{tenths} {unit}")
}
