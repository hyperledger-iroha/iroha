//! Fixed-row codec, resource accounting and authenticated-value equivalence.

use super::*;
use crate::backend::compact_protocol::{
    FixedAir, Geometry, profile::Binding, test_fixture::FixedColumnsAir,
};
use norito::{DecodeLimits, NoritoDeserialize, NoritoSerialize};

#[derive(Debug, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "fastpq:test:fixed-row-container:v1")]
struct Rows {
    rows: Vec<RowValues>,
}

fn raw(row: &RowValues) -> Vec<u8> {
    let _flags = norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
    norito::codec::encode_with_header_flags(row).0
}

#[test]
fn exact_little_endian_payload_roundtrips_every_boundary_and_alignment() {
    let boundary = [0, 1, 0x0102_0304_0506_0708, GOLDILOCKS_MODULUS - 1];
    let original = (0..RowValues::WIDTH)
        .map(|index| boundary[index % boundary.len()])
        .collect::<Vec<_>>();
    let row = RowValues::from_vec(original.clone()).unwrap();
    let bytes = raw(&row);
    assert_eq!(bytes.len(), 2_736);
    assert_eq!(
        bytes,
        original
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        norito::core::encoded_payload_len(&row).unwrap(),
        RowValues::BYTES
    );
    for offset in 0..8 {
        let mut backing = vec![0; offset];
        backing.extend_from_slice(&bytes);
        let (decoded, used) =
            norito::core::decode_field_canonical::<RowValues>(&backing[offset..]).unwrap();
        assert_eq!(used, RowValues::BYTES);
        assert_eq!(&*decoded, original);
    }
}

#[test]
fn fixed_row_payload_is_independent_of_ambient_layout_flags() {
    let values = (0..RowValues::WIDTH)
        .map(|column| (column as u64) * 0x0102_0304)
        .collect();
    let row = RowValues::from_vec(values).unwrap();
    let expected = raw(&row);
    for flags in
        (u8::MIN..=u8::MAX).filter(|&flags| norito::core::validate_header_flags(flags).is_ok())
    {
        let _ambient = norito::core::DecodeFlagsGuard::enter(flags);
        assert_eq!(norito::codec::encode_with_header_flags(&row).0, expected);
        let (decoded, used) = norito::core::decode_field_canonical::<RowValues>(&expected).unwrap();
        assert_eq!(decoded, row);
        assert_eq!(used, RowValues::BYTES);
        assert_eq!(norito::core::get_decode_flags(), flags);
    }
}

#[test]
fn wrong_spans_and_noncanonical_scalars_are_rejected() {
    for width in [0, 341, 343, 512] {
        assert!(RowValues::from_vec(vec![0; width]).is_err());
    }
    let bytes = raw(&RowValues::zero());
    for length in [
        0,
        1,
        RowValues::BYTES - 1,
        RowValues::BYTES + 1,
        RowValues::BYTES + 8,
    ] {
        let mut malformed = bytes.clone();
        malformed.resize(length, 0);
        assert!(matches!(
            norito::core::decode_field_canonical::<RowValues>(&malformed),
            Err(norito::Error::LengthMismatch)
        ));
    }
    for column in [0, RowValues::WIDTH / 2, RowValues::WIDTH - 1] {
        for invalid in [GOLDILOCKS_MODULUS, u64::MAX] {
            let mut malformed = bytes.clone();
            malformed[column * 8..column * 8 + 8].copy_from_slice(&invalid.to_le_bytes());
            assert!(matches!(
                norito::core::decode_field_canonical::<RowValues>(&malformed),
                Err(norito::Error::Message(_))
            ));
            let mut values = vec![0; RowValues::WIDTH];
            values[column] = invalid;
            assert!(RowValues::from_vec(values).is_err());
        }
    }
}

#[test]
fn exact_frame_and_complete_inline_row_allocation_are_charged() {
    let rows = Rows {
        rows: vec![RowValues::zero(), RowValues::zero()],
    };
    let bytes = norito::encode_canonical(&rows).unwrap();
    assert_eq!(bytes.len(), 5_526);
    assert_eq!(norito::core::encoded_frame_len(&rows).unwrap(), bytes.len());
    let budget = DecodeLimits::new(2, bytes.len(), 2, 1024 * 1024, 16);
    let (decoded, usage) = norito::core::with_decode_limits_measured(budget, || {
        norito::decode_canonical_with_limits::<Rows>(&bytes, budget)
    });
    assert_eq!(decoded.unwrap(), rows);
    assert_eq!(usage.total_elements(), 2);
    assert!(usage.total_allocated_bytes() >= 2 * size_of::<RowValues>());
    let exact = DecodeLimits::new(2, bytes.len(), 2, usage.total_allocated_bytes(), 16);
    assert_eq!(
        norito::decode_canonical_with_limits::<Rows>(&bytes, exact).unwrap(),
        rows
    );
    let below = DecodeLimits::new(2, bytes.len(), 2, usage.total_allocated_bytes() - 1, 16);
    assert!(matches!(
        norito::decode_canonical_with_limits::<Rows>(&bytes, below),
        Err(norito::Error::TotalAllocationExceeded { .. })
    ));
    let one_element = DecodeLimits::new(2, bytes.len(), 1, 1024 * 1024, 16);
    assert!(matches!(
        norito::core::with_decode_limits_scope(one_element, || {
            norito::decode_canonical_with_limits::<Rows>(&bytes, budget)
        }),
        Err(norito::Error::TotalElementsExceeded { .. })
    ));
}

#[test]
fn fixed_row_roundtrip_preserves_six_lane_root_and_air_evaluation() {
    let air = FixedColumnsAir::new(7);
    let geometry = Geometry::new(&air).unwrap();
    let binding = Binding::new(&air, &geometry).unwrap();
    let original = air.row();
    let bytes = raw(&RowValues::from_vec(original.clone()).unwrap());
    let (decoded, _) = norito::core::decode_field_canonical::<RowValues>(&bytes).unwrap();
    let role = crate::backend::MerkleTreeRoleV1::AirTrace;
    let sibling = fastpq_isi::GoldilocksDigest384V1::default();
    let root = |values: &[u64]| {
        let mut index = 3;
        let mut digest = binding.row(index, values).unwrap();
        for level in 1..=19 {
            let (left, right) = if index & 1 == 0 {
                (digest, sibling)
            } else {
                (sibling, digest)
            };
            index >>= 1;
            digest = binding.parent(role, level, index, left, right).unwrap();
        }
        digest
    };
    assert_eq!(root(&original), root(&decoded));
    assert_eq!(
        air.evaluate(3, &original, &original).unwrap(),
        air.evaluate(3, &decoded, &decoded).unwrap()
    );
    let mut changed = decoded.clone();
    changed[17] += 1;
    let mut changed_original = original.clone();
    changed_original[17] += 1;
    assert_eq!(
        air.evaluate(3, &changed_original, &original).unwrap(),
        air.evaluate(3, &changed, &decoded).unwrap()
    );
    assert_ne!(root(&original), root(&changed));
    assert_ne!(
        air.evaluate(3, &decoded, &decoded).unwrap(),
        air.evaluate(3, &changed, &decoded).unwrap()
    );
}
