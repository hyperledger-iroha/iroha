//! Retained ordinary-transaction Load finality evidence.
//!
//! Decoding these bytes validates their shape only. The installed native proof owner
//! authenticates its fixed history anchor, exact source key, proof and both carried claims.

use super::{
    WalletResult, decode_frame_v1, encode_frame_v1, invalid_v1, require_canonical_field_v1,
    require_version_v1,
};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Complete canonical finality-evidence frame bound, including the proof and both claims.
/// This is local recovery custody; it does not change the 10,000-byte peer-message bound.
pub const KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1: usize = 16_384;
/// Exact native recursive accumulator encoding width.
pub const KAGEMUSHA_WALLET_LOAD_FINALITY_CLAIM_BYTES_V1: usize = 544;

/// Original compact proof evidence retained with an ordinary Load receipt.
///
/// Neither this record nor its decoder grants monetary authority. The independently
/// installed proof policy reconstructs the terminal endpoints from the anchor and receipt,
/// checks the exact proof length and source key, verifies the proof and decides both claims.
#[derive(Debug, Clone, PartialEq, Eq, Decode, Encode, IntoSchema, norito::NoritoSchema)]
#[norito_schema(
    name = "iroha_data_model::kagemusha::kagemusha_wallet_v1::KagemushaWalletLoadFinalityV1"
)]
pub struct KagemushaWalletLoadFinalityV1 {
    /// First-release version, exactly one.
    pub version: u16,
    /// Canonical scalar digest of the independently installed history anchor.
    pub anchor_digest: [u8; 32],
    /// Canonical ordinary receipt digest.
    pub receipt_digest: [u8; 32],
    /// Original terminal proof bytes, whose exact length is fixed by the installed source key.
    pub proof: Vec<u8>,
    /// Exact carried Pallas claim encoding.
    pub pallas_claim: [u8; KAGEMUSHA_WALLET_LOAD_FINALITY_CLAIM_BYTES_V1],
    /// Exact carried Vesta claim encoding.
    pub vesta_claim: [u8; KAGEMUSHA_WALLET_LOAD_FINALITY_CLAIM_BYTES_V1],
}

impl KagemushaWalletLoadFinalityV1 {
    /// Validate bounded data shape without checking a proof or interpreting its claims.
    ///
    /// # Errors
    /// Rejects another version, noncanonical digest fields, an empty proof or an oversized frame.
    pub fn validate(&self) -> WalletResult<()> {
        require_version_v1("load_finality.version", self.version)?;
        require_canonical_field_v1("load_finality.anchor_digest", &self.anchor_digest)?;
        require_canonical_field_v1("load_finality.receipt_digest", &self.receipt_digest)?;
        if self.proof.is_empty() {
            return Err(invalid_v1("load_finality.proof"));
        }
        if self.proof.len() > KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1 {
            return Err(
                super::KagemushaWalletValidationErrorV1::EncodedSizeExceeded {
                    actual: self.proof.len(),
                    max: KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1,
                },
            );
        }
        encode_frame_v1(self, KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1)?;
        Ok(())
    }

    /// Encode one validated complete canonical evidence frame.
    ///
    /// # Errors
    /// Rejects malformed fields, encoding errors or an oversized frame.
    pub fn to_canonical_bytes(&self) -> WalletResult<Vec<u8>> {
        self.validate()?;
        encode_frame_v1(self, KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1)
    }

    /// Decode one bounded canonical evidence frame without granting proof authority.
    ///
    /// # Errors
    /// Rejects oversized, noncanonical or malformed data before native proof verification.
    pub fn decode_canonical(bytes: &[u8]) -> WalletResult<Self> {
        let evidence: Self = decode_frame_v1(bytes, KAGEMUSHA_WALLET_LOAD_FINALITY_MAX_BYTES_V1)?;
        evidence.validate()?;
        Ok(evidence)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_canonical_evidence_roundtrip_is_shape_only() {
        let evidence = KagemushaWalletLoadFinalityV1 {
            version: 1,
            anchor_digest: [1; 32],
            receipt_digest: [2; 32],
            // Exercise custody of a full-width terminal proof and both carried claims.
            // These arbitrary bytes test the codec bound only, never proof acceptance.
            proof: vec![7; 9_856],
            pallas_claim: [0; 544],
            vesta_claim: [0; 544],
        };
        let encoded = evidence.to_canonical_bytes().unwrap();
        assert_eq!(
            KagemushaWalletLoadFinalityV1::decode_canonical(&encoded).unwrap(),
            evidence
        );
        let mut trailing = encoded.clone();
        trailing.push(0);
        assert!(KagemushaWalletLoadFinalityV1::decode_canonical(&trailing).is_err());
        assert!(
            KagemushaWalletLoadFinalityV1::decode_canonical(&encoded[..encoded.len() - 1]).is_err()
        );
        for bad in [
            KagemushaWalletLoadFinalityV1 {
                version: 2,
                ..evidence.clone()
            },
            KagemushaWalletLoadFinalityV1 {
                anchor_digest: [255; 32],
                ..evidence.clone()
            },
            KagemushaWalletLoadFinalityV1 {
                receipt_digest: [255; 32],
                ..evidence.clone()
            },
            KagemushaWalletLoadFinalityV1 {
                proof: Vec::new(),
                ..evidence.clone()
            },
            KagemushaWalletLoadFinalityV1 {
                proof: vec![7; 16_384],
                ..evidence
            },
        ] {
            assert!(bad.to_canonical_bytes().is_err());
            assert!(
                KagemushaWalletLoadFinalityV1::decode_canonical(
                    &norito::encode_canonical(&bad).unwrap()
                )
                .is_err()
            );
        }
    }
}
