use std::{
    path::{Path, PathBuf},
    process::ExitCode,
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
    #[arg(long, value_name = "X,Y", default_value = "100,100")]
    window_position: Pair,

    /// Window size as W,H.
    #[arg(long, value_name = "W,H", default_value = "640,480")]
    window_size: Pair,

    /// Icon size, up to 128.
    #[arg(long, value_name = "POINTS", default_value_t = 128)]
    icon_size: u16,

    /// Label text size, between 10 and 16.
    #[arg(long, value_name = "POINTS", default_value_t = 16)]
    text_size: u16,

    /// Position an entry's icon, given as NAME X Y.
    #[arg(long, value_name = "NAME", num_args = 3, action = clap::ArgAction::Append)]
    icon: Vec<String>,

    /// Add a shortcut to the Applications folder at X Y.
    #[arg(long, value_names = ["X", "Y"], num_args = 2)]
    app_drop_link: Option<Vec<u32>>,

    /// Create a symbolic link, given as NAME=TARGET or just TARGET.
    #[arg(long, value_name = "NAME=TARGET", action = clap::ArgAction::Append)]
    symlink: Vec<String>,

    /// Filesystem to format the image with.
    #[arg(long, value_enum, default_value_t = Filesystem::HfsPlus)]
    fs: Filesystem,

    /// Never mount the image; the volume icon is then unavailable.
    #[arg(long)]
    no_mount: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Filesystem {
    /// HFS+, readable by older systems and more third-party tools.
    #[value(name = "hfs+")]
    HfsPlus,
    /// APFS.
    Apfs,
}

impl From<Filesystem> for FileSystem {
    fn from(value: Filesystem) -> Self {
        match value {
            Filesystem::HfsPlus => Self::HfsPlus,
            Filesystem::Apfs => Self::Apfs,
        }
    }
}

/// Two numbers written as `a,b`.
#[derive(Clone, Copy)]
struct Pair {
    first: f64,
    second: f64,
}

impl std::str::FromStr for Pair {
    type Err = &'static str;

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let (first, second) = text.split_once(',').ok_or("expected two numbers as a,b")?;

        Ok(Self {
            first: first.trim().parse().map_err(|_| "not a number")?,
            second: second.trim().parse().map_err(|_| "not a number")?,
        })
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
    let mut symlinks: Vec<Symlink> = Vec::new();
    let mut icons: Vec<(String, Point)> = Vec::new();

    for spec in &args.symlink {
        symlinks.push(parse_symlink(spec)?);
    }

    for triple in args.icon.chunks(3) {
        let name = triple[0].clone();
        let x = parse_point(&triple[1])?;
        let y = parse_point(&triple[2])?;

        icons.push((name, Point { x, y }));
    }

    if let Some(position) = &args.app_drop_link {
        let name = "Applications".to_owned();

        symlinks.push(Symlink {
            name: name.clone(),
            target: PathBuf::from("/Applications"),
        });
        icons.push((
            name,
            Point {
                x: position[0],
                y: position[1],
            },
        ));
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

/// Accept `NAME=TARGET`, or just `TARGET` when the name can be derived.
fn parse_symlink(spec: &str) -> Result<Symlink, mkdmg::Error> {
    let (name, target) = match spec.split_once('=') {
        Some((name, target)) => (name.to_owned(), PathBuf::from(target)),
        None => {
            let target = PathBuf::from(spec);
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

    Ok(Symlink { name, target })
}

fn default_volume_name(output: &Path) -> String {
    output
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("Untitled")
        .to_owned()
}

fn parse_point(text: &str) -> Result<u32, mkdmg::Error> {
    text.parse().map_err(|_| mkdmg::Error::InvalidArgument {
        value: text.to_owned(),
        expected: "icon coordinate",
    })
}
