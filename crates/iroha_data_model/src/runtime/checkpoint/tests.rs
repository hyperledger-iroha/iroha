//! The committed adoption anchor and startup floors: statements, transition rules and the
//! boundary they set.

use iroha_crypto::{Algorithm, HashOf, KeyPair};

use super::*;
use crate::{
    block::BlockHeader,
    runtime::{definition::SelectionId, lifecycle::committee_digest},
};

const HOUR_MS: u64 = 60 * 60 * 1000;
const DAY_MS: u64 = 24 * HOUR_MS;

/// Taira's committed values of the design.
const POLICY: RuntimeCheckpointPolicyV1 = RuntimeCheckpointPolicyV1 {
    cadence_ms: 15 * 60 * 1000,
    retain_all_ms: DAY_MS,
    retain_spacing_ms: DAY_MS,
    recovery_window_ms: 7 * DAY_MS,
    floor_notice_ms: DAY_MS,
};

fn network(byte: u8) -> NetworkId {
    NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
        Hash::prehashed([byte; 32]),
    ))
}

fn keys(count: u8) -> Vec<KeyPair> {
    (1..=count)
        .map(|seed| KeyPair::from_seed(vec![seed; 32], Algorithm::Ed25519))
        .collect()
}

fn committee(keys: &[KeyPair]) -> Vec<PublicKey> {
    keys.iter().map(|key| key.public_key().clone()).collect()
}

fn record(keys: &[KeyPair]) -> AdoptionRecordV1 {
    AdoptionRecordV1 {
        network: network(7),
        height: 5524,
        block_hash: [1; 32],
        predecessor_world_root: [2; 32],
        imported_state_root: [3; 32],
        committee: committee_digest(&committee(keys)).unwrap(),
        selection: SelectionId([5; 32]),
        authorization: [0; 32],
    }
}

fn anchor(keys: &[KeyPair]) -> RuntimeAdoptionAnchorV1 {
    let record = record(keys);
    RuntimeAdoptionAnchorV1 {
        attestations: keys
            .iter()
            .map(|key| {
                RuntimeAdoptionAnchorV1::statement(&record, key.public_key().clone())
                    .unwrap()
                    .sign(key.private_key())
                    .unwrap()
            })
            .collect(),
        record,
    }
}

fn floor_at(height: u64) -> StartupFloorV1 {
    StartupFloorV1 {
        network: network(7),
        height,
        block_hash: [height.to_le_bytes()[0]; 32],
        committed_ms: height * 4_000,
    }
}

fn statements(keys: &[KeyPair], floor: &StartupFloorV1) -> Vec<SignedStartupFloorAttestationV1> {
    keys.iter()
        .map(|key| {
            RuntimeStartupFloorV1::statement(floor, key.public_key().clone())
                .unwrap()
                .sign(key.private_key())
                .unwrap()
        })
        .collect()
}

/// A committed floor that is in force from `effective_ms`.
fn committed(keys: &[KeyPair], floor: StartupFloorV1, effective_ms: u64) -> RuntimeStartupFloorV1 {
    RuntimeStartupFloorV1 {
        floor,
        effective_ms,
        attestations: statements(keys, &floor),
    }
}

/// The chain's committed block hashes: the one each test record names at its height.
fn chain(height: u64) -> Option<[u8; 32]> {
    Some(if height == 5524 {
        [1; 32]
    } else {
        [height.to_le_bytes()[0]; 32]
    })
}

#[test]
fn the_boundaries_round_trip_and_name_their_block() {
    let keys = keys(4);
    let anchor = RuntimeBoundaryV1::AdoptionAnchor(anchor(&keys));
    let floor = RuntimeBoundaryV1::StartupFloor(committed(&keys, floor_at(6000), 99));
    for boundary in [&anchor, &floor] {
        let bytes = norito::to_bytes(boundary).unwrap();
        assert_eq!(
            &norito::decode_from_bytes::<RuntimeBoundaryV1>(&bytes).unwrap(),
            boundary
        );
        let json = norito::json::to_json(boundary).unwrap();
        assert_eq!(
            &norito::json::from_str::<RuntimeBoundaryV1>(&json).unwrap(),
            boundary
        );
    }
    assert_eq!((anchor.height(), anchor.block_hash()), (5524, [1; 32]));
    assert_eq!(
        (floor.height(), floor.block_hash()),
        (6000, floor_at(6000).block_hash)
    );
    assert!(anchor.in_force(0));
    assert!(!floor.in_force(98) && floor.in_force(99));
    assert_ne!(floor_at(6000).id().unwrap(), floor_at(6001).id().unwrap());
}

#[test]
fn every_validator_ratifies_the_anchor_once() {
    let keys = keys(4);
    let committee = committee(&keys);
    let anchor = anchor(&keys);
    let check = |anchor: &RuntimeAdoptionAnchorV1,
                 lowest: Option<&RuntimeBoundaryV1>,
                 committee: &[PublicKey],
                 tip: u64| {
        anchor.check_ratification(lowest, &network(7), committee, tip, chain)
    };
    assert!(check(&anchor, None, &committee, 9000).is_ok());
    assert!(matches!(
        check(
            &anchor,
            Some(&RuntimeBoundaryV1::AdoptionAnchor(anchor.clone())),
            &committee,
            9000
        ),
        Err(CheckpointError::AnchorRatified)
    ));
    assert!(matches!(
        check(
            &anchor,
            Some(&RuntimeBoundaryV1::StartupFloor(committed(
                &keys,
                floor_at(6000),
                1
            ))),
            &committee,
            9000
        ),
        Err(CheckpointError::AnchorAfterFloor)
    ));
    assert!(matches!(
        anchor.check_ratification(None, &network(8), &committee, 9000, chain),
        Err(CheckpointError::Network)
    ));
    assert!(matches!(
        check(&anchor, None, &committee, 5523),
        Err(CheckpointError::NotCommitted)
    ));
    assert!(matches!(
        anchor.check_ratification(None, &network(7), &committee, 9000, |_| Some([9; 32])),
        Err(CheckpointError::BlockHash)
    ));

    // All n sign: a quorum, a foreign signer, a repeated signer or a statement of another
    // record does not ratify.
    let mut quorum = anchor.clone();
    quorum.attestations.pop();
    assert!(matches!(
        check(&quorum, None, &committee, 9000),
        Err(CheckpointError::Coverage)
    ));
    let mut repeated = anchor.clone();
    repeated.attestations[3] = repeated.attestations[0].clone();
    assert!(matches!(
        check(&repeated, None, &committee, 9000),
        Err(CheckpointError::Coverage)
    ));
    let outsider = KeyPair::from_seed(vec![9; 32], Algorithm::Ed25519);
    let mut foreign = anchor.clone();
    foreign.attestations[3] =
        RuntimeAdoptionAnchorV1::statement(&anchor.record, outsider.public_key().clone())
            .unwrap()
            .sign(outsider.private_key())
            .unwrap();
    assert!(matches!(
        check(&foreign, None, &committee, 9000),
        Err(CheckpointError::Coverage)
    ));
    let mut another = anchor.clone();
    another.record.imported_state_root = [0xee; 32];
    assert!(
        matches!(
            check(&another, None, &committee, 9000),
            Err(CheckpointError::Coverage)
        ),
        "the statements bind every field of the record"
    );
    assert!(matches!(
        check(&anchor, None, &committee[..3], 9000),
        Err(CheckpointError::Committee)
    ));

    // The record names the committee of the adoption height; the committee that ratifies it
    // may have changed since.
    let later = self::keys(7);
    let mut ratified_later = anchor.clone();
    ratified_later.attestations = later
        .iter()
        .map(|key| {
            RuntimeAdoptionAnchorV1::statement(&anchor.record, key.public_key().clone())
                .unwrap()
                .sign(key.private_key())
                .unwrap()
        })
        .collect();
    assert!(check(&ratified_later, None, &self::committee(&later), 9000).is_ok());
}

#[test]
fn the_floor_only_moves_forward_to_a_recoverable_checkpoint_after_notice() {
    let keys = keys(4);
    let committee = committee(&keys);
    let record = record(&keys);
    let floor = floor_at(6000);
    // The floor block was committed at 24_000_000 ms; now is ten days later.
    let now = floor.committed_ms + 10 * DAY_MS;
    let raise = |floor: StartupFloorV1,
                 attestations: Vec<SignedStartupFloorAttestationV1>,
                 latest: Option<&RuntimeStartupFloorV1>,
                 policy: Option<&RuntimeCheckpointPolicyV1>,
                 now_ms: u64| {
        RuntimeStartupFloorV1::raise(
            floor,
            attestations,
            latest,
            policy,
            Some(&record),
            &network(7),
            &committee,
            9000,
            now_ms,
            chain,
        )
    };
    let signed = |floor: StartupFloorV1| statements(&keys, &floor);
    let first = raise(floor, signed(floor), None, Some(&POLICY), now).expect("the first floor");
    // The kernel, not the relayer, decides when the floor comes into force.
    assert_eq!(first, committed(&keys, floor, now + POLICY.floor_notice_ms));
    assert!(matches!(
        raise(floor, signed(floor), None, None, now),
        Err(CheckpointError::NoPolicy)
    ));

    // The recovery window is measured to the time the floor comes into force.
    let oldest = floor.committed_ms + POLICY.recovery_window_ms - POLICY.floor_notice_ms;
    assert!(raise(floor, signed(floor), None, Some(&POLICY), oldest).is_ok());
    assert!(matches!(
        raise(floor, signed(floor), None, Some(&POLICY), oldest - 1),
        Err(CheckpointError::NotRecoverable)
    ));
    let mut endless = POLICY;
    endless.floor_notice_ms = u64::MAX;
    assert!(matches!(
        raise(floor, signed(floor), None, Some(&endless), now),
        Err(CheckpointError::Notice)
    ));

    // The next floor lies above the latest one and waits for it to come into force.
    let later = first.effective_ms + 30 * DAY_MS;
    let next = floor_at(7000);
    let second = raise(next, signed(next), Some(&first), Some(&POLICY), later).expect("raise");
    assert_eq!(second.effective_ms, later + POLICY.floor_notice_ms);
    assert!(matches!(
        raise(
            next,
            signed(next),
            Some(&first),
            Some(&POLICY),
            first.effective_ms - 1
        ),
        Err(CheckpointError::Pending)
    ));
    for regressed in [floor_at(5999), floor] {
        assert!(matches!(
            raise(
                regressed,
                signed(regressed),
                Some(&first),
                Some(&POLICY),
                later
            ),
            Err(CheckpointError::Regressed)
        ));
    }
    let mut at_the_anchor = floor_at(5524);
    at_the_anchor.block_hash = [1; 32];
    for at_or_below_the_anchor in [floor_at(5523), at_the_anchor] {
        assert!(matches!(
            raise(
                at_or_below_the_anchor,
                signed(at_or_below_the_anchor),
                None,
                Some(&POLICY),
                at_or_below_the_anchor.committed_ms + 10 * DAY_MS
            ),
            Err(CheckpointError::Regressed)
        ));
    }

    // The floor names a committed block of this network, and all n validators signed it.
    let ahead = floor_at(9001);
    assert!(matches!(
        raise(ahead, signed(ahead), None, Some(&POLICY), now),
        Err(CheckpointError::NotCommitted)
    ));
    let mut forged = floor;
    forged.block_hash = [0xdd; 32];
    assert!(matches!(
        raise(forged, signed(forged), None, Some(&POLICY), now),
        Err(CheckpointError::BlockHash)
    ));
    let mut foreign = floor;
    foreign.network = network(8);
    assert!(matches!(
        raise(foreign, signed(foreign), None, Some(&POLICY), now),
        Err(CheckpointError::Network)
    ));
    let mut quorum = signed(floor);
    quorum.pop();
    assert!(matches!(
        raise(floor, quorum, None, Some(&POLICY), now),
        Err(CheckpointError::Coverage)
    ));
    let mut another = floor;
    another.committed_ms += 1;
    assert!(
        matches!(
            raise(another, signed(floor), None, Some(&POLICY), now),
            Err(CheckpointError::Coverage)
        ),
        "the statements bind the committed time the recovery window is measured from"
    );
}

#[test]
fn the_history_boundary_is_the_highest_boundary_in_force() {
    let keys = keys(4);
    let anchor = RuntimeBoundaryV1::AdoptionAnchor(anchor(&keys));
    let first = RuntimeBoundaryV1::StartupFloor(committed(&keys, floor_at(6000), 100));
    let second = RuntimeBoundaryV1::StartupFloor(committed(&keys, floor_at(7000), 200));
    assert_eq!(history_boundary([], 0), None);
    assert_eq!(history_boundary([&anchor], 0), Some((5524, [1; 32])));
    assert_eq!(
        history_boundary([&anchor, &first], 99),
        Some((5524, [1; 32])),
        "a floor waiting out its notice sets no boundary yet"
    );
    assert_eq!(
        history_boundary([&anchor, &first], 100),
        Some((6000, floor_at(6000).block_hash))
    );
    assert_eq!(
        history_boundary([&anchor, &first, &second], 199),
        Some((6000, floor_at(6000).block_hash))
    );
    assert_eq!(
        history_boundary([&first, &second], 200),
        Some((7000, floor_at(7000).block_hash))
    );
    assert_eq!(history_boundary([&first], 99), None);
}

#[test]
fn a_floor_statement_is_not_an_adoption_statement() {
    let keys = keys(4);
    let floor = floor_at(6000);
    let signed = RuntimeStartupFloorV1::statement(&floor, keys[0].public_key().clone())
        .unwrap()
        .sign(keys[0].private_key())
        .unwrap();
    assert!(signed.verify_for(&floor).is_ok());
    assert!(signed.verify_for(&floor_at(6001)).is_err());
    let mut relabeled = signed.clone();
    relabeled.attestation.validator = keys[1].public_key().clone();
    assert!(relabeled.verify_for(&floor).is_err());
    // The same key's signature over the same identity bytes under the adoption purpose does
    // not verify as a floor statement.
    let adoption = AdoptionAttestationV1 {
        adoption: floor.id().unwrap(),
        validator: keys[0].public_key().clone(),
    }
    .sign(keys[0].private_key())
    .unwrap();
    let crossed = SignedStartupFloorAttestationV1 {
        attestation: signed.attestation.clone(),
        signature: adoption.signature,
    };
    assert!(crossed.verify_for(&floor).is_err());
}
