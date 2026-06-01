//! Error types for `lucio-core`.

use camino::Utf8PathBuf;

/// Result alias used throughout the crate.
pub type Result<T> = std::result::Result<T, Error>;

/// Everything that can go wrong while inspecting or cloning Vivaldi profiles.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum Error {
    /// The platform default user-data directory could not be determined.
    #[error(
        "could not determine the Vivaldi user-data directory for this platform; \
         pass --user-data-dir <PATH>"
    )]
    UserDataDirNotFound,

    /// The resolved user-data directory does not exist on disk.
    #[error("Vivaldi user-data directory does not exist: {0}")]
    UserDataDirMissing(Utf8PathBuf),

    /// A filesystem path was not valid UTF-8 (camino requires UTF-8 paths).
    #[error("path is not valid UTF-8: {0}")]
    NonUtf8Path(std::path::PathBuf),

    /// The `Local State` registry file is missing.
    #[error("`Local State` not found at {0} (is this a Vivaldi user-data directory?)")]
    LocalStateMissing(Utf8PathBuf),

    /// `Local State` did not have the structure we expect.
    #[error("`Local State` is malformed: {0}")]
    MalformedLocalState(String),

    /// The Vivaldi application/binary could not be found to open a profile.
    #[error(
        "could not find the Vivaldi application to open the profile; \
         install Vivaldi or re-run with --no-launch"
    )]
    VivaldiNotFound,

    /// Launching Vivaldi to register the profile failed.
    #[error("failed to launch Vivaldi to register the profile: {source}")]
    LaunchFailed {
        /// The underlying spawn error.
        #[source]
        source: std::io::Error,
    },

    /// No profile matched the user-provided query.
    #[error("no profile matches {0:?} — run `lucio list` to see available profiles")]
    ProfileNotFound(String),

    /// More than one profile matched the query.
    #[error("{query:?} is ambiguous; it matches {matches:?} — pass the exact directory name")]
    AmbiguousProfile {
        /// The original query.
        query: String,
        /// The directory names that matched.
        matches: Vec<String>,
    },

    /// The source profile directory is missing on disk.
    #[error("source profile directory not found: {0}")]
    SourceProfileMissing(Utf8PathBuf),

    /// The destination profile directory already exists.
    #[error("destination profile directory already exists: {0}")]
    ProfileDirExists(Utf8PathBuf),

    /// A JSON file could not be parsed or serialized.
    #[error("failed to process JSON at {path}: {source}")]
    Json {
        /// The file involved.
        path: Utf8PathBuf,
        /// The underlying serde error.
        #[source]
        source: serde_json::Error,
    },

    /// An underlying I/O error, annotated with the path it concerns.
    #[error("I/O error at {path}: {source}")]
    Io {
        /// The file or directory involved.
        path: Utf8PathBuf,
        /// The underlying I/O error.
        #[source]
        source: std::io::Error,
    },
}

impl Error {
    /// Build a JSON error annotated with the offending path.
    pub(crate) fn json(path: impl Into<Utf8PathBuf>, source: serde_json::Error) -> Self {
        Self::Json {
            path: path.into(),
            source,
        }
    }

    /// Build an I/O error annotated with the offending path.
    pub(crate) fn io(path: impl Into<Utf8PathBuf>, source: std::io::Error) -> Self {
        Self::Io {
            path: path.into(),
            source,
        }
    }
}
