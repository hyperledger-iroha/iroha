//! Canonical wire types for the authenticated SoraFS hedging and billing API.
//!
//! Client and server code must use these shared types directly. Duplicating a
//! structurally identical Norito type under another Rust name changes its
//! schema hash unless an explicit schema name is retained.

use std::{error::Error, fmt};

use norito::derive::{NoritoDeserialize, NoritoSerialize};

/// Stable Norito schema name for one billing acknowledgement proof.
pub const BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1: &str =
    "iroha.torii.v1.sorafs.billing.acknowledgement_proof";

/// Lowercase hash of [`BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1`] under the
/// Norito V1 type-name schema domain.
pub const BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_HASH_HEX_V1: &str =
    "fe75acabe03d788012f2e7c556319997";

/// Maximum external authentication-proof bytes accepted by the V1 route.
pub const BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1: usize = 64 * 1024;

/// Canonical owner proof submitted when acknowledging one published statement.
#[derive(Clone, PartialEq, Eq, NoritoSerialize, NoritoDeserialize)]
#[norito(schema_name = "iroha.torii.v1.sorafs.billing.acknowledgement_proof")]
pub struct BillingAcknowledgementProofV1 {
    /// Non-zero client-generated idempotency nonce authenticated by the
    /// canonical Torii request signature and by the external owner proof.
    pub request_nonce: [u8; 32],
    /// Bounded proof over the service's proof-independent request digest.
    pub authentication_proof: Vec<u8>,
}

impl BillingAcknowledgementProofV1 {
    /// Construct a canonical proof from a lowercase hexadecimal request nonce.
    ///
    /// # Errors
    ///
    /// Rejects a nonce that is not exactly 32 non-zero lowercase hexadecimal
    /// bytes or a proof outside the inclusive `1..=64 KiB` bound.
    pub fn try_from_hex(
        request_nonce_hex: &str,
        authentication_proof: Vec<u8>,
    ) -> Result<Self, BillingAcknowledgementProofErrorV1> {
        if request_nonce_hex.len() != 64
            || !request_nonce_hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(BillingAcknowledgementProofErrorV1::InvalidRequestNonce);
        }
        let mut request_nonce = [0_u8; 32];
        hex::decode_to_slice(request_nonce_hex, &mut request_nonce)
            .map_err(|_| BillingAcknowledgementProofErrorV1::InvalidRequestNonce)?;
        Self::try_new(request_nonce, authentication_proof)
    }

    /// Construct a canonical proof from an exact binary request nonce.
    ///
    /// # Errors
    ///
    /// Rejects a zero nonce or a proof outside the inclusive `1..=64 KiB`
    /// bound.
    pub fn try_new(
        request_nonce: [u8; 32],
        authentication_proof: Vec<u8>,
    ) -> Result<Self, BillingAcknowledgementProofErrorV1> {
        let proof = Self {
            request_nonce,
            authentication_proof,
        };
        proof.validate()?;
        Ok(proof)
    }

    /// Validate the complete V1 request body.
    ///
    /// # Errors
    ///
    /// Rejects a zero nonce or a proof outside the inclusive `1..=64 KiB`
    /// bound.
    pub fn validate(&self) -> Result<(), BillingAcknowledgementProofErrorV1> {
        if self.request_nonce == [0; 32] {
            return Err(BillingAcknowledgementProofErrorV1::InvalidRequestNonce);
        }
        if self.authentication_proof.is_empty()
            || self.authentication_proof.len() > BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1
        {
            return Err(
                BillingAcknowledgementProofErrorV1::InvalidAuthenticationProofLength {
                    actual: self.authentication_proof.len(),
                },
            );
        }
        Ok(())
    }
}

impl fmt::Debug for BillingAcknowledgementProofV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("BillingAcknowledgementProofV1")
            .field("request_nonce", &hex::encode(self.request_nonce))
            .field("authentication_proof", &"[REDACTED]")
            .finish()
    }
}

/// Canonical-construction failure for a billing acknowledgement proof.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BillingAcknowledgementProofErrorV1 {
    /// The request nonce is zero or not one canonical lowercase 32-byte digest.
    InvalidRequestNonce,
    /// The authentication proof is empty or exceeds the V1 byte ceiling.
    InvalidAuthenticationProofLength {
        /// Observed proof length.
        actual: usize,
    },
}

impl fmt::Display for BillingAcknowledgementProofErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRequestNonce => formatter.write_str(
                "billing acknowledgement request nonce must be one non-zero lowercase 32-byte hexadecimal digest",
            ),
            Self::InvalidAuthenticationProofLength { actual } => write!(
                formatter,
                "billing acknowledgement authentication proof must contain 1..={BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1} bytes; observed {actual}",
            ),
        }
    }
}

impl Error for BillingAcknowledgementProofErrorV1 {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_schema_name_and_roundtrip_are_exact() {
        assert_eq!(
            <BillingAcknowledgementProofV1 as norito::NoritoSerialize>::schema_hash(),
            norito::core::schema_hash_for_name(BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_NAME_V1)
        );
        assert_eq!(
            hex::encode(<BillingAcknowledgementProofV1 as norito::NoritoSerialize>::schema_hash()),
            BILLING_ACKNOWLEDGEMENT_PROOF_SCHEMA_HASH_HEX_V1
        );
        let proof = BillingAcknowledgementProofV1::try_new([0x91; 32], vec![0xa5; 64])
            .expect("canonical proof");
        let bytes = norito::to_bytes(&proof).expect("encode canonical proof");
        assert_eq!(bytes.len(), 146);
        assert_eq!(
            hex::encode(&bytes),
            format!(
                "4e5254300000fe75acabe03d788012f2e7c556319997006a0000000000000080460fddbba276090220{}484000000000000000{}",
                "91".repeat(32),
                "a5".repeat(64),
            )
        );
        let decoded: BillingAcknowledgementProofV1 =
            norito::decode_from_bytes(&bytes).expect("decode canonical proof");
        assert_eq!(decoded, proof);
        assert_eq!(
            norito::to_bytes(&decoded).expect("re-encode canonical proof"),
            bytes,
            "framed bytes must remain deterministic"
        );
    }

    #[test]
    fn construction_is_strict_and_debug_redacts_proof() {
        for nonce in [
            "00".repeat(32),
            "AA".repeat(32),
            "0x11".repeat(16),
            "11".repeat(31),
        ] {
            assert!(
                BillingAcknowledgementProofV1::try_from_hex(&nonce, vec![1]).is_err(),
                "nonce {nonce:?} must fail",
            );
        }
        assert!(BillingAcknowledgementProofV1::try_new([1; 32], Vec::new()).is_err());
        assert!(
            BillingAcknowledgementProofV1::try_new(
                [1; 32],
                vec![0; BILLING_ACKNOWLEDGEMENT_PROOF_MAX_BYTES_V1 + 1],
            )
            .is_err()
        );

        let proof = BillingAcknowledgementProofV1::try_new([0x22; 32], vec![0xa5; 32])
            .expect("canonical proof");
        let debug = format!("{proof:?}");
        assert!(debug.contains("[REDACTED]"));
        assert!(!debug.contains("165"));
    }
}
