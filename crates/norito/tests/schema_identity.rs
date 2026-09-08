//! Canonical identity qualification against the current frame codec.

use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::{BTreeMap, BTreeSet, BinaryHeap, HashMap, HashSet, LinkedList, VecDeque},
    marker::PhantomData,
    num::{NonZeroU16, NonZeroU32, NonZeroU64},
    rc::Rc,
    sync::Arc,
};

use norito::schema::identity::frame_hash;
use norito::{
    DeserializePayload, NoritoDeserialize, NoritoSchema, NoritoSerialize, core::Header, json,
};

mod original {
    use super::*;

    #[derive(Debug, PartialEq, NoritoSchema, NoritoSerialize, NoritoDeserialize)]
    #[norito_schema(
        name = "norito_group_06::schema_identity::original::Leaf",
        frame = "example.status.leaf"
    )]
    #[norito(schema_name = "example.status.leaf")]
    pub struct Leaf {
        pub value: u32,
    }
}

mod relocated {
    use super::*;

    #[derive(Debug, PartialEq, NoritoSchema, NoritoSerialize, NoritoDeserialize)]
    #[norito_schema(
        name = "norito_group_06::schema_identity::original::Leaf",
        frame = "example.status.leaf"
    )]
    #[norito(schema_name = "example.status.leaf")]
    pub struct Leaf {
        pub value: u32,
    }
}

#[derive(NoritoSchema)]
#[norito_schema(name = "example::Marker")]
struct Marker;

#[derive(NoritoSchema)]
#[norito_schema(name = "example::Envelope")]
struct Envelope<'a, T: ?Sized, const N: usize, const C: char, const B: bool>(PhantomData<&'a T>);

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn record<T>(name: &str, value: T) -> json::Value
where
    T: NoritoSchema + NoritoSerialize + NoritoDeserialize<'static>,
{
    frame_record(name, value, Some(<T as NoritoDeserialize>::schema_hash()))
}

fn frame_record<T: NoritoSchema + NoritoSerialize>(
    name: &str,
    value: T,
    deserialized_hash: Option<[u8; 16]>,
) -> json::Value {
    let frame = norito::to_bytes(&value).expect("capture current codec frame");
    let nominal = std::any::type_name::<T>();
    let serialized_hash = <T as NoritoSerialize>::schema_hash();
    assert_eq!(T::nominal_name(), nominal, "nominal identity for {name}");
    assert_eq!(
        frame_hash::<T>(),
        serialized_hash,
        "frame identity for {name}"
    );
    if let Some(deserialized_hash) = deserialized_hash {
        assert_eq!(
            serialized_hash, deserialized_hash,
            "both codec directions for {name}"
        );
    }
    assert_eq!(frame[6..22], serialized_hash);
    norito::json!({
        "case": name,
        "nominal": nominal,
        "frame_name": (T::frame_name()),
        "serialize_hash": (hex(&serialized_hash)),
        "deserialize_hash": (deserialized_hash.map(|hash| hex(&hash))),
        "frame_hex": (hex(&frame)),
    })
}

fn current_frames() -> Vec<json::Value> {
    let mut frames = vec![
        record("unit", ()),
        record("bool", true),
        record("char", 'λ'),
        record("u8", 7_u8),
        record("u16", 7_u16),
        record("u32", 7_u32),
        record("u64", 7_u64),
        record("u128", 7_u128),
        record("usize", 7_usize),
        record("i8", -7_i8),
        record("i16", -7_i16),
        record("i32", -7_i32),
        record("i64", -7_i64),
        record("i128", -7_i128),
        record("isize", -7_isize),
        record("f32", 1.25_f32),
        record("f64", 1.25_f64),
        record("nonzero16", NonZeroU16::new(7).unwrap()),
        record("nonzero32", NonZeroU32::new(7).unwrap()),
        record("nonzero64", NonZeroU64::new(7).unwrap()),
        record("string", String::from("abc")),
        record("borrowed_str", "abc"),
        record("cow_str", Cow::Borrowed("abc")),
        record("boxed_str", Box::<str>::from("abc")),
        frame_record("vec_borrowed_str", vec!["abc"], None),
        frame_record("vec_cow_str", vec![Cow::Borrowed("abc")], None),
        record("vec_boxed_str", vec![Box::<str>::from("abc")]),
        record("box", Box::new(7_u32)),
        record("rc", Rc::new(7_u32)),
        record("arc", Arc::new(7_u32)),
        record("cell", Cell::new(7_u32)),
        record("refcell", RefCell::new(7_u32)),
        record("option", Some(7_u32)),
        record("result", Result::<u32, String>::Err(String::from("abc"))),
        record("array", [7_u32, 8]),
        record("vec", vec![7_u32]),
        record("deque", VecDeque::from([7_u32])),
        record("list", LinkedList::from([7_u32])),
        record("heap", BinaryHeap::from([7_u32])),
        record("btreeset", BTreeSet::from([7_u32])),
        record("hashset", HashSet::from([7_u32])),
        record("btreemap", BTreeMap::from([(7_u32, 8_u64)])),
        record("hashmap", HashMap::from([(7_u32, 8_u64)])),
        record("phantom", PhantomData::<u32>),
        record("explicit_leaf", original::Leaf { value: 7 }),
        record(
            "nested_explicit_leaf",
            vec![Some(original::Leaf { value: 7 })],
        ),
    ];
    frames.push(record("tuple2", (0_u32, 1_u32)));
    frames.push(record("tuple3", (0_u32, 1_u32, 2_u32)));
    frames.push(record("tuple4", (0_u32, 1_u32, 2_u32, 3_u32)));
    frames.push(record("tuple5", (0_u32, 1_u32, 2_u32, 3_u32, 4_u32)));
    frames.push(record("tuple6", (0_u32, 1_u32, 2_u32, 3_u32, 4_u32, 5_u32)));
    frames.push(record(
        "tuple7",
        (0_u32, 1_u32, 2_u32, 3_u32, 4_u32, 5_u32, 6_u32),
    ));
    frames.push(record(
        "tuple8",
        (0_u32, 1_u32, 2_u32, 3_u32, 4_u32, 5_u32, 6_u32, 7_u32),
    ));
    frames.push(record(
        "tuple9",
        (
            0_u32, 1_u32, 2_u32, 3_u32, 4_u32, 5_u32, 6_u32, 7_u32, 8_u32,
        ),
    ));
    frames.push(record(
        "tuple10",
        (
            0_u32, 1_u32, 2_u32, 3_u32, 4_u32, 5_u32, 6_u32, 7_u32, 8_u32, 9_u32,
        ),
    ));
    frames.push(record(
        "tuple11",
        (
            0_u32, 1_u32, 2_u32, 3_u32, 4_u32, 5_u32, 6_u32, 7_u32, 8_u32, 9_u32, 10_u32,
        ),
    ));
    frames.push(record(
        "tuple12",
        (
            0_u32, 1_u32, 2_u32, 3_u32, 4_u32, 5_u32, 6_u32, 7_u32, 8_u32, 9_u32, 10_u32, 11_u32,
        ),
    ));
    frames
}

#[test]
fn current_frame_golden_matches_canonical_contract() {
    let expected: Vec<json::Value> =
        json::from_str(include_str!("fixtures/schema_identity_frames.json"))
            .expect("frame fixture");
    assert_eq!(current_frames(), expected);
}

#[test]
fn relocation_preserves_nominal_composition_and_explicit_root_projection() {
    type Before = Vec<Option<original::Leaf>>;
    type After = Vec<Option<relocated::Leaf>>;
    assert_eq!(Before::nominal_name(), After::nominal_name());
    assert_eq!(
        frame_hash::<After>(),
        <Before as NoritoSerialize>::schema_hash()
    );
    assert_eq!(original::Leaf::frame_name(), "example.status.leaf");
    assert!(!After::nominal_name().contains("example.status.leaf"));
    let old = norito::to_bytes(&vec![Some(original::Leaf { value: 9 })]).unwrap();
    let moved = norito::to_bytes(&vec![Some(relocated::Leaf { value: 9 })]).unwrap();
    assert_eq!(old[Header::SIZE..], moved[Header::SIZE..]);
    let archived = norito::from_bytes::<Before>(&old).unwrap();
    assert_eq!(
        Before::deserialize(archived),
        vec![Some(original::Leaf { value: 9 })]
    );

    // TODO: Switch active framing only after all payload identities are declared.
    // The current codec still sees the new Rust path in generic root headers.
    assert_ne!(old[6..22], moved[6..22]);
    let leaf = original::Leaf { value: 9 };
    let frame = norito::to_bytes(&leaf).unwrap();
    let archived = norito::from_bytes::<relocated::Leaf>(&frame).unwrap();
    assert_eq!(relocated::Leaf::deserialize(archived).value, 9);
    let mut wrong = frame;
    wrong[6] ^= 1;
    assert!(norito::from_bytes::<relocated::Leaf>(&wrong).is_err());
}

#[test]
fn generic_declarations_compose_type_and_const_arguments_without_codec_bounds() {
    type A = Envelope<'static, Marker, 3, 'λ', true>;
    assert_eq!(
        std::any::type_name::<A>(),
        "norito_group_06::schema_identity::Envelope<'_, norito_group_06::schema_identity::Marker, 3, 'λ', true>"
    );
    assert_eq!(
        A::nominal_name(),
        "example::Envelope<'_, example::Marker, 3, 'λ', true>"
    );
    assert_ne!(
        frame_hash::<A>(),
        frame_hash::<Envelope<'static, Marker, 4, 'λ', true>>()
    );
    assert_ne!(
        frame_hash::<A>(),
        frame_hash::<Envelope<'static, u8, 3, 'λ', true>>()
    );
    assert_ne!(
        frame_hash::<A>(),
        frame_hash::<Envelope<'static, Marker, 3, 'x', true>>()
    );
    assert_ne!(
        frame_hash::<A>(),
        frame_hash::<Envelope<'static, Marker, 3, 'λ', false>>()
    );
    assert_eq!(
        PhantomData::<Marker>::nominal_name(),
        "core::marker::PhantomData<example::Marker>"
    );
}

#[test]
fn constructor_without_arguments_is_nominal_name() {
    assert_eq!(
        norito::schema::identity::generic_name("example::Unit", &[]),
        "example::Unit"
    );
    assert_eq!(<&u32>::nominal_name(), "&u32");
    assert_eq!(str::nominal_name(), "str");
}
