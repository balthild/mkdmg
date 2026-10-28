use super::{BLOCK_BASE, Error, MAGIC, MASTER_SIZE, PAGE_SIZE, Record, Result, TOC_OFFSET, Value};

/// Smallest block the allocator tracks, as a base-2 exponent.
const MIN_LOG2: u32 = 5;

/// Base-2 exponent of the master block's size.
const MASTER_LOG2: u32 = 5;

/// Base-2 exponent of a B-tree page.
const PAGE_LOG2: u32 = 12;

/// Block ids are indices into the address table, in allocation order.
const MASTER_ID: u32 = 1;
const LEAF_ID: u32 = 2;

/// Serialize `records` into a complete `.DS_Store` image.
pub(super) fn serialize(records: &[Record]) -> Result<Vec<u8>> {
    let mut ordered = records.to_vec();
    // Finder orders by name case-insensitively, then by field code byte-wise.
    // Names that differ only in case are ordered byte-wise to stay canonical.
    ordered.sort_by_cached_key(|record| {
        (
            record.name.to_lowercase(),
            record.name.clone(),
            record.field,
        )
    });

    let leaf = encode_leaf(&ordered)?;

    for allocator_log2 in PAGE_LOG2 - 1..=PAGE_LOG2 + 4 {
        if let Some(image) = try_assemble(&leaf, allocator_log2)? {
            return Ok(image);
        }
    }

    Err(Error::AllocatorExhausted)
}

/// Build a leaf page: a null successor, the record count, then the records.
fn encode_leaf(records: &[Record]) -> Result<Vec<u8>> {
    let mut page = Vec::with_capacity(PAGE_SIZE);

    page.extend_from_slice(&0u32.to_be_bytes());
    page.extend_from_slice(&(records.len() as u32).to_be_bytes());
    for record in records {
        encode_record(record, &mut page);
    }

    if page.len() > PAGE_SIZE {
        return Err(Error::PageOverflow {
            used: page.len(),
            page: PAGE_SIZE,
        });
    }

    page.resize(PAGE_SIZE, 0);

    Ok(page)
}

/// Lay out the file with an allocator block of `allocator_log2`, or give up if
/// its bookkeeping does not fit.
fn try_assemble(leaf: &[u8], allocator_log2: u32) -> Result<Option<Vec<u8>>> {
    let mut buddy = Buddy::new();
    let allocator = buddy.allocate(allocator_log2)?;
    let master = buddy.allocate(MASTER_LOG2)?;
    let page = buddy.allocate(PAGE_LOG2)?;

    let addresses = [
        pack(allocator, allocator_log2),
        pack(master, MASTER_LOG2),
        pack(page, PAGE_LOG2),
    ];

    let bookkeeping = encode_bookkeeping(&addresses, buddy.buckets());
    if bookkeeping.len() > 1 << allocator_log2 {
        return Ok(None);
    }

    let mut image = vec![0u8; BLOCK_BASE + buddy.high_water as usize];
    write_header(&mut image, allocator, allocator_log2);
    write_block(&mut image, allocator, &bookkeeping)?;
    write_block(&mut image, master, &encode_master(leaf))?;
    write_block(&mut image, page, leaf)?;

    Ok(Some(image))
}

fn write_header(image: &mut [u8], allocator: u32, allocator_log2: u32) {
    let header = &mut image[..super::HEADER_LEN];

    header[0..4].copy_from_slice(&1u32.to_be_bytes());
    header[4..8].copy_from_slice(&MAGIC);
    header[8..12].copy_from_slice(&allocator.to_be_bytes());
    header[12..16].copy_from_slice(&(1u32 << allocator_log2).to_be_bytes());
    header[16..20].copy_from_slice(&allocator.to_be_bytes());
}

fn write_block(image: &mut [u8], offset: u32, content: &[u8]) -> Result<()> {
    let start = BLOCK_BASE + offset as usize;
    let end = start + content.len();

    let target = image
        .get_mut(start..end)
        .ok_or(Error::Internal("block lies outside the built image"))?;

    target.copy_from_slice(content);

    Ok(())
}

/// Describe the tree to Finder: a single leaf, direct from the root.
fn encode_master(leaf: &[u8]) -> Vec<u8> {
    let mut master = Vec::with_capacity(MASTER_SIZE);
    let count = u32::from_be_bytes(leaf[4..8].try_into().expect("leaf header"));

    master.extend_from_slice(&LEAF_ID.to_be_bytes());
    master.extend_from_slice(&0u32.to_be_bytes());
    master.extend_from_slice(&count.to_be_bytes());
    master.extend_from_slice(&1u32.to_be_bytes());
    master.extend_from_slice(&(PAGE_SIZE as u32).to_be_bytes());
    master
}

/// The allocator block: block address table, table of contents, free lists.
fn encode_bookkeeping(addresses: &[u32], free: &[Vec<u32>]) -> Vec<u8> {
    let mut block = Vec::new();

    block.extend_from_slice(&(addresses.len() as u32).to_be_bytes());
    block.extend_from_slice(&0u32.to_be_bytes());
    for address in addresses {
        block.extend_from_slice(&address.to_be_bytes());
    }

    // The table of contents sits at a fixed offset past the address table.
    block.resize(TOC_OFFSET, 0);

    block.extend_from_slice(&1u32.to_be_bytes());
    block.push(4);
    block.extend_from_slice(b"DSDB");
    block.extend_from_slice(&MASTER_ID.to_be_bytes());

    for bucket in free {
        block.extend_from_slice(&(bucket.len() as u32).to_be_bytes());
        for offset in bucket {
            block.extend_from_slice(&offset.to_be_bytes());
        }
    }

    block
}

/// Pack a block offset and its base-2 size into an address word.
const fn pack(offset: u32, log2: u32) -> u32 {
    offset | log2
}

fn encode_record(record: &Record, out: &mut Vec<u8>) {
    let units: Vec<u16> = record.name.encode_utf16().collect();

    out.extend_from_slice(&(units.len() as u32).to_be_bytes());
    for unit in &units {
        out.extend_from_slice(&unit.to_be_bytes());
    }

    out.extend_from_slice(&record.field.code());
    out.extend_from_slice(&record.value.tag());

    match &record.value {
        Value::Bool(value) => out.push(*value as u8),
        // A 4-byte slot holding a 16-bit value in its low half.
        Value::Short(value) => out.extend_from_slice(&(*value as u32).to_be_bytes()),
        Value::Long(value) => out.extend_from_slice(&value.to_be_bytes()),
        Value::Comp(value) | Value::AbsoluteTime(value) => {
            out.extend_from_slice(&value.to_be_bytes());
        }
        Value::Type(code) => out.extend_from_slice(code),
        Value::Blob(blob) => {
            out.extend_from_slice(&(blob.len() as u32).to_be_bytes());
            out.extend_from_slice(blob);
        }
        Value::Text(text) => {
            let units: Vec<u16> = text.encode_utf16().collect();
            out.extend_from_slice(&(units.len() as u32).to_be_bytes());
            for unit in &units {
                out.extend_from_slice(&unit.to_be_bytes());
            }
        }
    }
}

/// A buddy allocator over the file's block area.
///
/// Finder starts with one free block per size class, at offset `1 << log2`, so
/// every bucket covers `[1 << log2, 1 << (log2 + 1))` and the classes tile the
/// block area exactly.
struct Buddy {
    free: Vec<Vec<u32>>,
    high_water: u32,
}

impl Buddy {
    fn new() -> Self {
        let free = (0..32)
            .map(|log2: u32| {
                if log2 < MIN_LOG2 {
                    Vec::new()
                } else {
                    vec![1 << log2]
                }
            })
            .collect::<Vec<_>>();

        Self {
            free,
            high_water: 0,
        }
    }

    fn buckets(&self) -> &[Vec<u32>] {
        &self.free
    }

    fn allocate(&mut self, log2: u32) -> Result<u32> {
        let source = (log2..32)
            .find(|&log2| !self.free[log2 as usize].is_empty())
            .ok_or(Error::AllocatorExhausted)?;

        let bucket = &mut self.free[source as usize];
        let slot = bucket
            .iter()
            .enumerate()
            .min_by_key(|&(_, offset)| *offset)
            .map(|(slot, _)| slot)
            .expect("bucket is not empty");
        let offset = bucket.swap_remove(slot);

        for smaller in (log2..source).rev() {
            self.free[smaller as usize].push(offset + (1 << smaller));
        }

        self.free[log2 as usize].sort_unstable();
        self.high_water = self.high_water.max(offset + (1 << log2));

        Ok(offset)
    }
}
