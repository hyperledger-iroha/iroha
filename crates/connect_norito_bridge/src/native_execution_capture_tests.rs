//! Explicit SDK consumption of source-bound, genuine Kagami execution captures.
//!
//! The qualification runner selects and pins the producer and capture before this
//! test starts. The selected local checkpoint is independent of the proof page;
//! this test must never be used to trust a checkpoint from an untrusted response.

use std::io::Read as _;

use iroha_data_model::{
    block::proofs::{BlockProofs, TrustedBlockProofAnchor},
    sumeragi_finality::{
        SumeragiFinalityCheckpoint, SumeragiFinalityProof, SumeragiFinalityVerifier,
    },
    transaction::SignedTransaction,
};

#[test]
#[ignore = "requires an explicitly selected genuine Kagami native execution capture"]
fn genuine_kagami_execution_captures_verify_through_public_sdk_bridge() {
    let path = std::env::var_os("IROHA_KAGAMI_NATIVE_EXECUTION_CAPTURE")
        .expect("the native qualification runner must select the captured artifact");
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .unwrap()
        .take(64 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .unwrap();
    assert!(!bytes.is_empty() && bytes.len() <= 64 * 1024 * 1024);
    let captures: norito::json::Value = norito::json::from_slice(&bytes).unwrap();
    let captures = captures.as_array().unwrap();
    assert_eq!(captures.len(), 2);
    let mut roots = Vec::new();
    let mut pages = Vec::new();
    for (capture, lanes) in captures.iter().zip([1, 4]) {
        assert_eq!(capture["lane_count"].as_u64(), Some(lanes));
        let files = capture["files"].as_object().unwrap();
        assert_eq!(files.len(), 10);
        let get = |name: &str| hex::decode(files.get(name).unwrap().as_str().unwrap()).unwrap();
        let root_bytes = get("height-1-checkpoint.nrt");
        let tip_bytes = get("height-2-checkpoint.nrt");
        let root = SumeragiFinalityCheckpoint::decode_canonical(&root_bytes).unwrap();
        let tip = SumeragiFinalityCheckpoint::decode_canonical(&tip_bytes).unwrap();
        assert_eq!(root.encode_canonical().unwrap(), root_bytes);
        assert_eq!(tip.encode_canonical().unwrap(), tip_bytes);
        assert_eq!(root.height(), 1);
        assert_eq!(tip.height(), 2);
        let json = get("proof-page.json");
        let anchor = crate::verify_kagemusha_testnet_finality_anchor_from_chain_v1(
            root.network_id(),
            &root,
            &json,
        )
        .unwrap();
        assert_eq!(anchor.network_id, root.network_id());
        assert_eq!(anchor.checkpoint, tip);
        let mut verifier = SumeragiFinalityVerifier::from_trusted_checkpoint(
            &root,
            &root.network_id(),
            root.chain_id(),
        )
        .unwrap();
        let proofs: Vec<SumeragiFinalityProof> = norito::json::from_slice(&json).unwrap();
        assert_eq!(proofs.len(), 2);
        verifier.verify_retained_decision(&proofs[0]).unwrap();
        let verified = verifier.verify(&proofs[1]).unwrap();
        assert_eq!(
            verified.block().encode_wire().unwrap(),
            tip.tip().block_wire
        );
        let request_bytes = get("request.nrt");
        let request: SignedTransaction = norito::decode_canonical(&request_bytes).unwrap();
        assert_eq!(norito::encode_canonical(&request).unwrap(), request_bytes);
        request.verify_signature().unwrap();
        assert_eq!(request.network_id(), Some(&root.network_id()));
        assert_eq!(
            verified.block().external_transactions().collect::<Vec<_>>(),
            vec![&request]
        );
        assert!(
            verified
                .block()
                .network_output_at(0)
                .unwrap()
                .1
                .result
                .is_ok()
        );
        let entry_hash = request.hash_as_entrypoint();
        let trusted = TrustedBlockProofAnchor::from_verified_finality(
            verified.block(),
            &verified,
            &entry_hash,
        )
        .unwrap();
        let inclusion_bytes = get("request-proof.nrt");
        let inclusion: BlockProofs = norito::decode_canonical(&inclusion_bytes).unwrap();
        assert_eq!(
            norito::encode_canonical(&inclusion).unwrap(),
            inclusion_bytes
        );
        assert!(inclusion.verify(&trusted));
        let mut changed = inclusion.clone();
        changed.entry_hash = iroha_crypto::HashOf::from_untyped_unchecked(iroha_crypto::Hash::new(
            b"foreign native capture request",
        ));
        assert!(!changed.verify(&trusted));
        for mutation in 0..5 {
            let mut changed = proofs.clone();
            match mutation {
                0 => {
                    changed[1].block_wire.pop();
                }
                1 => {
                    changed[1].block_header = proofs[0].block_header;
                }
                2 => {
                    changed[1].committee.pop();
                }
                3 => {
                    changed.reverse();
                }
                _ => {
                    changed[1] = changed[0].clone();
                }
            }
            let malformed = norito::json::to_vec(&changed).unwrap();
            assert!(
                crate::verify_kagemusha_testnet_finality_anchor_from_chain_v1(
                    root.network_id(),
                    &root,
                    &malformed,
                )
                .is_err()
            );
        }
        assert!(
            crate::verify_kagemusha_testnet_finality_anchor_from_chain_v1(
                root.network_id(),
                &tip,
                &json,
            )
            .is_err()
        );
        roots.push(root);
        pages.push(json);
    }
    assert_ne!(roots[0].network_id(), roots[1].network_id());
    for (root, foreign_page) in [(&roots[0], &pages[1]), (&roots[1], &pages[0])] {
        assert!(
            crate::verify_kagemusha_testnet_finality_anchor_from_chain_v1(
                root.network_id(),
                root,
                foreign_page,
            )
            .is_err()
        );
    }
    assert!(
        crate::verify_kagemusha_testnet_finality_anchor_from_chain_v1(
            roots[1].network_id(),
            &roots[0],
            &pages[0],
        )
        .is_err()
    );
}
