//! Error type shared by every reader in this crate.

use std::path::PathBuf;

/// Everything that can go wrong while mounting, looking up or parsing assets.
///
/// Readers never panic on malformed input; they return one of these.
#[derive(Debug, thiserror::Error)]
pub enum AssetError {
    /// A logical path is malformed or tries to leave its root.
    #[error("invalid path {path:?}: {reason}")]
    Path {
        /// The path as given.
        path: String,
        /// Why it was rejected.
        reason: &'static str,
    },
    /// A requested file or folder does not exist.
    #[error("not found: {0}")]
    NotFound(String),
    /// The operating system reported an error.
    #[error("cannot read {path}: {source}")]
    Io {
        /// The file or folder involved.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: std::io::Error,
    },
    /// Bytes that do not follow a file format.
    #[error("malformed {format}: {detail}")]
    Format {
        /// The format being read, for example "WWD".
        format: &'static str,
        /// What is wrong.
        detail: String,
    },
    /// A request that cannot be satisfied with the mounted assets.
    #[error("{0}")]
    Invalid(String),
}

/// Result alias for this crate.
pub type Result<T> = std::result::Result<T, AssetError>;

pub(crate) fn format_error(format: &'static str, detail: impl Into<String>) -> AssetError {
    AssetError::Format {
        format,
        detail: detail.into(),
    }
}

pub(crate) fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> AssetError {
    AssetError::Io {
        path: path.into(),
        source,
    }
}
