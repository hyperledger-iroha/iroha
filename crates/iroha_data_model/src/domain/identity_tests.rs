//! Domain construction, canonical wire layouts and bounded decoding contracts.

use super::*;
use norito::{NoritoSchema, NoritoSerialize};

fn layouts() -> impl Iterator<Item = u8> {
    (0..=ncore::supported_header_flags())
        .filter(|flags| ncore::validate_header_flags(*flags).is_ok())
}

fn payload(value: &impl NoritoSerialize, flags: u8) -> Vec<u8> {
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let mut bytes = Vec::new();
    value
        .serialize(&mut ncore::Encoder::for_buffer(&mut bytes))
        .expect("serialize declared layout");
    bytes
}

fn roundtrip(id: &DomainId, flags: u8) {
    let bytes = payload(id, flags);
    let frame = ncore::frame_bare_with_header_flags::<DomainId>(&bytes, flags)
        .expect("frame declared layout");
    assert_eq!(
        norito::decode_from_bytes::<DomainId>(&frame).expect("decode canonical domain frame"),
        *id
    );
    let _flags = ncore::DecodeFlagsGuard::enter(flags);
    let (decoded, used) =
        <DomainId as ncore::DecodeFromSlice>::decode_from_slice(&bytes).expect("slice domain");
    assert_eq!(used, bytes.len());
    assert_eq!(decoded, *id);
    assert_eq!(payload(&decoded, flags), bytes);
    let (decoded, used) = ncore::decode_field_canonical::<DomainId>(&bytes)
        .expect("canonical archived domain decoder");
    assert_eq!((decoded, used), (id.clone(), bytes.len()));

    let nested = vec![Some(id.clone()), None, Some(id.clone())];
    let nested_bytes = payload(&nested, flags);
    let nested_frame =
        ncore::frame_bare_with_header_flags::<Vec<Option<DomainId>>>(&nested_bytes, flags)
            .expect("frame nested domains");
    assert_eq!(
        norito::decode_from_bytes::<Vec<Option<DomainId>>>(&nested_frame)
            .expect("decode nested domains"),
        nested
    );
}

#[test]
fn domain_identity_preserves_captured_frame_and_schema() {
    // Unchanged domain record from tests/fixtures/base_model_wire_identity_frames.json.
    let frame = hex::decode(concat!(
        "4e5254300000044c225b83d013da67e985809c84abcc001100000000000000",
        "a114d9f8ff20f9390208076172636869766507067061796e6574"
    ))
    .unwrap();
    let id = DomainId::try_new("archive", "paynet").unwrap();
    assert_eq!(norito::to_bytes(&id).unwrap(), frame);
    assert_eq!(norito::decode_from_bytes::<DomainId>(&frame).unwrap(), id);
    assert_eq!(
        DomainId::nominal_name(),
        "iroha_data_model::domain::model::DomainId"
    );
    for hash in [
        norito::schema::identity::frame_hash::<DomainId>(),
        norito::schema::identity::frame_hash::<DomainId>(),
    ] {
        assert_eq!(hex::encode(hash), "044c225b83d013da67e985809c84abcc");
    }
    assert_eq!(DomainId::decode(&mut &id.encode()[..]).unwrap(), id);
}

#[test]
fn domain_identity_preserves_every_advertised_field_layout() {
    let id = DomainId::try_new("a", "b").unwrap();
    for flags in layouts() {
        let packed = flags & ncore::header_flags::PACKED_STRUCT != 0;
        let compact = flags & ncore::header_flags::COMPACT_LEN != 0;
        let bitset = flags & ncore::header_flags::FIELD_BITSET != 0;
        let golden = match (packed, compact, bitset) {
            (false, false, false) => concat!(
                "0900000000000000010000000000000061",
                "0900000000000000010000000000000062"
            ),
            (false, true, false) => "020161020162",
            (true, false, false) => concat!(
                "000000000000000009000000000000001200000000000000",
                "010000000000000061010000000000000062"
            ),
            (true, true, false) => "00000000000000000200000000000000040000000000000001610162",
            (true, true, true) => "03020201610162",
            _ => unreachable!("the layout iterator validates advertised flag combinations"),
        };
        assert_eq!(
            hex::encode(payload(&id, flags)),
            golden,
            "flags {flags:#04x}"
        );
        for valid in [
            id.clone(),
            DomainId::try_new("under_score", "paynet").unwrap(),
            DomainId::try_new("例え", "テスト").unwrap(),
            DomainId::try_new("a".repeat(63), "b".repeat(63)).unwrap(),
        ] {
            roundtrip(&valid, flags);
        }
    }
}

#[test]
fn domain_identity_rejects_noncanonical_components_in_every_layout() {
    for label in [
        "a.b".to_owned(),
        "a。b".to_owned(),
        "a．b".to_owned(),
        "a｡b".to_owned(),
        "Treasury".to_owned(),
        "例え".to_owned(),
        "xn--".to_owned(),
        "xn--a".to_owned(),
        "-a".to_owned(),
        "a-".to_owned(),
        "a".repeat(64),
    ] {
        for name_is_invalid in [false, true] {
            // Only the owner can construct these invalid in-memory fixtures.
            // Encoding them exercises the actual unchanged DomainId field layout.
            let invalid = if name_is_invalid {
                DomainId::from_canonical_parts(label.parse().unwrap(), "valid".parse().unwrap())
            } else {
                DomainId::from_canonical_parts("valid".parse().unwrap(), label.parse().unwrap())
            };
            assert!(DomainId::decode(&mut &invalid.encode()[..]).is_err());
            for flags in layouts() {
                let bytes = payload(&invalid, flags);
                let frame = ncore::frame_bare_with_header_flags::<DomainId>(&bytes, flags).unwrap();
                assert!(
                    norito::decode_from_bytes::<DomainId>(&frame).is_err(),
                    "reject {label:?} in name={name_is_invalid}, flags={flags:#04x}"
                );
                let _flags = ncore::DecodeFlagsGuard::enter(flags);
                assert!(ncore::decode_field_canonical::<DomainId>(&bytes).is_err());
                assert!(<DomainId as ncore::DecodeFromSlice>::decode_from_slice(&bytes).is_err());
                let nested = vec![Some(invalid.clone())];
                let nested_frame = ncore::frame_bare_with_header_flags::<Vec<Option<DomainId>>>(
                    &payload(&nested, flags),
                    flags,
                )
                .unwrap();
                assert!(norito::decode_from_bytes::<Vec<Option<DomainId>>>(&nested_frame).is_err());
            }
        }
    }
}

#[test]
fn domain_identity_rejects_incomplete_or_extra_field_bytes_in_every_layout() {
    let id = DomainId::try_new("archive", "paynet").unwrap();
    for flags in layouts() {
        let bytes = payload(&id, flags);
        let mut extra = bytes.clone();
        extra.push(0);
        for invalid in [&bytes[..bytes.len() - 1], extra.as_slice()] {
            let frame = ncore::frame_bare_with_header_flags::<DomainId>(invalid, flags).unwrap();
            assert!(norito::decode_from_bytes::<DomainId>(&frame).is_err());
            let _flags = ncore::DecodeFlagsGuard::enter(flags);
            assert!(<DomainId as ncore::DecodeFromSlice>::decode_from_slice(invalid).is_err());
        }
        if flags & ncore::header_flags::PACKED_STRUCT != 0 {
            let mut wrong_header = bytes.clone();
            wrong_header[0] ^= 1;
            let frame =
                ncore::frame_bare_with_header_flags::<DomainId>(&wrong_header, flags).unwrap();
            assert!(norito::decode_from_bytes::<DomainId>(&frame).is_err());
        }
    }
}

#[test]
fn domain_binary_decoder_reserves_a_label_work_before_allocating() {
    let id = DomainId::try_new("例え", "centralbank").unwrap();
    let component_bytes = id.name().as_ref().len() + id.dataspace().as_ref().len();
    let label_allocation = component_bytes * 2 + (32 + 64 + 128) * core::mem::size_of::<u32>();
    let limits = |allocation| {
        ncore::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, allocation, usize::MAX)
    };
    let invalid = DomainId::from_canonical_parts("例え".parse().unwrap(), "valid".parse().unwrap());
    for flags in layouts() {
        let bytes = payload(&id, flags);
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        // Only the offset-table layout allocates header scratch: its audited
        // decoder reserves exactly three usize entries for two field ranges.
        let header_allocation = if flags & ncore::header_flags::PACKED_STRUCT != 0
            && flags & ncore::header_flags::FIELD_BITSET == 0
        {
            3 * core::mem::size_of::<usize>()
        } else {
            0
        };
        let expected = header_allocation + label_allocation;
        let (decoded, usage) = ncore::with_decode_limits_measured(limits(expected), || {
            ncore::decode_field_canonical::<DomainId>(&bytes)
        });
        assert_eq!(
            decoded.expect("exact canonicalization and header allocation budget"),
            (id.clone(), bytes.len()),
            "flags {flags:#04x}"
        );
        assert_eq!(
            usage.total_allocated_bytes(),
            expected,
            "flags {flags:#04x}"
        );
        let (rejected, usage) = ncore::with_decode_limits_measured(limits(expected - 1), || {
            ncore::decode_field_canonical::<DomainId>(&bytes)
        });
        assert!(rejected.unwrap_err().is_decode_resource_limit());
        assert_eq!(
            usage.total_allocated_bytes(),
            header_allocation,
            "do not start label allocation or normalization with a short budget: {flags:#04x}"
        );

        let bytes = payload(&invalid, flags);
        let (rejected, usage) =
            ncore::with_decode_limits_measured(limits(header_allocation), || {
                ncore::decode_field_canonical::<DomainId>(&bytes)
            });
        assert!(matches!(rejected, Err(ncore::Error::NonCanonicalEncoding)));
        assert_eq!(usage.total_allocated_bytes(), header_allocation);
    }
}

#[test]
fn domain_keys_roundtrip_after_binary_decoding_without_identity_collisions() {
    use std::collections::BTreeMap;
    let originals = [
        DomainId::try_new("a", "b").unwrap(),
        DomainId::try_new("a_b", "b").unwrap(),
        DomainId::try_new("a", "b_c").unwrap(),
        DomainId::try_new("例え", "テスト").unwrap(),
    ];
    let map: BTreeMap<_, _> = originals
        .iter()
        .enumerate()
        .map(|(index, id)| {
            (
                DomainId::decode(&mut &id.encode()[..]).unwrap(),
                index as u64,
            )
        })
        .collect();
    assert_eq!(map.len(), originals.len());
    let json = norito::json::to_json(&map).unwrap();
    assert_eq!(
        norito::json::from_json::<BTreeMap<DomainId, u64>>(&json).unwrap(),
        map
    );
    assert_eq!(
        norito::json::to_json_bounded(&map, json.len()).unwrap(),
        json
    );
}
