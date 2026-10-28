//! Build Apple disk images with a custom Finder appearance, without driving
//! the Finder or relying on an AppleScript.

pub mod alias;
mod command;
mod create;
pub mod dsstore;
mod error;
mod image;
pub mod layout;
pub mod spec;
mod stage;
mod volume;

pub use error::{Error, Result};
pub use spec::{Dmg, FileSystem, Mounting, Symlink};
