//! Shared golden vectors for the dense layout (`norito.md`, "Dense layout").
//!
//! `fixtures/norito_dense/vectors.json` pins, for every dense rule, the bare
//! payload of one value in the current compact layout and in the dense layout,
//! plus complete frames of a 16-aligned record. SDK codecs implement the dense
//! layout against this file. Regenerate it with
//! `NORITO_UPDATE_DENSE_VECTORS=1 cargo test -p norito --test norito_group_02 dense_layout_vectors`.

use std::{collections::BTreeMap, fmt::Write as _};

use norito::{
    NoritoDeserialize, NoritoSerialize,
    codec::{decode_bare_with_layout, encode_bare_with_layout},
    core::{DeserializePayload, SerializePayload, WireLayout},
    decode_canonical_with_layout, encode_canonical_with_layout,
};

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "norito.vectors.dense.Choice")]
enum Choice {
    Unit,
    One(String),
    #[codec(index = 300)]
    Far {
        value: u64,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "norito.vectors.dense.Record")]
struct Record {
    id: u32,
    key: [u8; 4],
    digests: Vec<[u8; 4]>,
    label: Option<String>,
    choice: Choice,
}

#[derive(Clone, Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito(decode_from_slice)]
#[norito_schema(name = "norito.vectors.dense.Wide")]
struct Wide {
    value: u128,
    tag: [u8; 2],
}

/// One bare vector: a value's compact and dense payloads.
struct Vector {
    name: &'static str,
    rust_type: &'static str,
    value: &'static str,
    compact: Vec<u8>,
    dense: Vec<u8>,
}

fn vector<T>(name: &'static str, rust_type: &'static str, value: &'static str, item: &T) -> Vector
where
    T: SerializePayload + for<'de> DeserializePayload<'de> + PartialEq + std::fmt::Debug,
{
    let compact = encode_bare_with_layout(item, WireLayout::Compact).expect("compact payload");
    let dense = encode_bare_with_layout(item, WireLayout::Dense).expect("dense payload");
    let decoded: T = decode_bare_with_layout(&dense, WireLayout::Dense).expect("dense decode");
    assert_eq!(&decoded, item, "{name}");
    let decoded: T =
        decode_bare_with_layout(&compact, WireLayout::Compact).expect("compact decode");
    assert_eq!(&decoded, item, "{name}");
    Vector {
        name,
        rust_type,
        value,
        compact,
        dense,
    }
}

fn vectors() -> Vec<Vector> {
    let record = Record {
        id: 7,
        key: [1, 2, 3, 4],
        digests: vec![[0xaa; 4], [0xbb; 4]],
        label: Some("hi".to_owned()),
        choice: Choice::Far { value: 1 },
    };
    vec![
        vector("u8_vec", "Vec<u8>", "[1, 2, 3]", &vec![1_u8, 2, 3]),
        vector(
            "u8_vec_128",
            "Vec<u8>",
            "128 bytes of 0x11 (two-byte count)",
            &vec![0x11_u8; 128],
        ),
        vector("u8_array", "[u8; 4]", "[1, 2, 3, 4]", &[1_u8, 2, 3, 4]),
        vector(
            "option_u8_array",
            "Option<[u8; 4]>",
            "Some([9, 9, 9, 9])",
            &Some([9_u8; 4]),
        ),
        vector("option_none", "Option<[u8; 4]>", "None", &None::<[u8; 4]>),
        vector(
            "vec_u8_array",
            "Vec<[u8; 2]>",
            "[[7, 7], [8, 8]]",
            &vec![[7_u8; 2], [8_u8; 2]],
        ),
        vector(
            "vec_string",
            "Vec<String>",
            "[\"ab\"]",
            &vec!["ab".to_owned()],
        ),
        vector(
            "vec_string_empty",
            "Vec<String>",
            "[]",
            &Vec::<String>::new(),
        ),
        vector(
            "map_string_u16",
            "BTreeMap<String, u16>",
            "{\"k\": 5}",
            &BTreeMap::from([("k".to_owned(), 5_u16)]),
        ),
        vector("enum_unit", "Choice (Unit = 0)", "Unit", &Choice::Unit),
        vector(
            "enum_tuple",
            "Choice (One(String) = 1)",
            "One(\"hi\")",
            &Choice::One("hi".to_owned()),
        ),
        vector(
            "enum_far",
            "Choice (Far { value: u64 } = 300)",
            "Far { value: 1 }",
            &Choice::Far { value: 1 },
        ),
        vector("result_err", "Result<u8, u8>", "Err(7)", &Err::<u8, u8>(7)),
        vector(
            "record",
            "Record { id: u32, key: [u8; 4], digests: Vec<[u8; 4]>, label: Option<String>, choice: Choice }",
            "{ id: 7, key: [1, 2, 3, 4], digests: [[0xaa; 4], [0xbb; 4]], label: Some(\"hi\"), choice: Far { value: 1 } }",
            &record,
        ),
    ]
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn quoted(text: &str) -> String {
    let mut out = String::from("\"");
    for character in text.chars() {
        match character {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            other => out.push(other),
        }
    }
    out.push('"');
    out
}

fn render() -> String {
    let wide = Wide {
        value: 0x0102_0304_0506_0708_090a_0b0c_0d0e_0f10,
        tag: [0xca, 0xfe],
    };
    let compact_frame = encode_canonical_with_layout(&wide, WireLayout::Compact).unwrap();
    let dense_frame = encode_canonical_with_layout(&wide, WireLayout::Dense).unwrap();
    let decoded: Wide = decode_canonical_with_layout(&dense_frame, WireLayout::Dense).unwrap();
    assert_eq!(decoded, wide);

    let mut out = String::new();
    out.push_str("{\n");
    out.push_str("  \"layout\": \"norito.md, Dense layout (header minor 1)\",\n");
    out.push_str("  \"bare\": [\n");
    let vectors = vectors();
    for (index, vector) in vectors.iter().enumerate() {
        let comma = if index + 1 == vectors.len() { "" } else { "," };
        writeln!(
            out,
            "    {{\"name\": {}, \"type\": {}, \"value\": {}, \"compact\": \"{}\", \"dense\": \"{}\"}}{comma}",
            quoted(vector.name),
            quoted(vector.rust_type),
            quoted(vector.value),
            hex(&vector.compact),
            hex(&vector.dense),
        )
        .unwrap();
    }
    out.push_str("  ],\n");
    out.push_str("  \"frames\": [\n");
    writeln!(
        out,
        "    {{\"name\": \"wide\", \"schema_name\": \"norito.vectors.dense.Wide\", \"type\": \"Wide {{ value: u128, tag: [u8; 2] }}\", \"value\": \"{{ value: 0x0102030405060708090a0b0c0d0e0f10, tag: [0xca, 0xfe] }}\", \"compact\": \"{}\", \"dense\": \"{}\"}}",
        hex(&compact_frame),
        hex(&dense_frame),
    )
    .unwrap();
    out.push_str("  ]\n");
    out.push_str("}\n");
    out
}

#[test]
fn dense_layout_vectors_match_the_shared_fixture() {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../fixtures/norito_dense/vectors.json"
    );
    let rendered = render();
    if std::env::var_os("NORITO_UPDATE_DENSE_VECTORS").is_some() {
        std::fs::create_dir_all(std::path::Path::new(path).parent().unwrap()).unwrap();
        std::fs::write(path, &rendered).unwrap();
        return;
    }
    let committed = std::fs::read_to_string(path).expect("dense vectors fixture");
    assert_eq!(
        committed, rendered,
        "fixtures/norito_dense/vectors.json is stale; regenerate it as the module docs say"
    );
}

#[test]
fn dense_vectors_pin_each_rule() {
    let by_name = |name: &str| {
        vectors()
            .into_iter()
            .find(|vector| vector.name == name)
            .unwrap_or_else(|| panic!("vector {name}"))
    };
    assert_eq!(by_name("u8_vec").dense, [3, 1, 2, 3]);
    assert_eq!(by_name("u8_vec_128").dense[..2], [0x80, 0x01]);
    assert_eq!(by_name("u8_array").dense, [1, 2, 3, 4]);
    assert_eq!(by_name("option_u8_array").dense, [1, 4, 9, 9, 9, 9]);
    assert_eq!(by_name("vec_u8_array").dense, [2, 2, 7, 7, 2, 8, 8]);
    assert_eq!(by_name("enum_unit").dense, [0]);
    assert_eq!(by_name("enum_far").dense[..2], [0xac, 0x02]);
    assert_eq!(by_name("map_string_u16").dense, [1, 2, 1, b'k', 2, 5, 0]);
}
