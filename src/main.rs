use std::{
    path::{Path, PathBuf},
    process::ExitCode,
    str::FromStr,
};

use clap::{Parser, ValueEnum};
use mkdmg::{
    Dmg, FileSystem, Mounting, Symlink,
    layout::{Point, WindowRect},
};

/// Build a Finder-styled disk image without driving the Finder.
#[derive(Parser)]
#[command(version)]
struct Args {
    /// Files and bundles to copy into the volume.
    #[arg(required = true, value_name = "SOURCE")]
    sources: Vec<PathBuf>,

    /// Where to write the finished image.
    #[arg(short, long, value_name = "FILE")]
    output: PathBuf,

    /// Volume name; defaults to the output file name.
    #[arg(long, value_name = "NAME")]
    volname: Option<String>,

    /// Window background image, drawn stretched to the window size.
    #[arg(long, value_name = "IMAGE")]
    background: Option<PathBuf>,

    /// Replacement icon for the volume itself.
    #[arg(long, value_name = "ICNS")]
    volume_icon: Option<PathBuf>,

    /// Window origin as X,Y, with Y counted up from the bottom of the screen.
    #[arg(long, value_name = "X,Y", default_value = "400,400")]
    window_position: PairArg<f64>,

    /// Window size as W,H.
    #[arg(long, value_name = "W,H", default_value = "640,480")]
    window_size: PairArg<f64>,

    /// Icon size in points.
    #[arg(long, value_name = "POINTS", default_value_t = 128)]
    icon_size: u16,

    /// Label text size in points.
    #[arg(long, value_name = "POINTS", default_value_t = 16)]
    text_size: u16,

    /// Position an entry's icon, given as NAME:X,Y.
    #[arg(long, value_name = "NAME:X,Y", action = clap::ArgAction::Append)]
    icon: Vec<IconArg>,

    /// Add a shortcut to the Applications folder at X,Y.
    #[arg(long, value_name = "X,Y")]
    app_drop_link: Option<PairArg<u32>>,

    /// Create a symbolic link, given as NAME=TARGET or just TARGET.
    #[arg(long, value_name = "NAME=TARGET", action = clap::ArgAction::Append)]
    symlink: Vec<SymlinkArg>,

    /// Filesystem to format the image with.
    #[arg(long, value_enum, default_value_t = FsArg::HfsPlus)]
    fs: FsArg,

    /// Never mount the image; the volume icon is then unavailable.
    #[arg(long)]
    no_mount: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum FsArg {
    /// HFS+, readable by older systems and more third-party tools.
    #[value(name = "hfs+")]
    HfsPlus,
    /// APFS.
    Apfs,
}

impl From<FsArg> for FileSystem {
    fn from(value: FsArg) -> Self {
        match value {
            FsArg::HfsPlus => Self::HfsPlus,
            FsArg::Apfs => Self::Apfs,
        }
    }
}

/// Two numbers written as `a,b`.
#[derive(Clone, Copy)]
struct PairArg<T> {
    first: T,
    second: T,
}

impl<T: FromStr> FromStr for PairArg<T> {
    type Err = &'static str;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (first, second) = text.split_once(',').ok_or("expected two numbers as a,b")?;

        Ok(Self {
            first: first.trim().parse().map_err(|_| "not a number")?,
            second: second.trim().parse().map_err(|_| "not a number")?,
        })
    }
}

impl From<PairArg<u32>> for Point {
    fn from(pair: PairArg<u32>) -> Self {
        Self {
            x: pair.first,
            y: pair.second,
        }
    }
}

/// An icon to place, written as `NAME:X,Y`.
#[derive(Clone)]
struct IconArg {
    name: String,
    position: PairArg<u32>,
}

impl FromStr for IconArg {
    type Err = &'static str;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (name, position) = text
            .split_once(':')
            .ok_or("expected a name and a position as NAME:X,Y")?;

        Ok(Self {
            name: name.to_owned(),
            position: position.parse()?,
        })
    }
}

impl From<IconArg> for (String, Point) {
    fn from(icon: IconArg) -> Self {
        (icon.name, icon.position.into())
    }
}

/// A symbolic link to create, written as `NAME=TARGET` or just `TARGET`.
#[derive(Clone)]
struct SymlinkArg(Symlink);

impl FromStr for SymlinkArg {
    type Err = mkdmg::Error;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (name, target) = match text.split_once('=') {
            Some((name, target)) => (name.to_owned(), PathBuf::from(target)),
            None => {
                let target = PathBuf::from(text);
                let name = target
                    .file_name()
                    .and_then(|name| name.to_str())
                    .ok_or_else(|| mkdmg::Error::NamelessSource {
                        path: target.clone(),
                    })?
                    .to_owned();

                (name, target)
            }
        };

        Ok(Self(Symlink { name, target }))
    }
}

impl From<SymlinkArg> for Symlink {
    fn from(arg: SymlinkArg) -> Self {
        arg.0
    }
}

fn main() -> ExitCode {
    match run(Args::parse()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("mkdmg: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(args: Args) -> mkdmg::Result<()> {
    let mut symlinks: Vec<Symlink> = args.symlink.into_iter().map(Into::into).collect();
    let mut icons: Vec<(String, Point)> = args.icon.into_iter().map(Into::into).collect();

    if let Some(position) = args.app_drop_link {
        let name = "Applications".to_owned();

        symlinks.push(Symlink {
            name: name.clone(),
            target: PathBuf::from("/Applications"),
        });
        icons.push((name, position.into()));
    }

    Dmg {
        volume_name: args
            .volname
            .unwrap_or_else(|| default_volume_name(&args.output)),
        output: args.output,
        contents: args.sources,
        symlinks,
        icons,
        background: args.background,
        volume_icon: args.volume_icon,
        window: WindowRect {
            x: args.window_position.first,
            y: args.window_position.second,
            width: args.window_size.first,
            height: args.window_size.second,
        },
        icon_size: args.icon_size,
        text_size: args.text_size,
        filesystem: args.fs.into(),
        mounting: if args.no_mount {
            Mounting::Never
        } else {
            Mounting::WhenNeeded
        },
    }
    .create()
}

fn default_volume_name(output: &Path) -> String {
    output
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Untitled")
        .to_owned()
}
