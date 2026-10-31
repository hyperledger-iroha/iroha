//! Sole X.509 State selection, native verification and atomic nullifier effect owner.

use super::*;
use crate::{
    executor::attachment_custody::x509::X509Binding,
    privacy_state::PrivacyZkX509AuthoritativeStateV1,
    privacy_verifier::{VerifiedZkX509CertificateEffectV1, verify_privacy_zk_x509_attached_v1},
};
use iroha_data_model::{
    isi::privacy::SubmitPrivacyZkX509CertificateV1,
    privacy::{IrohaZkX509StarkP256StatementV1, PrivacyStatementDigestV1},
};

/// Execute an already-authorized request using its original signed attachment binding.
/// No proof/cached-success token or generic direct-submission binding is accepted.
pub(crate) fn execute_x509_submission(
    request: &SubmitPrivacyZkX509CertificateV1,
    binding: &X509Binding,
    proof: &[u8],
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    binding.require_original_execution(request, proof, authority, state_transaction)?;
    let admitted = resolve_admitted_privacy_protocol_v1(
        &state_transaction.world.privacy_protocol_policies,
        PrivacyProtocolIdV1::IrohaZkX509StarkP256V1,
        state_transaction.block_height(),
    )
    .map_err(|error| invalid_privacy_parameter(error.to_string()))?;
    execute_x509_after_admission(
        request,
        binding,
        proof,
        authority,
        state_transaction,
        *admitted.profile(),
        *admitted.protocol_limits(),
    )
}

// The sole post-admission implementation, matching the existing generic submission
// split. The public handler resolves the selected executable construction and current
// committed protocol policy. Component tests cannot establish that public admission boundary.
pub(super) fn execute_x509_after_admission(
    request: &SubmitPrivacyZkX509CertificateV1,
    binding: &X509Binding,
    proof: &[u8],
    authority: &AccountId,
    state_transaction: &mut StateTransaction<'_, '_>,
    profile: PrivacyCompiledProfileSnapshotV1,
    protocol_limits: PrivacyProtocolLimitsV1,
) -> Result<(), Error> {
    binding.require_original_execution(request, proof, authority, state_transaction)?;
    let statement = binding.statement();
    let expected_action_index = state_transaction.next_privacy_action_index();
    let (statement_digest, encoded_action_bytes) = request
        .validate_materialized_against_profile_v1(
            statement,
            proof,
            &profile,
            &protocol_limits,
            &state_transaction
                .world
                .privacy_consensus_policy
                .get()
                .current_limits,
        )
        .map_err(|source| {
            state_transaction.attempt_error_to_instruction_error(privacy_verification_error(
                PrivacyVerificationErrorV1::AttachedZkX509(Box::new(source)),
            ))
        })?;
    state_transaction.preflight_privacy_action(expected_action_index, encoded_action_bytes)?;
    let genesis_hash = state_transaction
        .block_hashes()
        .first()
        .map(|hash| *hash.as_ref())
        .ok_or_else(|| {
            Error::InvariantViolation(
                "X.509 proof admission requires a committed genesis block".into(),
            )
        })?;
    let (snapshot, selected_key, consumed) =
        select_current_state(statement, authority, state_transaction)?;
    let effects = verify_privacy_zk_x509_attached_v1(
        request,
        statement,
        proof,
        &PrivacyVerificationContextV1 {
            profile: &profile,
            protocol_limits: &protocol_limits,
            consensus_limits: &state_transaction
                .world
                .privacy_consensus_policy
                .get()
                .current_limits,
            network_id: &state_transaction.network_id,
            genesis_hash,
            current_height: state_transaction.block_height(),
            expected_action_index,
            block_timestamp_ms: state_transaction.block_unix_timestamp_ms(),
            pgc_state: None,
            orchard_state: None,
            proof_managed_state: None,
            zk_x509_state: Some(PrivacyZkX509VerificationStateV1 {
                authoritative_state: &snapshot,
                certificate_nullifier_consumed: consumed,
            }),
            bootle_lantern_policy: None,
            vega_issuer_record: None,
        },
    )
    .map_err(|error| {
        state_transaction.attempt_error_to_instruction_error(privacy_verification_error(error))
    })?;
    if effects.role() != PrivacyActionRoleV1::ZkX509CertificatePresentation
        || effects.protocol_id() != PrivacyProtocolIdV1::IrohaZkX509StarkP256V1
        || effects.statement_digest() != statement_digest
        || effects.action_index() != expected_action_index
        || effects.encoded_action_bytes() != encoded_action_bytes
    {
        return Err(Error::InvariantViolation(
            "native X.509 verifier returned effects inconsistent with its request".into(),
        ));
    }
    let VerifiedPrivacyLedgerEffectsV1::ZkX509Certificate(effect) = effects.into_ledger() else {
        return Err(Error::InvariantViolation(
            "native X.509 verifier returned another effect".into(),
        ));
    };
    apply_verified_effect(
        statement,
        &snapshot,
        selected_key,
        effect,
        statement_digest,
        expected_action_index,
        encoded_action_bytes,
        state_transaction,
    )
}

fn select_current_state(
    statement: &IrohaZkX509StarkP256StatementV1,
    authority: &AccountId,
    state_transaction: &StateTransaction<'_, '_>,
) -> Result<
    (
        PrivacyZkX509AuthoritativeStateV1,
        PrivacyNullifierKeyV1,
        bool,
    ),
    Error,
> {
    if authority != &statement.wallet_account {
        return Err(invalid_privacy_parameter(
            "X.509 certificate proof must be submitted by its bound wallet account",
        ));
    }
    if state_transaction
        .world
        .accounts
        .get(&statement.wallet_account)
        .is_none()
    {
        return Err(invalid_privacy_parameter(
            "X.509 statement wallet account does not exist",
        ));
    }
    let snapshot = load_privacy_zk_x509_authoritative_state_v1(
        statement.trust_anchor_id,
        statement.certificate_policy_id,
        state_transaction
            .world
            .privacy_consensus_policy
            .get()
            .current_limits
            .retained_root_count,
        &state_transaction.world.privacy_commitments,
        &state_transaction.world.privacy_roots,
        &state_transaction.world.privacy_root_heads,
    )
    .map_err(|error| {
        Error::InvariantViolation(
            format!("trusted X.509 authoritative state failed validation: {error}").into(),
        )
    })?;
    let nullifier_key = PrivacyNullifierKeyV1::zk_x509_certificate_nullifier(
        snapshot.namespace(),
        statement.certificate_nullifier,
    )
    .map_err(invalid_privacy_parameter)?;
    let consumed = if let Some(record) = state_transaction
        .world
        .privacy_nullifiers
        .get(&nullifier_key)
    {
        record.validate().map_err(|error| {
            Error::InvariantViolation(
                format!("persisted X.509 certificate-nullifier provenance is invalid: {error}")
                    .into(),
            )
        })?;
        if !matches!(
            record,
            PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier { .. }
        ) {
            return Err(Error::InvariantViolation(
                "persisted X.509 certificate nullifier has wrong-role provenance".into(),
            ));
        }
        true
    } else {
        false
    };
    Ok((snapshot, nullifier_key, consumed))
}

#[allow(clippy::too_many_arguments)]
fn apply_verified_effect(
    statement: &IrohaZkX509StarkP256StatementV1,
    snapshot: &PrivacyZkX509AuthoritativeStateV1,
    selected_key: PrivacyNullifierKeyV1,
    effect: VerifiedZkX509CertificateEffectV1,
    statement_digest: PrivacyStatementDigestV1,
    expected_action_index: u32,
    encoded_action_bytes: u64,
    state_transaction: &mut StateTransaction<'_, '_>,
) -> Result<(), Error> {
    let trust_anchor = snapshot.trust_anchor();
    let certificate_policy = snapshot.certificate_policy();
    let crl = snapshot.crl_record();
    if effect.namespace != snapshot.namespace()
        || effect.certificate_nullifier != statement.certificate_nullifier
        || effect.trust_anchor_record_digest != trust_anchor.record_digest
        || effect.trust_anchor_record_epoch != trust_anchor.record_epoch
        || effect.certificate_policy_record_digest != certificate_policy.record_digest
        || effect.certificate_policy_record_epoch != certificate_policy.record_epoch
        || effect.crl_record_digest != crl.record_digest
        || effect.crl_record_epoch != crl.record_epoch
    {
        return Err(Error::InvariantViolation(
            "native X.509 effect is inconsistent with trusted state or its statement".into(),
        ));
    }
    let nullifier_key = PrivacyNullifierKeyV1::zk_x509_certificate_nullifier(
        effect.namespace,
        effect.certificate_nullifier,
    )
    .map_err(|error| {
        Error::InvariantViolation(
            format!("verified X.509 certificate nullifier is invalid: {error}").into(),
        )
    })?;
    if selected_key != nullifier_key {
        return Err(Error::InvariantViolation(
            "native X.509 effect selected a different replay key".into(),
        ));
    }
    if let Some(record) = state_transaction
        .world
        .privacy_nullifiers
        .get(&nullifier_key)
    {
        record.validate().map_err(|error| {
            Error::InvariantViolation(
                format!("persisted X.509 certificate-nullifier provenance is invalid: {error}")
                    .into(),
            )
        })?;
        if !matches!(
            record,
            PrivacyStateItemRecordV1::ZkX509VerifiedCertificateNullifier { .. }
        ) {
            return Err(Error::InvariantViolation(
                "persisted X.509 certificate nullifier has wrong-role provenance".into(),
            ));
        }
        return Err(invalid_privacy_parameter(
            "X.509 certificate nullifier was already consumed",
        ));
    }
    let provenance = PrivacyStateItemRecordV1::zk_x509_verified_certificate_nullifier(
        effect.trust_anchor_record_digest,
        effect.trust_anchor_record_epoch,
        effect.certificate_policy_record_digest,
        effect.certificate_policy_record_epoch,
        effect.crl_record_digest,
        effect.crl_record_epoch,
        statement_digest,
        state_transaction.block_height(),
        expected_action_index,
    )
    .map_err(invalid_privacy_parameter)?;
    state_transaction.reserve_privacy_action(expected_action_index, encoded_action_bytes)?;
    state_transaction
        .world
        .privacy_nullifiers
        .insert(nullifier_key, provenance);
    Ok(())
}

impl Execute for SubmitPrivacyZkX509CertificateV1 {
    fn execute(
        self,
        _authority: &AccountId,
        _state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        Err(invalid_privacy_parameter(
            "X.509 request requires its original signed attachment owner",
        ))
    }
}
