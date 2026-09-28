//! Canonical signed intent for a proved IVM transaction.
//!
//! A proof must bind an intent fixed before the proof and execution outputs exist.
//! This module projects the complete unsigned payload, replacing only the
//! proof-derived fields that would otherwise make its digest self-referential.

use super::{Executable, SignedTransaction, TransactionPayload};
use crate::proof::ProofAttachmentList;
use iroha_crypto::Hash;
use iroha_primitives::const_vec::ConstVec;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};
use thiserror::Error;

/// Domain separator for the canonical proved-IVM signed intent digest.
pub const IVM_PROVED_TRANSACTION_INTENT_DIGEST_DOMAIN_V1: &[u8] =
    b"iroha.ivm-proved.transaction-intent-digest.v1";

/// Digest of a proved-IVM transaction's canonical normalized unsigned payload.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    norito::NoritoSchema,
)]
#[norito_schema(name = "iroha_data_model::transaction::signed::IvmProvedTransactionIntentDigestV1")]
pub struct IvmProvedTransactionIntentDigestV1([u8; 32]);

impl IvmProvedTransactionIntentDigestV1 {
    #[cfg(test)]
    pub(crate) const fn from_bytes_for_test(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Return the fixed-width digest bytes.
    #[must_use]
    pub const fn into_bytes(self) -> [u8; 32] {
        self.0
    }
}

/// Failure to project a proved-IVM transaction onto its canonical signed intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum IvmProvedTransactionIntentErrorV1 {
    /// The transaction does not carry a proved IVM executable.
    #[error("proved-IVM intent requires an IvmProved executable")]
    WrongExecutable,
    /// The current proved-IVM proof carrier is missing.
    #[error("proved-IVM intent requires a proof attachment")]
    MissingProofAttachment,
    /// The normalized attachment list could not be represented canonically.
    #[error("proved-IVM intent attachment list is invalid: {0}")]
    InvalidProofAttachmentList(#[from] crate::proof::ProofAttachmentListError),
    /// Canonical Norito encoding of the normalized unsigned payload failed.
    #[error("canonical proved-IVM intent payload encoding failed")]
    PayloadEncodingFailure,
    /// The canonical payload length cannot be represented in the fixed digest frame.
    #[error("proved-IVM intent payload length overflow")]
    PayloadLengthOverflow,
}

fn normalized_proved_payload(
    payload: &TransactionPayload,
) -> Result<TransactionPayload, IvmProvedTransactionIntentErrorV1> {
    let mut normalized = payload.clone();
    let Executable::IvmProved(proved) = &mut normalized.instructions else {
        return Err(IvmProvedTransactionIntentErrorV1::WrongExecutable);
    };

    // Overlay instructions and event/gas commitments are execution outputs.
    // Keep the bytecode and all other signed transaction fields verbatim.
    proved.overlay = ConstVec::new_empty();
    proved.events_commitment = Hash::prehashed([0; Hash::LENGTH]);
    proved.gas_policy_commitment = Hash::prehashed([0; Hash::LENGTH]);

    // Current proved-IVM admission assigns attachment zero to this execution.
    // A later attachment, if present, is independent and stays byte-identical.
    let mut attachments = normalized
        .attachments
        .take()
        .ok_or(IvmProvedTransactionIntentErrorV1::MissingProofAttachment)?
        .into_vec();
    let proof_carrier = attachments
        .first_mut()
        .ok_or(IvmProvedTransactionIntentErrorV1::MissingProofAttachment)?;
    proof_carrier.proof.bytes.clear();
    // The envelope hash is defined from proof bytes. The backend, verifying-key
    // identity/commitment, lane witness, and following attachments are independent.
    proof_carrier.envelope_hash = None;
    normalized.attachments = Some(ProofAttachmentList::try_from(attachments)?);
    Ok(normalized)
}

impl TransactionPayload {
    /// Return the exact canonical unsigned-payload preimage for a proved-IVM intent.
    ///
    /// The complete payload remains in the preimage, including the exact network
    /// or genesis domain, authority, fees, time, nonce, metadata, bytecode,
    /// verifying-key identity, and all independent attachments. Only the first
    /// attachment's proof bytes and their envelope hash, plus the proved
    /// executable's overlay and two output commitments, are normalized.
    ///
    /// This helper does not authorize private execution. Admission must verify
    /// a complete execution relation before it applies effects.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-proved executable, missing proof attachment,
    /// invalid normalized attachment list, or canonical encoding failure.
    pub fn ivm_proved_intent_projection_bytes_v1(
        &self,
    ) -> Result<Vec<u8>, IvmProvedTransactionIntentErrorV1> {
        norito::encode_canonical(&normalized_proved_payload(self)?)
            .map_err(|_| IvmProvedTransactionIntentErrorV1::PayloadEncodingFailure)
    }

    /// Hash the canonical normalized unsigned payload with a typed V1 domain.
    ///
    /// Length framing prevents concatenation ambiguity. Proof bytes and
    /// execution outputs can change without feeding a hash of themselves back
    /// into the proof statement; all independent signed fields remain bound.
    ///
    /// # Errors
    ///
    /// Returns an error when canonical projection or fixed-width length
    /// conversion fails.
    pub fn ivm_proved_intent_digest_v1(
        &self,
    ) -> Result<IvmProvedTransactionIntentDigestV1, IvmProvedTransactionIntentErrorV1> {
        let encoded = self.ivm_proved_intent_projection_bytes_v1()?;
        let length = u64::try_from(encoded.len())
            .map_err(|_| IvmProvedTransactionIntentErrorV1::PayloadLengthOverflow)?;
        let mut hasher = blake3::Hasher::new();
        hasher.update(IVM_PROVED_TRANSACTION_INTENT_DIGEST_DOMAIN_V1);
        hasher.update(&length.to_le_bytes());
        hasher.update(&encoded);
        Ok(IvmProvedTransactionIntentDigestV1(
            *hasher.finalize().as_bytes(),
        ))
    }
}

impl SignedTransaction {
    /// Derive the proved-IVM intent from this transaction's signed payload.
    ///
    /// Authorization signatures are outside the intent preimage; the complete
    /// payload, apart from proof-derived fields, remains bound.
    ///
    /// # Errors
    ///
    /// Returns an error if the payload is not a canonically projectable
    /// proved-IVM transaction.
    pub fn ivm_proved_intent_digest_v1(
        &self,
    ) -> Result<IvmProvedTransactionIntentDigestV1, IvmProvedTransactionIntentErrorV1> {
        self.payload.ivm_proved_intent_digest_v1()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        Level,
        account::AccountId,
        isi::{InstructionBox, Log},
        proof::{ProofAttachment, ProofBox, VerifyingKeyId},
        transaction::{
            IvmBytecode, IvmProved, TransactionAdmissionIntent, TransactionBuilder,
            TransactionDomain, signed::FeePaymentIntent,
        },
    };
    use iroha_crypto::{PrivateKey, PublicKey};
    use iroha_model_base::{metadata::Metadata, name::Name};
    use iroha_primitives::json::Json;
    use std::{
        num::{NonZeroU32, NonZeroU64},
        str::FromStr,
    };

    fn proof_attachment(backend: &str, name: &str, proof_bytes: Vec<u8>) -> ProofAttachment {
        let mut attachment = ProofAttachment::new_ref(
            backend.into(),
            ProofBox::new(backend.into(), proof_bytes),
            VerifyingKeyId::new(backend, name),
        );
        attachment.envelope_hash = Some(Hash::new(&attachment.proof.bytes).into());
        attachment
    }

    fn fixture_payload() -> (TransactionPayload, PrivateKey) {
        let private_key: PrivateKey =
            "802620CCF31D85E3B32A4BEA59987CE0C78E3B8E2DB93881468AB2435FE45D5C9DCD53"
                .parse()
                .expect("fixed test key");
        let authority = AccountId::new(PublicKey::from(private_key.clone()));
        let mut first = proof_attachment("stark/fri", "ivm_v1", vec![1, 2, 3]);
        first.vk_commitment = Some([7; 32]);
        let attachments = ProofAttachmentList::try_from(vec![
            first,
            proof_attachment("halo2/ipa", "independent", vec![4, 5, 6]),
        ])
        .expect("bounded proof list");
        let executable = Executable::IvmProved(IvmProved {
            bytecode: IvmBytecode::from_compiled(vec![1, 2, 3, 4]),
            overlay: vec![InstructionBox::from(Log::new(Level::INFO, "effect".into()))].into(),
            events_commitment: Hash::new(b"events"),
            gas_policy_commitment: Hash::new(b"gas"),
        });
        let mut builder = TransactionBuilder::new(
            super::super::test_network_id(0x31),
            authority,
            FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(10_000)),
        )
        .with_executable(executable)
        .with_attachments(attachments);
        builder.set_nonce(NonZeroU32::new(3).expect("nonzero"));
        (builder.into_payload().expect("valid payload"), private_key)
    }

    fn assert_retained_mutation(
        payload: &TransactionPayload,
        mutate: impl FnOnce(&mut TransactionPayload),
    ) {
        let expected = payload
            .ivm_proved_intent_digest_v1()
            .expect("baseline intent");
        let mut changed = payload.clone();
        mutate(&mut changed);
        assert_ne!(
            changed
                .ivm_proved_intent_digest_v1()
                .expect("changed intent"),
            expected
        );
    }

    fn assert_derived_mutation(
        payload: &TransactionPayload,
        mutate: impl FnOnce(&mut TransactionPayload),
    ) {
        let expected = payload
            .ivm_proved_intent_projection_bytes_v1()
            .expect("baseline projection");
        let mut changed = payload.clone();
        mutate(&mut changed);
        assert_ne!(changed, *payload, "mutation must change signed payload");
        assert_eq!(
            changed
                .ivm_proved_intent_projection_bytes_v1()
                .expect("changed projection"),
            expected
        );
        assert_eq!(
            changed
                .ivm_proved_intent_digest_v1()
                .expect("changed intent"),
            payload
                .ivm_proved_intent_digest_v1()
                .expect("baseline intent")
        );
    }

    fn mutate_first_attachment(
        payload: &mut TransactionPayload,
        change: impl FnOnce(&mut ProofAttachment),
    ) {
        let mut attachments = payload
            .attachments
            .take()
            .expect("fixture attachments")
            .into_vec();
        change(&mut attachments[0]);
        payload.attachments = Some(ProofAttachmentList::try_from(attachments).expect("bounded"));
    }

    #[test]
    fn every_independent_signed_field_remains_in_the_intent() {
        let (payload, _) = fixture_payload();
        assert_retained_mutation(&payload, |changed| {
            changed.domain = TransactionDomain::Network(super::super::test_network_id(0x32));
        });
        assert_retained_mutation(&payload, |changed| {
            changed.domain = TransactionDomain::Genesis;
        });
        assert_retained_mutation(&payload, |changed| {
            let other: PrivateKey =
                "802620AF3F96DEEF44348FEB516C057558972CEC4C75C4DB9C5B3AAC843668854BF828"
                    .parse()
                    .expect("fixed other test key");
            changed.authority = AccountId::new(PublicKey::from(other));
        });
        assert_retained_mutation(&payload, |changed| changed.creation_time_ms += 1);
        assert_retained_mutation(&payload, |changed| {
            changed.time_to_live_ms = NonZeroU64::new(1);
        });
        assert_retained_mutation(&payload, |changed| {
            changed.nonce = NonZeroU32::new(4);
        });
        assert_retained_mutation(&payload, |changed| {
            changed.fee_payment = FeePaymentIntent::authority(Vec::new(), NonZeroU64::new(10_001));
        });
        assert_retained_mutation(&payload, |changed| {
            changed.admission_intent = TransactionAdmissionIntent::QueuePlanSynced;
        });
        assert_retained_mutation(&payload, |changed| {
            let mut metadata = Metadata::default();
            metadata.insert(
                Name::from_str("intent_context").expect("valid key"),
                Json::from(1_u64),
            );
            changed.metadata = metadata;
        });
        assert_retained_mutation(&payload, |changed| {
            let Executable::IvmProved(proved) = &mut changed.instructions else {
                panic!("fixture is proved IVM")
            };
            proved.bytecode = IvmBytecode::from_compiled(vec![1, 2, 3, 5]);
        });
        assert_retained_mutation(&payload, |changed| {
            mutate_first_attachment(changed, |attachment| {
                attachment.vk_ref = VerifyingKeyId::new("stark/fri", "other_vk");
            });
        });
        assert_retained_mutation(&payload, |changed| {
            mutate_first_attachment(changed, |attachment| {
                attachment.backend = "halo2/ipa".into();
                attachment.proof.backend = "halo2/ipa".into();
                attachment.vk_ref = VerifyingKeyId::new("halo2/ipa", "ivm_v1");
            });
        });
        assert_retained_mutation(&payload, |changed| {
            mutate_first_attachment(changed, |attachment| {
                attachment.vk_commitment = Some([8; 32]);
            });
        });
        assert_retained_mutation(&payload, |changed| {
            let mut attachments = changed
                .attachments
                .take()
                .expect("fixture attachments")
                .into_vec();
            attachments[1].proof.bytes = vec![4, 5, 7];
            attachments[1].envelope_hash = Some(Hash::new(&attachments[1].proof.bytes).into());
            changed.attachments =
                Some(ProofAttachmentList::try_from(attachments).expect("bounded"));
        });
    }

    #[test]
    fn only_proof_derived_fields_are_normalized() {
        let (payload, private_key) = fixture_payload();
        assert_derived_mutation(&payload, |changed| {
            let Executable::IvmProved(proved) = &mut changed.instructions else {
                panic!("fixture is proved IVM")
            };
            proved.overlay = Vec::<InstructionBox>::new().into();
        });
        assert_derived_mutation(&payload, |changed| {
            let Executable::IvmProved(proved) = &mut changed.instructions else {
                panic!("fixture is proved IVM")
            };
            proved.events_commitment = Hash::new(b"other events");
        });
        assert_derived_mutation(&payload, |changed| {
            let Executable::IvmProved(proved) = &mut changed.instructions else {
                panic!("fixture is proved IVM")
            };
            proved.gas_policy_commitment = Hash::new(b"other gas");
        });
        assert_derived_mutation(&payload, |changed| {
            mutate_first_attachment(changed, |attachment| {
                attachment.proof.bytes = vec![9, 8, 7];
                attachment.envelope_hash = Some(Hash::new(&attachment.proof.bytes).into());
            });
        });
        assert_derived_mutation(&payload, |changed| {
            mutate_first_attachment(changed, |attachment| {
                attachment.envelope_hash = None;
            });
        });

        let signed = TransactionBuilder::from_payload(payload.clone())
            .expect("ordinary payload")
            .sign(&private_key);
        assert_eq!(
            signed.ivm_proved_intent_digest_v1().expect("signed intent"),
            payload
                .ivm_proved_intent_digest_v1()
                .expect("payload intent")
        );
        let mut tampered = signed.clone();
        mutate_first_attachment(&mut tampered.payload, |attachment| {
            attachment.proof.bytes = vec![9, 8, 7];
            attachment.envelope_hash = Some(Hash::new(&attachment.proof.bytes).into());
        });
        assert_ne!(tampered.hash(), signed.hash());
        tampered
            .verify_signature()
            .expect_err("normalizing an intent does not weaken transaction signatures");
    }

    #[test]
    fn projection_rejects_missing_carrier_or_wrong_executable() {
        let (mut payload, _) = fixture_payload();
        payload.attachments = None;
        assert!(matches!(
            payload.ivm_proved_intent_digest_v1(),
            Err(IvmProvedTransactionIntentErrorV1::MissingProofAttachment)
        ));
        payload.instructions = Executable::Ivm(IvmBytecode::from_compiled(vec![1, 2, 3]));
        assert!(matches!(
            payload.ivm_proved_intent_digest_v1(),
            Err(IvmProvedTransactionIntentErrorV1::WrongExecutable)
        ));
    }

    #[test]
    fn projection_ignores_ambient_norito_encode_flags() {
        let (payload, _) = fixture_payload();
        let expected = payload
            .ivm_proved_intent_projection_bytes_v1()
            .expect("canonical projection");
        let alternate =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let _ambient = norito::core::DecodeFlagsGuard::enter(alternate);
        assert_eq!(
            payload
                .ivm_proved_intent_projection_bytes_v1()
                .expect("canonical projection under alternate ambient flags"),
            expected
        );
    }
}
