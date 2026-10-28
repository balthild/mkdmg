//! Mounting a writable image to give the volume its own icon.
//!
//! Every other part of the appearance lives in ordinary files that staging can
//! prepare before the image exists. The volume icon is the exception: Finder
//! only reads `.VolumeIcon.icns` when the volume's *root directory* carries the
//! `kHasCustomIcon` bit in its FinderInfo, and that directory is created by the
//! filesystem formatter. It cannot be reached through the staging directory,
//! so the image is mounted once and the attribute written directly.

use std::{
    fs,
    path::{Path, PathBuf},
};

use tempfile::TempDir;
use xattr::FileExt;

use crate::{Error, Result, image};

/// `kHasCustomIcon` in the FinderInfo flag word.
const HAS_CUSTOM_ICON: u16 = 0x0400;

/// Byte offset of the flag word inside a directory's FinderInfo.
const FLAGS_OFFSET: usize = 8;

/// Size of the FinderInfo attribute.
const FINDER_INFO_LENGTH: usize = 32;

/// Attribute Finder reads before honouring `.VolumeIcon.icns`.
const FINDER_INFO: &str = "com.apple.FinderInfo";

/// A mounted image, detached when dropped.
pub(crate) struct Mounted {
    mountpoint: PathBuf,
    attached: bool,
    // Keeps the private mount point alive for as long as the image is mounted.
    _scratch: TempDir,
}

impl Mounted {
    /// Mount `image` at a private mount point.
    ///
    /// A private mount point avoids the suffixes `/Volumes` appends to
    /// duplicate volume names and keeps the unfinished volume out of Finder.
    pub(crate) fn mount(image: &Path) -> Result<Self> {
        let scratch = TempDir::new().map_err(|error| Error::io(std::env::temp_dir(), error))?;

        image::attach(image, scratch.path())?;
        Ok(Self {
            mountpoint: scratch.path().to_owned(),
            attached: true,
            _scratch: scratch,
        })
    }

    pub(crate) fn root(&self) -> &Path {
        &self.mountpoint
    }

    /// Drop what the system wrote while the image was mounted, then detach.
    ///
    /// The cleanup has to happen while the volume is still mounted, so it
    /// cannot live in `Drop`.
    pub(crate) fn finish(&mut self) -> Result<()> {
        for name in [".fseventsd", ".Trashes", ".Spotlight-V100"] {
            let _ = fs::remove_dir_all(self.mountpoint.join(name));
        }

        self.detach()
    }

    fn detach(&mut self) -> Result<()> {
        if !self.attached {
            return Ok(());
        }

        image::detach(&self.mountpoint)?;
        self.attached = false;

        Ok(())
    }
}

impl Drop for Mounted {
    fn drop(&mut self) {
        // A leftover mount blocks the next build, so a failed detach must not
        // be silently dropped; `image::detach` already retries and forces.
        let _ = self.detach();
    }
}

/// Mark the mounted volume as having a custom icon.
pub(crate) fn set_volume_icon(mountpoint: &Path) -> Result<()> {
    let root = fs::File::open(mountpoint).map_err(|error| Error::io(mountpoint, error))?;

    let mut info = root
        .get_xattr(FINDER_INFO)
        .map_err(|error| Error::io(mountpoint, error))?
        .unwrap_or_else(|| vec![0; FINDER_INFO_LENGTH]);
    if info.len() < FINDER_INFO_LENGTH {
        info.resize(FINDER_INFO_LENGTH, 0);
    }

    let flags = u16::from_be_bytes([info[FLAGS_OFFSET], info[FLAGS_OFFSET + 1]]);
    let flags = flags | HAS_CUSTOM_ICON;
    info[FLAGS_OFFSET..FLAGS_OFFSET + 2].copy_from_slice(&flags.to_be_bytes());

    root.set_xattr(FINDER_INFO, &info)
        .map_err(|error| Error::io(mountpoint, error))
}
