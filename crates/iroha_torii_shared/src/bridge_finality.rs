//! Exact progress bindings for challenge-bound finality attestation reads.

use iroha_data_model::NetworkId;
use iroha_model_base::peer::PeerId;
use norito::derive::{JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize};

/// Stable HTTP 409 code for a requested, applied, or reducer tip height mismatch.
pub const BRIDGE_FINALITY_ATTESTATION_TIP_MISMATCH_CODE: &str =
    "bridge_finality_attestation_tip_mismatch";
/// Maximum complete encoded tip-mismatch envelope accepted by native clients.
pub const BRIDGE_FINALITY_ATTESTATION_TIP_MISMATCH_MAX_BYTES: usize = 4096;

/// Exact request and observed heights for retrying one finality attestation snapshot.
///
/// This unsigned progress observation is not proof of finality. Clients must retain
/// their deadline and authenticate a fresh successful attestation before completion.
#[derive(
    JsonDeserialize, JsonSerialize, NoritoDeserialize, NoritoSerialize, Debug, Clone, PartialEq, Eq,
)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_torii_shared::bridge_finality::BridgeFinalityAttestationTipMismatchV1"
)]
pub struct BridgeFinalityAttestationTipMismatchV1 {
    /// Exact requested attestation height.
    pub requested_height: u64,
    /// Applied tip from the immutable state view used by the attestation builder.
    pub applied_height: u64,
    /// Durable decision height in the independently captured reducer status.
    pub status_height: u64,
    /// Exact nonzero challenge supplied by the caller.
    pub challenge: [u8; 32],
    /// Configured node identity which would sign a successful attestation.
    pub node_id: PeerId,
    /// Genesis-derived identity of the selected node's state.
    pub network_id: NetworkId,
}

impl BridgeFinalityAttestationTipMismatchV1 {
    /// Require a nonzero request and a concrete difference between height snapshots.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.requested_height > 0
            && self.applied_height > 0
            && self.status_height > 0
            && (self.requested_height != self.applied_height
                || self.status_height != self.applied_height)
            && self.challenge != [0; 32]
    }

    /// Bind progress to the caller's exact height, challenge, node and network.
    #[must_use]
    pub fn matches(
        &self,
        requested_height: u64,
        challenge: [u8; 32],
        node_id: &PeerId,
        network_id: NetworkId,
    ) -> bool {
        self.is_valid()
            && self.requested_height == requested_height
            && self.challenge == challenge
            && &self.node_id == node_id
            && self.network_id == network_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};

    #[test]
    fn tip_mismatch_requires_exact_selector_and_real_height_progress() {
        let key = KeyPair::try_from_seed(vec![71; 32], Algorithm::BlsNormal).unwrap();
        let node_id = PeerId::new(key.public_key().clone());
        let network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"tip mismatch test",
        )));
        let value = BridgeFinalityAttestationTipMismatchV1 {
            requested_height: 10,
            applied_height: 9,
            status_height: 10,
            challenge: [7; 32],
            node_id: node_id.clone(),
            network_id,
        };
        assert!(value.matches(10, [7; 32], &node_id, network_id));
        assert!(!value.matches(9, [7; 32], &node_id, network_id));
        assert!(!value.matches(10, [8; 32], &node_id, network_id));
        let other = KeyPair::try_from_seed(vec![72; 32], Algorithm::BlsNormal).unwrap();
        assert!(!value.matches(
            10,
            [7; 32],
            &PeerId::new(other.public_key().clone()),
            network_id
        ));
        let foreign = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            b"other network",
        )));
        assert!(!value.matches(10, [7; 32], &node_id, foreign));
        for (requested, applied, status, valid) in [
            (9, 10, 10, true),
            (10, 10, 9, true),
            (10, 10, 10, false),
            (0, 9, 10, false),
            (10, 0, 10, false),
            (10, 9, 0, false),
        ] {
            let changed = BridgeFinalityAttestationTipMismatchV1 {
                requested_height: requested,
                applied_height: applied,
                status_height: status,
                ..value.clone()
            };
            assert_eq!(changed.is_valid(), valid);
        }
        assert!(
            !BridgeFinalityAttestationTipMismatchV1 {
                challenge: [0; 32],
                ..value.clone()
            }
            .is_valid()
        );
        let json = norito::json::to_vec(&value).unwrap();
        assert_eq!(
            norito::json::from_slice::<BridgeFinalityAttestationTipMismatchV1>(&json).unwrap(),
            value
        );
        let encoded = norito::to_bytes(&value).unwrap();
        assert_eq!(
            norito::decode_canonical_with_limits::<BridgeFinalityAttestationTipMismatchV1>(
                &encoded,
                norito::canonical_decode_limits(encoded.len())
            )
            .unwrap(),
            value
        );
        let details = crate::ErrorDetails {
            bridge_finality_attestation_tip_mismatch: Some(value),
            ..crate::ErrorDetails::default()
        };
        assert!(!details.is_empty());
    }
}
