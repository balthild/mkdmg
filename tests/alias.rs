use mkdmg::alias::Alias;

fn sample() -> Vec<u8> {
    Alias {
        volume_name: "Sample",
        folder_name: ".background",
        file_name: "background.tiff",
    }
    .encode()
    .expect("encode")
}

/// Walk the record far enough to list the extras tags.
fn extras_tags(record: &[u8]) -> Vec<i16> {
    // The fixed header ends here: 10 bytes of prologue plus 142 bytes of
    // Finder's version-2 fields.
    let mut position = 150;
    let mut tags = Vec::new();

    loop {
        let tag = i16::from_be_bytes(record[position..position + 2].try_into().unwrap());
        position += 2;
        if tag == -1 {
            return tags;
        }

        let length = u16::from_be_bytes(record[position..position + 2].try_into().unwrap());
        position += 2 + length as usize + (length as usize % 2);
        tags.push(tag);
    }
}

#[test]
fn writes_a_self_describing_record() {
    let record = sample();

    assert_eq!(&record[..4], &[0; 4]);
    assert_eq!(
        u16::from_be_bytes(record[4..6].try_into().unwrap()) as usize,
        record.len()
    );
    assert_eq!(u16::from_be_bytes(record[6..8].try_into().unwrap()), 2);

    assert_eq!(record[10], 6, "volume name length");
    assert_eq!(&record[11..17], b"Sample");

    let name_offset = 0x32;
    assert_eq!(record[name_offset], 15, "file name length");
    assert_eq!(
        &record[name_offset + 1..name_offset + 16],
        b"background.tiff"
    );
}

#[test]
fn writes_the_extras_finder_expects() {
    assert_eq!(extras_tags(&sample()), [0, 16, 17, 14, 15, 18, 19]);
}

#[test]
fn keeps_the_record_length_even_free() {
    // An odd-length folder name must be padded, or later extras shift.
    let record = Alias {
        volume_name: "Sample",
        folder_name: ".bg",
        file_name: "a.tiff",
    }
    .encode()
    .expect("encode");

    assert_eq!(extras_tags(&record), [0, 16, 17, 14, 15, 18, 19]);
    assert_eq!(record.len() % 2, 0);
}

#[test]
fn rejects_names_that_do_not_fit() {
    let error = Alias {
        volume_name: &"v".repeat(28),
        folder_name: ".background",
        file_name: "background.tiff",
    }
    .encode()
    .expect_err("volume name is too long");

    assert!(error.to_string().contains("volume name"));

    let error = Alias {
        volume_name: "Sample",
        folder_name: ".background",
        file_name: &"f".repeat(64),
    }
    .encode()
    .expect_err("file name is too long");

    assert!(error.to_string().contains("file name"));
}
