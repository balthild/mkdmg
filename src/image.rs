//! Creating, mounting and converting disk images with `hdiutil`.

use std::{fs, path::Path};

use crate::{
    Error, Result,
    command::{arg, run},
    spec::FileSystem,
};

const HDIUTIL: &str = "/usr/bin/hdiutil";

/// Create a writable image whose contents come from `source`.
pub(crate) fn create(
    source: &Path,
    volume_name: &str,
    filesystem: FileSystem,
    target: &Path,
) -> Result<()> {
    run(
        HDIUTIL,
        vec![
            arg("create"),
            arg("-srcfolder"),
            arg(source),
            arg("-volname"),
            arg(volume_name),
            arg("-fs"),
            arg(filesystem.hdiutil_name()),
            arg("-format"),
            arg("UDRW"),
            arg("-ov"),
            arg(target),
        ],
    )
    .map(|_| ())
}

/// Compress a writable image into the final read-only form.
///
/// `hdiutil convert` refuses to replace an existing file, so the target is
/// removed first. No hdiutil command here runs `-quiet`, which would also
/// swallow the message explaining a failure.
pub(crate) fn convert(source: &Path, target: &Path) -> Result<()> {
    if target.exists() {
        fs::remove_file(target).map_err(|error| Error::io(target, error))?;
    }

    run(
        HDIUTIL,
        vec![
            arg("convert"),
            arg(source),
            arg("-format"),
            arg("UDZO"),
            arg("-o"),
            arg(target),
        ],
    )
    .map(|_| ())
}

/// Attach `image` at `mountpoint` and report the device it became.
///
/// `-nobrowse` keeps it out of Finder's sidebar, which matters because the
/// image is finished before anyone should look at it.
pub(crate) fn attach(image: &Path, mountpoint: &Path) -> Result<String> {
    let output = run(
        HDIUTIL,
        vec![
            arg("attach"),
            arg(image),
            arg("-readwrite"),
            arg("-nobrowse"),
            arg("-noautoopen"),
            arg("-noverify"),
            arg("-mountpoint"),
            arg(mountpoint),
        ],
    )?;

    output
        .lines()
        .find_map(|line| {
            let mut fields = line.split_whitespace();
            let device = fields.next()?;

            device.starts_with("/dev/").then(|| device.to_owned())
        })
        .ok_or_else(|| crate::Error::ToolOutput {
            command: format!("hdiutil attach {}", image.display()),
            details: format!(": no device in output {output:?}"),
        })
}

/// Detach by mount point, forcing it on the last attempt.
pub(crate) fn detach(mountpoint: &Path) -> Result<()> {
    let mut last = None;

    for attempt in 0..DETACH_ATTEMPTS {
        if attempt > 0 {
            std::thread::sleep(std::time::Duration::from_millis(200 * attempt as u64));
        }

        match run(HDIUTIL, vec![arg("detach"), arg(mountpoint)]) {
            Ok(_) => return Ok(()),
            Err(error) => last = Some(error),
        }
    }

    run(HDIUTIL, vec![arg("detach"), arg(mountpoint), arg("-force")])
        .map(|_| ())
        .map_err(|_| last.expect("at least one attempt failed"))
}

/// How many times a busy volume is retried before forcing the detach.
const DETACH_ATTEMPTS: usize = 4;
