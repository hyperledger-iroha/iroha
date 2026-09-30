//! Authenticated, non-authorizing finality anchors for testnet mint observation.
//!
//! The complete initial checkpoint must be pinned independently of Torii's operation status and
//! finality response. This module checks every consecutive signed native decision before the
//! diagnostic owner may pin the resulting checkpoint for a pre-reserved operation.

use iroha_core_zk::kagemusha_v1_recursion::KagemushaVerifiedFinalityChainV1;
use iroha_data_model::{
    NetworkId,
    isi::kagemusha_v1::KagemushaFinalityTrustAnchorV1,
    sumeragi_finality::{SumeragiFinalityCheckpoint, SumeragiFinalityProof},
};

use crate::committed_transaction_inclusion::MAX_CHAIN_JSON_BYTES;
#[cfg(unix)]
use crate::{
    kagemusha_testnet_observation_v1::pin_kagemusha_testnet_authenticated_finality_anchor_v1,
    kagemusha_testnet_publication_v1::TestnetPublicationPermitV1,
};

const MAX_CHAIN_BUNDLES: usize = 4096;

fn verify_chain_token_from_json_v1(
    expected_network_id: NetworkId,
    trusted_checkpoint: &SumeragiFinalityCheckpoint,
    chain_json: &[u8],
) -> Result<KagemushaVerifiedFinalityChainV1, String> {
    if chain_json.is_empty() || chain_json.len() > MAX_CHAIN_JSON_BYTES {
        return Err("KAGEMUSHA finality chain exceeds its bound".to_owned());
    }
    let chain_json = std::str::from_utf8(chain_json)
        .map_err(|_| "KAGEMUSHA finality chain is not UTF-8".to_owned())?;
    let chain: Vec<SumeragiFinalityProof> = norito::json::from_json(chain_json)
        .map_err(|error| format!("invalid KAGEMUSHA finality chain: {error}"))?;
    if chain.is_empty() || chain.len() > MAX_CHAIN_BUNDLES {
        return Err("KAGEMUSHA finality chain must contain 1..4096 bundles".to_owned());
    }
    KagemushaVerifiedFinalityChainV1::verify(expected_network_id, trusted_checkpoint, &chain)
        .map_err(|error| format!("KAGEMUSHA signed finality chain failed: {error}"))
}

/// Verify a consecutive native Sumeragi proof page from an independently selected checkpoint.
///
/// `trusted_checkpoint` must come from an authenticated operator checkpoint, never the
/// supplied JSON, an operation-status hint, or a local journal. The returned checkpoint is
/// inspectable evidence only; it grants neither hardware nor monetary authority.
///
/// # Errors
///
/// Rejects an empty or oversized chain, malformed JSON, wrong network or selected checkpoint, invalid
/// validator certificates, nonconsecutive heights, or inconsistent commitments.
pub fn verify_kagemusha_testnet_finality_anchor_from_chain_v1(
    expected_network_id: NetworkId,
    trusted_checkpoint: &SumeragiFinalityCheckpoint,
    chain_json: &[u8],
) -> Result<KagemushaFinalityTrustAnchorV1, String> {
    verify_chain_token_from_json_v1(expected_network_id, trusted_checkpoint, chain_json)
        .map(|verified| verified.anchor().clone())
}

/// Verify a finality chain and pin its resulting checkpoint for an already reserved testnet top-up.
///
/// Only a trusted Rust host may call this method. The host must obtain the complete initial checkpoint from
/// separately authenticated configuration and the operation ID from its private reservation.
/// The owner rejects missing reservations, changed pins, and a network outside its signed release.
/// The C/JNI caller cannot install a pin or substitute finality coordinates.
/// The returned anchor is taken from the same verified token passed to the owner;
/// an exact retry reports `false` with that anchor.
///
/// # Errors
///
/// Rejects invalid finality, missing native owner or reservation, wrong release network, or a
/// replacement operation pin. No diagnostic lineage advances on failure.
#[cfg(unix)]
pub(crate) fn pin_kagemusha_testnet_authenticated_finality_chain_v1(
    publication: &TestnetPublicationPermitV1<'_>,
    operation_id: [u8; 32],
    expected_network_id: NetworkId,
    trusted_checkpoint: &SumeragiFinalityCheckpoint,
    chain_json: &[u8],
) -> Result<(bool, KagemushaFinalityTrustAnchorV1), String> {
    publication.require_valid()?;
    verify_then_pin_chain(
        expected_network_id,
        trusted_checkpoint,
        chain_json,
        |verified| {
            pin_kagemusha_testnet_authenticated_finality_anchor_v1(
                publication,
                operation_id,
                verified,
            )
        },
    )
}

#[cfg(unix)]
pub(crate) fn verify_then_pin_chain(
    expected_network_id: NetworkId,
    trusted_checkpoint: &SumeragiFinalityCheckpoint,
    chain_json: &[u8],
    pin: impl FnOnce(&KagemushaVerifiedFinalityChainV1) -> Result<bool, String>,
) -> Result<(bool, KagemushaFinalityTrustAnchorV1), String> {
    let verified =
        verify_chain_token_from_json_v1(expected_network_id, trusted_checkpoint, chain_json)?;
    let newly_pinned = pin(&verified)?;
    Ok((newly_pinned, verified.anchor().clone()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn network() -> NetworkId {
        checkpoint(1).network_id()
    }

    fn checkpoint(height: u64) -> SumeragiFinalityCheckpoint {
        let bytes: &[u8] = match height {
            1 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../fixtures/sumeragi/native-finality/genesis-checkpoint.nrt"
            )),
            2 => include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../fixtures/sumeragi/native-finality/height-2-checkpoint.nrt"
            )),
            _ => panic!("fixture checkpoint height"),
        };
        SumeragiFinalityCheckpoint::decode_canonical(bytes).unwrap()
    }
    #[test]
    fn finality_chain_requires_bounded_authenticated_bundles() {
        assert!(
            verify_kagemusha_testnet_finality_anchor_from_chain_v1(network(), &checkpoint(1), b"")
                .is_err()
        );
        assert!(
            verify_kagemusha_testnet_finality_anchor_from_chain_v1(
                network(),
                &checkpoint(1),
                b"[]"
            )
            .is_err()
        );
        assert!(
            verify_kagemusha_testnet_finality_anchor_from_chain_v1(
                network(),
                &checkpoint(1),
                b"[{}]"
            )
            .is_err()
        );
        assert!(
            verify_kagemusha_testnet_finality_anchor_from_chain_v1(
                network(),
                &checkpoint(1),
                &[0xff]
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn pin_cannot_be_created_from_invalid_finality() {
        let gate = crate::kagemusha_testnet_publication_v1::TestnetPublicationGateV1::for_test();
        let publication = gate.dispatch().unwrap();
        assert!(
            pin_kagemusha_testnet_authenticated_finality_chain_v1(
                &publication.permit(),
                [7; 32],
                network(),
                &checkpoint(1),
                b"[]"
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn invalid_chain_never_reaches_native_pin() {
        let mut called = false;
        assert!(
            verify_then_pin_chain(network(), &checkpoint(1), b"[]", |_| {
                called = true;
                Ok(true)
            })
            .is_err()
        );
        assert!(!called);
    }
    #[cfg(unix)]
    #[test]
    fn genuine_native_page_is_verified_before_pin_and_rejects_replay() {
        let root = checkpoint(1);
        let tip = checkpoint(2);
        let proofs = vec![root.tip().clone(), tip.tip().clone()];
        let json = norito::json::to_json(&proofs).unwrap();
        let anchor = verify_kagemusha_testnet_finality_anchor_from_chain_v1(
            root.network_id(),
            &root,
            json.as_bytes(),
        )
        .unwrap();
        assert_eq!(anchor.network_id, root.network_id());
        assert_eq!(anchor.checkpoint, tip);
        let mut called = 0;
        let (new_pin, pinned) =
            verify_then_pin_chain(root.network_id(), &root, json.as_bytes(), |verified| {
                assert_eq!(verified.first_checkpoint(), &root);
                assert_eq!(verified.anchor(), &anchor);
                called += 1;
                Ok(true)
            })
            .unwrap();
        assert!(new_pin);
        assert_eq!(pinned, anchor);
        assert_eq!(called, 1);
        let replay = norito::json::to_json(&vec![root.tip(), root.tip()]).unwrap();
        assert!(
            verify_then_pin_chain(root.network_id(), &root, replay.as_bytes(), |_| {
                called += 1;
                Ok(true)
            })
            .is_err()
        );
        assert_eq!(called, 1);
        assert!(
            verify_then_pin_chain(root.network_id(), &tip, json.as_bytes(), |_| {
                called += 1;
                Ok(true)
            })
            .is_err()
        );
        assert_eq!(called, 1);
    }
}
