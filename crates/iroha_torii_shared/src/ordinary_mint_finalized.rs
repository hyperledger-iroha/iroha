//! Account-authenticated read of one exact original ordinary debit and its real native finality.
//! Decoding the selector or response never supplies a Mint, FI, DATA or incoming State grant.
use iroha_data_model::{NetworkId, account::AccountId};
use norito::derive::{NoritoDeserialize, NoritoSerialize};

/// Sole canonical signed-body target; no ordinary operation is decoded by the OEM status route.
pub const ORDINARY_MINT_FINALIZED_ROUTE_V1: &str = "/v1/kagemusha/ordinary/top-up/finality";
/// Finite signed read selector; complete proof and request travel in the response.
pub const ORDINARY_MINT_FINALIZED_REQUEST_MAX_BYTES_V1: usize = 16 * 1024;

/// Exact immutable operation read coordinates. The HTTP signature authenticates all these bytes
/// under the current actual payer account. A hash here is a read selector, never a source grant.
#[derive(Debug, Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize, norito::NoritoSchema)]
#[norito_schema(name = "iroha_torii_shared::ordinary_mint_finalized::OrdinaryMintFinalizedReadV1")]
pub struct OrdinaryMintFinalizedReadV1 {
    /// Sole first-release version.
    pub version: u16,
    /// Independently retained genesis-derived network.
    pub network_id: NetworkId,
    /// Same actual payer W, authenticated against current World account controllers.
    pub payer: AccountId,
    /// Original Native Mint operation; absence is pending, never permission to debit again.
    pub operation_id: [u8; 32],
    /// SHA256 of the full sole unsigned request including the original paired proof/ciphertext.
    pub request_original_sha256: [u8; 32],
    /// SHA256 of the complete immutable signed Core decision consumed by this operation.
    pub issuer_decision_original_sha256: [u8; 32],
}
impl OrdinaryMintFinalizedReadV1 {
    /// Check finite data only. No policy, finality or financial owner is constructed.
    /// # Errors
    /// Refuses wrong version, missing selectors or a full canonical body beyond its finite bound.
    pub fn canonical_wire(&self) -> Result<Vec<u8>, norito::Error> {
        if self.version != 1
            || [
                self.operation_id,
                self.request_original_sha256,
                self.issuer_decision_original_sha256,
            ]
            .contains(&[0; 32])
            || norito::canonical_frame_len(self)? > ORDINARY_MINT_FINALIZED_REQUEST_MAX_BYTES_V1
        {
            return Err(norito::Error::Message(
                "ordinary finalized read selector rejected".into(),
            ));
        }
        norito::encode_canonical(self)
    }
    /// Decode the sole complete canonical selector before any original receipt is borrowed.
    /// # Errors
    /// Refuses oversize, trailing bytes, another layout or missing immutable coordinates.
    pub fn decode_original(raw: &[u8]) -> Result<Self, norito::Error> {
        if raw.is_empty() || raw.len() > ORDINARY_MINT_FINALIZED_REQUEST_MAX_BYTES_V1 {
            return Err(norito::Error::Message(
                "ordinary finalized read original bound rejected".into(),
            ));
        }
        let value: Self = norito::decode_canonical_with_limits(
            raw,
            norito::canonical_decode_limits(ORDINARY_MINT_FINALIZED_REQUEST_MAX_BYTES_V1),
        )?;
        if value.canonical_wire()? != raw {
            return Err(norito::Error::Message(
                "ordinary finalized read original differs".into(),
            ));
        }
        Ok(value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair};
    fn selector() -> OrdinaryMintFinalizedReadV1 {
        let key = KeyPair::from_seed(vec![7; 32], Algorithm::Ed25519);
        OrdinaryMintFinalizedReadV1 {
            version: 1,
            network_id: NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::prehashed([8; 32]),
            )),
            payer: AccountId::new(key.public_key().clone()),
            operation_id: [1; 32],
            request_original_sha256: [2; 32],
            issuer_decision_original_sha256: [3; 32],
        }
    }
    #[test]
    fn finalized_selector_pins_all_immutable_coordinates_and_exact_framing() {
        let s = selector();
        let raw = s.canonical_wire().unwrap();
        assert_eq!(
            OrdinaryMintFinalizedReadV1::decode_original(&raw).unwrap(),
            s
        );
        let mut changed = s.clone();
        changed.issuer_decision_original_sha256[0] ^= 1;
        assert_ne!(changed.canonical_wire().unwrap(), raw);
        let mut trailing = raw;
        trailing.push(0);
        assert!(OrdinaryMintFinalizedReadV1::decode_original(&trailing).is_err());
        changed.operation_id = [0; 32];
        assert!(changed.canonical_wire().is_err());
    }
    #[test]
    fn finalized_selector_refuses_absence_and_over_budget_before_decode() {
        assert!(OrdinaryMintFinalizedReadV1::decode_original(&[]).is_err());
        assert!(
            OrdinaryMintFinalizedReadV1::decode_original(&vec![
                0;
                ORDINARY_MINT_FINALIZED_REQUEST_MAX_BYTES_V1
                    + 1
            ])
            .is_err()
        );
    }
}
