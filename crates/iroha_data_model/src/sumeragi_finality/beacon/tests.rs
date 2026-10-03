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

#[test]
fn inline_payload_and_streamed_identifier_match_exact_canonical_bytes() {
    let mut original = pulse();
    original.context.epoch = 0x0102_0304_0506_0708;
    original.height = 0x1112_1314_1516_1718;
    original.round = 0x2122_2324_2526_2728;
    original.finalized_chain_anchor.height = 0x3132_3334_3536_3738;
    let mut expected = b"iroha.global-threshold-beacon.pulse-payload.v1\0".to_vec();
    expected.extend_from_slice(&[0, 1]);
    expected.extend_from_slice(original.network_id.as_bytes());
    expected.extend_from_slice(&[2; 32]);
    expected.extend_from_slice(&[3; 32]);
    expected.extend_from_slice(&[4; 32]);
    expected.extend_from_slice(&[5; 32]);
    expected.extend_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
    expected.extend_from_slice(&[6; 32]);
    expected.extend_from_slice(&[7; 32]);
    expected.extend_from_slice(&[8; 32]);
    expected.extend_from_slice(&[0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18]);
    expected.extend_from_slice(&[0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28]);
    expected.extend_from_slice(&[0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38]);
    expected.extend_from_slice(original.finalized_chain_anchor.block_hash.as_ref());
    let inline: [u8; GLOBAL_BEACON_PULSE_PAYLOAD_LEN_V1] =
        global_threshold_beacon_pulse_payload_v1(&original);
    assert_eq!(inline.as_slice(), expected);

    // Independent concatenation checks the exact domain, u32 big-endian length,
    // payload and outputs, including an explicitly supplied verified seed.
    let verified_seed = [0xA7; 32];
    let mut preimage = b"iroha.global-threshold-beacon.pulse-id.v1\0".to_vec();
    preimage.extend_from_slice(&u32::try_from(expected.len()).unwrap().to_be_bytes());
    preimage.extend_from_slice(&expected);
    preimage.extend_from_slice(&original.signature);
    preimage.extend_from_slice(&verified_seed);
    assert_eq!(
        global_threshold_beacon_pulse_id_v1(&original, verified_seed),
        *Hash::new(&preimage).as_ref()
    );
    assert_ne!(
        global_threshold_beacon_pulse_id_v1(&original, verified_seed),
        global_threshold_beacon_pulse_id_v1(&original, original.seed)
    );
}
