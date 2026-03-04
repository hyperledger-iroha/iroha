use sorafs_manifest::{CouncilSignature, GovernanceProofs};

#[test]
fn governance_proofs_default_is_empty() {
    let proofs = GovernanceProofs::default();
    assert!(proofs.council_signatures.is_empty());
}

#[test]
fn governance_proofs_roundtrip() {
    let proofs = GovernanceProofs {
        council_signatures: vec![
            CouncilSignature {
                signer: [0x11; 32],
                signature: vec![0x22; 64],
            },
            CouncilSignature {
                signer: [0xAA; 32],
                signature: vec![0xBB; 64],
            },
        ],
    };
    let bytes = norito::to_bytes(&proofs).expect("serialize proofs");
    let decoded: GovernanceProofs = norito::decode_from_bytes(&bytes).expect("deserialize proofs");
    assert_eq!(decoded, proofs);
}
