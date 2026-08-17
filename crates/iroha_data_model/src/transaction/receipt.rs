//! Transaction submission receipt types and signing helpers.
use super::{SignedTransaction, signed::TransactionEntrypoint};
use iroha_crypto::{Algorithm, HashOf, KeyPair, PublicKey, Signature};
use iroha_schema::IntoSchema;
use norito::{
    codec::{Decode, Encode},
    core::NoritoSerialize,
};
fn verify_signature_for_signer(
    signature: &Signature,
    signer: &PublicKey,
    payload: &[u8],
) -> Result<(), iroha_crypto::Error> {
    match signer.try_algorithm() {
        Ok(Algorithm::Ed25519) => {
            iroha_crypto::ed25519_parse_signature(signature.payload())?;
        }
        Ok(Algorithm::MlDsa) => {
            iroha_crypto::mldsa65_parse_signature(signature.payload())?;
        }
        _ => {}
    }
    signature.verify(signer, payload)
}
/// Domain tag for transaction submission receipt signatures.
pub const TX_SUBMISSION_RECEIPT_DOMAIN: &str = "iroha.tx.submission.receipt@v1";
/// Canonical payload signed by a Torii node when accepting a transaction submission.
#[derive(Clone, Debug, PartialEq, Eq, Encode, Decode, IntoSchema)]
#[cfg_attr(
    feature = "json",
    derive(crate::DeriveJsonSerialize, crate::DeriveJsonDeserialize)
)]
#[norito(deny_unknown_fields)]
pub struct TransactionSubmissionReceiptPayload {
    /// Canonical hash of the submitted transaction entrypoint.
    pub entrypoint_hash: HashOf<TransactionEntrypoint>,
    /// Hash of the inner signed transaction, when the entrypoint carries one.
    #[norito(required)]
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
#[norito(deny_unknown_fields)]
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
        verify_signature_for_signer(
            &self.signature,
            &self.payload.signer,
            &self.payload.signing_bytes(),
        )
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Encode)]
    struct PreReleaseTransactionSubmissionReceiptPayload {
        tx_hash: HashOf<SignedTransaction>,
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        signed_transaction_hash: Option<HashOf<SignedTransaction>>,
        submitted_at_ms: u64,
        submitted_at_height: u64,
        signer: PublicKey,
    }

    #[derive(Encode)]
    struct PreReleaseReceiptPayloadWithoutSignedTransactionHash {
        entrypoint_hash: HashOf<TransactionEntrypoint>,
        submitted_at_ms: u64,
        submitted_at_height: u64,
        signer: PublicKey,
    }

    fn checked_random_keypair() -> KeyPair {
        KeyPair::try_random().expect("generate checked transaction receipt fixture keypair")
    }
    fn checked_ed25519_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::Ed25519)
            .expect("generate checked Ed25519 transaction receipt fixture keypair")
    }
    fn checked_mldsa_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::MlDsa)
            .expect("generate checked ML-DSA transaction receipt fixture keypair")
    }
    fn sample_receipt_payload(key_pair: &KeyPair) -> TransactionSubmissionReceiptPayload {
        TransactionSubmissionReceiptPayload {
            entrypoint_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed(
                [0xA5; 32],
            )),
            signed_transaction_hash: Some(HashOf::from_untyped_unchecked(
                iroha_crypto::Hash::prehashed([0xB6; 32]),
            )),
            submitted_at_ms: 42,
            submitted_at_height: 7,
            signer: key_pair.public_key().clone(),
        }
    }
    const SMALL_ORDER_ED25519_SIGNATURE_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_ED25519_SIGNATURE_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];
    fn signature_with_payload(replacement_payload: &[u8]) -> Signature {
        Signature::from_bytes(replacement_payload)
    }
    #[test]
    fn submission_receipt_roundtrips_signature() {
        let key_pair = checked_random_keypair();
        let payload = sample_receipt_payload(&key_pair);
        let receipt = TransactionSubmissionReceipt::try_sign(payload.clone(), &key_pair)
            .expect("sign receipt");
        assert!(receipt.verify().is_ok());
        let receipt = TransactionSubmissionReceipt::sign(payload, &key_pair);
        assert!(receipt.verify().is_ok());
    }

    #[test]
    fn submission_receipt_payload_current_layout_roundtrips_canonically() {
        let payload = sample_receipt_payload(&checked_random_keypair());
        let encoded = norito::encode_canonical(&payload).expect("encode current receipt payload");
        let decoded = norito::decode_canonical::<TransactionSubmissionReceiptPayload>(&encoded)
            .expect("decode current receipt payload");
        assert_eq!(decoded, payload);
    }

    #[cfg(feature = "json")]
    #[test]
    fn submission_receipt_json_requires_nullable_hash_and_closed_signed_objects() {
        let key_pair = checked_random_keypair();
        let mut payload = sample_receipt_payload(&key_pair);
        payload.signed_transaction_hash = None;
        let payload_json = norito::json::to_json(&payload)
            .expect("serialize receipt payload with no inner signed transaction");
        let expected_payload_json = format!(
            "{{\"entrypoint_hash\":{entrypoint_hash},\"signed_transaction_hash\":null,\"submitted_at_ms\":42,\"submitted_at_height\":7,\"signer\":{signer}}}",
            entrypoint_hash = norito::json::to_json(&payload.entrypoint_hash)
                .expect("serialize receipt entrypoint hash"),
            signer = norito::json::to_json(&payload.signer).expect("serialize receipt signer"),
        );
        assert_eq!(payload_json, expected_payload_json);
        assert_eq!(
            norito::json::from_str::<TransactionSubmissionReceiptPayload>(&payload_json)
                .expect("deserialize exact receipt payload JSON"),
            payload
        );

        let canonical = norito::json::to_value(&payload).expect("serialize receipt payload");
        let mut missing_hash = canonical.clone();
        assert!(
            missing_hash
                .as_object_mut()
                .expect("receipt payload JSON object")
                .remove("signed_transaction_hash")
                .is_some()
        );
        assert!(
            norito::json::from_value::<TransactionSubmissionReceiptPayload>(missing_hash).is_err(),
            "the first-release receipt payload must require signed_transaction_hash even when null"
        );
        let mut unknown_payload = canonical;
        unknown_payload
            .as_object_mut()
            .expect("receipt payload JSON object")
            .insert("pre_release_field".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<TransactionSubmissionReceiptPayload>(unknown_payload)
                .is_err(),
            "receipt payload unknown fields must fail closed"
        );

        let receipt = TransactionSubmissionReceipt::sign(payload, &key_pair);
        let receipt_json =
            norito::json::to_json(&receipt).expect("serialize signed submission receipt");
        let expected_receipt_json = format!(
            "{{\"payload\":{payload},\"signature\":{signature}}}",
            payload =
                norito::json::to_json(&receipt.payload).expect("serialize signed receipt payload"),
            signature =
                norito::json::to_json(&receipt.signature).expect("serialize receipt signature"),
        );
        assert_eq!(receipt_json, expected_receipt_json);
        assert_eq!(
            norito::json::from_str::<TransactionSubmissionReceipt>(&receipt_json)
                .expect("deserialize exact signed receipt JSON"),
            receipt
        );
        let mut unknown_receipt =
            norito::json::to_value(&receipt).expect("serialize signed receipt");
        unknown_receipt
            .as_object_mut()
            .expect("signed receipt JSON object")
            .insert("pre_release_field".to_owned(), norito::json::Value::Null);
        assert!(
            norito::json::from_value::<TransactionSubmissionReceipt>(unknown_receipt).is_err(),
            "signed receipt unknown fields must fail closed"
        );
    }

    #[test]
    fn submission_receipt_payload_rejects_pre_release_layout_without_inner_hash_slot() {
        let key_pair = checked_random_keypair();
        let current = sample_receipt_payload(&key_pair);
        let pre_release = PreReleaseReceiptPayloadWithoutSignedTransactionHash {
            entrypoint_hash: current.entrypoint_hash,
            submitted_at_ms: current.submitted_at_ms,
            submitted_at_height: current.submitted_at_height,
            signer: current.signer,
        };
        let encoded = norito::encode_canonical(&pre_release)
            .expect("encode pre-release receipt payload without the inner hash slot");
        assert!(
            norito::decode_canonical::<TransactionSubmissionReceiptPayload>(&encoded).is_err(),
            "the first-release receipt payload must reject the shortened layout without signed_transaction_hash"
        );
    }

    #[test]
    fn submission_receipt_payload_rejects_pre_release_duplicate_hash_layout() {
        let key_pair = checked_random_keypair();
        let current = sample_receipt_payload(&key_pair);
        let pre_release = PreReleaseTransactionSubmissionReceiptPayload {
            tx_hash: HashOf::from_untyped_unchecked(iroha_crypto::Hash::prehashed([0xC7; 32])),
            entrypoint_hash: current.entrypoint_hash,
            signed_transaction_hash: current.signed_transaction_hash,
            submitted_at_ms: current.submitted_at_ms,
            submitted_at_height: current.submitted_at_height,
            signer: current.signer,
        };
        let encoded =
            norito::encode_canonical(&pre_release).expect("encode pre-release receipt payload");
        assert!(
            norito::decode_canonical::<TransactionSubmissionReceiptPayload>(&encoded).is_err(),
            "the retired duplicate tx_hash layout must not decode as the first-release payload"
        );
    }
    #[test]
    fn submission_receipt_rejects_malformed_ed25519_signature() {
        let key_pair = checked_ed25519_keypair();
        let receipt =
            TransactionSubmissionReceipt::sign(sample_receipt_payload(&key_pair), &key_pair);
        let mut small_order_signature = receipt.signature.payload().to_vec();
        small_order_signature[..SMALL_ORDER_ED25519_SIGNATURE_R.len()]
            .copy_from_slice(&SMALL_ORDER_ED25519_SIGNATURE_R);
        let mut noncanonical_signature = receipt.signature.payload().to_vec();
        noncanonical_signature[..NONCANONICAL_ED25519_SIGNATURE_R.len()]
            .copy_from_slice(&NONCANONICAL_ED25519_SIGNATURE_R);
        for (label, replacement_payload) in [
            ("all-zero", vec![0_u8; 64]),
            ("short", vec![0x42_u8; 32]),
            ("small-order", small_order_signature),
            ("noncanonical", noncanonical_signature),
        ] {
            let mut invalid_receipt = receipt.clone();
            invalid_receipt.signature = signature_with_payload(&replacement_payload);
            let err = invalid_receipt
                .verify()
                .expect_err("malformed receipt signature must fail admission");
            assert!(
                matches!(
                    err,
                    iroha_crypto::Error::BadSignature | iroha_crypto::Error::Parse(_)
                ),
                "{label} receipt signature failed with unexpected error: {err:?}"
            );
        }
    }
    #[test]
    fn submission_receipt_rejects_malformed_mldsa_signature_lengths() {
        let key_pair = checked_mldsa_keypair();
        let receipt =
            TransactionSubmissionReceipt::sign(sample_receipt_payload(&key_pair), &key_pair);
        receipt
            .verify()
            .expect("valid ML-DSA transaction receipt signature verifies");
        let valid_signature = receipt.signature.payload().to_vec();
        for (label, replacement_payload) in [
            (
                "short",
                valid_signature[..valid_signature.len() - 1].to_vec(),
            ),
            ("overlong", {
                let mut payload = valid_signature.clone();
                payload.push(0x5B);
                payload
            }),
        ] {
            let mut invalid_receipt = receipt.clone();
            invalid_receipt.signature = signature_with_payload(&replacement_payload);
            let err = invalid_receipt
                .verify()
                .expect_err("malformed ML-DSA receipt signature length must fail admission");
            assert!(
                matches!(
                    err,
                    iroha_crypto::Error::BadSignature | iroha_crypto::Error::Parse(_)
                ),
                "{label} receipt ML-DSA signature length failed with unexpected error: {err:?}"
            );
        }
    }
}
