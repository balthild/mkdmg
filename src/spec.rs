//! The declarative description of an image to build.

use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

use crate::{
    Error, Result,
    layout::{Point, WindowRect},
};

/// Filesystem to format the image with.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FileSystem {
    /// Readable by macOS 10.12 and earlier, and by more third-party tools.
    #[default]
    HfsPlus,
    Apfs,
}

impl FileSystem {
    /// The name `hdiutil -fs` expects.
    pub const fn hdiutil_name(self) -> &'static str {
        match self {
            Self::HfsPlus => "HFS+",
            Self::Apfs => "APFS",
        }
    }
}

/// Whether the build may mount the image while finishing it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mounting {
    /// Mount when the volume icon has to be written.
    #[default]
    WhenNeeded,
    /// Never mount; the volume icon is then unavailable.
    Never,
}

/// A symbolic link to create in the volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Symlink {
    /// Name of the link inside the volume.
    pub name: String,
    /// Where the link points.
    pub target: PathBuf,
}

/// An image to build.
#[derive(Debug, Clone)]
pub struct Dmg {
    /// Volume name shown by Finder. Finder resolves the background image
    /// through it, so it must match what the finished image is called.
    pub volume_name: String,
    /// Where to write the finished image.
    pub output: PathBuf,
    /// Files and bundles copied into the volume root under their own names.
    pub contents: Vec<PathBuf>,
    pub symlinks: Vec<Symlink>,
    /// Icon positions, keyed by entry name inside the volume.
    pub icons: Vec<(String, Point)>,
    /// Window background image, drawn stretched to the window size.
    pub background: Option<PathBuf>,
    /// Replacement `.icns` for the volume itself.
    pub volume_icon: Option<PathBuf>,
    pub window: WindowRect,
    pub icon_size: u16,
    pub text_size: u16,
    pub filesystem: FileSystem,
    pub mounting: Mounting,
}

impl Default for Dmg {
    fn default() -> Self {
        Self {
            volume_name: String::new(),
            output: PathBuf::new(),
            contents: Vec::new(),
            symlinks: Vec::new(),
            icons: Vec::new(),
            background: None,
            volume_icon: None,
            window: WindowRect {
                x: 100.0,
                y: 100.0,
                width: 640.0,
                height: 480.0,
            },
            icon_size: 128,
            text_size: 16,
            filesystem: FileSystem::default(),
            mounting: Mounting::default(),
        }
    }
}

impl Dmg {
    /// Reject a description that cannot produce a coherent image.
    pub fn validate(&self) -> Result<()> {
        if self.volume_name.is_empty() {
            return Err(Error::EmptyVolumeName(self.volume_name.clone()));
        }

        if self.filesystem == FileSystem::Apfs && self.mounting == Mounting::Never {
            return Err(Error::ApfsNeedsMount);
        }

        if self.volume_icon.is_some() && self.mounting == Mounting::Never {
            return Err(Error::VolumeIconNeedsMount);
        }

        let entries = self.entries()?;
        for (name, _) in &self.icons {
            if !entries.contains(name) {
                return Err(Error::UnknownIconItem(name.clone()));
            }
        }

        Ok(())
    }

    /// Names that will exist in the volume root, hidden ones aside.
    fn entries(&self) -> Result<BTreeSet<String>> {
        let mut names = BTreeSet::new();

        for path in &self.contents {
            let name = file_name(path)?;
            if !names.insert(name.clone()) {
                return Err(Error::DuplicateEntry(name));
            }
        }

        for link in &self.symlinks {
            if !names.insert(link.name.clone()) {
                return Err(Error::DuplicateEntry(link.name.clone()));
            }
        }

        Ok(names)
    }
}

/// The last path component, as a string.
pub(crate) fn file_name(path: &Path) -> Result<String> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(str::to_owned)
        .ok_or_else(|| Error::NamelessSource {
            path: path.to_owned(),
        })
}
