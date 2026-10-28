use thiserror::Error;

/// Failures while reading or writing a `.DS_Store` file.
#[derive(Debug, Error)]
pub enum Error {
    #[error("the file is too short to hold a .DS_Store header: {len} bytes")]
    Truncated { len: usize },

    #[error("bad magic {0:?}, expected b\"Bud1\"")]
    BadMagic([u8; 4]),

    #[error("unexpected header alignment {0:#010x}")]
    BadAlignment(u32),

    #[error("the two allocator offsets disagree: {first:#x} and {second:#x}")]
    AllocatorOffsetMismatch { first: u32, second: u32 },

    #[error("block {block} is outside the {len}-entry block table")]
    BlockOutOfRange { block: usize, len: usize },

    #[error("the table of contents has no DSDB entry")]
    MissingMasterBlock,

    #[error("{what} at offset {offset:#x} extends past the end of the file")]
    OutOfBounds { what: &'static str, offset: usize },

    #[error("unknown value type {0:?}")]
    UnknownValueType([u8; 4]),

    #[error("record text is not valid UTF-16: {0:?}")]
    InvalidName(Vec<u8>),

    #[error("node {node} is referenced more than once")]
    CyclicNode { node: usize },

    #[error("records need {used} bytes but a page holds {page}")]
    PageOverflow { used: usize, page: usize },

    #[error("the allocator has no room for another block")]
    AllocatorExhausted,

    #[error("internal layout error: {0}")]
    Internal(&'static str),
}
