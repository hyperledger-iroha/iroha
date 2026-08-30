//! Native authentication and projection for the closed Exact12 JavaScript action flow.
//!
//! This boundary checks the already-signed canonical transaction, exact `NetworkId`, privacy
//! intent binding, operation shape, and consensus envelope limits. It intentionally does not
//! verify the proof: only a committed Torii result establishes ledger acceptance.

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

use super::{
    authenticated_transaction_details::canonical_authority, parse_transaction_network_id_bytes,
};

pub(crate) const SIGNED_ACTION_MAX_BYTES_V1: usize = 10 * 1024 * 1024;
pub(crate) const SIGNED_ACTION_PROJECTION_BYTES_V1: usize = 4 * Hash::LENGTH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SignedActionProjectionV1 {
    pub transaction_hash: [u8; Hash::LENGTH],
    pub transaction_intent_digest: [u8; Hash::LENGTH],
    pub statement_digest: [u8; Hash::LENGTH],
    pub proof_envelope_hash: [u8; Hash::LENGTH],
}

impl SignedActionProjectionV1 {
    #[must_use]
    pub(crate) fn to_fixed_bytes(self) -> [u8; SIGNED_ACTION_PROJECTION_BYTES_V1] {
        let mut output = [0_u8; SIGNED_ACTION_PROJECTION_BYTES_V1];
        output[0..32].copy_from_slice(&self.transaction_hash);
        output[32..64].copy_from_slice(&self.transaction_intent_digest);
        output[64..96].copy_from_slice(&self.statement_digest);
        output[96..128].copy_from_slice(&self.proof_envelope_hash);
        output
    }
}

/// Resolve the public closed-union discriminant used by the JS/N-API boundary.
pub(crate) fn operation_from_index(index: u32) -> Option<PrivacyOperationSchemaV1> {
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

pub(crate) fn inspect_signed_action_v1(
    signed_transaction_versioned: &[u8],
    network_id_bytes: &[u8],
    authority_literal: &str,
    operation_index: u32,
) -> Result<SignedActionProjectionV1, String> {
    if signed_transaction_versioned.is_empty()
        || signed_transaction_versioned.len() > SIGNED_ACTION_MAX_BYTES_V1
    {
        return Err("signed Exact12 transaction is outside its closed byte bound".to_owned());
    }
    let operation = operation_from_index(operation_index)
        .ok_or_else(|| "Exact12 operation discriminant is outside the closed union".to_owned())?;
    let expected_network_id = parse_transaction_network_id_bytes(network_id_bytes)
        .map_err(|error| error.reason.clone())?;
    let expected_authority = canonical_authority(authority_literal)?;
    let canonical_genesis_hash: [u8; Hash::LENGTH] = network_id_bytes
        .try_into()
        .map_err(|_| "NetworkId must contain exactly 32 bytes".to_owned())?;
    let signed = SignedTransaction::decode_all_versioned(signed_transaction_versioned)
        .map_err(|_| "signed Exact12 transaction is not current versioned Norito".to_owned())?;
    if signed.encode_versioned() != signed_transaction_versioned {
        return Err("signed Exact12 transaction is not the exact canonical wire".to_owned());
    }
    signed
        .verify_signature()
        .map_err(|_| "signed Exact12 transaction has an invalid authority signature".to_owned())?;
    if signed.authority() != &expected_authority {
        return Err(
            "signed Exact12 transaction authority differs from canonicalAuth account".to_owned(),
        );
    }
    if signed.network_id() != Some(&expected_network_id) {
        return Err("signed Exact12 transaction belongs to another NetworkId".to_owned());
    }
    let (transaction_intent_digest, submission) = signed
        .privacy_transaction_intent_binding_if_present_v1()
        .map_err(|_| "signed Exact12 transaction has an invalid privacy intent binding".to_owned())?
        .ok_or_else(|| "signed Exact12 transaction contains no direct privacy action".to_owned())?;
    let envelope = &submission.envelope;
    if envelope.protocol_id != operation.protocol_id()
        || envelope.statement.protocol_id() != operation.protocol_id()
        || envelope.proof.protocol_id() != operation.protocol_id()
        || !exact_operation_shape(operation, &envelope.statement, &envelope.proof)
    {
        return Err("signed Exact12 transaction does not match the requested operation".to_owned());
    }
    envelope
        .validate_with_limits(&PrivacyConsensusLimitsV1::taira_default())
        .map_err(|_| "signed Exact12 transaction carries an invalid proof envelope".to_owned())?;
    let context = envelope.statement.context();
    if context.network_id != expected_network_id
        || context.action_index != 0
        || context.transaction_intent_digest != transaction_intent_digest
    {
        return Err(
            "signed Exact12 statement context differs from the transaction binding".to_owned(),
        );
    }
    if let PrivacyStatementV1::IrohaZkX509StarkP256V0(statement) = &envelope.statement {
        validate_zk_x509_credential_proof_container_v1(
            statement,
            canonical_genesis_hash,
            envelope.proof.bytes().as_bytes(),
        )
        .map_err(|_| {
            "signed ZK-X509 action has an invalid credential-proof container".to_owned()
        })?;
    }
    let envelope_encoding = norito::to_bytes(envelope)
        .map_err(|_| "signed Exact12 proof envelope could not be encoded".to_owned())?;
    Ok(SignedActionProjectionV1 {
        transaction_hash: *signed.hash().as_ref(),
        transaction_intent_digest: *transaction_intent_digest.as_bytes(),
        statement_digest: *envelope.statement_digest.as_bytes(),
        proof_envelope_hash: *Hash::new(&envelope_encoding).as_ref(),
    })
}
