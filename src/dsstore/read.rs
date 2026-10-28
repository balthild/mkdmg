use std::collections::HashSet;

use super::{
    BLOCK_BASE, Error, Field, HEADER_LEN, MAGIC, Record, Result, Store, TOC_OFFSET, Value,
};

/// Parse a complete `.DS_Store` image.
pub(super) fn parse(bytes: &[u8]) -> Result<Store> {
    let allocator = Allocator::parse(bytes)?;
    let master = Master::parse(bytes, &allocator)?;

    let mut records = Vec::with_capacity(master.records as usize);
    let mut walker = Walker {
        bytes,
        allocator: &allocator,
        visited: HashSet::new(),
        records: &mut records,
    };
    walker.node(master.root, master.height)?;

    Ok(records.into())
}

/// Where every allocated block starts, indexed by block id.
struct Allocator {
    /// Absolute file offsets of each block.
    starts: Vec<usize>,
}

impl Allocator {
    fn parse(bytes: &[u8]) -> Result<Self> {
        let alignment = word(bytes, 0, "header alignment")?;
        if alignment != 1 {
            return Err(Error::BadAlignment(alignment));
        }

        let magic = bytes
            .get(4..8)
            .ok_or(Error::Truncated { len: bytes.len() })?;
        if magic != MAGIC {
            return Err(Error::BadMagic(magic.try_into().expect("four bytes")));
        }

        let table = word(bytes, 8, "allocator offset")? as usize;
        let repeated = word(bytes, 16, "allocator offset")? as usize;
        if table != repeated {
            return Err(Error::AllocatorOffsetMismatch {
                first: table as u32,
                second: repeated as u32,
            });
        }

        let table_start = BLOCK_BASE
            .checked_add(table)
            .ok_or(Error::Internal("allocator offset overflows"))?;
        let count = word(bytes, table_start, "block count")? as usize;

        let mut starts = Vec::with_capacity(count);
        for id in 0..count {
            let at = table_start + 8 + id * 4;
            let encoded = word(bytes, at, "block address")?;
            starts.push(BLOCK_BASE + encode_offset(encoded) as usize);
        }

        Ok(Self { starts })
    }

    fn start(&self, id: usize) -> Result<usize> {
        self.starts.get(id).copied().ok_or(Error::BlockOutOfRange {
            block: id,
            len: self.starts.len(),
        })
    }
}

/// The B-tree root and its depth.
struct Master {
    root: usize,
    height: u32,
    records: u32,
}

impl Master {
    fn parse(bytes: &[u8], allocator: &Allocator) -> Result<Self> {
        let id = allocator.find_master(bytes)?;
        let at = allocator.start(id)?;

        Ok(Self {
            root: word(bytes, at, "master root")? as usize,
            height: word(bytes, at + 4, "master height")?,
            records: word(bytes, at + 8, "master record count")?,
        })
    }
}

/// Depth-first record extraction.
struct Walker<'a> {
    bytes: &'a [u8],
    allocator: &'a Allocator,
    visited: HashSet<usize>,
    records: &'a mut Vec<Record>,
}

impl Walker<'_> {
    /// Visit `id`, which sits at `height` edges above the leaves.
    fn node(&mut self, id: usize, height: u32) -> Result<()> {
        if !self.visited.insert(id) {
            return Err(Error::CyclicNode { node: id });
        }

        let start = self.allocator.start(id)?;
        let next = word(self.bytes, start, "node successor")? as usize;
        let count = word(self.bytes, start + 4, "node record count")?;
        let mut position = start + 8;

        for _ in 0..count {
            let child = if height > 0 {
                let child = word(self.bytes, position, "node child")? as usize;
                position += 4;

                Some(child)
            } else {
                None
            };

            let record = decode_record(self.bytes, &mut position)?;
            if let Some(child) = child {
                self.node(child, height - 1)?;
            }

            self.records.push(record);
        }

        if height > 0 && next != 0 {
            self.node(next, height - 1)?;
        }

        Ok(())
    }
}

impl Allocator {
    /// Look up the master block id in the table of contents.
    fn find_master(&self, bytes: &[u8]) -> Result<usize> {
        let table_start = self
            .starts
            .first()
            .copied()
            .ok_or(Error::MissingMasterBlock)?;
        let toc = table_start + TOC_OFFSET;
        let count = word(bytes, toc, "table of contents")?;

        let mut position = toc + 4;
        for _ in 0..count {
            let length = *bytes.get(position).ok_or(Error::OutOfBounds {
                what: "toc key",
                offset: position,
            })? as usize;
            position += 1;

            let key = slice(bytes, position, length, "toc key")?;
            position += length;

            let value = word(bytes, position, "toc value")? as usize;
            position += 4;

            if key == b"DSDB" {
                return Ok(value);
            }
        }

        Err(Error::MissingMasterBlock)
    }
}

fn decode_record(bytes: &[u8], position: &mut usize) -> Result<Record> {
    let name_length = word(bytes, *position, "record name length")? as usize;
    *position += 4;

    let raw = slice(bytes, *position, name_length * 2, "record name")?;
    *position += name_length * 2;

    let name = decode_utf16(raw).ok_or_else(|| Error::InvalidName(raw.to_vec()))?;

    let field = Field::new(
        slice(bytes, *position, 4, "record field")?
            .try_into()
            .expect("four bytes"),
    );
    *position += 4;

    let tag: [u8; 4] = slice(bytes, *position, 4, "record type")?
        .try_into()
        .expect("four bytes");
    *position += 4;

    let value = match &tag {
        b"bool" => {
            let byte = slice(bytes, *position, 1, "boolean value")?[0];
            *position += 1;

            Value::Bool(byte != 0)
        }
        b"shor" => {
            let raw = word(bytes, *position, "short value")?;
            *position += 4;

            Value::Short(raw as u16)
        }
        b"long" => {
            let raw = word(bytes, *position, "long value")?;
            *position += 4;

            Value::Long(raw)
        }
        b"comp" => {
            let raw = double_word(bytes, *position, "comp value")?;
            *position += 8;

            Value::Comp(raw)
        }
        b"dutc" => {
            let raw = double_word(bytes, *position, "time value")?;
            *position += 8;

            Value::AbsoluteTime(raw)
        }
        b"type" => {
            let raw = slice(bytes, *position, 4, "type value")?;
            *position += 4;

            Value::Type(raw.try_into().expect("four bytes"))
        }
        b"blob" => {
            let length = word(bytes, *position, "blob length")? as usize;
            *position += 4;

            let raw = slice(bytes, *position, length, "blob")?;
            *position += length;

            Value::Blob(raw.to_vec())
        }
        b"ustr" => {
            let length = word(bytes, *position, "text length")? as usize;
            *position += 4;

            let raw = slice(bytes, *position, length * 2, "text")?;
            *position += length * 2;

            Value::Text(decode_utf16(raw).ok_or_else(|| Error::InvalidName(raw.to_vec()))?)
        }
        other => return Err(Error::UnknownValueType(*other)),
    };

    Ok(Record { name, field, value })
}

fn decode_utf16(raw: &[u8]) -> Option<String> {
    let (pairs, _) = raw.as_chunks::<2>();
    let units: Vec<u16> = pairs
        .iter()
        .map(|pair| u16::from_be_bytes(*pair))
        .collect();

    String::from_utf16(&units).ok()
}

/// Clear the size bits of an encoded block address, leaving the offset.
const fn encode_offset(encoded: u32) -> u32 {
    (encoded >> 5) << 5
}

fn word(bytes: &[u8], at: usize, what: &'static str) -> Result<u32> {
    let raw = slice(bytes, at, 4, what)?;

    Ok(u32::from_be_bytes(raw.try_into().expect("four bytes")))
}

fn double_word(bytes: &[u8], at: usize, what: &'static str) -> Result<u64> {
    let raw = slice(bytes, at, 8, what)?;

    Ok(u64::from_be_bytes(raw.try_into().expect("eight bytes")))
}

fn slice<'a>(bytes: &'a [u8], at: usize, length: usize, what: &'static str) -> Result<&'a [u8]> {
    let end = at
        .checked_add(length)
        .ok_or(Error::OutOfBounds { what, offset: at })?;

    bytes
        .get(at..end)
        .ok_or(Error::OutOfBounds { what, offset: at })
}

const _: () = assert!(HEADER_LEN == 36);
