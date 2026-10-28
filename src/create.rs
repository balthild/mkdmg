//! Putting the pieces together.

use std::path::{Path, PathBuf};

use tempfile::TempDir;

use crate::{
    Error, Result, image,
    spec::Dmg,
    stage::Staging,
    volume::{self, Mounted},
};

impl Dmg {
    /// Build the image described by this value.
    ///
    /// The image is assembled and compressed next to `output`, so both steps
    /// stay on one filesystem.
    pub fn create(&self) -> Result<()> {
        self.validate()?;

        let workspace = Workspace::new(&self.output)?;

        let staging = Staging::prepare(self, workspace.path())?;
        let writable = workspace.path().join("writable.dmg");
        image::create(
            staging.path(),
            &self.volume_name,
            self.filesystem,
            &writable,
        )?;
        drop(staging);

        if self.volume_icon.is_some() {
            let mut mounted = Mounted::mount(&writable)?;
            volume::set_volume_icon(mounted.root())?;
            mounted.finish()?;
        }

        image::convert(&writable, &self.output)
    }
}

/// A scratch directory beside the output, so that neither staging nor the
/// writable image has to cross a filesystem boundary.
struct Workspace {
    root: TempDir,
}

impl Workspace {
    fn new(output: &Path) -> Result<Self> {
        let parent = match output.parent() {
            Some(parent) if !parent.as_os_str().is_empty() => parent.to_owned(),
            _ => PathBuf::from("."),
        };

        let root = TempDir::new_in(&parent).map_err(|error| Error::io(&parent, error))?;

        Ok(Self { root })
    }

    fn path(&self) -> &Path {
        self.root.path()
    }
}
