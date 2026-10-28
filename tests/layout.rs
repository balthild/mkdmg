use std::io::Cursor;

use mkdmg::{
    dsstore::{Field, Store, Value},
    layout::{Layout, Point, WindowRect},
};

fn layout<'a>(background: Option<&'a str>) -> Layout<'a> {
    Layout {
        volume_name: "Sample",
        window: WindowRect {
            x: 200.0,
            y: 400.0,
            width: 660.0,
            height: 400.0,
        },
        icon_size: 128,
        text_size: 16,
        background,
    }
}

fn blob(store: &Store, field: Field) -> &[u8] {
    match store.get(".", field).expect("record") {
        Value::Blob(bytes) => bytes,
        other => panic!("{field} is {other:?}, not a blob"),
    }
}

fn plist_of(store: &Store, field: Field) -> plist::Dictionary {
    let value: plist::Value = plist::Value::from_reader(Cursor::new(blob(store, field))).unwrap();

    match value {
        plist::Value::Dictionary(dictionary) => dictionary,
        other => panic!("{field} is {other:?}, not a dictionary"),
    }
}

#[test]
fn describes_the_window_finder_expects() {
    let store = layout(Some("background.tiff"))
        .records(&[
            ("Sample.app".to_owned(), Point { x: 150, y: 190 }),
            ("Applications".to_owned(), Point { x: 450, y: 190 }),
        ])
        .expect("records");

    assert_eq!(store.get(".", Field::VSRN), Some(&Value::Long(1)));

    let window = plist_of(&store, Field::BWSP);
    assert_eq!(
        window.get("WindowBounds"),
        Some(&plist::Value::String("{{200, 400}, {660, 400}}".into()))
    );
    assert_eq!(
        window.get("ShowToolbar"),
        Some(&plist::Value::Boolean(false))
    );

    let view = plist_of(&store, Field::ICVP);
    assert_eq!(
        view.get("backgroundType"),
        Some(&plist::Value::Integer(2.into()))
    );
    assert_eq!(view.get("iconSize"), Some(&plist::Value::Real(128.0)));
    assert_eq!(
        view.get("arrangeBy"),
        Some(&plist::Value::String("none".into()))
    );
    let Some(plist::Value::Data(alias)) = view.get("backgroundImageAlias") else {
        panic!("backgroundImageAlias is missing");
    };
    assert_eq!(&alias[..4], &[0; 4], "application signature");
    assert_eq!(
        u16::from_be_bytes(alias[4..6].try_into().unwrap()) as usize,
        alias.len(),
        "record length"
    );
    assert_eq!(u16::from_be_bytes(alias[6..8].try_into().unwrap()), 2);

    assert_eq!(store.get(".", Field::ICVL), Some(&Value::Type(*b"icnv")));
}

#[test]
fn places_icons_where_asked() {
    let store = layout(None)
        .records(&[("Sample.app".to_owned(), Point { x: 150, y: 190 })])
        .expect("records");

    let Value::Blob(blob) = store.get("Sample.app", Field::ILOC).expect("Iloc") else {
        panic!("Iloc is not a blob");
    };

    assert_eq!(
        blob,
        &[
            0, 0, 0, 150, 0, 0, 0, 190, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0, 0
        ]
    );
}

#[test]
fn uses_a_solid_background_without_an_image() {
    let store = layout(None).records(&[]).expect("records");
    let view = plist_of(&store, Field::ICVP);

    assert_eq!(
        view.get("backgroundType"),
        Some(&plist::Value::Integer(1.into()))
    );
    assert_eq!(view.get("backgroundImageAlias"), None);
}

#[test]
fn keeps_fractional_window_coordinates() {
    let mut spec = layout(None);
    spec.window.x = 100.5;

    let store = spec.records(&[]).expect("records");
    let window = plist_of(&store, Field::BWSP);

    assert_eq!(
        window.get("WindowBounds"),
        Some(&plist::Value::String("{{100.5, 400}, {660, 400}}".into()))
    );
}

#[test]
fn rejects_an_empty_window() {
    let mut spec = layout(None);
    spec.window.width = 0.0;

    assert!(spec.records(&[]).is_err());
}
