//! Transaction submission receipt types and signing helpers.

use iroha_crypto::{HashOf, KeyPair, PublicKey, Signature};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    core::NoritoSerialize,
};

use super::{SignedTransaction, signed::TransactionEntrypoint};

/// Domain tag for transaction submission receipt signatures.
pub const TX_SUBMISSION_RECEIPT_DOMAIN: &str = "iroha.tx.submission.receipt@v1";

/// Canonical payload signed by a Torii node when accepting a transaction submission.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
pub struct TransactionSubmissionReceiptPayload {
    /// Canonical transaction entrypoint hash exposed under the legacy field name.
    pub tx_hash: HashOf<SignedTransaction>,
    /// Hash of the submitted transaction entrypoint.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Hash of the inner signed transaction, when the entrypoint carries one.
    #[norito(default)]
    #[norito(skip_serializing_if = "Option::is_none")]
    pub signed_transaction_hash: Option<HashOf<SignedTransaction>>,
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
        let payload_len = self
            .encoded_len_exact()
            .unwrap_or_else(|| self.encoded_len());
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
    /// Fallibly create a signed receipt from the payload.
    ///
    /// # Errors
    /// Returns any backend signing error from `iroha_crypto`.
    pub fn try_sign(
        payload: TransactionSubmissionReceiptPayload,
        key_pair: &KeyPair,
    ) -> Result<Self, iroha_crypto::Error> {
        let signature = Signature::try_new(key_pair.private_key(), &payload.signing_bytes())?;
        Ok(Self { payload, signature })
    }

    /// Create a signed receipt from the payload.
    #[must_use]
    pub fn sign(payload: TransactionSubmissionReceiptPayload, key_pair: &KeyPair) -> Self {
        Self::try_sign(payload, key_pair)
            .expect("signing should succeed for a valid receipt key and payload")
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
            entrypoint_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                [0xA5; 32],
            )),
            signed_transaction_hash: Some(HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0xB6; 32]),
            )),
            submitted_at_ms: 42,
            submitted_at_height: 7,
            signer: key_pair.public_key().clone(),
        };
        let receipt = TransactionSubmissionReceipt::try_sign(payload.clone(), &key_pair)
            .expect("sign receipt");
        assert!(receipt.verify().is_ok());
        let receipt = TransactionSubmissionReceipt::sign(payload, &key_pair);
        assert!(receipt.verify().is_ok());
    }
}
