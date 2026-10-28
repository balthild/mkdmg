//! Turns a description of the volume window into `.DS_Store` records.

use thiserror::Error;

use crate::{
    alias,
    dsstore::{self, Field, Store, Value},
};

/// Folder holding the window background image, relative to the volume root.
pub const BACKGROUND_FOLDER: &str = ".background";

/// Encoded into `icvp.backgroundImageAlias`; Finder draws the image stretched
/// to the window and does not scale it.
pub const BACKGROUND_TYPE_PICTURE: i64 = 2;

/// Encoded into `icvp.backgroundType` when the window uses a solid colour.
pub const BACKGROUND_TYPE_COLOUR: i64 = 1;

/// Failures while describing a window.
#[derive(Debug, Error)]
pub enum Error {
    #[error(transparent)]
    Alias(#[from] alias::Error),

    #[error(transparent)]
    DsStore(#[from] dsstore::Error),

    #[error("could not encode window settings: {0}")]
    PropertyList(#[from] plist::Error),

    #[error("window size {width}x{height} is not positive")]
    EmptyWindow { width: f64, height: f64 },
}

/// A Finder window rectangle, in AppKit coordinates.
///
/// `y` counts upwards from the bottom of the screen, which is how Finder stores
/// it. Converting a top-down position would need the screen height of whichever
/// machine later opens the image, so the value is used verbatim.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WindowRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl WindowRect {
    /// Render the `{{x, y}, {width, height}}` string AppKit expects in `bwsp`.
    fn appkit_string(&self) -> String {
        format!(
            "{{{{{x}, {y}}}, {{{width}, {height}}}}}",
            x = number(self.x),
            y = number(self.y),
            width = number(self.width),
            height = number(self.height),
        )
    }
}

/// Where an icon sits, in points from the window's top-left corner.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: u32,
    pub y: u32,
}

/// The window appearance to write for a volume.
#[derive(Debug, Clone)]
pub struct Layout<'a> {
    /// Volume name; Finder resolves the background alias through it.
    pub volume_name: &'a str,
    pub window: WindowRect,
    pub icon_size: u16,
    pub text_size: u16,
    /// Background image file name inside [`BACKGROUND_FOLDER`].
    pub background: Option<&'a str>,
}

impl Layout<'_> {
    /// Build the records Finder reads to restore the window.
    ///
    /// `items` pairs an entry in the volume with the position of its icon.
    pub fn records(&self, items: &[(String, Point)]) -> Result<Store, Error> {
        if self.window.width <= 0.0 || self.window.height <= 0.0 {
            return Err(Error::EmptyWindow {
                width: self.window.width,
                height: self.window.height,
            });
        }

        let mut store = Store::new();

        store.set(".", Field::VSRN, 1u32);
        store.set(".", Field::BWSP, self.browser_window());
        store.set(".", Field::ICVP, self.icon_view()?);
        store.set(".", Field::ICVL, Value::Type(*b"icnv"));

        for (name, position) in items {
            store.set(name.as_str(), Field::ILOC, icon_location(*position));
        }

        Ok(store)
    }

    fn browser_window(&self) -> Value {
        let mut settings = plist::Dictionary::new();

        settings.insert(
            "WindowBounds".into(),
            plist::Value::String(self.window.appkit_string()),
        );
        for key in [
            "ContainerShowSidebar",
            "ShowSidebar",
            "ShowStatusBar",
            "ShowTabView",
            "ShowToolbar",
        ] {
            settings.insert(key.into(), plist::Value::Boolean(false));
        }

        Value::Blob(binary_plist(&plist::Value::Dictionary(settings)))
    }

    fn icon_view(&self) -> Result<Value, Error> {
        let mut options = plist::Dictionary::new();

        options.insert("arrangeBy".into(), plist::Value::String("none".into()));
        options.insert(
            "backgroundType".into(),
            plist::Value::Integer(
                match self.background {
                    Some(_) => BACKGROUND_TYPE_PICTURE,
                    None => BACKGROUND_TYPE_COLOUR,
                }
                .into(),
            ),
        );
        for key in [
            "backgroundColorRed",
            "backgroundColorGreen",
            "backgroundColorBlue",
        ] {
            options.insert(key.into(), plist::Value::Real(1.0));
        }
        options.insert("gridOffsetX".into(), plist::Value::Real(0.0));
        options.insert("gridOffsetY".into(), plist::Value::Real(0.0));
        options.insert("gridSpacing".into(), plist::Value::Real(100.0));
        options.insert("iconSize".into(), plist::Value::Real(self.icon_size as f64));
        options.insert("labelOnBottom".into(), plist::Value::Boolean(true));
        options.insert("showIconPreview".into(), plist::Value::Boolean(true));
        options.insert("showItemInfo".into(), plist::Value::Boolean(false));
        options.insert("textSize".into(), plist::Value::Real(self.text_size as f64));
        options.insert("viewOptionsVersion".into(), plist::Value::Integer(1.into()));

        if let Some(background) = self.background {
            let alias = alias::Alias {
                volume_name: self.volume_name,
                folder_name: BACKGROUND_FOLDER,
                file_name: background,
            }
            .encode()?;

            options.insert("backgroundImageAlias".into(), plist::Value::Data(alias));
        }

        Ok(Value::Blob(binary_plist(&plist::Value::Dictionary(
            options,
        ))))
    }
}

/// Encode an `Iloc` payload: two coordinates and a fixed trailer.
fn icon_location(position: Point) -> Value {
    let mut blob = Vec::with_capacity(16);

    blob.extend_from_slice(&position.x.to_be_bytes());
    blob.extend_from_slice(&position.y.to_be_bytes());
    blob.extend_from_slice(&[0xFF; 6]);
    blob.extend_from_slice(&[0x00; 2]);

    Value::Blob(blob)
}

fn binary_plist(value: &plist::Value) -> Vec<u8> {
    let mut bytes = Vec::new();

    value
        .to_writer_binary(&mut bytes)
        .expect("writing a property list to memory cannot fail");

    bytes
}

/// Format a coordinate the way AppKit parses it: integers without a fraction.
fn number(value: f64) -> String {
    if value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        value.to_string()
    }
}
