//! Authenticated, non-authorizing finality anchors for testnet mint observation.
//!
//! The first height-context ID must be pinned independently of Torii's operation status and
//! finality-bundle response. This module checks every consecutive signed bundle before the
//! native diagnostic owner may pin the last context for a pre-reserved operation.

use iroha_core::zk::kagemusha_v1_recursion::KagemushaVerifiedFinalityChainV1;
use iroha_data_model::{
    NetworkId, block::consensus_v2::HeightContextId, bridge::BridgeFinalityBundle,
    isi::kagemusha_v1::KagemushaFinalityTrustAnchorV1,
};

use crate::committed_transaction_inclusion::MAX_CHAIN_JSON_BYTES;
#[cfg(unix)]
use crate::kagemusha_testnet_observation_v1::pin_kagemusha_testnet_authenticated_finality_anchor_v1;

const MAX_CHAIN_BUNDLES: usize = 4096;

fn verify_chain_token_from_json_v1(
    expected_network_id: NetworkId,
    trusted_first_context_id: HeightContextId,
    chain_json: &[u8],
) -> Result<KagemushaVerifiedFinalityChainV1, String> {
    if chain_json.is_empty() || chain_json.len() > MAX_CHAIN_JSON_BYTES {
        return Err("KAGEMUSHA finality chain exceeds its bound".to_owned());
    }
    let chain_json = std::str::from_utf8(chain_json)
        .map_err(|_| "KAGEMUSHA finality chain is not UTF-8".to_owned())?;
    let chain: Vec<BridgeFinalityBundle> = norito::json::from_json(chain_json)
        .map_err(|error| format!("invalid KAGEMUSHA finality chain: {error}"))?;
    if chain.is_empty() || chain.len() > MAX_CHAIN_BUNDLES {
        return Err("KAGEMUSHA finality chain must contain 1..4096 bundles".to_owned());
    }
    KagemushaVerifiedFinalityChainV1::verify(expected_network_id, trusted_first_context_id, &chain)
        .map_err(|error| format!("KAGEMUSHA signed finality chain failed: {error}"))
}

/// Verify a consecutive Sumeragi-v2 finality chain from an independent first context.
///
/// `trusted_first_context_id` must come from an authenticated operator checkpoint, never the
/// supplied JSON, an operation-status hint, or a local journal. The returned context is
/// inspectable evidence only; it grants neither hardware nor monetary authority.
///
/// # Errors
///
/// Rejects an empty or oversized chain, malformed JSON, wrong network or context, invalid
/// validator certificates, nonconsecutive heights, or inconsistent commitments.
pub fn verify_kagemusha_testnet_finality_anchor_from_chain_v1(
    expected_network_id: NetworkId,
    trusted_first_context_id: HeightContextId,
    chain_json: &[u8],
) -> Result<KagemushaFinalityTrustAnchorV1, String> {
    verify_chain_token_from_json_v1(expected_network_id, trusted_first_context_id, chain_json)
        .map(|verified| verified.anchor())
}

/// Verify a finality chain and pin its last context for an already reserved testnet top-up.
///
/// Only a trusted Rust host may call this method. The host must obtain the first context from
/// separately authenticated configuration and the operation ID from its private reservation.
/// The owner rejects missing reservations, changed pins, and a network outside its signed release.
/// The C/JNI caller cannot install a pin or substitute finality coordinates.
///
/// # Errors
///
/// Rejects invalid finality, missing native owner or reservation, wrong release network, or a
/// replacement operation pin. No diagnostic lineage advances on failure.
#[cfg(unix)]
pub(crate) fn pin_kagemusha_testnet_authenticated_finality_chain_v1(
    operation_id: [u8; 32],
    expected_network_id: NetworkId,
    trusted_first_context_id: HeightContextId,
    chain_json: &[u8],
) -> Result<bool, String> {
    let verified =
        verify_chain_token_from_json_v1(expected_network_id, trusted_first_context_id, chain_json)?;
    pin_kagemusha_testnet_authenticated_finality_anchor_v1(operation_id, &verified)
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Hash, HashOf};

    fn network() -> NetworkId {
        NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::prehashed([3; 32])))
    }

    fn first_context() -> HeightContextId {
        HeightContextId(HashOf::from_untyped_unchecked(Hash::prehashed([5; 32])))
    }

    #[test]
    fn finality_chain_requires_bounded_authenticated_bundles() {
        assert!(
            verify_kagemusha_testnet_finality_anchor_from_chain_v1(network(), first_context(), b"")
                .is_err()
        );
        assert!(
            verify_kagemusha_testnet_finality_anchor_from_chain_v1(
                network(),
                first_context(),
                b"[]"
            )
            .is_err()
        );
        assert!(
            verify_kagemusha_testnet_finality_anchor_from_chain_v1(
                network(),
                first_context(),
                b"[{}]"
            )
            .is_err()
        );
        assert!(
            verify_kagemusha_testnet_finality_anchor_from_chain_v1(
                network(),
                first_context(),
                &[0xff]
            )
            .is_err()
        );
    }

    #[cfg(unix)]
    #[test]
    fn pin_cannot_be_created_from_invalid_finality() {
        assert!(
            pin_kagemusha_testnet_authenticated_finality_chain_v1(
                [7; 32],
                network(),
                first_context(),
                b"[]"
            )
            .is_err()
        );
    }
}
