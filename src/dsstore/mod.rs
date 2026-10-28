//! Reader and writer for Apple's `.DS_Store` files.
//!
//! A `.DS_Store` is a big-endian buddy-allocated file holding a B-tree of
//! `(filename, field, value)` records. Finder reads it to restore a window's
//! size, view options, background image and icon positions.
//!
//! The format is undocumented; this module implements what Finder actually
//! writes, as observed in files produced by macOS 26.

mod error;
mod read;
mod value;
mod write;

use thiserror::Error;

pub use error::Error;
pub use value::{Field, Value};

/// Shorthand for results produced by this module.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// Size of every B-tree page, and of the block the allocator hands out for one.
pub(crate) const PAGE_SIZE: usize = 4096;

/// Offset of the file's buddy-allocated area. Every block offset is relative
/// to this position.
pub(crate) const BLOCK_BASE: usize = 4;

/// File magic, following a 32-bit alignment word.
pub(crate) const MAGIC: [u8; 4] = *b"Bud1";

/// Bytes preceding the first block: the 32-bit alignment word and the header.
pub(crate) const HEADER_LEN: usize = 36;

/// Offset of the table of contents within the allocator block. The space in
/// between holds the block address table.
pub(crate) const TOC_OFFSET: usize = 0x408;

/// Size of the master (DSDB) block.
pub(crate) const MASTER_SIZE: usize = 32;

/// A single Finder metadata record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Record {
    /// File or directory name, or `.` for the directory itself.
    pub name: String,
    pub field: Field,
    pub value: Value,
}

impl Record {
    pub fn new(name: impl Into<String>, field: Field, value: impl Into<Value>) -> Self {
        Self {
            name: name.into(),
            field,
            value: value.into(),
        }
    }
}

/// An ordered collection of records that serialises to a `.DS_Store` file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Store {
    records: Vec<Record>,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// Parse a `.DS_Store` file.
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        read::parse(bytes)
    }

    /// Serialize into a complete `.DS_Store` file image.
    pub fn to_bytes(&self) -> Result<Vec<u8>> {
        write::serialize(&self.records)
    }

    /// Records in the order they were added; serialisation re-sorts them.
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    pub fn get(&self, name: &str, field: Field) -> Option<&Value> {
        self.records
            .iter()
            .find(|record| record.name == name && record.field == field)
            .map(|record| &record.value)
    }

    /// Replace the record for `(name, field)`, or append it.
    pub fn set(&mut self, name: impl Into<String>, field: Field, value: impl Into<Value>) {
        self.insert(Record::new(name, field, value));
    }

    /// Replace the record for the same `(name, field)`, or append it.
    ///
    /// A malformed store may hold duplicates; all of them are dropped so that
    /// the written file stays canonical.
    pub fn insert(&mut self, record: Record) {
        self.records
            .retain(|existing| existing.name != record.name || existing.field != record.field);
        self.records.push(record);
    }

    /// Remove every record for `(name, field)`, returning how many were found.
    pub fn remove(&mut self, name: &str, field: Field) -> usize {
        let before = self.records.len();
        self.records
            .retain(|record| record.name != name || record.field != field);
        before - self.records.len()
    }
}

impl From<Vec<Record>> for Store {
    fn from(records: Vec<Record>) -> Self {
        Self { records }
    }
}
