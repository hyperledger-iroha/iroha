//! Carrier framing, schema binding, value bounds and original-resource regressions.

use std::collections::BTreeMap;

use norito::core::{DecodeAttemptErrorKind, SerializePayload, WireLayout};

use super::*;

#[path = "tests/keyed_layout.rs"]
mod keyed_layout;

#[test]
fn validated_payload_borrows_the_original_canonical_record_and_refuses_suffixes() {
    let schema = CarrierSchemaV1 {
        nodes: vec![Node::U16],
    };
    let frame = encode_carrier_v1(&schema, &CarrierValueV1::U16(256)).unwrap();
    let payload = validated_carrier_payload_v1(&schema, &frame).unwrap();
    assert_eq!(payload, [0, 1]);
    let offset = payload.as_ptr() as usize - frame.as_ptr() as usize;
    assert_eq!(payload.as_ptr(), frame[offset..].as_ptr());
    assert!(offset + payload.len() <= frame.len());
    let mut suffixed = frame;
    suffixed.push(0);
    invalid(validated_carrier_payload_v1(&schema, &suffixed));
}
use crate::runtime::definition::{
    CarrierFieldV1 as Field, CarrierNodeV1 as Node, CarrierVariantV1 as Variant, MAX_CARRIER_DEPTH,
};

fn schema(nodes: Vec<Node>) -> CarrierSchemaV1 {
    CarrierSchemaV1 { nodes }
}

fn bare<T: SerializePayload>(value: &T) -> Vec<u8> {
    norito::codec::encode_bare_with_layout(value, WireLayout::Compact)
        .expect("native compact value")
}

fn frame(schema: &CarrierSchemaV1, payload: Vec<u8>) -> Vec<u8> {
    norito::encode_canonical(&CarrierRecordV1 {
        schema_hash: carrier_schema_hash_v1(schema).expect("schema"),
        payload,
    })
    .expect("record")
}

fn invalid<T: std::fmt::Debug>(result: Result<T, DecodeAttemptError>) {
    assert_eq!(result.unwrap_err().kind(), DecodeAttemptErrorKind::Invalid);
}

fn roundtrip(schema: &CarrierSchemaV1, value: &CarrierValueV1) -> Vec<u8> {
    let encoded = encode_carrier_v1(schema, value).expect("encode carrier");
    assert_eq!(
        decode_carrier_v1(schema, &encoded).expect("decode carrier"),
        *value
    );
    let payload =
        validated_carrier_payload_v1(schema, &encoded).expect("borrowed validation parity");
    let record: CarrierRecordV1 = norito::decode_canonical(&encoded).unwrap();
    assert_eq!(payload, record.payload);
    if payload.len() <= MAX_CARRIER_KEY_BYTES {
        assert_eq!(validated_carrier_key_v1(schema, payload).unwrap(), payload);
    }
    encoded
}

#[test]
fn all_scalar_nodes_use_their_exact_native_compact_codec() {
    macro_rules! scalar {
        ($node:expr, $variant:ident, $value:expr) => {{
            let native = $value;
            let schema = schema(vec![$node]);
            let value = CarrierValueV1::$variant(native.clone());
            let payload = encode_carrier_key_v1(&schema, &value).unwrap();
            assert_eq!(payload, bare(&native));
            assert_eq!(decode_carrier_key_v1(&schema, &payload).unwrap(), value);
            roundtrip(&schema, &value);
        }};
    }
    scalar!(Node::Bool, Bool, true);
    scalar!(Node::U8, U8, u8::MAX);
    scalar!(Node::U16, U16, 256_u16);
    scalar!(Node::U32, U32, u32::MAX);
    scalar!(Node::U64, U64, u64::MAX);
    scalar!(Node::U128, U128, u128::MAX);
    scalar!(Node::I64, I64, i64::MIN);
    scalar!(
        Node::String { max_bytes: 16 },
        String,
        String::from("Iroha π")
    );
    scalar!(Node::Bytes { max_bytes: 4 }, Bytes, vec![0_u8, 128, 255]);
    scalar!(Node::Hash, Hash, Hash::new(b"carrier hash"));
    scalar!(
        Node::AccountId,
        AccountId,
        AccountId::new(
            iroha_crypto::KeyPair::from_seed(vec![7; 32], iroha_crypto::Algorithm::Ed25519)
                .public_key()
                .clone()
        )
    );
    scalar!(
        Node::AssetDefinitionId,
        AssetDefinitionId,
        AssetDefinitionId::derive_from_components(
            iroha_model_base::domain::DomainId::try_new("carrier", "universal").unwrap(),
            "coin".parse().unwrap()
        )
    );
    let unit = schema(vec![Node::Unit]);
    assert!(
        encode_carrier_key_v1(&unit, &CarrierValueV1::Unit)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        decode_carrier_key_v1(&unit, &[]).unwrap(),
        CarrierValueV1::Unit
    );
    roundtrip(&unit, &CarrierValueV1::Unit);
    invalid(decode_carrier_key_v1(&unit, &[0]));
}

#[test]
fn scalar_golden_bytes_and_byte_vector_specialization_are_explicit() {
    assert_eq!(
        encode_carrier_key_v1(&schema(vec![Node::U16]), &CarrierValueV1::U16(256)).unwrap(),
        [0, 1]
    );
    assert_eq!(
        encode_carrier_key_v1(&schema(vec![Node::I64]), &CarrierValueV1::I64(-1)).unwrap(),
        [255; 8]
    );
    assert_eq!(
        encode_carrier_key_v1(
            &schema(vec![Node::String { max_bytes: 2 }]),
            &CarrierValueV1::String("π".into())
        )
        .unwrap(),
        [2, 0xcf, 0x80]
    );
    let mut bytes = 2_u64.to_le_bytes().to_vec();
    bytes.extend_from_slice(&[5, 9]);
    assert_eq!(
        encode_carrier_key_v1(
            &schema(vec![Node::Bytes { max_bytes: 2 }]),
            &CarrierValueV1::Bytes(vec![5, 9])
        )
        .unwrap(),
        bytes
    );
    let vector = schema(vec![
        Node::U8,
        Node::Vec {
            element: 0,
            max_len: 2,
        },
    ]);
    let values = CarrierValueV1::Vec(vec![CarrierValueV1::U8(5), CarrierValueV1::U8(9)]);
    let mut expected = 2_u64.to_le_bytes().to_vec();
    expected.extend_from_slice(&[1, 5, 1, 9]);
    assert_eq!(encode_carrier_key_v1(&vector, &values).unwrap(), expected);
    assert_ne!(
        expected, bytes,
        "generic vectors always frame children; Bytes uses the native raw-byte codec"
    );
    roundtrip(&vector, &values);
}

#[test]
fn containers_follow_declared_shape_and_native_field_framing() {
    let schema = schema(vec![
        Node::U16,
        Node::Option { inner: 0 },
        Node::Vec {
            element: 1,
            max_len: 3,
        },
        Node::String { max_bytes: 16 },
        Node::Struct {
            fields: vec![
                Field {
                    name: "title".into(),
                    node: 3,
                },
                Field {
                    name: "items".into(),
                    node: 2,
                },
            ],
        },
    ]);
    let value = CarrierValueV1::Struct(vec![
        CarrierValueV1::String("carrier".into()),
        CarrierValueV1::Vec(vec![
            CarrierValueV1::Option(vec![CarrierValueV1::U16(513)]),
            CarrierValueV1::Option(vec![]),
        ]),
    ]);
    let payload = encode_carrier_key_v1(&schema, &value).unwrap();
    assert_eq!(
        payload,
        bare(&(String::from("carrier"), vec![Some(513_u16), None]))
    );
    roundtrip(&schema, &value);
    for end in 0..payload.len() {
        invalid(decode_carrier_key_v1(&schema, &payload[..end]));
    }
    let mut suffix = payload;
    suffix.push(0);
    invalid(decode_carrier_key_v1(&schema, &suffix));
    invalid(encode_carrier_v1(&schema, &CarrierValueV1::Struct(vec![])));
}

#[test]
fn enum_payload_is_always_one_framed_child_including_unit() {
    let schema = schema(vec![
        Node::Unit,
        Node::U16,
        Node::Enum {
            variants: vec![
                Variant {
                    discriminant: 7,
                    name: "empty".into(),
                    node: 0,
                },
                Variant {
                    discriminant: 19,
                    name: "number".into(),
                    node: 1,
                },
            ],
        },
    ]);
    let empty = CarrierValueV1::Enum {
        discriminant: 7,
        payload: vec![CarrierValueV1::Unit],
    };
    assert_eq!(
        encode_carrier_key_v1(&schema, &empty).unwrap(),
        [7, 0, 0, 0, 0]
    );
    let number = CarrierValueV1::Enum {
        discriminant: 19,
        payload: vec![CarrierValueV1::U16(513)],
    };
    assert_eq!(
        encode_carrier_key_v1(&schema, &number).unwrap(),
        [19, 0, 0, 0, 2, 1, 2]
    );
    roundtrip(&schema, &empty);
    roundtrip(&schema, &number);
    invalid(decode_carrier_key_v1(&schema, &[7, 0, 0, 0]));
    invalid(decode_carrier_key_v1(&schema, &[8, 0, 0, 0, 0]));
    invalid(encode_carrier_v1(
        &schema,
        &CarrierValueV1::Enum {
            discriminant: 7,
            payload: vec![],
        },
    ));
    invalid(encode_carrier_v1(
        &schema,
        &CarrierValueV1::Enum {
            discriminant: 8,
            payload: vec![CarrierValueV1::Unit],
        },
    ));
}

#[test]
fn maps_require_encoded_byte_order_even_when_native_typed_order_differs() {
    let schema = schema(vec![
        Node::U16,
        Node::Bool,
        Node::Map {
            key: 0,
            value: 1,
            max_len: 2,
        },
    ]);
    let byte_ordered = CarrierValueV1::Map(vec![
        (CarrierValueV1::U16(256), CarrierValueV1::Bool(false)),
        (CarrierValueV1::U16(1), CarrierValueV1::Bool(true)),
    ]);
    let payload = encode_carrier_key_v1(&schema, &byte_ordered).unwrap();
    let mut expected = 2_u64.to_le_bytes().to_vec();
    expected.extend_from_slice(&[2, 0, 1, 1, 0, 2, 1, 0, 1, 1]);
    assert_eq!(payload, expected);
    roundtrip(&schema, &byte_ordered);
    let typed: BTreeMap<u16, bool> = [(1, true), (256, false)].into();
    assert_ne!(payload, bare(&typed));
    invalid(decode_carrier_key_v1(&schema, &bare(&typed)));
    for entries in [
        vec![
            (CarrierValueV1::U16(1), CarrierValueV1::Bool(true)),
            (CarrierValueV1::U16(256), CarrierValueV1::Bool(false)),
        ],
        vec![
            (CarrierValueV1::U16(1), CarrierValueV1::Bool(true)),
            (CarrierValueV1::U16(1), CarrierValueV1::Bool(false)),
        ],
    ] {
        invalid(encode_carrier_v1(&schema, &CarrierValueV1::Map(entries)));
    }
    let mut duplicate = 2_u64.to_le_bytes().to_vec();
    duplicate.extend_from_slice(&[2, 1, 0, 1, 0, 2, 1, 0, 1, 1]);
    invalid(decode_carrier_key_v1(&schema, &duplicate));
}

#[test]
fn schema_identity_commits_names_bounds_and_every_node() {
    let original = schema(vec![
        Node::U8,
        Node::Struct {
            fields: vec![Field {
                name: "a".into(),
                node: 0,
            }],
        },
    ]);
    let hash = carrier_schema_hash_v1(&original).unwrap();
    let canonical = norito::encode_canonical(&original).unwrap();
    assert_eq!(
        hash,
        <[u8; 32]>::from(Hash::new_from_chunks(&[
            CARRIER_SCHEMA_HASH_DOMAIN_V1,
            &canonical
        ]))
    );
    let mut changed = original.clone();
    let Node::Struct { fields } = &mut changed.nodes[1] else {
        unreachable!()
    };
    fields[0].name = "b".into();
    assert_ne!(hash, carrier_schema_hash_v1(&changed).unwrap());
    let bytes = roundtrip(
        &original,
        &CarrierValueV1::Struct(vec![CarrierValueV1::U8(7)]),
    );
    invalid(decode_carrier_v1(&changed, &bytes));
    assert_ne!(
        carrier_schema_hash_v1(&schema(vec![Node::String { max_bytes: 1 }])).unwrap(),
        carrier_schema_hash_v1(&schema(vec![Node::String { max_bytes: 2 }])).unwrap()
    );
    invalid(carrier_schema_hash_v1(&schema(vec![])));
    invalid(carrier_schema_hash_v1(&schema(vec![Node::Option {
        inner: 0,
    }])));
}

#[test]
fn record_roundtrip_does_not_confer_payload_validity_or_allow_other_layouts() {
    let schema = schema(vec![Node::Bool]);
    let good = roundtrip(&schema, &CarrierValueV1::Bool(true));
    let record: CarrierRecordV1 = norito::decode_canonical(&good).unwrap();
    assert_eq!(norito::encode_canonical(&record).unwrap(), good);
    for layout in [WireLayout::Fixed, WireLayout::Dense] {
        invalid(decode_carrier_v1(
            &schema,
            &norito::encode_canonical_with_layout(&record, layout).unwrap(),
        ));
    }
    let bad = CarrierRecordV1 {
        schema_hash: record.schema_hash,
        payload: vec![2],
    };
    let bad_frame = norito::encode_canonical(&bad).unwrap();
    assert_eq!(
        norito::decode_canonical::<CarrierRecordV1>(&bad_frame).unwrap(),
        bad
    );
    invalid(decode_carrier_v1(&schema, &bad_frame));
    let wrong = CarrierRecordV1 {
        schema_hash: [0; 32],
        ..record
    };
    invalid(decode_carrier_v1(
        &schema,
        &norito::encode_canonical(&wrong).unwrap(),
    ));
    for end in 0..good.len() {
        invalid(decode_carrier_v1(&schema, &good[..end]));
    }
    let mut suffix = good;
    suffix.push(0);
    invalid(decode_carrier_v1(&schema, &suffix));
}

#[test]
fn malformed_leaf_tags_lengths_and_hash_markers_are_rejected() {
    invalid(decode_carrier_key_v1(&schema(vec![Node::Bool]), &[2]));
    invalid(decode_carrier_key_v1(&schema(vec![Node::U16]), &[1, 0, 0]));
    invalid(decode_carrier_key_v1(
        &schema(vec![Node::String { max_bytes: 4 }]),
        &[1, 0xff],
    ));
    invalid(decode_carrier_key_v1(
        &schema(vec![Node::String { max_bytes: 4 }]),
        &[0x81, 0, b'a'],
    ));
    invalid(decode_carrier_key_v1(&schema(vec![Node::Hash]), &[0; 32]));
    let option = schema(vec![Node::Unit, Node::Option { inner: 0 }]);
    for malformed in [&[2][..], &[0, 0][..], &[1][..], &[1, 1, 0][..]] {
        invalid(decode_carrier_key_v1(&option, malformed));
    }
    invalid(encode_carrier_v1(
        &option,
        &CarrierValueV1::Option(vec![CarrierValueV1::Unit, CarrierValueV1::Unit]),
    ));
    invalid(encode_carrier_v1(
        &schema(vec![Node::Bool]),
        &CarrierValueV1::U8(1),
    ));
}

#[test]
fn declared_lengths_and_key_bound_apply_before_reconstruction() {
    let text = schema(vec![Node::String { max_bytes: 2 }]);
    roundtrip(&text, &CarrierValueV1::String("π".into()));
    invalid(encode_carrier_v1(
        &text,
        &CarrierValueV1::String("abc".into()),
    ));
    invalid(decode_carrier_key_v1(&text, &bare(&String::from("abc"))));
    let bytes = schema(vec![Node::Bytes { max_bytes: 2 }]);
    invalid(encode_carrier_v1(
        &bytes,
        &CarrierValueV1::Bytes(vec![1, 2, 3]),
    ));
    invalid(decode_carrier_key_v1(&bytes, &3_u64.to_le_bytes()));
    let vector = schema(vec![
        Node::Unit,
        Node::Vec {
            element: 0,
            max_len: 2,
        },
    ]);
    invalid(encode_carrier_v1(
        &vector,
        &CarrierValueV1::Vec(vec![CarrierValueV1::Unit; 3]),
    ));
    invalid(decode_carrier_key_v1(&vector, &3_u64.to_le_bytes()));
    let key_schema = schema(vec![Node::Bytes { max_bytes: 4096 }]);
    let largest = CarrierValueV1::Bytes(vec![0; MAX_CARRIER_KEY_BYTES - 8]);
    let key = encode_carrier_key_v1(&key_schema, &largest).unwrap();
    assert_eq!(key.len(), MAX_CARRIER_KEY_BYTES);
    assert_eq!(decode_carrier_key_v1(&key_schema, &key).unwrap(), largest);
    invalid(encode_carrier_key_v1(
        &key_schema,
        &CarrierValueV1::Bytes(vec![0; MAX_CARRIER_KEY_BYTES - 7]),
    ));
    invalid(decode_carrier_key_v1(
        &key_schema,
        &vec![0; MAX_CARRIER_KEY_BYTES + 1],
    ));
    let map = schema(vec![
        Node::Bytes { max_bytes: 4096 },
        Node::Unit,
        Node::Map {
            key: 0,
            value: 1,
            max_len: 1,
        },
    ]);
    invalid(encode_carrier_v1(
        &map,
        &CarrierValueV1::Map(vec![(
            CarrierValueV1::Bytes(vec![0; 4096]),
            CarrierValueV1::Unit,
        )]),
    ));
    let mut oversized_key = 1_u64.to_le_bytes().to_vec();
    oversized_key.extend_from_slice(&[0x88, 0x20]); // 4104-byte native Bytes payload.
    oversized_key.extend_from_slice(&bare(&vec![0_u8; 4096]));
    oversized_key.push(0);
    invalid(decode_carrier_v1(&map, &frame(&map, oversized_key)));
}

#[test]
fn node_and_depth_bounds_count_semantic_children_even_when_payloads_are_empty() {
    let vector = schema(vec![
        Node::Unit,
        Node::Vec {
            element: 0,
            max_len: MAX_CARRIER_VALUE_NODES as u32,
        },
    ]);
    let largest = CarrierValueV1::Vec(vec![CarrierValueV1::Unit; MAX_CARRIER_VALUE_NODES - 1]);
    roundtrip(&vector, &largest);
    invalid(encode_carrier_v1(
        &vector,
        &CarrierValueV1::Vec(vec![CarrierValueV1::Unit; MAX_CARRIER_VALUE_NODES]),
    ));
    let too_many = (MAX_CARRIER_VALUE_NODES as u64).to_le_bytes();
    invalid(decode_carrier_v1(
        &vector,
        &frame(&vector, too_many.to_vec()),
    ));
    let mut nodes = vec![Node::Unit];
    let mut value = CarrierValueV1::Unit;
    for index in 0..MAX_CARRIER_DEPTH - 1 {
        nodes.push(Node::Option {
            inner: index as u32,
        });
        value = CarrierValueV1::Option(vec![value]);
    }
    roundtrip(&schema(nodes.clone()), &value);
    nodes.push(Node::Option {
        inner: MAX_CARRIER_DEPTH as u32 - 1,
    });
    invalid(carrier_schema_hash_v1(&schema(nodes)));
}

#[test]
fn complete_frame_and_schema_frames_have_independent_bounds() {
    let schema = schema(vec![Node::Bytes {
        max_bytes: MAX_CARRIER_FRAME_BYTES as u32,
    }]);
    roundtrip(
        &schema,
        &CarrierValueV1::Bytes(vec![0; MAX_CARRIER_FRAME_BYTES / 2]),
    );
    invalid(encode_carrier_v1(
        &schema,
        &CarrierValueV1::Bytes(vec![0; MAX_CARRIER_FRAME_BYTES - 8]),
    ));
    invalid(decode_carrier_v1(
        &schema,
        &vec![0; MAX_CARRIER_FRAME_BYTES + 1],
    ));
    let mut nodes = vec![Node::Unit];
    for index in 1..=128 {
        nodes.push(Node::Struct {
            fields: (0..64)
                .map(|field| Field {
                    name: format!("field_{index}_{field}"),
                    node: 0,
                })
                .collect(),
        });
    }
    let large_schema = CarrierSchemaV1 { nodes };
    large_schema.validate().unwrap();
    assert!(norito::canonical_frame_len(&large_schema).unwrap() > MAX_CARRIER_SCHEMA_FRAME_BYTES);
    invalid(carrier_schema_hash_v1(&large_schema));
}

#[test]
fn explicit_compact_layout_is_independent_of_and_restores_ambient_layout() {
    let schema = schema(vec![
        Node::U16,
        Node::Vec {
            element: 0,
            max_len: 2,
        },
    ]);
    let value = CarrierValueV1::Vec(vec![CarrierValueV1::U16(3)]);
    let expected = encode_carrier_v1(&schema, &value).unwrap();
    for layout in [WireLayout::Fixed, WireLayout::Dense] {
        let _layout = norito::core::WireLayoutGuard::enter(layout);
        let flags = norito::core::get_decode_flags();
        assert_eq!(encode_carrier_v1(&schema, &value).unwrap(), expected);
        assert_eq!(decode_carrier_v1(&schema, &expected).unwrap(), value);
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
}

#[test]
fn original_allocation_field_sequence_and_depth_refusals_are_preserved() {
    let schema = schema(vec![Node::Unit, Node::Option { inner: 0 }]);
    let value = CarrierValueV1::Option(vec![CarrierValueV1::Unit]);
    let bytes = encode_carrier_v1(&schema, &value).unwrap();
    for limits in [
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
        norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, usize::MAX),
        norito::DecodeLimits::new(0, usize::MAX, usize::MAX, usize::MAX, usize::MAX),
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0),
    ] {
        norito::with_decode_limits_scope(limits, || {
            assert_eq!(
                decode_carrier_v1(&schema, &bytes).unwrap_err().kind(),
                DecodeAttemptErrorKind::EnclosingLimit
            );
        });
    }
    // Exercise custom container nesting without an outer record's typed decoder.
    let limits = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 1);
    norito::with_decode_limits_scope(limits, || {
        let error = decode_carrier_key_v1(&schema, &[1, 0]).unwrap_err();
        assert_eq!(error.kind(), DecodeAttemptErrorKind::EnclosingLimit);
        assert!(matches!(
            error.error().decode_resource_error(),
            Some(norito::core::DecodeResourceError::NestingDepthExceeded {
                depth: 2,
                limit: 1,
                ..
            })
        ));
    });
    assert_eq!(decode_carrier_v1(&schema, &bytes).unwrap(), value);
    assert_eq!(decode_carrier_key_v1(&schema, &[1, 0]).unwrap(), value);
}

#[test]
fn allocation_free_schema_preflight_matches_the_installed_descriptor_relation() {
    let mut candidates = vec![schema(vec![]), schema(vec![Node::Unit; 257])];
    for bound in [0, 1, 1 << 20, (1 << 20) + 1] {
        candidates.extend([
            schema(vec![Node::String { max_bytes: bound }]),
            schema(vec![Node::Bytes { max_bytes: bound }]),
            schema(vec![
                Node::Unit,
                Node::Vec {
                    element: 0,
                    max_len: bound,
                },
            ]),
            schema(vec![
                Node::Unit,
                Node::Map {
                    key: 0,
                    value: 0,
                    max_len: bound,
                },
            ]),
        ]);
    }
    for reference in [0, 1, u32::MAX] {
        candidates.push(schema(vec![Node::Unit, Node::Option { inner: reference }]));
    }
    for name in [
        String::new(),
        "legal._:-@/09".into(),
        "illegal name".into(),
        "π".into(),
        "x".repeat(192),
        "x".repeat(193),
    ] {
        candidates.push(schema(vec![
            Node::Unit,
            Node::Struct {
                fields: vec![Field { name, node: 0 }],
            },
        ]));
    }
    for count in [0, 1, 64, 65] {
        candidates.push(schema(vec![
            Node::Unit,
            Node::Struct {
                fields: (0..count)
                    .map(|index| Field {
                        name: format!("field_{index}"),
                        node: 0,
                    })
                    .collect(),
            },
        ]));
    }
    candidates.push(schema(vec![
        Node::Unit,
        Node::Struct {
            fields: vec![
                Field {
                    name: "same".into(),
                    node: 0
                };
                2
            ],
        },
    ]));
    for discriminants in [[0, 1], [1, 1], [1, 0]] {
        candidates.push(schema(vec![
            Node::Unit,
            Node::Enum {
                variants: discriminants
                    .into_iter()
                    .enumerate()
                    .map(|(index, discriminant)| Variant {
                        name: format!("variant_{index}"),
                        node: 0,
                        discriminant,
                    })
                    .collect(),
            },
        ]));
    }
    let mut nodes = vec![Node::Unit];
    for index in 0..MAX_CARRIER_DEPTH {
        nodes.push(Node::Option {
            inner: index as u32,
        });
        candidates.push(schema(nodes.clone()));
    }
    let no_allocation = norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, 64);
    for candidate in candidates {
        let accepted = candidate.validate().is_ok();
        norito::with_decode_limits_scope(no_allocation, || {
            assert_eq!(codec::validate_schema(&candidate).is_ok(), accepted);
            if !accepted {
                invalid(carrier_schema_hash_v1(&candidate));
            }
        });
    }
}

#[test]
fn storage_positions_roundtrip_and_order_by_exact_definition_then_bare_key() {
    let key = CarrierStorageKeyV1 {
        storage: DefinitionId([7; 32]),
        key: vec![0, 1],
    };
    let frame = norito::encode_canonical(&key).unwrap();
    let physical = bare(&key);
    let (owner, borrowed) = borrow_carrier_storage_key_v1(&physical).unwrap();
    assert_eq!(owner, key.storage);
    assert_eq!(borrowed, key.key);
    let offset = borrowed.as_ptr() as usize - physical.as_ptr() as usize;
    assert_eq!(borrowed.as_ptr(), physical[offset..].as_ptr());
    let physical_value = bare(&frame);
    assert_eq!(
        borrow_carrier_storage_value_v1(&physical_value).unwrap(),
        frame
    );
    let mut bad_key = physical;
    bad_key.push(0);
    invalid(borrow_carrier_storage_key_v1(&bad_key));
    let mut bad_value = physical_value;
    bad_value.push(0);
    invalid(borrow_carrier_storage_value_v1(&bad_value));
    assert_eq!(
        norito::decode_canonical::<CarrierStorageKeyV1>(&frame).unwrap(),
        key
    );
    let json = norito::json::to_json(&key).unwrap();
    assert_eq!(
        norito::json::from_json::<CarrierStorageKeyV1>(&json).unwrap(),
        key
    );
    assert!(
        key < CarrierStorageKeyV1 {
            storage: key.storage,
            key: vec![1, 0]
        }
    );
    assert!(
        key < CarrierStorageKeyV1 {
            storage: DefinitionId([8; 32]),
            key: vec![]
        }
    );
}

#[test]
fn streaming_definition_identity_matches_the_existing_exact_encoding() {
    let descriptor = keyed::backing_descriptor();
    let expected = descriptor.id().unwrap();
    for layout in [WireLayout::Fixed, WireLayout::Compact, WireLayout::Dense] {
        let _guard = norito::core::WireLayoutGuard::enter(layout);
        assert_eq!(carrier_definition_id_v1(&descriptor).unwrap(), expected);
    }
}

#[test]
fn borrowed_validation_keeps_original_bytes_without_container_or_account_allocations() {
    use crate::account::{MultisigMember, MultisigPolicy};
    use crate::amx_prepare_streaming_allocations::measured;
    use norito::core::{DecodeBudgetContext, DecodeLimits};
    let account = AccountId::new_multisig(
        MultisigPolicy::new(
            2,
            [21, 22]
                .into_iter()
                .map(|seed| {
                    let key = iroha_crypto::KeyPair::from_seed(
                        vec![seed; 32],
                        iroha_crypto::Algorithm::Ed25519,
                    );
                    MultisigMember::new(key.public_key().clone(), 1).unwrap()
                })
                .collect(),
        )
        .unwrap(),
    );
    let schema = schema(vec![
        Node::AccountId,
        Node::String { max_bytes: 512 },
        Node::Bytes { max_bytes: 512 },
        Node::Struct {
            fields: vec![
                Field {
                    name: "owner".into(),
                    node: 0,
                },
                Field {
                    name: "label".into(),
                    node: 1,
                },
                Field {
                    name: "data".into(),
                    node: 2,
                },
            ],
        },
        Node::Vec {
            element: 3,
            max_len: 16,
        },
    ]);
    let value = CarrierValueV1::Vec(
        (0..8)
            .map(|_| {
                CarrierValueV1::Struct(vec![
                    CarrierValueV1::AccountId(account.clone()),
                    CarrierValueV1::String("original".repeat(32)),
                    CarrierValueV1::Bytes(vec![9; 256]),
                ])
            })
            .collect(),
    );
    let bytes = encode_carrier_v1(&schema, &value).unwrap();
    let expected = validated_carrier_payload_v1(&schema, &bytes).unwrap();
    let pool =
        iroha_allocation::AllocationBudget::new(DecodeBudgetContext::allocation_layout().size());
    let mut grant = pool
        .try_reserve(DecodeBudgetContext::allocation_layout())
        .unwrap();
    let context = DecodeBudgetContext::from_reservation(
        DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX),
        &mut grant,
    )
    .unwrap();
    pool.set_limit_bytes(0);
    let (result, allocations) =
        measured(|| context.with(|| validated_carrier_payload_v1(&schema, &bytes)));
    let payload = result.unwrap();
    assert_eq!(allocations, [0; 3]);
    assert_eq!(payload, expected);
    assert_eq!(payload.as_ptr(), expected.as_ptr());
    assert_eq!(decode_carrier_v1(&schema, &bytes).unwrap(), value);
}

#[test]
fn borrowed_validation_matches_owned_values_and_corrupt_payloads() {
    use crate::account::{MultisigMember, MultisigPolicy};
    let key = iroha_crypto::KeyPair::from_seed(vec![0x77; 32], iroha_crypto::Algorithm::Ed25519);
    let accounts = [
        AccountId::new(key.public_key().clone()),
        AccountId::new_multisig(
            MultisigPolicy::new(
                1,
                vec![MultisigMember::new(key.public_key().clone(), 1).unwrap()],
            )
            .unwrap(),
        ),
    ];
    let mut cases = vec![
        (schema(vec![Node::Bool]), CarrierValueV1::Bool(true)),
        (
            schema(vec![Node::String { max_bytes: 4 }]),
            CarrierValueV1::String("π".into()),
        ),
        (
            schema(vec![Node::Bytes { max_bytes: 4 }]),
            CarrierValueV1::Bytes(vec![1, 3]),
        ),
        (
            schema(vec![
                Node::U16,
                Node::Vec {
                    element: 0,
                    max_len: 3,
                },
            ]),
            CarrierValueV1::Vec(vec![CarrierValueV1::U16(256), CarrierValueV1::U16(9)]),
        ),
        (
            schema(vec![Node::Unit, Node::Option { inner: 0 }]),
            CarrierValueV1::Option(vec![CarrierValueV1::Unit]),
        ),
    ];
    cases.extend(accounts.into_iter().map(|account| {
        (
            schema(vec![Node::AccountId]),
            CarrierValueV1::AccountId(account),
        )
    }));
    for (schema, value) in cases {
        let payload = encode_carrier_key_v1(&schema, &value).unwrap();
        for cut in 0..=payload.len() {
            assert_eq!(
                decode_carrier_key_v1(&schema, &payload[..cut]).is_ok(),
                validated_carrier_key_v1(&schema, &payload[..cut]).is_ok(),
                "prefix {cut}"
            );
        }
        for index in 0..payload.len() {
            let mut changed = payload.clone();
            changed[index] ^= 1;
            assert_eq!(
                decode_carrier_key_v1(&schema, &changed).is_ok(),
                validated_carrier_key_v1(&schema, &changed).is_ok(),
                "mutation {index}"
            );
        }
    }
}

#[test]
fn borrowed_validation_preserves_enclosing_refusal_and_streamed_schema_identity() {
    let schema = schema(vec![
        Node::Unit,
        Node::Vec {
            element: 0,
            max_len: 3,
        },
    ]);
    let bytes =
        encode_carrier_v1(&schema, &CarrierValueV1::Vec(vec![CarrierValueV1::Unit])).unwrap();
    let encoded = norito::encode_canonical(&schema).unwrap();
    assert_eq!(
        carrier_schema_hash_v1(&schema).unwrap(),
        <[u8; 32]>::from(Hash::new_from_chunks(&[
            CARRIER_SCHEMA_HASH_DOMAIN_V1,
            &encoded
        ]))
    );
    for limits in [
        norito::DecodeLimits::new(usize::MAX, 0, usize::MAX, usize::MAX, usize::MAX),
        norito::DecodeLimits::new(usize::MAX, usize::MAX, 0, usize::MAX, usize::MAX),
        norito::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, usize::MAX, 0),
    ] {
        norito::with_decode_limits_scope(limits, || {
            let error = validated_carrier_payload_v1(&schema, &bytes).unwrap_err();
            assert_eq!(error.kind(), DecodeAttemptErrorKind::EnclosingLimit);
            assert!(error.error().decode_resource_error().is_some());
        });
    }
}
