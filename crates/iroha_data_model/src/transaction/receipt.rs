//! Transaction submission receipt types and signing helpers.

use iroha_crypto::{HashOf, KeyPair, PublicKey, Signature};
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

use super::SignedTransaction;

/// Domain tag for transaction submission receipt signatures.
pub const TX_SUBMISSION_RECEIPT_DOMAIN: &str = "iroha.tx.submission.receipt@v1";

/// Canonical payload signed by a Torii node when accepting a transaction submission.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TransactionSubmissionReceiptPayload {
    /// Hash of the submitted transaction.
    pub tx_hash: HashOf<SignedTransaction>,
    /// Unix timestamp (ms) when Torii accepted the submission.
    pub submitted_at_ms: u64,
    /// Block height observed when the receipt was issued.
    pub submitted_at_height: u64,
    /// Public key of the node that issued the receipt.
    pub signer: PublicKey,
}

impl TransactionSubmissionReceiptPayload {
    /// Deterministic signing bytes for this receipt payload.
    #[must_use]
    pub fn signing_bytes(&self) -> Vec<u8> {
        let domain = TX_SUBMISSION_RECEIPT_DOMAIN.as_bytes();
        let payload_len = self.encoded_len();
        let mut bytes = Vec::with_capacity(domain.len() + payload_len);
        bytes.extend_from_slice(domain);
        self.encode_to(&mut bytes);
        bytes
    }
}

/// Signed receipt acknowledging transaction submission.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TransactionSubmissionReceipt {
    /// Canonical receipt payload.
    pub payload: TransactionSubmissionReceiptPayload,
    /// Signature over the canonical payload.
    pub signature: Signature,
}

impl TransactionSubmissionReceipt {
    /// Create a signed receipt from the payload.
    #[must_use]
    pub fn sign(payload: TransactionSubmissionReceiptPayload, key_pair: &KeyPair) -> Self {
        let signature = Signature::new(key_pair.private_key(), &payload.signing_bytes());
        Self { payload, signature }
    }

    /// Verify the receipt signature against the payload signer.
    ///
    /// # Errors
    /// Returns any signature verification error from `iroha_crypto` if the signature is invalid.
    pub fn verify(&self) -> Result<(), iroha_crypto::Error> {
        self.signature
            .verify(&self.payload.signer, &self.payload.signing_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submission_receipt_roundtrips_signature() {
        let key_pair = KeyPair::random();
        let payload = TransactionSubmissionReceiptPayload {
            tx_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0xA5; 32])),
            submitted_at_ms: 42,
            submitted_at_height: 7,
            signer: key_pair.public_key().clone(),
        };
        let receipt = TransactionSubmissionReceipt::sign(payload, &key_pair);
        assert!(receipt.verify().is_ok());
    }
}
