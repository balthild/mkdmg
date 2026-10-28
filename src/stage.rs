//! Building the directory tree that becomes the image's contents.

use std::{fs, os::unix::fs::symlink, path::Path};

use tempfile::TempDir;

use crate::{
    Error, Result,
    command::{arg, run},
    layout::{BACKGROUND_FOLDER, Layout},
    spec::{Dmg, file_name},
};

/// Name Finder looks for when drawing the volume's own icon.
const VOLUME_ICON_FILE: &str = ".VolumeIcon.icns";

/// A prepared volume tree, removed when dropped.
#[derive(Debug)]
pub(crate) struct Staging {
    root: TempDir,
}

impl Staging {
    /// Populate a fresh directory with everything the volume must contain,
    /// including a `.DS_Store` describing how Finder should present it.
    pub(crate) fn prepare(spec: &Dmg, parent: &Path) -> Result<Self> {
        let root = TempDir::new_in(parent).map_err(|error| Error::io(parent, error))?;
        let path = root.path();

        for source in &spec.contents {
            let target = path.join(file_name(source)?);
            ditto(source, &target)?;
        }

        for link in &spec.symlinks {
            let target = path.join(&link.name);
            symlink(&link.target, &target).map_err(|error| Error::io(&target, error))?;
        }

        let background_name = match &spec.background {
            Some(background) => Some(self::background(path, background)?),
            None => None,
        };

        if let Some(icon) = &spec.volume_icon {
            let target = path.join(VOLUME_ICON_FILE);
            fs::copy(icon, &target).map_err(|error| Error::io(&target, error))?;
        }

        write_ds_store(spec, path, background_name.as_deref())?;

        Ok(Self { root })
    }

    pub(crate) fn path(&self) -> &Path {
        self.root.path()
    }
}

fn write_ds_store(spec: &Dmg, root: &Path, background: Option<&str>) -> Result<()> {
    let layout = Layout {
        volume_name: &spec.volume_name,
        window: spec.window,
        icon_size: spec.icon_size,
        text_size: spec.text_size,
        background,
    };

    let bytes = layout.records(&spec.icons)?.to_bytes()?;
    let path = root.join(".DS_Store");

    fs::write(&path, bytes).map_err(|error| Error::io(&path, error))
}

/// Copy the background image into `.background` and report its file name.
///
/// Finder draws the image without scaling, so it should be exactly as large as
/// the window's content area.
fn background(root: &Path, source: &Path) -> Result<String> {
    let folder = root.join(BACKGROUND_FOLDER);
    fs::create_dir_all(&folder).map_err(|error| Error::io(&folder, error))?;

    let name = file_name(source)?;
    let target = folder.join(&name);
    fs::copy(source, &target).map_err(|error| Error::io(&target, error))?;

    Ok(name)
}

/// Copy a bundle or file with its metadata, which `ditto` preserves and a
/// plain recursive copy does not.
fn ditto(source: &Path, target: &Path) -> Result<()> {
    run("/usr/bin/ditto", vec![arg(source), arg(target)]).map(|_| ())
}
