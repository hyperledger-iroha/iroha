//! Fixed claim codec controls, also runnable through the isolated allocation probe crate.

use super::*;
use std::io::Write as _;

fn binding() -> ClaimBindingV1 {
    let mut network_id = [0; 32];
    network_id[31] = 1;
    ClaimBindingV1 {
        network_id,
        // No invented nonzero restriction on an opaque derived owner-marker digest.
        owner_marker_digest: [0; 32],
        session_id: [1; 32],
    }
}

fn slots() -> [ControlSlotV1; 2] {
    [
        ControlSlotV1::Advance {
            predecessor_revision: 0,
        },
        ControlSlotV1::Check { challenge: [2; 32] },
    ]
}

#[test]
fn both_fixed_shapes_roundtrip_with_only_the_output_retained_in_the_fixture_pool() {
    // A local codec fixture, not a substitute for the production original State pool.
    let budget = AllocationBudget::new(MAX_CLAIM_BYTES_V1);
    for slot in slots() {
        let descriptor = ClaimDescriptorV1::new(binding(), slot, &[3; 64]).unwrap();
        let frame = ClaimFrameV1::encode(&descriptor, &budget).unwrap();
        assert!(frame.0.belongs_to(&budget));
        assert_eq!(budget.reserved_bytes(), frame.0.capacity());
        assert_eq!(frame.0.capacity(), frame.0.as_slice().len());
        assert_eq!(decode_claim(frame.0.as_slice()).unwrap(), descriptor);
        norito::verify_exact_canonical_frame(&descriptor, frame.0.as_slice()).unwrap();
        drop(frame);
        assert_eq!(budget.reserved_bytes(), 0);
    }
}

#[test]
fn field_policy_matches_existing_network_session_and_challenge_constraints() {
    let valid = binding();
    for slot in slots() {
        assert!(ClaimDescriptorV1::new(valid, slot, &[1]).is_ok());
        let mut wrong_network = valid;
        wrong_network.network_id[31] = 2;
        assert!(ClaimDescriptorV1::new(wrong_network, slot, &[1]).is_err());
        let mut zero_session = valid;
        zero_session.session_id = [0; 32];
        assert!(ClaimDescriptorV1::new(zero_session, slot, &[1]).is_err());
    }
    assert!(
        ClaimDescriptorV1::new(valid, ControlSlotV1::Check { challenge: [0; 32] }, &[1]).is_err()
    );
    // A slot is an inert identity; signed instruction validation owns revision advancement.
    assert!(
        ClaimDescriptorV1::new(
            valid,
            ControlSlotV1::Advance {
                predecessor_revision: u64::MAX,
            },
            &[1],
        )
        .is_ok()
    );
}

#[test]
fn descriptor_bounds_and_domain_hash_bind_exact_bytes_without_decoding_a_transaction() {
    let slot = slots()[0];
    let wire = vec![3; MAX_WIRE_BYTES_V1];
    let descriptor = ClaimDescriptorV1::new(binding(), slot, &wire).unwrap();
    assert!(descriptor.matches_wire(&wire));
    assert!(!descriptor.matches_wire(&wire[..wire.len() - 1]));
    let mut changed = wire.clone();
    changed[MAX_WIRE_BYTES_V1 / 2] ^= 1;
    assert!(!descriptor.matches_wire(&changed));
    assert_ne!(descriptor.wire_digest, *blake3::hash(&wire).as_bytes());
    assert!(ClaimDescriptorV1::new(binding(), slot, &[]).is_err());
    assert!(ClaimDescriptorV1::new(binding(), slot, &vec![3; MAX_WIRE_BYTES_V1 + 1]).is_err());
    let mut invalid = descriptor;
    invalid.version = 0;
    assert!(invalid.validate().is_err());
    invalid = descriptor;
    invalid.wire_length = 0;
    assert!(invalid.validate().is_err());
    invalid.wire_length = (MAX_WIRE_BYTES_V1 + 1) as u32;
    assert!(invalid.validate().is_err());
}

#[test]
fn all_single_byte_mutations_truncations_and_trailing_data_refuse_both_shapes() {
    let budget = AllocationBudget::new(MAX_CLAIM_BYTES_V1);
    for slot in slots() {
        let descriptor = ClaimDescriptorV1::new(binding(), slot, &[3; 64]).unwrap();
        let frame = ClaimFrameV1::encode(&descriptor, &budget).unwrap();
        for length in 0..frame.0.as_slice().len() {
            assert!(decode_claim(&frame.0.as_slice()[..length]).is_err());
        }
        for index in 0..frame.0.as_slice().len() {
            let mut changed = frame.0.as_slice().to_vec();
            changed[index] ^= 1;
            assert!(
                decode_claim(&changed).is_err(),
                "accepted mutation at {index}"
            );
        }
        let mut trailing = frame.0.as_slice().to_vec();
        trailing.push(0);
        assert!(decode_claim(&trailing).is_err());
        assert!(decode_claim(&vec![0; MAX_CLAIM_BYTES_V1 + 1]).is_err());
    }
}

#[test]
fn resource_and_codec_errors_do_not_release_unowned_credit_or_erase_codec_origin() {
    let descriptor = ClaimDescriptorV1::new(binding(), slots()[0], &[3; 64]).unwrap();
    let length = norito::canonical_frame_len(&descriptor).unwrap();
    let budget = AllocationBudget::new(length);
    let held = budget.try_reserve_bytes(length).unwrap();
    assert!(ClaimFrameV1::encode(&descriptor, &budget).is_err());
    assert_eq!(budget.reserved_bytes(), length);
    drop(held);
    let frame = ClaimFrameV1::encode(&descriptor, &budget).unwrap();
    let mut malformed = frame.0.as_slice().to_vec();
    malformed[0] ^= 1;
    assert!(matches!(
        decode_claim(&malformed),
        Err(ControlJournalErrorV1::Codec(_))
    ));
    assert_eq!(budget.reserved_bytes(), length);
    drop(frame);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn frame_writer_refuses_growth_and_leaves_initialized_bytes_unchanged() {
    let budget = AllocationBudget::new(1);
    let mut writer = ClaimFrameV1(ChargedBuffer::new(1, &budget).unwrap());
    writer.write_all(&[7]).unwrap();
    assert!(writer.write_all(&[8]).is_err());
    assert_eq!(writer.0.as_slice(), &[7]);
    writer.flush().unwrap();
    assert_eq!(budget.reserved_bytes(), 1);
    drop(writer);
    assert_eq!(budget.reserved_bytes(), 0);
}

#[test]
fn inline_slot_names_are_exact_disjoint_and_reject_an_empty_challenge() {
    let zero = SlotNamesV1::new(slots()[0]).unwrap();
    assert_eq!(zero.claim(), "a0000000000000000.claim");
    assert_eq!(zero.wire(), "a0000000000000000.wire");
    let last = SlotNamesV1::new(ControlSlotV1::Advance {
        predecessor_revision: u64::MAX,
    })
    .unwrap();
    assert_eq!(last.claim(), "affffffffffffffff.claim");
    let challenge = SlotNamesV1::new(slots()[1]).unwrap();
    assert_eq!(challenge.claim().as_encoded_bytes().len(), 71);
    assert_eq!(challenge.wire().as_encoded_bytes().len(), 70);
    assert_ne!(challenge.claim(), zero.claim());
    assert_ne!(challenge.wire(), last.wire());
    assert!(SlotNamesV1::new(ControlSlotV1::Check { challenge: [0; 32] }).is_err());
    let mut encoded = [0; 6];
    encode_hex(&[0, 0xab, 0xff], &mut encoded);
    assert_eq!(&encoded, b"00abff");
}
