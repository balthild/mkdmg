use std::{
    io,
    path::{Path, PathBuf},
    process::ExitStatus,
};

use thiserror::Error;

/// Shorthand for results produced by this crate.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Everything that can go wrong while building an image.
#[derive(Debug, Error)]
pub enum Error {
    #[error("I/O error on {}: {source}", path.display())]
    Io {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("{command} failed ({status}){details}")]
    Command {
        command: String,
        status: ExitStatus,
        details: String,
    },

    #[error("{value:?} is not a valid {expected}")]
    InvalidArgument {
        value: String,
        expected: &'static str,
    },

    #[error(transparent)]
    DsStore(#[from] crate::dsstore::Error),

    #[error(transparent)]
    Alias(#[from] crate::alias::Error),

    #[error(transparent)]
    Layout(#[from] crate::layout::Error),

    #[error("volume name {0:?} is empty")]
    EmptyVolumeName(String),

    #[error("{path} has no file name")]
    NamelessSource { path: PathBuf },

    #[error("{0:?} appears more than once in the volume")]
    DuplicateEntry(String),

    #[error("no entry named {0:?} exists in the volume, so its icon cannot be positioned")]
    UnknownIconItem(String),

    #[error("an APFS image cannot be built with --no-mount")]
    ApfsNeedsMount,

    #[error("a volume icon cannot be set with --no-mount")]
    VolumeIconNeedsMount,
}

impl Error {
    pub(crate) fn io(path: impl AsRef<Path>, source: io::Error) -> Self {
        Self::Io {
            path: path.as_ref().to_owned(),
            source,
        }
    }
}
