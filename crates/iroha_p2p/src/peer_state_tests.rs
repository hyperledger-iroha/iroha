// Handshake-state admission tests included from `peer::run::state::tests`.

use super::*;

fn consensus_caps(fingerprint: [u8; 32]) -> ConsensusConfigCaps {
    ConsensusConfigCaps {
        execution_policy_hash: [0xB0; 32],
        nexus_policy_digest: [0xC1; 32],
        v2_config_fingerprint: fingerprint,
        ivm_gas_schedule_hash: [0xD2; 32],
    }
}

#[test]
fn v2_peer_admission_compares_canonical_shared_config_fingerprint() {
    let expected = consensus_caps([0xA5; 32]);
    assert_eq!(
        consensus_config_mismatch(&expected, &expected),
        None,
        "identical canonical admission digests must be accepted",
    );

    let changed = consensus_caps([0x5A; 32]);
    let mismatch = consensus_config_mismatch(&expected, &changed)
        .expect("different shared v2 config hashes must be rejected");
    assert!(mismatch.contains("v2_config_fingerprint mismatch"));
    assert!(mismatch.contains(&hex_bytes(&[0xA5; 32])));
    assert!(mismatch.contains(&hex_bytes(&[0x5A; 32])));
}
