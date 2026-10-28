use std::fmt;

/// A four-byte record field code, such as `Iloc`.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Field([u8; 4]);

impl Field {
    /// Icon position within a Finder window.
    pub const ILOC: Self = Self(*b"Iloc");
    /// Browser window settings (a property list).
    pub const BWSP: Self = Self(*b"bwsp");
    /// Icon view options (a property list).
    pub const ICVP: Self = Self(*b"icvp");
    /// Default Finder view style (a property list).
    pub const ICVL: Self = Self(*b"icvl");
    /// Record of the Finder's own bookkeeping version.
    pub const VSRN: Self = Self(*b"vSrn");

    /// Wrap a raw four-character code.
    pub const fn new(code: [u8; 4]) -> Self {
        Self(code)
    }

    /// The raw four-character code.
    pub const fn code(self) -> [u8; 4] {
        self.0
    }
}

impl From<[u8; 4]> for Field {
    fn from(code: [u8; 4]) -> Self {
        Self(code)
    }
}

impl fmt::Debug for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Field({})", String::from_utf8_lossy(&self.0))
    }
}

impl fmt::Display for Field {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&String::from_utf8_lossy(&self.0))
    }
}

/// A record value in its on-disk representation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Bool(bool),
    /// A 32-bit slot of which only the low 16 bits carry data.
    Short(u16),
    Long(u32),
    Comp(u64),
    /// A CoreFoundation absolute time, in seconds since 2001-01-01.
    AbsoluteTime(u64),
    /// A four-character type code.
    Type([u8; 4]),
    Blob(Vec<u8>),
    Text(String),
}

impl Value {
    /// The on-disk type tag.
    pub const fn tag(&self) -> [u8; 4] {
        match self {
            Self::Bool(_) => *b"bool",
            Self::Short(_) => *b"shor",
            Self::Long(_) => *b"long",
            Self::Comp(_) => *b"comp",
            Self::AbsoluteTime(_) => *b"dutc",
            Self::Type(_) => *b"type",
            Self::Blob(_) => *b"blob",
            Self::Text(_) => *b"ustr",
        }
    }
}

impl From<bool> for Value {
    fn from(value: bool) -> Self {
        Self::Bool(value)
    }
}

impl From<u16> for Value {
    fn from(value: u16) -> Self {
        Self::Short(value)
    }
}

impl From<u32> for Value {
    fn from(value: u32) -> Self {
        Self::Long(value)
    }
}

impl From<u64> for Value {
    fn from(value: u64) -> Self {
        Self::Comp(value)
    }
}

impl From<[u8; 4]> for Value {
    fn from(value: [u8; 4]) -> Self {
        Self::Type(value)
    }
}

impl From<Vec<u8>> for Value {
    fn from(value: Vec<u8>) -> Self {
        Self::Blob(value)
    }
}

impl From<&str> for Value {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for Value {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}
