//! Exact canonical pulse context framing and public shape regressions.
use super::*;
use crate::consensus::{GlobalThresholdBeaconChainAnchorV1, GlobalThresholdBeaconPulseContextV1};
use iroha_crypto::{Algorithm, HashOf, KeyPair};

fn pulse() -> FinalizedGlobalThresholdBeaconPulseV1 {
    let key = KeyPair::from_seed(vec![17; 32], Algorithm::BlsNormal);
    // Shape verification requires a canonical nonzero G1 point. This is not a
    // threshold-signature authority fixture; execution verifies that separately.
    let (_, point) = key.public_key().try_to_bytes().unwrap();
    let mut pulse = FinalizedGlobalThresholdBeaconPulseV1 {
        version: GLOBAL_THRESHOLD_BEACON_VERSION_V1,
        network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"test network",
        ))),
        session_id: [2; 32],
        roster_hash: [3; 32],
        transcript_hash: [4; 32],
        context: GlobalThresholdBeaconPulseContextV1 {
            instance: [5; 32],
            epoch: 0,
            epoch_context_id: [6; 32],
            parent_consensus_hash: [7; 32],
            parent_result: [8; 32],
        },
        height: 2,
        round: 0,
        finalized_chain_anchor: GlobalThresholdBeaconChainAnchorV1 {
            height: 1,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"signed genesis")),
        },
        signature: point.try_into().unwrap(),
        seed: [9; 32],
        pulse_id: [0; 32],
    };
    pulse.pulse_id = global_threshold_beacon_pulse_id_v1(&pulse, pulse.seed);
    pulse
}

#[test]
fn canonical_payload_and_identifier_bind_every_native_context_field() {
    let original = pulse();
    validate_beacon_pulse_shape(&original).unwrap();
    let payload = global_threshold_beacon_pulse_payload_v1(&original);
    assert_eq!(
        payload.len(),
        GLOBAL_BEACON_PULSE_PAYLOAD_DOMAIN_V1.len() + 2 + 32 * 9 + 8 * 4
    );
    for index in 0..5 {
        let mut changed = original;
        match index {
            0 => changed.context.instance[0] ^= 1,
            1 => changed.context.epoch += 1,
            2 => changed.context.epoch_context_id[0] ^= 1,
            3 => changed.context.parent_consensus_hash[0] ^= 1,
            4 => changed.context.parent_result[0] ^= 1,
            _ => unreachable!(),
        }
        assert_ne!(global_threshold_beacon_pulse_payload_v1(&changed), payload);
        assert_ne!(
            global_threshold_beacon_pulse_id_v1(&changed, changed.seed),
            original.pulse_id
        );
        assert!(matches!(
            validate_beacon_pulse_shape(&changed),
            Err(BeaconPulseShapeError::PulseIdMismatch)
        ));
    }
    let encoded = norito::encode_canonical(&original).unwrap();
    let decoded: FinalizedGlobalThresholdBeaconPulseV1 =
        norito::decode_canonical(&encoded).unwrap();
    assert_eq!(decoded, original);
    assert_eq!(global_threshold_beacon_pulse_payload_v1(&decoded), payload);
}

#[test]
fn inert_native_context_fails_before_pulse_identifier_validation() {
    for index in 0..4 {
        let mut changed = pulse();
        match index {
            0 => changed.context.instance = [0; 32],
            1 => changed.context.epoch_context_id = [0; 32],
            2 => changed.context.parent_consensus_hash = [0; 32],
            3 => changed.context.parent_result = [0; 32],
            _ => unreachable!(),
        }
        changed.pulse_id = global_threshold_beacon_pulse_id_v1(&changed, changed.seed);
        assert!(matches!(
            validate_beacon_pulse_shape(&changed),
            Err(BeaconPulseShapeError::PulseContextMismatch)
        ));
    }
}

#[test]
fn payload_omits_derived_outputs_but_identifier_authenticates_them() {
    let original = pulse();
    let mut changed = original;
    changed.signature[0] ^= 1;
    changed.seed[0] ^= 1;
    changed.pulse_id[0] ^= 1;
    assert_eq!(
        global_threshold_beacon_pulse_payload_v1(&changed),
        global_threshold_beacon_pulse_payload_v1(&original)
    );
    assert_ne!(
        global_threshold_beacon_pulse_id_v1(&changed, changed.seed),
        original.pulse_id
    );
}
