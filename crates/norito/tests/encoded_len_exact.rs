//! Tests for encoded_len_exact to ensure exact sizing and buffer preallocation.

use std::num::{NonZeroU16, NonZeroU32, NonZeroU64};

use norito::{NoritoDeserialize, NoritoSerialize, to_bytes};

#[test]
fn primitive_exact_len() {
    norito::core::reset_decode_state();
    let v: u32 = 123;
    let exact = v.encoded_len_exact().expect("exact len");
    assert_eq!(exact, 4);
    let bytes = to_bytes(&v).expect("encode");
    assert_eq!(bytes.len(), norito::core::Header::SIZE + exact);
    norito::core::reset_decode_state();
}

#[test]
fn string_exact_len_matches() {
    norito::core::reset_decode_state();
    let s = String::from("hello");
    let exact = match s.encoded_len_exact() {
        Some(len) => len,
        None => return,
    };
    let expected = norito::core::len_prefix_len(s.len()) + s.len();
    assert_eq!(exact, expected);
    let bytes = to_bytes(&s).expect("encode");
    assert_eq!(bytes.len(), norito::core::Header::SIZE + expected);
    norito::core::reset_decode_state();
}

#[derive(Debug, PartialEq, NoritoSerialize, NoritoDeserialize, iroha_schema::IntoSchema)]
struct S {
    a: u32,
    b: String,
}

#[test]
fn derive_struct_exact_len() {
    norito::core::reset_decode_state();
    let s = S {
        a: 7,
        b: "xyz".into(),
    };
    let inner_a = 4usize;
    let inner_b = norito::core::len_prefix_len(3) + 3;
    let expected = norito::core::len_prefix_len(inner_a)
        + inner_a
        + norito::core::len_prefix_len(inner_b)
        + inner_b;
    let exact = match s.encoded_len_exact() {
        Some(len) => len,
        None => return,
    };
    assert_eq!(exact, expected);
    let bytes = to_bytes(&s).expect("encode");
    assert_eq!(bytes.len(), norito::core::Header::SIZE + expected);
    norito::core::reset_decode_state();
}

#[test]
fn tuple_exact_len_matches_encoded_payload() {
    norito::core::reset_decode_state();
    let tuple = (String::from("id"), vec![1_u8, 2, 3]);
    let exact = tuple.encoded_len_exact().expect("tuple exact len");
    let bytes = to_bytes(&tuple).expect("encode");
    assert_eq!(bytes.len(), norito::core::Header::SIZE + exact);
    norito::core::reset_decode_state();
}

#[test]
fn option_exact_len() {
    norito::core::reset_decode_state();
    let some = Some(5u32);
    let some_exact = some
        .encoded_len_exact()
        .expect("Some should have exact len when its value does");
    assert_eq!(
        some_exact,
        1 + norito::core::len_prefix_len(4) + 4,
        "Some encodes as discriminator, length prefix, and payload"
    );

    let none: Option<u32> = None;
    let exact = none
        .encoded_len_exact()
        .expect("None should have exact len");
    assert_eq!(exact, 1, "None encodes as only the discriminator tag");
    norito::core::reset_decode_state();
}

#[test]
fn nonzero_exact_len_matches_primitive_width() {
    norito::core::reset_decode_state();
    assert_eq!(
        NonZeroU16::new(1)
            .expect("nonzero")
            .encoded_len_exact()
            .expect("exact len"),
        2
    );
    assert_eq!(
        NonZeroU32::new(1)
            .expect("nonzero")
            .encoded_len_exact()
            .expect("exact len"),
        4
    );
    assert_eq!(
        NonZeroU64::new(1)
            .expect("nonzero")
            .encoded_len_exact()
            .expect("exact len"),
        8
    );
    norito::core::reset_decode_state();
}

#[test]
fn vec_sequential_exact_len() {
    norito::core::reset_decode_state();
    let v: Vec<u32> = vec![1, 2, 3, 4];
    let exact = match v.encoded_len_exact() {
        Some(len) => len,
        None => return,
    };
    let expected =
        norito::core::len_prefix_len(v.len()) + v.len() * (norito::core::len_prefix_len(4) + 4);
    assert_eq!(exact, expected);
    let bytes = to_bytes(&v).expect("encode");
    assert_eq!(bytes.len(), norito::core::Header::SIZE + expected);
    norito::core::reset_decode_state();
}
