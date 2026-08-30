//! Native authentication and projection for the closed Exact12 mobile action flow.
//!
//! Managed callers pass an already-signed, canonical versioned transaction, the exact
//! genesis-derived `NetworkId`, the exact canonical-request authority, and one closed operation
//! discriminant.  This boundary verifies the transaction signature and authority binding,
//! validates the proof envelope against Taira's consensus limits, and rejects protocol/operation
//! substitution.  It deliberately does not verify a proof locally: only committed Torii execution
//! establishes acceptance.

use iroha_core::privacy_engines::validate_zk_x509_credential_proof_container_v1;
use iroha_crypto::Hash;
use iroha_data_model::{
    privacy::{
        IrohaZkAmsProofV1, PrivacyConsensusLimitsV1, PrivacyOperationSchemaV1, PrivacyProofV1,
        PrivacyStatementV1, PrivacyZkAmsActionV1,
    },
    transaction::SignedTransaction,
};
use iroha_version::codec::{DecodeVersioned as _, EncodeVersioned as _};

use super::{authenticated_transaction_details::canonical_authority, network_id_from_raw_bytes};

pub(super) const PRIVACY_EXACT12_SIGNED_ACTION_MAX_BYTES_V1: usize = 10 * 1024 * 1024;
pub(super) const PRIVACY_EXACT12_SIGNED_ACTION_PROJECTION_BYTES_V1: usize = 4 * Hash::LENGTH;

/// Four authenticated public digests projected from one exact signed action.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PrivacyExact12SignedActionProjectionV1 {
    pub transaction_hash: [u8; Hash::LENGTH],
    pub transaction_intent_digest: [u8; Hash::LENGTH],
    pub statement_digest: [u8; Hash::LENGTH],
    pub proof_envelope_hash: [u8; Hash::LENGTH],
}

impl PrivacyExact12SignedActionProjectionV1 {
    /// Fixed ABI-22 projection used by Swift and both JVM namespaces.
    #[must_use]
    pub(super) fn to_fixed_bytes(self) -> [u8; PRIVACY_EXACT12_SIGNED_ACTION_PROJECTION_BYTES_V1] {
        let mut output = [0_u8; PRIVACY_EXACT12_SIGNED_ACTION_PROJECTION_BYTES_V1];
        output[0..32].copy_from_slice(&self.transaction_hash);
        output[32..64].copy_from_slice(&self.transaction_intent_digest);
        output[64..96].copy_from_slice(&self.statement_digest);
        output[96..128].copy_from_slice(&self.proof_envelope_hash);
        output
    }
}

pub(super) fn operation_from_index(index: i32) -> Option<PrivacyOperationSchemaV1> {
    Some(match index {
        0 => PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1,
        1 => PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1,
        2 => PrivacyOperationSchemaV1::VeRangeRangeProofV1,
        3 => PrivacyOperationSchemaV1::ZkAmsBatchAdmissionActionV1,
        4 => PrivacyOperationSchemaV1::ZkAmsProvisionAccountActionV1,
        5 => PrivacyOperationSchemaV1::VegaCredentialPresentationV1,
        6 => PrivacyOperationSchemaV1::ZkX509IdentityPresentationV1,
        7 => PrivacyOperationSchemaV1::JindoPolynomialEvaluationV1,
        8 => PrivacyOperationSchemaV1::BootleLanternCredentialPresentationV1,
        9 => PrivacyOperationSchemaV1::OrchardNoteActionV1,
        10 => PrivacyOperationSchemaV1::FcmpMembershipPaymentV1,
        11 => PrivacyOperationSchemaV1::IvmPrivateNoteActionV1,
        12 => PrivacyOperationSchemaV1::PqMaspNoteActionV1,
        _ => return None,
    })
}

fn exact_operation_shape(
    operation: PrivacyOperationSchemaV1,
    statement: &PrivacyStatementV1,
    proof: &PrivacyProofV1,
) -> bool {
    match operation {
        PrivacyOperationSchemaV1::ZkAceAuthorizationActionV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::ZkAcePqAuthorizationV0(_),
                PrivacyProofV1::ZkAcePqAuthorizationV0(_)
            )
        ),
        PrivacyOperationSchemaV1::AnonymousPgcPaymentActionV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::AnonymousPgcKOutOfNV1(_),
                PrivacyProofV1::AnonymousPgcKOutOfNV1(_)
            )
        ),
        PrivacyOperationSchemaV1::VeRangeRangeProofV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::VeRangeTransparentRangeV1(_),
                PrivacyProofV1::VeRangeTransparentRangeV1(_)
            )
        ),
        PrivacyOperationSchemaV1::ZkAmsBatchAdmissionActionV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::IrohaZkAmsV1(statement),
                PrivacyProofV1::IrohaZkAmsV1(
                    IrohaZkAmsProofV1::MaskedRelaxedSpartanBatchAdmission(_)
                )
            ) if matches!(&statement.action, PrivacyZkAmsActionV1::BatchAdmission(_))
        ),
        PrivacyOperationSchemaV1::ZkAmsProvisionAccountActionV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::IrohaZkAmsV1(statement),
                PrivacyProofV1::IrohaZkAmsV1(
                    IrohaZkAmsProofV1::Ristretto255LsagProvisionAccount(_)
                )
            ) if matches!(&statement.action, PrivacyZkAmsActionV1::ProvisionAccount(_))
        ),
        PrivacyOperationSchemaV1::VegaCredentialPresentationV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::VegaExistingCredentialZkV0(_),
                PrivacyProofV1::VegaExistingCredentialZkV0(_)
            )
        ),
        PrivacyOperationSchemaV1::ZkX509IdentityPresentationV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::IrohaZkX509StarkP256V0(_),
                PrivacyProofV1::IrohaZkX509StarkP256V0(_)
            )
        ),
        PrivacyOperationSchemaV1::JindoPolynomialEvaluationV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::IrohaJindoPolynomialCommitmentV0(_),
                PrivacyProofV1::IrohaJindoPolynomialCommitmentV0(_)
            )
        ),
        PrivacyOperationSchemaV1::BootleLanternCredentialPresentationV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::IrohaBootleLanternAnoncredV1(_),
                PrivacyProofV1::IrohaBootleLanternAnoncredV1(_)
            )
        ),
        PrivacyOperationSchemaV1::OrchardNoteActionV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::OrchardHalo2ActionsV1(_),
                PrivacyProofV1::OrchardHalo2ActionsV1(_)
            )
        ),
        PrivacyOperationSchemaV1::FcmpMembershipPaymentV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::MoneroFcmpPlusPlusV1(_),
                PrivacyProofV1::MoneroFcmpPlusPlusV1(_)
            )
        ),
        PrivacyOperationSchemaV1::IvmPrivateNoteActionV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::IrohaIvmPrivateNoteStarkV1(_),
                PrivacyProofV1::IrohaIvmPrivateNoteStarkV1(_)
            )
        ),
        PrivacyOperationSchemaV1::PqMaspNoteActionV1 => matches!(
            (statement, proof),
            (
                PrivacyStatementV1::PqMaspStarkV0(_),
                PrivacyProofV1::PqMaspStarkV0(_)
            )
        ),
    }
}

pub(super) fn inspect_signed_privacy_exact12_action_v1(
    signed_transaction_versioned: &[u8],
    network_id: &[u8],
    authority_literal: &str,
    operation_index: i32,
) -> Result<PrivacyExact12SignedActionProjectionV1, &'static str> {
    if signed_transaction_versioned.is_empty()
        || signed_transaction_versioned.len() > PRIVACY_EXACT12_SIGNED_ACTION_MAX_BYTES_V1
    {
        return Err("signed Exact12 transaction is outside its closed byte bound");
    }
    let operation = operation_from_index(operation_index)
        .ok_or("Exact12 operation discriminant is outside the closed union")?;
    let expected_network_id = network_id_from_raw_bytes(network_id)?;
    let expected_authority = canonical_authority(authority_literal)?;
    let signed = SignedTransaction::decode_all_versioned(signed_transaction_versioned)
        .map_err(|_| "signed Exact12 transaction is not current versioned Norito")?;
    if signed.encode_versioned() != signed_transaction_versioned {
        return Err("signed Exact12 transaction is not the exact canonical wire");
    }
    signed
        .verify_signature()
        .map_err(|_| "signed Exact12 transaction has an invalid authority signature")?;
    if signed.authority() != &expected_authority {
        return Err("signed Exact12 transaction authority differs from canonicalAuth account");
    }
    if signed.network_id() != Some(&expected_network_id) {
        return Err("signed Exact12 transaction belongs to another NetworkId");
    }
    let (transaction_intent_digest, submission) = signed
        .privacy_transaction_intent_binding_if_present_v1()
        .map_err(|_| "signed Exact12 transaction has an invalid privacy intent binding")?
        .ok_or("signed Exact12 transaction contains no direct privacy action")?;
    let envelope = &submission.envelope;
    if envelope.protocol_id != operation.protocol_id()
        || !exact_operation_shape(operation, &envelope.statement, &envelope.proof)
    {
        return Err("signed Exact12 transaction does not match the requested operation");
    }
    if operation == PrivacyOperationSchemaV1::ZkX509IdentityPresentationV1 {
        let PrivacyStatementV1::IrohaZkX509StarkP256V0(statement) = &envelope.statement else {
            return Err("signed ZK-X509 transaction has an invalid statement variant");
        };
        validate_zk_x509_credential_proof_container_v1(
            statement,
            *expected_network_id.as_bytes(),
            envelope.proof.bytes().as_bytes(),
        )
        .map_err(|_| "signed ZK-X509 transaction has an invalid credential proof container")?;
    }
    envelope
        .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
        .map_err(|_| "signed Exact12 transaction carries an invalid proof envelope")?;
    let context = envelope.statement.context();
    if context.network_id != expected_network_id
        || context.action_index != 0
        || context.transaction_intent_digest != transaction_intent_digest
    {
        return Err("signed Exact12 statement context differs from the transaction binding");
    }
    let envelope_encoding = norito::to_bytes(envelope)
        .map_err(|_| "signed Exact12 proof envelope could not be encoded")?;
    Ok(PrivacyExact12SignedActionProjectionV1 {
        transaction_hash: *signed.hash().as_ref(),
        transaction_intent_digest: *transaction_intent_digest.as_bytes(),
        statement_digest: *envelope.statement_digest.as_bytes(),
        proof_envelope_hash: *Hash::new(&envelope_encoding).as_ref(),
    })
}
