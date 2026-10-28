use std::path::{Path, PathBuf};

use mkdmg::{
    Dmg, FileSystem, Mounting, Symlink,
    dsstore::{Field, Store, Value},
    layout::{Point, WindowRect},
};
use tempfile::TempDir;

fn spec(root: &Path) -> Dmg {
    Dmg {
        volume_name: "Build Test".into(),
        output: root.join("out.dmg"),
        contents: Vec::new(),
        symlinks: vec![Symlink {
            name: "Applications".into(),
            target: PathBuf::from("/Applications"),
        }],
        icons: vec![("Applications".into(), Point { x: 400, y: 200 })],
        background: None,
        volume_icon: None,
        window: WindowRect {
            x: 100.0,
            y: 100.0,
            width: 640.0,
            height: 400.0,
        },
        icon_size: 96,
        text_size: 14,
        filesystem: FileSystem::HfsPlus,
        mounting: Mounting::Never,
    }
}

#[test]
fn refuses_apfs_without_mounting() {
    let root = TempDir::new().unwrap();
    let mut image = spec(root.path());
    image.filesystem = FileSystem::Apfs;

    assert!(image.create().is_err(), "APFS needs a mount");
}

#[test]
fn refuses_a_volume_icon_without_mounting() {
    let root = TempDir::new().unwrap();
    let mut image = spec(root.path());
    image.volume_icon = Some(PathBuf::from(
        "/System/Library/CoreServices/CoreTypes.bundle/Contents/Resources/GenericApplicationIcon.icns",
    ));

    assert!(image.create().is_err(), "the icon needs a mount");
}

#[test]
fn refuses_icons_for_entries_that_do_not_exist() {
    let root = TempDir::new().unwrap();
    let mut image = spec(root.path());
    image
        .icons
        .push(("Missing.app".into(), Point { x: 1, y: 1 }));

    let error = image.create().expect_err("no such entry");

    assert!(error.to_string().contains("Missing.app"));
}

#[test]
fn refuses_duplicate_entries() {
    let root = TempDir::new().unwrap();
    let mut image = spec(root.path());
    image.symlinks.push(Symlink {
        name: "Applications".into(),
        target: PathBuf::from("/System/Applications"),
    });

    assert!(image.create().is_err());
}

/// A read-only mount that detaches when dropped.
struct Attached {
    mountpoint: PathBuf,
    scratch: TempDir,
}

impl Attached {
    fn new(image: &Path) -> Self {
        let scratch = TempDir::new().unwrap();
        let output = std::process::Command::new("/usr/bin/hdiutil")
            .args([
                "attach",
                "-readonly",
                "-nobrowse",
                "-noautoopen",
                "-mountpoint",
            ])
            .arg(scratch.path())
            .arg(image)
            .output()
            .expect("hdiutil must launch");

        assert!(
            output.status.success(),
            "attach failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );

        Self {
            mountpoint: scratch.path().to_owned(),
            scratch,
        }
    }
}

impl Drop for Attached {
    fn drop(&mut self) {
        let _ = std::process::Command::new("/usr/bin/hdiutil")
            .args(["detach", "-force"])
            .arg(&self.mountpoint)
            .status();

        let _ = &self.scratch;
    }
}

#[test]
fn builds_an_image_finder_can_read() {
    let root = TempDir::new().unwrap();
    let mut image = spec(root.path());

    // A plain file is enough to prove the copy path; app bundles only add size.
    let source = root.path().join("Read Me.txt");
    std::fs::write(&source, "hello").unwrap();
    image.contents.push(source);
    image.icons[0].1 = Point { x: 400, y: 200 };
    image
        .icons
        .push(("Read Me.txt".into(), Point { x: 200, y: 200 }));

    image.create().expect("build");

    let attached = Attached::new(&image.output);

    assert!(attached.mountpoint.join("Applications").is_symlink());
    assert!(attached.mountpoint.join("Read Me.txt").exists());

    let store = Store::parse(&std::fs::read(attached.mountpoint.join(".DS_Store")).unwrap())
        .expect("parse the store we wrote");

    assert_eq!(store.get(".", Field::VSRN), Some(&Value::Long(1)));
    assert_eq!(store.get(".", Field::ICVL), Some(&Value::Type(*b"icnv")));
    assert_eq!(
        store.get("Read Me.txt", Field::ILOC),
        Some(&Value::Blob(vec![
            0, 0, 0, 200, 0, 0, 0, 200, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0
        ]))
    );
}
