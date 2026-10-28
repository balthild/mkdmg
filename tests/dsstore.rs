use mkdmg::dsstore::{Field, Record, Store, Value};

/// A `.DS_Store` written by Finder for a two-item volume with a background.
fn finder_store() -> Store {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/finder-hfs.ds");

    Store::parse(&std::fs::read(path).expect("fixture")).expect("parse fixture")
}

fn icon_position(store: &Store, name: &str) -> (u32, u32) {
    let Value::Blob(blob) = store.get(name, Field::ILOC).expect("Iloc record") else {
        panic!("Iloc is not a blob");
    };

    let x = u32::from_be_bytes(blob[0..4].try_into().unwrap());
    let y = u32::from_be_bytes(blob[4..8].try_into().unwrap());

    (x, y)
}

#[test]
fn parses_a_finder_written_store() {
    let store = finder_store();
    let names: Vec<&str> = store.records().iter().map(|r| r.name.as_str()).collect();

    assert_eq!(
        names,
        [".", ".", ".", "Applications", "ArtCraft Launcher.app"]
    );
    assert_eq!(store.get(".", Field::VSRN), Some(&Value::Long(1)));
    assert_eq!(icon_position(&store, "Applications"), (574, 205));
    assert_eq!(icon_position(&store, "ArtCraft Launcher.app"), (326, 205));

    let Value::Blob(bwsp) = store.get(".", Field::BWSP).expect("bwsp record") else {
        panic!("bwsp is not a blob");
    };
    assert!(bwsp.starts_with(b"bplist00"));
}

#[test]
fn round_trips_without_losing_records() {
    let original = finder_store();
    let written = original.to_bytes().expect("serialize");

    assert_eq!(
        written.len(),
        8196,
        "single page, allocator, master and header"
    );

    let reparsed = Store::parse(&written).expect("reparse");
    assert_eq!(reparsed, original);
}

#[test]
fn sorts_records_by_lowercased_name_then_field() {
    let store = Store::from(vec![
        Record::new("beta", Field::new(*b"Iloc"), vec![0u8; 16]),
        Record::new(".", Field::VSRN, 1u32),
        Record::new("Beta", Field::new(*b"Iloc"), vec![0u8; 16]),
        Record::new(".", Field::BWSP, Vec::<u8>::new()),
    ]);

    let reparsed = Store::parse(&store.to_bytes().expect("serialize")).expect("reparse");
    let keys: Vec<(&str, Field)> = reparsed
        .records()
        .iter()
        .map(|record| (record.name.as_str(), record.field))
        .collect();

    assert_eq!(
        keys,
        [
            (".", Field::BWSP),
            (".", Field::VSRN),
            ("Beta", Field::new(*b"Iloc")),
            ("beta", Field::new(*b"Iloc")),
        ]
    );
}

#[test]
fn writes_stores_with_text_and_booleans() {
    let store = Store::from(vec![
        Record::new("Grüße", Field::new(*b"cmmt"), "héllo"),
        Record::new(".", Field::new(*b"dscl"), false),
        Record::new(".", Field::new(*b"icvt"), 16u16),
    ]);

    let reparsed = Store::parse(&store.to_bytes().expect("serialize")).expect("reparse");

    assert_eq!(
        reparsed.get("Grüße", Field::new(*b"cmmt")),
        Some(&Value::Text("héllo".into()))
    );
    assert_eq!(
        reparsed.get(".", Field::new(*b"dscl")),
        Some(&Value::Bool(false))
    );
    assert_eq!(
        reparsed.get(".", Field::new(*b"icvt")),
        Some(&Value::Short(16))
    );
}

#[test]
fn rejects_damaged_headers() {
    let mut bytes = finder_store().to_bytes().expect("serialize");

    bytes[0] = 0x02;
    assert!(Store::parse(&bytes).is_err());

    let mut bytes = finder_store().to_bytes().expect("serialize");
    bytes[4..8].copy_from_slice(b"Bud2");
    assert!(Store::parse(&bytes).is_err());

    let mut bytes = finder_store().to_bytes().expect("serialize");
    bytes.truncate(16);
    assert!(Store::parse(&bytes).is_err());
}

#[test]
fn refuses_more_records_than_a_page_holds() {
    let records: Vec<Record> = (0..500)
        .map(|index| {
            Record::new(
                format!("item-{index:04}"),
                Field::new(*b"cmmt"),
                "x".repeat(40),
            )
        })
        .collect();

    let error = Store::from(records).to_bytes().expect_err("must not fit");

    assert!(error.to_string().contains("a page holds"));
}
