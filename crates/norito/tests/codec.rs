#![allow(clippy::manual_div_ceil)]
use norito::{
    Decode, Encode,
    codec::{Decode as _, DecodeAll as _, Encode as _},
};

#[derive(Encode, Decode, PartialEq, Debug, iroha_schema::IntoSchema)]
struct Sample(u32);

#[derive(Encode, Decode, PartialEq, Debug, iroha_schema::IntoSchema)]
enum Color {
    Red,
    Green(u8),
    Blue(u8),
}

#[test]
fn codec_roundtrip() {
    let original = Sample(42);
    let bytes = original.encode();
    let decoded = Sample::decode(&mut &bytes[..]).expect("decode");
    assert_eq!(original, decoded);
}

#[test]
fn decode_all_roundtrip() {
    let original = Sample(7);
    let bytes = norito::to_bytes(&original).expect("encode header");
    let decoded = norito::decode_from_bytes::<Sample>(&bytes).expect("decode");
    assert_eq!(original, decoded);
}

#[test]
fn decode_all_rejects_trailing_bytes() {
    let mut bytes = norito::to_bytes(&Sample(7)).expect("encode header");
    bytes.extend_from_slice(&[1, 2, 3]);
    assert!(norito::decode_from_bytes::<Sample>(&bytes).is_err());
}

#[test]
fn string_roundtrip() {
    let value = String::from("norito");
    let bytes = value.encode();
    let decoded = String::decode_all(&mut &bytes[..]).expect("string decode");
    assert_eq!(value, decoded);
}

#[test]
fn enum_roundtrip() {
    use Color::*;
    let variants = [Red, Green(3), Blue(10)];
    for v in variants {
        let bytes = v.encode();
        let decoded = Color::decode_all(&mut &bytes[..]).expect("enum decode");
        assert_eq!(v, decoded);
    }
}

#[test]
fn enum_fixed_field_includes_length_header() {
    // Sequential layout emits an outer length header for variant payloads.
    // For `u8` variants the length header is 8 bytes followed by the payload.
    let bytes = Color::Green(7).encode();
    assert_eq!(bytes.len(), 4 + 8 + 1, "tag + length header + payload");

    let bytes2 = Color::Blue(9).encode();
    assert_eq!(bytes2.len(), 4 + 8 + 1, "tag + length header + payload");
}

#[test]
fn enum_string_field_includes_nested_lengths() {
    // Sequential layout encodes both the outer variant length and the inner string length.
    let v = Custom::Text("hi".into());
    let bytes = v.encode();
    // 4 bytes tag + 8 bytes outer length + (8 bytes inner length + 2 bytes data)
    assert_eq!(bytes.len(), 4 + 8 + 8 + 2);
}

#[test]
fn signed_roundtrip() {
    let i8_val: i8 = -1;
    let bytes = i8_val.encode();
    let decoded = i8::decode_all(&mut &bytes[..]).expect("i8 decode");
    assert_eq!(i8_val, decoded);

    let i16_val: i16 = -2;
    let bytes = i16_val.encode();
    let decoded = i16::decode_all(&mut &bytes[..]).expect("i16 decode");
    assert_eq!(i16_val, decoded);

    let i32_val: i32 = -3;
    let bytes = i32_val.encode();
    let decoded = i32::decode_all(&mut &bytes[..]).expect("i32 decode");
    assert_eq!(i32_val, decoded);

    let i64_val: i64 = -4;
    let bytes = i64_val.encode();
    let decoded = i64::decode_all(&mut &bytes[..]).expect("i64 decode");
    assert_eq!(i64_val, decoded);

    let i128_val: i128 = -5;
    let bytes = i128_val.encode();
    let decoded = i128::decode_all(&mut &bytes[..]).expect("i128 decode");
    assert_eq!(i128_val, decoded);
}

#[test]
fn unsigned_roundtrip() {
    let u8_val: u8 = 5;
    let bytes = u8_val.encode();
    let decoded = u8::decode_all(&mut &bytes[..]).expect("u8 decode");
    assert_eq!(u8_val, decoded);

    let u16_val: u16 = 6;
    let bytes = u16_val.encode();
    let decoded = u16::decode_all(&mut &bytes[..]).expect("u16 decode");
    assert_eq!(u16_val, decoded);

    let u32_val: u32 = 7;
    let bytes = u32_val.encode();
    let decoded = u32::decode_all(&mut &bytes[..]).expect("u32 decode");
    assert_eq!(u32_val, decoded);

    let u64_val: u64 = 8;
    let bytes = u64_val.encode();
    let decoded = u64::decode_all(&mut &bytes[..]).expect("u64 decode");
    assert_eq!(u64_val, decoded);

    let u128_val: u128 = 9;
    let bytes = u128_val.encode();
    let decoded = u128::decode_all(&mut &bytes[..]).expect("u128 decode");
    assert_eq!(u128_val, decoded);
}

#[test]
fn size_roundtrip() {
    let usize_val: usize = 10;
    let bytes = usize_val.encode();
    let decoded = usize::decode_all(&mut &bytes[..]).expect("usize decode");
    assert_eq!(usize_val, decoded);

    let isize_val: isize = -11;
    let bytes = isize_val.encode();
    let decoded = isize::decode_all(&mut &bytes[..]).expect("isize decode");
    assert_eq!(isize_val, decoded);
}

#[test]
fn float_roundtrip() {
    let f32_val: f32 = std::f32::consts::PI;
    let bytes = f32_val.encode();
    let decoded = f32::decode_all(&mut &bytes[..]).expect("f32 decode");
    assert_eq!(f32_val, decoded);

    let f64_val: f64 = -std::f64::consts::E;
    let bytes = f64_val.encode();
    let decoded = f64::decode_all(&mut &bytes[..]).expect("f64 decode");
    assert_eq!(f64_val, decoded);
}

#[test]
fn option_roundtrip() {
    let value = Some(9u32);
    let bytes = value.encode();
    let decoded = Option::<u32>::decode_all(&mut &bytes[..]).expect("option decode");
    assert_eq!(value, decoded);

    let none: Option<u32> = None;
    let bytes = none.encode();
    let decoded = Option::<u32>::decode_all(&mut &bytes[..]).expect("option decode");
    assert_eq!(none, decoded);

    let string_some: Option<String> = Some("hello".into());
    let bytes = string_some.encode();
    let decoded = Option::<String>::decode_all(&mut &bytes[..]).expect("option string decode");
    assert_eq!(string_some, decoded);

    let string_none: Option<String> = None;
    let bytes = string_none.encode();
    let decoded = Option::<String>::decode_all(&mut &bytes[..]).expect("option string decode");
    assert_eq!(string_none, decoded);
}

#[test]
fn bool_roundtrip() {
    for &value in &[true, false] {
        let bytes = value.encode();
        let decoded = bool::decode_all(&mut &bytes[..]).expect("bool decode");
        assert_eq!(value, decoded);
    }
}

#[test]
fn char_roundtrip() {
    let ch = '火';
    let bytes = ch.encode();
    let decoded = char::decode_all(&mut &bytes[..]).expect("char decode");
    assert_eq!(ch, decoded);
}

#[test]
fn unit_roundtrip() {
    let value = ();
    let bytes = value.encode();
    <()>::decode_all(&mut &bytes[..]).expect("unit decode");
}

#[test]
fn vec_roundtrip() {
    let value = vec![1u32, 2, 3, 4];
    let bytes = value.encode();
    let decoded = Vec::<u32>::decode_all(&mut &bytes[..]).expect("vec decode");
    assert_eq!(value, decoded);
}

#[test]
fn vec_string_roundtrip() {
    let value = vec![String::from("foo"), String::from("bar")];
    let bytes = value.encode();
    let decoded = Vec::<String>::decode_all(&mut &bytes[..]).expect("vec decode");
    assert_eq!(value, decoded);
}

#[test]
fn array_roundtrip() {
    let value = [1u32, 2, 3];
    let bytes = value.encode();
    let decoded = <[u32; 3]>::decode_all(&mut &bytes[..]).expect("array decode");
    assert_eq!(value, decoded);
}

#[test]
fn array_string_roundtrip() {
    let value: [String; 2] = [String::from("foo"), String::from("bar")];
    let bytes = value.encode();
    let decoded = <[String; 2]>::decode_all(&mut &bytes[..]).expect("array decode");
    assert_eq!(value, decoded);
}

#[test]
fn result_roundtrip() {
    let ok: Result<u32, i32> = Ok(5);
    let bytes = ok.encode();
    let decoded = Result::<u32, i32>::decode_all(&mut &bytes[..]).expect("result ok");
    assert_eq!(ok, decoded);

    let err: Result<u32, i32> = Err(-7);
    let bytes = err.encode();
    let decoded = Result::<u32, i32>::decode_all(&mut &bytes[..]).expect("result err");
    assert_eq!(err, decoded);
}

#[test]
fn result_string_roundtrip() {
    let ok: Result<String, String> = Ok("ok".into());
    let bytes = ok.encode();
    let decoded = Result::<String, String>::decode_all(&mut &bytes[..]).expect("result ok");
    assert_eq!(ok, decoded);

    let err: Result<String, String> = Err("err".into());
    let bytes = err.encode();
    let decoded = Result::<String, String>::decode_all(&mut &bytes[..]).expect("result err");
    assert_eq!(err, decoded);
}

#[test]
fn box_roundtrip() {
    let value: Box<u32> = Box::new(77);
    let bytes = value.encode();
    let decoded = Box::<u32>::decode_all(&mut &bytes[..]).expect("box decode");
    assert_eq!(value, decoded);

    let str_box: Box<String> = Box::new("boxed".into());
    let bytes = str_box.encode();
    let decoded = Box::<String>::decode_all(&mut &bytes[..]).expect("box decode");
    assert_eq!(str_box, decoded);
}

#[test]
fn rc_arc_roundtrip() {
    use std::{rc::Rc, sync::Arc};

    let rc: Rc<String> = Rc::new("rc".into());
    let bytes = rc.encode();
    let decoded = Rc::<String>::decode_all(&mut &bytes[..]).expect("rc decode");
    assert_eq!(rc, decoded);

    let arc: Arc<i32> = Arc::new(101);
    let bytes = arc.encode();
    let decoded = Arc::<i32>::decode_all(&mut &bytes[..]).expect("arc decode");
    assert_eq!(arc, decoded);
}

#[test]
fn map_roundtrip() {
    use std::collections::{BTreeMap, HashMap};

    let mut bmap = BTreeMap::new();
    bmap.insert(1u32, String::from("one"));
    bmap.insert(2, String::from("two"));
    let bytes = bmap.encode();
    let decoded = BTreeMap::<u32, String>::decode_all(&mut &bytes[..]).expect("btreemap");
    assert_eq!(bmap, decoded);

    let mut hmap = HashMap::new();
    hmap.insert(String::from("a"), 1u32);
    hmap.insert(String::from("b"), 2);
    let bytes = hmap.encode();
    let decoded = HashMap::<String, u32>::decode_all(&mut &bytes[..]).expect("hashmap");
    assert_eq!(hmap, decoded);
}

#[test]
fn set_roundtrip() {
    use std::collections::{BTreeSet, HashSet};

    let mut bset = BTreeSet::new();
    bset.insert(1u32);
    bset.insert(2);
    let bytes = bset.encode();
    let decoded = BTreeSet::<u32>::decode_all(&mut &bytes[..]).expect("btreeset");
    assert_eq!(bset, decoded);

    let mut hset = HashSet::new();
    hset.insert(String::from("a"));
    hset.insert(String::from("b"));
    let bytes = hset.encode();
    let decoded = HashSet::<String>::decode_all(&mut &bytes[..]).expect("hashset");
    assert_eq!(hset, decoded);
}

#[test]
fn vecdeque_roundtrip() {
    use std::collections::VecDeque;

    let mut deque = VecDeque::new();
    deque.push_back(String::from("a"));
    deque.push_back(String::from("b"));
    let bytes = deque.encode();
    let decoded = VecDeque::<String>::decode_all(&mut &bytes[..]).expect("vecdeque");
    assert_eq!(deque, decoded);
}

#[test]
fn linkedlist_roundtrip() {
    use std::collections::LinkedList;

    let mut list = LinkedList::new();
    list.push_back(1u32);
    list.push_back(2);
    let bytes = list.encode();
    let decoded = LinkedList::<u32>::decode_all(&mut &bytes[..]).expect("linkedlist");
    assert_eq!(list, decoded);
}

#[test]
fn binaryheap_roundtrip() {
    use std::collections::BinaryHeap;

    let mut heap = BinaryHeap::new();
    heap.push(3u32);
    heap.push(1);
    heap.push(2);
    let bytes = heap.encode();
    let decoded = BinaryHeap::<u32>::decode_all(&mut &bytes[..]).expect("binaryheap");
    assert_eq!(heap.clone().into_sorted_vec(), decoded.into_sorted_vec());
}

#[test]
fn tuple_roundtrip() {
    let value = (1u32, String::from("two"));
    let bytes = value.encode();
    let decoded = <(u32, String)>::decode_all(&mut &bytes[..]).expect("tuple decode");
    assert_eq!(value, decoded);

    let triple = (1u8, false, String::from("tri"));
    let bytes = triple.encode();
    let decoded = <(u8, bool, String)>::decode_all(&mut &bytes[..]).expect("triple decode");
    assert_eq!(triple, decoded);
}

#[test]
fn large_tuple_roundtrip() {
    let value = (
        1u8,
        2u16,
        3u32,
        4u64,
        String::from("five"),
        String::from("six"),
        true,
        7i32,
        8i64,
        9u128,
        10i128,
        String::from("twelve"),
    );
    let bytes = value.encode();
    let decoded = <(
        u8,
        u16,
        u32,
        u64,
        String,
        String,
        bool,
        i32,
        i64,
        u128,
        i128,
        String,
    )>::decode_all(&mut &bytes[..])
    .expect("large tuple decode");
    assert_eq!(value, decoded);
}

#[test]
fn decode_helper_roundtrip() {
    let original = Sample(123);
    let bytes = norito::to_bytes(&original).expect("encode header");
    let decoded = norito::decode_from_bytes::<Sample>(&bytes).expect("decode");
    assert_eq!(original, decoded);
}

#[derive(Encode, Decode, PartialEq, Debug, iroha_schema::IntoSchema)]
struct Mixed {
    name: String,
    nums: Vec<u32>,
}

#[test]
fn struct_with_variable_fields_roundtrip() {
    let value = Mixed {
        name: "hi".into(),
        nums: vec![1, 2, 3],
    };
    let bytes = norito::to_bytes(&value).expect("encode");
    eprintln!("encoded(len={})", bytes.len());
    let decoded: Mixed = norito::decode_from_bytes(&bytes).expect("mixed");
    eprintln!("decoded={decoded:?}");
    assert_eq!(value, decoded);
}

#[derive(Encode, Decode, PartialEq, Debug, iroha_schema::IntoSchema)]
enum Custom {
    #[codec(index = 5)]
    Text(String),
    Numbers(Vec<u32>),
}

#[test]
fn enum_with_custom_index_roundtrip() {
    use Custom::*;
    let variants = [Text("hello".into()), Numbers(vec![1, 2])];
    for v in variants {
        let bytes = v.encode();
        let decoded = Custom::decode_all(&mut &bytes[..]).expect("enum");
        assert_eq!(v, decoded);
    }
}

#[test]
fn cell_refcell_roundtrip() {
    use std::cell::{Cell, RefCell};

    let cell = Cell::new(9u64);
    let bytes = cell.encode();
    let decoded = Cell::<u64>::decode_all(&mut &bytes[..]).expect("cell");
    assert_eq!(cell.get(), decoded.get());

    let refcell = RefCell::new(String::from("text"));
    let bytes = refcell.encode();
    let decoded = RefCell::<String>::decode_all(&mut &bytes[..]).expect("refcell");
    assert_eq!(*refcell.borrow(), *decoded.borrow());
}

#[test]
fn phantomdata_roundtrip() {
    use core::marker::PhantomData;

    let value: PhantomData<u32> = PhantomData;
    let bytes = value.encode();
    let decoded = PhantomData::<u32>::decode_all(&mut &bytes[..]).expect("phantom");
    assert_eq!(value, decoded);
}
