//! Core Norito functionality tests.

use std::io::Write;

use byteorder::{ByteOrder, LittleEndian};
use iroha_schema::IntoSchema;
use norito::core::*;

fn crc64_test(data: &[u8]) -> u64 {
    let mut digest = crc64fast::Digest::new();
    digest.write(data);
    digest.sum64()
}

#[test]
fn deterministic_primitives() {
    let value: u32 = 123;
    let bytes1 = to_bytes(&value).unwrap();
    let bytes2 = to_bytes(&value).unwrap();
    assert_eq!(bytes1, bytes2, "serialization is deterministic");
}

#[test]
fn zero_copy_roundtrip() {
    let value: bool = true;
    let bytes = to_bytes(&value).unwrap();
    let decoded: bool = decode_from_bytes(&bytes).unwrap();
    assert_eq!(value, decoded);
}

#[test]
fn string_roundtrip() {
    let value = String::from("norito");
    let bytes = to_bytes(&value).unwrap();
    let decoded: String = decode_from_bytes(&bytes).unwrap();
    assert_eq!(value, decoded);
}

#[test]
fn str_roundtrip() {
    let value: &str = "slice";
    let bytes = to_bytes(&value).unwrap();
    let decoded = decode_from_bytes::<&str>(&bytes).unwrap();
    assert_eq!(value, decoded);
}

#[test]
fn cow_str_roundtrip() {
    use std::borrow::Cow;

    let borrowed: Cow<str> = Cow::Borrowed("borrowed");
    let bytes = to_bytes(&borrowed).unwrap();
    let decoded = decode_from_bytes::<Cow<'_, str>>(&bytes).unwrap();
    assert_eq!(borrowed, decoded);

    let owned: Cow<str> = Cow::Owned(String::from("owned"));
    let bytes = to_bytes(&owned).unwrap();
    let decoded = decode_from_bytes::<Cow<'_, str>>(&bytes).unwrap();
    assert_eq!(owned, decoded);
}

#[test]
fn size_roundtrip() {
    let usize_val: usize = 42;
    let bytes = to_bytes(&usize_val).unwrap();
    let decoded: usize = decode_from_bytes(&bytes).unwrap();
    assert_eq!(usize_val, decoded);

    let isize_val: isize = -43;
    let bytes = to_bytes(&isize_val).unwrap();
    let decoded: isize = decode_from_bytes(&bytes).unwrap();
    assert_eq!(isize_val, decoded);
}

#[test]
fn nonzero_u16_roundtrip() {
    use std::num::NonZeroU16;

    let value = NonZeroU16::new(5u16).unwrap();
    let bytes = to_bytes(&value).unwrap();
    let archived = from_bytes::<NonZeroU16>(&bytes).unwrap();
    let decoded = <NonZeroU16 as NoritoDeserialize>::deserialize(archived);
    assert_eq!(value, decoded);
}

#[test]
fn header_serialization() {
    let value: u8 = 10;
    let bytes = to_bytes(&value).unwrap();
    assert_eq!(&bytes[..4], &MAGIC);
    assert_eq!(bytes[4], VERSION_MAJOR);
    assert_eq!(bytes[5], VERSION_MINOR);
    assert_eq!(&bytes[6..22], &<u8 as NoritoSerialize>::schema_hash());
    assert_eq!(bytes[22], 0);
    let len = LittleEndian::read_u64(&bytes[23..31]);
    assert_eq!(len, 1);
    let checksum = LittleEndian::read_u64(&bytes[31..39]);
    assert_eq!(checksum, crc64_test(&bytes[Header::SIZE..]));
}

#[test]
fn checksum_validation() {
    let value: u32 = 5;
    let mut bytes = to_bytes(&value).unwrap();
    let last = bytes.len() - 1;
    bytes[last] ^= 0x01;
    assert!(matches!(
        from_bytes::<u32>(&bytes),
        Err(Error::ChecksumMismatch)
    ));
}

#[repr(C)]
struct A(u32, u32);

impl NoritoSerialize for A {
    fn serialize<W: Write>(&self, mut w: W) -> Result<(), Error> {
        self.0.serialize(&mut w)?;
        self.1.serialize(&mut w)
    }
}

#[repr(C)]
struct B(u64);

impl<'a> NoritoDeserialize<'a> for B {
    fn deserialize(archived: &'a Archived<B>) -> Self {
        let ptr = archived as *const Archived<B> as *const u64;
        B(u64::from_le(unsafe { *ptr }))
    }
}

#[test]
fn schema_mismatch() {
    let value = A(1, 2);
    let bytes = to_bytes(&value).unwrap();
    assert!(matches!(
        from_bytes::<B>(&bytes),
        Err(Error::SchemaMismatch)
    ));
}

#[test]
fn core_decode_rejects_schema_mismatch() {
    let bytes = to_bytes(&123u32).unwrap();
    let err = decode_from_bytes::<i32>(&bytes).expect_err("schema mismatch must error");
    assert!(matches!(err, Error::SchemaMismatch));
}

#[test]
fn version_error() {
    let value: u8 = 3;
    let mut bytes = to_bytes(&value).unwrap();
    bytes[4] = VERSION_MAJOR.wrapping_add(1);
    assert!(matches!(
        from_bytes::<u8>(&bytes),
        Err(Error::UnsupportedVersion { .. })
    ));
}

#[test]
fn array_roundtrip() {
    let value = [1u32, 2, 3];
    let bytes = to_bytes(&value).unwrap();
    let decoded: [u32; 3] = decode_from_bytes(&bytes).unwrap();
    assert_eq!(value, decoded);
}

#[test]
fn array_string_roundtrip() {
    let value: [String; 2] = [String::from("foo"), String::from("bar")];
    let bytes = to_bytes(&value).unwrap();
    let decoded: [String; 2] = decode_from_bytes(&bytes).unwrap();
    assert_eq!(value, decoded);
}

#[test]
fn btreemap_roundtrip() {
    use std::collections::BTreeMap;

    let mut map = BTreeMap::new();
    map.insert(1u32, String::from("one"));
    map.insert(2, String::from("two"));
    let bytes = to_bytes(&map).unwrap();
    let decoded: BTreeMap<u32, String> = decode_from_bytes(&bytes).unwrap();
    assert_eq!(map, decoded);
}

#[test]
fn hashmap_roundtrip() {
    use std::collections::HashMap;

    let mut map = HashMap::new();
    map.insert(String::from("a"), 1u32);
    map.insert(String::from("b"), 2);
    let bytes = to_bytes(&map).unwrap();
    let decoded: HashMap<String, u32> = decode_from_bytes(&bytes).unwrap();
    assert_eq!(map, decoded);
}

#[test]
fn set_roundtrip() {
    use std::collections::{BTreeSet, HashSet};

    let mut bset = BTreeSet::new();
    bset.insert(String::from("a"));
    bset.insert(String::from("b"));
    let bytes = to_bytes(&bset).unwrap();
    let decoded: BTreeSet<String> = decode_from_bytes(&bytes).unwrap();
    assert_eq!(bset, decoded);

    let mut hset = HashSet::new();
    hset.insert(1u32);
    hset.insert(2);
    let bytes = to_bytes(&hset).unwrap();
    let decoded: HashSet<u32> = decode_from_bytes(&bytes).unwrap();
    assert_eq!(hset, decoded);
}

#[test]
fn vecdeque_roundtrip() {
    use std::collections::VecDeque;

    let mut deque = VecDeque::new();
    deque.push_back(String::from("a"));
    deque.push_back(String::from("b"));
    let bytes = to_bytes(&deque).unwrap();
    let decoded: VecDeque<String> = decode_from_bytes(&bytes).unwrap();
    assert_eq!(deque, decoded);
}

#[test]
fn linkedlist_roundtrip() {
    use std::collections::LinkedList;

    let mut list = LinkedList::new();
    list.push_back(1u32);
    list.push_back(2);
    let bytes = to_bytes(&list).unwrap();
    let decoded: LinkedList<u32> = decode_from_bytes(&bytes).unwrap();
    assert_eq!(list, decoded);
}

#[test]
fn binaryheap_roundtrip() {
    use std::collections::BinaryHeap;

    let mut heap = BinaryHeap::new();
    heap.push(3u32);
    heap.push(1);
    heap.push(2);
    let bytes = to_bytes(&heap).unwrap();
    let decoded: BinaryHeap<u32> = decode_from_bytes(&bytes).unwrap();
    assert_eq!(heap.clone().into_sorted_vec(), decoded.into_sorted_vec());
}

#[test]
fn tuple_roundtrip() {
    let value = (1u32, String::from("two"));
    let bytes = to_bytes(&value).unwrap();
    let decoded: (u32, String) = decode_from_bytes(&bytes).unwrap();
    assert_eq!(value, decoded);
}

#[test]
fn triple_roundtrip() {
    let value = (1u8, false, String::from("tri"));
    let bytes = to_bytes(&value).unwrap();
    let archived = from_bytes::<(u8, bool, String)>(&bytes).unwrap();
    let decoded = <(u8, bool, String) as NoritoDeserialize>::deserialize(archived);
    assert_eq!(value, decoded);
}

#[derive(IntoSchema, NoritoSerialize, NoritoDeserialize, PartialEq, Debug)]
struct Mixed {
    name: String,
    nums: Vec<u32>,
}

#[test]
fn derive_struct_variable_roundtrip() {
    let value = Mixed {
        name: "hi".into(),
        nums: vec![1, 2, 3],
    };
    let bytes = to_bytes(&value).unwrap();
    let archived = from_bytes::<Mixed>(&bytes).unwrap();
    let decoded = Mixed::deserialize(archived);
    assert_eq!(value, decoded);
}

#[derive(IntoSchema, NoritoSerialize, NoritoDeserialize, PartialEq, Debug)]
enum Custom {
    #[codec(index = 5)]
    Text(String),
    Numbers(Vec<u32>),
}

#[test]
fn derive_enum_custom_index_roundtrip() {
    use Custom::*;
    let variants = [Text("hello".into()), Numbers(vec![1, 2])];
    for v in variants {
        let bytes = to_bytes(&v).unwrap();
        let archived = from_bytes::<Custom>(&bytes).unwrap();
        let decoded = Custom::deserialize(archived);
        assert_eq!(v, decoded);
    }
}

#[test]
fn rc_arc_roundtrip() {
    use std::{rc::Rc, sync::Arc};

    let rc = Rc::new(String::from("rc"));
    let bytes = to_bytes(&rc).unwrap();
    let archived = from_bytes::<Rc<String>>(&bytes).unwrap();
    let decoded = <Rc<String> as NoritoDeserialize>::deserialize(archived);
    assert_eq!(rc, decoded);

    let arc = Arc::new(99u64);
    let bytes = to_bytes(&arc).unwrap();
    let archived = from_bytes::<Arc<u64>>(&bytes).unwrap();
    let decoded = <Arc<u64> as NoritoDeserialize>::deserialize(archived);
    assert_eq!(arc, decoded);
}

#[test]
fn cell_refcell_roundtrip() {
    use std::cell::{Cell, RefCell};

    let cell = Cell::new(5u32);
    let bytes = to_bytes(&cell).unwrap();
    let archived = from_bytes::<Cell<u32>>(&bytes).unwrap();
    let decoded = <Cell<u32> as NoritoDeserialize>::deserialize(archived);
    assert_eq!(cell.get(), decoded.get());

    let refcell = RefCell::new(String::from("inner"));
    let bytes = to_bytes(&refcell).unwrap();
    let archived = from_bytes::<RefCell<String>>(&bytes).unwrap();
    let decoded = <RefCell<String> as NoritoDeserialize>::deserialize(archived);
    assert_eq!(*refcell.borrow(), *decoded.borrow());
}

#[test]
fn phantomdata_roundtrip() {
    use core::marker::PhantomData;

    let phantom: PhantomData<u32> = PhantomData;
    let bytes = to_bytes(&phantom).unwrap();
    let archived = from_bytes::<PhantomData<u32>>(&bytes).unwrap();
    let decoded = <PhantomData<u32> as NoritoDeserialize>::deserialize(archived);
    assert_eq!(phantom, decoded);
}
