//! Version-2 Finder alias records.
//!
//! Finder stores the path of an icon-view background image as an alias blob
//! inside `.DS_Store`. Resolution is driven by the volume name and the path
//! *within* that volume; catalog node ids, the volume creation date, the
//! filesystem signature and the recorded mount point are all repaired by
//! Finder. That is what allows a background to be described before the disk
//! image exists.

use thiserror::Error;

/// Failures while encoding an alias record.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{field} {name:?} needs {length} bytes, but an alias holds at most {max}")]
    NameTooLong {
        field: &'static str,
        name: String,
        length: usize,
        max: usize,
    },
}

/// Which fixed-width Pascal name field overflowed.
#[derive(Debug, Clone, Copy)]
enum NameField {
    Volume,
    Target,
}

impl NameField {
    const fn label(self) -> &'static str {
        match self {
            Self::Volume => "volume name",
            Self::Target => "file name",
        }
    }
}

/// Total size of the volume name field: one length byte plus 27 name bytes.
const VOLUME_NAME_FIELD: usize = 28;

/// Total size of the target name field: one length byte plus 63 name bytes.
const FILE_NAME_FIELD: usize = 64;

const VERSION: u16 = 2;
const KIND_FILE: u16 = 0;

/// Volume signature recorded for HFS volumes; Finder rewrites it as needed.
const FILESYSTEM_HFS_PLUS: [u8; 2] = *b"H+";

/// Disk type marking a removable volume.
const DISK_EJECTABLE: u16 = 5;

/// Catalog node id of a volume's root folder.
const ROOT_CATALOG_NODE: u32 = 2;

/// Attribute word Finder writes for an ordinary volume.
const VOLUME_ATTRIBUTES: u32 = 0x0000_0D02;

const TAG_CARBON_FOLDER_NAME: i16 = 0;
const TAG_HIGH_RES_VOLUME_CREATION_DATE: i16 = 16;
const TAG_HIGH_RES_CREATION_DATE: i16 = 17;
const TAG_UNICODE_FILENAME: i16 = 14;
const TAG_UNICODE_VOLUME_NAME: i16 = 15;
const TAG_POSIX_PATH: i16 = 18;
const TAG_POSIX_PATH_TO_MOUNTPOINT: i16 = 19;
const TAG_END: i16 = -1;

/// An alias pointing at a file inside a disk image's volume.
#[derive(Debug, Clone)]
pub struct Alias<'a> {
    /// Name the volume will have when mounted.
    pub volume_name: &'a str,
    /// Folder inside the volume holding the file, relative to its root.
    pub folder_name: &'a str,
    /// Name of the file itself.
    pub file_name: &'a str,
}

impl Alias<'_> {
    /// Encode a complete version-2 alias record.
    pub fn encode(&self) -> Result<Vec<u8>, Error> {
        let volume = pascal(self.volume_name, VOLUME_NAME_FIELD, NameField::Volume)?;
        let target = pascal(self.file_name, FILE_NAME_FIELD, NameField::Target)?;

        let mut record = Vec::new();

        // Application signature, a zero size placeholder and the version.
        record.extend_from_slice(&[0; 4]);
        record.extend_from_slice(&0u16.to_be_bytes());
        record.extend_from_slice(&VERSION.to_be_bytes());

        record.extend_from_slice(&KIND_FILE.to_be_bytes());
        record.extend_from_slice(&volume);
        record.extend_from_slice(&0u32.to_be_bytes());
        record.extend_from_slice(&FILESYSTEM_HFS_PLUS);
        record.extend_from_slice(&DISK_EJECTABLE.to_be_bytes());
        record.extend_from_slice(&ROOT_CATALOG_NODE.to_be_bytes());
        record.extend_from_slice(&target);
        record.extend_from_slice(&(ROOT_CATALOG_NODE + 1).to_be_bytes());
        record.extend_from_slice(&0u32.to_be_bytes());
        record.extend_from_slice(&[0; 4]);
        record.extend_from_slice(&[0; 4]);
        record.extend_from_slice(&(-1i16).to_be_bytes());
        record.extend_from_slice(&(-1i16).to_be_bytes());
        record.extend_from_slice(&VOLUME_ATTRIBUTES.to_be_bytes());
        record.extend_from_slice(&0u16.to_be_bytes());
        record.extend_from_slice(&[0; 10]);

        self.push_extras(&mut record);

        // The record is self-describing: bytes 4..6 hold its total length.
        let length = record.len() as u16;
        record[4..6].copy_from_slice(&length.to_be_bytes());

        Ok(record)
    }

    /// Extras in the order Finder writes them.
    fn push_extras(&self, record: &mut Vec<u8>) {
        let posix_path = format!("/{}/{}", self.folder_name, self.file_name);
        let mount_point = format!("/Volumes/{}", self.volume_name);

        extra(record, TAG_CARBON_FOLDER_NAME, self.folder_name.as_bytes());
        extra(record, TAG_HIGH_RES_VOLUME_CREATION_DATE, &[0; 8]);
        extra(record, TAG_HIGH_RES_CREATION_DATE, &[0; 8]);
        unicode_extra(record, TAG_UNICODE_FILENAME, self.file_name);
        unicode_extra(record, TAG_UNICODE_VOLUME_NAME, self.volume_name);
        extra(record, TAG_POSIX_PATH, posix_path.as_bytes());
        extra(record, TAG_POSIX_PATH_TO_MOUNTPOINT, mount_point.as_bytes());

        record.extend_from_slice(&TAG_END.to_be_bytes());
        record.extend_from_slice(&0u16.to_be_bytes());
    }
}

/// Encode a fixed-width Pascal name field.
fn pascal(name: &str, width: usize, field: NameField) -> Result<Vec<u8>, Error> {
    let bytes = name.as_bytes();
    if bytes.len() + 1 > width {
        return Err(Error::NameTooLong {
            field: field.label(),
            name: name.to_owned(),
            length: bytes.len(),
            max: width - 1,
        });
    }

    let mut encoded = vec![0; width];
    encoded[0] = bytes.len() as u8;
    encoded[1..bytes.len() + 1].copy_from_slice(bytes);

    Ok(encoded)
}

/// Append a tagged payload, padded so every entry stays an even number of bytes.
fn extra(record: &mut Vec<u8>, tag: i16, value: &[u8]) {
    record.extend_from_slice(&tag.to_be_bytes());
    record.extend_from_slice(&(value.len() as u16).to_be_bytes());
    record.extend_from_slice(value);

    if value.len() % 2 == 1 {
        record.push(0);
    }
}

/// Append a UTF-16BE string extra, which carries its own code-unit count.
fn unicode_extra(record: &mut Vec<u8>, tag: i16, text: &str) {
    let units: Vec<u16> = text.encode_utf16().collect();

    let mut payload = Vec::with_capacity(2 + units.len() * 2);
    payload.extend_from_slice(&(units.len() as u16).to_be_bytes());
    for unit in &units {
        payload.extend_from_slice(&unit.to_be_bytes());
    }

    extra(record, tag, &payload);
}
