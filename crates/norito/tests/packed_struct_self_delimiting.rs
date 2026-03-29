//! Regression tests for packed-struct decoding of self-delimiting fields.

use std::collections::{BTreeMap, BTreeSet};

use norito::codec::{Decode, Encode, decode_adaptive, encode_adaptive};

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
struct NamedPackedSelfDelimiting {
    domains: BTreeSet<String>,
    alias: Option<String>,
    metadata: BTreeMap<String, String>,
}

#[derive(Debug, PartialEq, Eq, Encode, Decode)]
struct TuplePackedSelfDelimiting(BTreeSet<String>, Option<String>, Vec<String>);

#[test]
fn named_struct_roundtrips_non_empty_self_delimiting_fields() {
    let value = NamedPackedSelfDelimiting {
        domains: BTreeSet::from([String::from("wonderland")]),
        alias: Some(String::from("alice")),
        metadata: BTreeMap::from([(String::from("title"), String::from("queen"))]),
    };

    let bytes = encode_adaptive(&value);
    let decoded: NamedPackedSelfDelimiting =
        decode_adaptive(&bytes).expect("decode named packed self-delimiting fields");

    assert_eq!(decoded, value);
}

#[test]
fn tuple_struct_roundtrips_non_empty_self_delimiting_fields() {
    let value = TuplePackedSelfDelimiting(
        BTreeSet::from([String::from("wonderland")]),
        Some(String::from("alice")),
        vec![String::from("alpha"), String::from("beta")],
    );

    let bytes = encode_adaptive(&value);
    let decoded: TuplePackedSelfDelimiting =
        decode_adaptive(&bytes).expect("decode tuple packed self-delimiting fields");

    assert_eq!(decoded, value);
}
