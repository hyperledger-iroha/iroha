//! Encode/decode roundtrip tests for consensus persistence records.
use core::convert::TryFrom;

use iroha_crypto::{Hash, HashOf, KeyPair};
use iroha_data_model::{
    block::{
        BlockHeader,
        consensus::{CertPhase, PERMISSIONED_TAG, Qc, QcAggregate},
    },
    consensus::{VrfEpochRecord, VrfLateRevealRecord, VrfParticipantRecord},
    peer::PeerId,
};
use norito::codec::{Decode, Encode};

fn sample_hash(seed: u8) -> Hash {
    let mut bytes = [0u8; Hash::LENGTH];
    for (idx, byte) in bytes.iter_mut().enumerate() {
        let idx_u8 = u8::try_from(idx).expect("hash length fits into u8");
        *byte = seed.wrapping_add(idx_u8);
    }
    Hash::prehashed(bytes)
}

fn sample_block_hash(seed: u8) -> HashOf<BlockHeader> {
    HashOf::from_untyped_unchecked(sample_hash(seed))
}

fn assert_roundtrip<T>(value: &T)
where
    T: Encode + Decode + PartialEq + core::fmt::Debug,
{
    let bytes = value.encode();
    let mut cursor = bytes.as_slice();
    let decoded = T::decode(&mut cursor).expect("decode succeeds");
    assert!(cursor.is_empty(), "decode must consume all bytes");
    assert_eq!(decoded, *value, "roundtrip must preserve value");
}

#[test]
fn qc_roundtrip() {
    let validator_set = vec![
        PeerId::from(KeyPair::random().public_key().clone()),
        PeerId::from(KeyPair::random().public_key().clone()),
    ];
    let qc = Qc {
        phase: CertPhase::Commit,
        subject_block_hash: sample_block_hash(0x10),
        parent_state_root: sample_hash(0x11),
        post_state_root: sample_hash(0x20),
        height: 42,
        view: 7,
        epoch: 3,
        mode_tag: PERMISSIONED_TAG.to_string(),
        highest_qc: None,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set_hash_version: 1,
        validator_set,
        aggregate: QcAggregate {
            signers_bitmap: vec![0xAA, 0x55],
            bls_aggregate_signature: vec![0x90, 0x91, 0x92, 0x93],
        },
    };
    assert_roundtrip(&qc);
}

#[test]
fn vrf_participant_record_roundtrip_variants() {
    let variants = [
        VrfParticipantRecord {
            signer: 1,
            commitment: None,
            reveal: None,
            last_updated_height: 10,
        },
        VrfParticipantRecord {
            signer: 2,
            commitment: Some([0x11; 32]),
            reveal: None,
            last_updated_height: 20,
        },
        VrfParticipantRecord {
            signer: 3,
            commitment: None,
            reveal: Some([0x22; 32]),
            last_updated_height: 30,
        },
        VrfParticipantRecord {
            signer: 4,
            commitment: Some([0x33; 32]),
            reveal: Some([0x44; 32]),
            last_updated_height: 40,
        },
    ];

    for record in &variants {
        assert_roundtrip(record);
    }
}

#[test]
fn vrf_epoch_record_roundtrip() {
    let participants = vec![
        VrfParticipantRecord {
            signer: 5,
            commitment: Some([0xAA; 32]),
            reveal: Some([0xBB; 32]),
            last_updated_height: 100,
        },
        VrfParticipantRecord {
            signer: 6,
            commitment: Some([0xCC; 32]),
            reveal: None,
            last_updated_height: 110,
        },
    ];
    let record = VrfEpochRecord {
        epoch: 9,
        seed: [0x55; 32],
        epoch_length: 180,
        commit_deadline_offset: 45,
        reveal_deadline_offset: 120,
        roster_len: 12,
        finalized: true,
        updated_at_height: 720,
        participants,
        late_reveals: vec![VrfLateRevealRecord {
            signer: 7,
            reveal: [0xDD; 32],
            noted_at_height: 730,
        }],
        committed_no_reveal: vec![7, 8],
        no_participation: vec![9, 10],
        penalties_applied: false,
        penalties_applied_at_height: None,
        validator_election: None,
    };
    assert_roundtrip(&record);
}

#[test]
fn vrf_epoch_record_empty_sets_roundtrip() {
    let record = VrfEpochRecord {
        epoch: 11,
        seed: [0x66; 32],
        epoch_length: 200,
        commit_deadline_offset: 60,
        reveal_deadline_offset: 140,
        roster_len: 5,
        finalized: false,
        updated_at_height: 880,
        participants: Vec::new(),
        late_reveals: Vec::new(),
        committed_no_reveal: Vec::new(),
        no_participation: Vec::new(),
        penalties_applied: false,
        penalties_applied_at_height: None,
        validator_election: None,
    };
    assert_roundtrip(&record);
}
