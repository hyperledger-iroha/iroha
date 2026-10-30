//! Proof-free X.509 requests bound to one original signed action occurrence.

use super::*;
use crate::privacy::{
    IrohaZkX509StarkP256StatementV1, PrivacyCompiledProfileSnapshotV1, PrivacyProofBytesV1,
    PrivacyProofEnvelopeValidationError, PrivacyProtocolLimitsV1, PrivacyStatementDigestV1,
    PrivacyStatementV1, PrivacyStatementValidationError, PrivacyTransactionIntentDigestV1,
    validate_privacy_profile_identity_v1,
};
use crate::transaction::signed_attachments::{
    ActionIntentV1, ActionOperationV1, ActionPublicInputsDigestV1, ActionRoleV1, AttachmentKindV1,
    AttachmentRoleV1, OrderedIntentDigestV1, SignedAttachmentErrorV1, domain_separated_digest,
};

/// Domain of the complete proof-free X.509 public-input statement.
pub const ZK_X509_REQUEST_PUBLIC_INPUTS_DOMAIN_V1: &[u8] =
    b"iroha.privacy.zk-x509-request-public-inputs.v1";

isi! {
    /// Verify one original signed X.509 attachment and atomically consume its nullifier.
    ///
    /// The request contains no proof bytes. Its derived intent field must be ZERO.
    #[derive(DeriveJsonSerialize, DeriveJsonDeserialize)]
    #[norito(deny_unknown_fields)]
    #[norito_schema(name = "iroha_data_model::isi::privacy::SubmitPrivacyZkX509CertificateV1")]
    pub struct SubmitPrivacyZkX509CertificateV1 {
        /// Complete independent public inputs; the intent digest is unmaterialized.
        pub statement: IrohaZkX509StarkP256StatementV1,
        /// Exactly one Proof/ActionProof occurrence of the original signed action.
        pub proof_ref: ActionAttachmentRefV1,
    }
}
impl crate::seal::Instruction for SubmitPrivacyZkX509CertificateV1 {}

/// Structural or binding refusal; success does not establish authorization or proof validity.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PrivacyZkX509RequestErrorV1 {
    /// Independent or materialized statement validation failed.
    #[error("invalid X509 request statement: {0}")]
    Statement(#[from] PrivacyStatementValidationError),
    /// The original signed action did not bind the exact request.
    #[error("invalid X509 request action binding: {0}")]
    Attachment(#[from] SignedAttachmentErrorV1),
    /// Governed activation, proof shape or charged byte bounds failed.
    #[error("invalid activated X509 request: {0}")]
    Envelope(#[from] PrivacyProofEnvelopeValidationError),
    /// The sole action-proof slot must be slot zero.
    #[error("X509 request requires exactly one Proof/ActionProof slot at zero")]
    AttachmentShape,
    /// The ordered intent must be computed from the original payload.
    #[error("X509 materialization requires a nonzero original ordered intent")]
    ZeroOrderedIntent,
    /// The supplied statement changed independent inputs or omitted its derived binding.
    #[error("materialized X509 statement differs from its proof-free request")]
    MaterializedStatementMismatch,
}

impl PrivacyZkX509RequestErrorV1 {
    /// Preserve the original enclosing resource refusal from one observed encoding.
    pub(super) fn from_encoding_attempt(original: norito::core::DecodeAttemptError) -> Self {
        Self::Attachment(SignedAttachmentErrorV1::from_encoding_attempt(original))
    }
}

impl SubmitPrivacyZkX509CertificateV1 {
    /// Sole first-release wire identifier of an executing X.509 credential request.
    pub const WIRE_ID: &'static str = "iroha.privacy.submit_zk_x509_certificate.v1";

    /// Canonical request bound derived from the existing tagged-statement ceiling.
    ///
    /// The request replaces the statement enum's u32 discriminant with the canonical
    /// action/slot reference field. Both retain the same length-prefixed statement
    /// payload; the maintained instruction framing owner supplies type-specific padding.
    #[must_use]
    pub fn canonical_frame_byte_limit_v1() -> usize {
        let flags = norito::core::default_encode_flags();
        let field = |bytes: usize| norito::core::len_prefix_len_with_flags(bytes, flags) + bytes;
        let reference_payload = 2 * field(core::mem::size_of::<u32>());
        let statement_framing =
            crate::isi::framed_instruction_payload_len_for::<PrivacyStatementV1>(0)
                .expect("fixed statement framing fits usize");
        let payload = crate::privacy::TAIRA_PRIVACY_MAX_STATEMENT_AND_ENCRYPTED_OUTPUT_BYTES_PER_TRANSACTION_V1 as usize
            - statement_framing - core::mem::size_of::<u32>() + field(reference_payload);
        crate::isi::framed_instruction_payload_len_for::<Self>(payload)
            .expect("fixed request ceiling fits usize")
    }

    /// Canonical InstructionBox bound from the registered wire tuple and request-frame ceiling.
    #[must_use]
    pub fn canonical_instruction_box_frame_byte_limit_v1() -> usize {
        let _canonical =
            norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags());
        let tuple = crate::isi::encoded_instruction_tuple_len(
            Self::WIRE_ID,
            Self::canonical_frame_byte_limit_v1(),
        )
        .expect("fixed registered request tuple fits usize");
        crate::isi::framed_instruction_payload_len_for::<crate::isi::InstructionBox>(tuple)
            .expect("fixed instruction box ceiling fits usize")
    }

    /// Construct independent request inputs for subsequent validation and materialization.
    #[must_use]
    pub fn new(
        statement: IrohaZkX509StarkP256StatementV1,
        proof_ref: ActionAttachmentRefV1,
    ) -> Self {
        Self {
            statement,
            proof_ref,
        }
    }

    /// Validate independent fields, canonical statement bounds and the reserved derived field.
    ///
    /// # Errors
    /// Rejects a supplied nonzero intent, malformed statement, invalid limits or nonzero slot.
    pub fn validate_proof_free_v1(
        &self,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(), PrivacyZkX509RequestErrorV1> {
        self.statement.validate_proof_free_v1(limits)?;
        if self.proof_ref.slot_index != 0 {
            return Err(PrivacyZkX509RequestErrorV1::AttachmentShape);
        }
        Ok(())
    }

    /// Hash the entire independent statement in its named domain, with no proof or derived intent.
    ///
    /// Intrinsic first-release bounds apply here; execution additionally applies committed limits.
    /// The exact request reference is bound by the normalized executable and action descriptor.
    ///
    /// # Errors
    /// Returns a structural or canonical encoding refusal.
    pub fn public_inputs_digest_v1(
        &self,
    ) -> Result<ActionPublicInputsDigestV1, PrivacyZkX509RequestErrorV1> {
        self.validate_proof_free_v1(&PrivacyConsensusLimitsV1::taira_default())?;
        let encoded =
            norito::core::classify_decode_attempt(|| norito::encode_canonical(&self.statement))
                .map_err(PrivacyZkX509RequestErrorV1::from_encoding_attempt)?;
        Ok(ActionPublicInputsDigestV1::new(domain_separated_digest(
            ZK_X509_REQUEST_PUBLIC_INPUTS_DOMAIN_V1,
            &encoded,
        )?))
    }

    /// Check the original action's operation, role, reference, proof shape and independent inputs.
    ///
    /// The runtime must separately authenticate this action's source, authority and invocation.
    ///
    /// # Errors
    /// Rejects any substituted action or malformed independent request.
    pub fn validate_action_v1(
        &self,
        action: &ActionIntentV1,
    ) -> Result<(), PrivacyZkX509RequestErrorV1> {
        if action.action_index != self.proof_ref.action_index
            || action.operation != ActionOperationV1::NativeInstruction(Self::WIRE_ID.to_owned())
            || action.role != ActionRoleV1::ProofEffect
        {
            return Err(SignedAttachmentErrorV1::InvocationTargetMismatch {
                action_index: self.proof_ref.action_index,
            }
            .into());
        }
        if self.proof_ref.slot_index != 0
            || action.attachments.len() != 1
            || action.attachments[0].kind != AttachmentKindV1::Proof
            || action.attachments[0].role != AttachmentRoleV1::ActionProof
        {
            return Err(PrivacyZkX509RequestErrorV1::AttachmentShape);
        }
        if action.public_inputs_digest != self.public_inputs_digest_v1()? {
            return Err(SignedAttachmentErrorV1::PublicInputsMismatch {
                action_index: action.action_index,
            }
            .into());
        }
        Ok(())
    }

    /// Derive the native statement from the original signed ActionStatement domain.
    ///
    /// The action digest binds the normalized ordered intent, attachment action ordinal,
    /// operation, role, actual declared authority/invocation and independent inputs. It
    /// is never accepted as a guest-supplied field. `context.action_index` remains the
    /// privacy ordinal and is not replaced by the attachment action ordinal.
    /// This computation grants no reusable runtime authorization capability.
    ///
    /// # Errors
    /// Rejects mismatched original action inputs or invalid materialized statement bounds.
    pub fn materialize_statement_v1(
        &self,
        action: &ActionIntentV1,
        intent: OrderedIntentDigestV1,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<IrohaZkX509StarkP256StatementV1, PrivacyZkX509RequestErrorV1> {
        self.validate_proof_free_v1(limits)?;
        self.validate_action_v1(action)?;
        if intent.is_zero() {
            return Err(PrivacyZkX509RequestErrorV1::ZeroOrderedIntent);
        }
        let mut statement = self.statement.clone();
        statement.context.transaction_intent_digest = PrivacyTransactionIntentDigestV1::new(
            *action.statement_digest_for_v1(intent)?.as_bytes(),
        );
        PrivacyStatementV1::IrohaZkX509StarkP256V1(statement.clone()).validate(limits)?;
        Ok(statement)
    }

    /// Validate activated materialized inputs and original borrowed proof bytes.
    ///
    /// Returns the canonical statement digest and exact request-frame plus proof-byte charge.
    /// No synthetic envelope or proof allocation is constructed. The runtime still owns
    /// canonical native profile selection, the original ActionStatement derivation, trusted State,
    /// authority, current privacy ordinal, native verification and atomic effect application.
    ///
    /// # Errors
    /// Rejects profile/limit mismatch, changed independent fields, malformed proof
    /// shape, canonical encoding failure or checked charge overflow.
    pub fn validate_materialized_against_profile_v1(
        &self,
        materialized: &IrohaZkX509StarkP256StatementV1,
        raw_proof: &[u8],
        profile: &PrivacyCompiledProfileSnapshotV1,
        protocol_limits: &PrivacyProtocolLimitsV1,
        limits: &PrivacyConsensusLimitsV1,
    ) -> Result<(PrivacyStatementDigestV1, u64), PrivacyZkX509RequestErrorV1> {
        let protocol = PrivacyProtocolIdV1::IrohaZkX509StarkP256V1;
        validate_privacy_profile_identity_v1(
            profile,
            protocol,
            protocol.expected_proof_system(),
            protocol.expected_engine(),
            &materialized.context,
        )?;
        self.validate_proof_free_v1(limits)?;
        let mut independent = materialized.clone();
        independent.context.transaction_intent_digest =
            PrivacyTransactionIntentDigestV1::new([0; 32]);
        if materialized.context.transaction_intent_digest.is_zero() || independent != self.statement
        {
            return Err(PrivacyZkX509RequestErrorV1::MaterializedStatementMismatch);
        }
        let statement = PrivacyStatementV1::IrohaZkX509StarkP256V1(materialized.clone());
        protocol_limits
            .validate_with_ceiling(&profile.protocol_limits)
            .map_err(PrivacyProofEnvelopeValidationError::ProfileProtocolLimits)?;
        protocol_limits
            .validate_statement(&statement)
            .map_err(PrivacyProofEnvelopeValidationError::ProfileStatementLimits)?;
        statement.validate(limits)?;
        PrivacyProofBytesV1::validate_bytes_v1(raw_proof, limits)
            .map_err(PrivacyProofEnvelopeValidationError::Proof)?;
        let digest = norito::core::classify_decode_attempt(|| statement.digest())
            .map_err(PrivacyZkX509RequestErrorV1::from_encoding_attempt)?;
        let request_bytes =
            norito::core::classify_decode_attempt(|| norito::canonical_frame_len(self))
                .map_err(PrivacyZkX509RequestErrorV1::from_encoding_attempt)?;
        let bytes = u64::try_from(request_bytes)
            .ok()
            .and_then(|request| u64::try_from(raw_proof.len()).ok()?.checked_add(request))
            .ok_or(PrivacyProofEnvelopeValidationError::EncodedLengthOverflow)?;
        if bytes > u64::from(limits.max_action_bytes) {
            return Err(PrivacyProofEnvelopeValidationError::ActionTooLarge {
                bytes,
                max: limits.max_action_bytes,
            }
            .into());
        }
        if bytes > u64::from(limits.max_privacy_bytes_per_transaction) {
            return Err(
                PrivacyProofEnvelopeValidationError::TransactionPrivacyPayloadTooLarge {
                    bytes,
                    max: limits.max_privacy_bytes_per_transaction,
                }
                .into(),
            );
        }
        Ok((digest, bytes))
    }
}
