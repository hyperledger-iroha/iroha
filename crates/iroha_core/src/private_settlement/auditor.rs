//! Online local-auditor approval for atomic private settlement legs.
//!
//! This module is the only runtime path that opens a private business capsule.
//! It validates the exact public context, decrypts for one governed auditor,
//! decodes one canonical typed plaintext, evaluates local policy, and returns a
//! purpose-specific approval without returning or logging plaintext material.

use super::{
    audit::open_private_settlement_audit_capsule_v1,
    sidecar_store::{
        PrivateSettlementAuditorSidecarViewV1, PrivateSettlementSidecarLifecycleV1,
        verify_private_settlement_availability_certificate_v1,
    },
};
use crate::privacy_engines::atomic_private_settlement::validate_audit_openings_v1;
use iroha_crypto::{Algorithm, HybridSecretKey, KeyPair, SignatureOf};
use iroha_data_model::{
    account::AccountId,
    nexus::{
        ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, AtomicPrivateSettlementV1,
        PrivateSettlementAuditApprovalBodyV1, PrivateSettlementAuditApprovalV1,
        PrivateSettlementAuditPlaintextV1, PrivateSettlementAuditPolicyV1,
        PrivateSettlementPoolGovernanceV1,
    },
};
use thiserror::Error;

/// Private context presented to one configured local policy evaluator.
#[derive(Clone, Copy)]
pub struct PrivateSettlementAuditEvaluationV1<'a> {
    /// Exact decrypted business and note-opening material.
    pub plaintext: &'a PrivateSettlementAuditPlaintextV1,
    /// Exact public bundle manifest.
    pub manifest: &'a AtomicPrivateSettlementV1,
    /// Exact governed local auditor policy.
    pub audit_policy: &'a PrivateSettlementAuditPolicyV1,
    /// Authoritative height at which approval is requested.
    pub authoritative_height: u64,
}

impl core::fmt::Debug for PrivateSettlementAuditEvaluationV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("PrivateSettlementAuditEvaluationV1")
            .field("bundle_id", &self.manifest.bundle_id)
            .field("leg_ordinal", &self.plaintext.leg_ordinal)
            .field("authoritative_height", &self.authoritative_height)
            .finish_non_exhaustive()
    }
}

/// Dataspace-local business policy applied after authenticated decryption.
///
/// Implementations must not log, persist, or export the supplied plaintext.
/// Returning `false` produces one redacted policy-rejection result.
pub trait PrivateSettlementAuditPolicyEvaluatorV1: Send + Sync {
    /// Return whether this exact private leg is approved by local policy.
    fn approves(&self, context: PrivateSettlementAuditEvaluationV1<'_>) -> bool;
}

impl<F> PrivateSettlementAuditPolicyEvaluatorV1 for F
where
    F: for<'a> Fn(PrivateSettlementAuditEvaluationV1<'a>) -> bool + Send + Sync,
{
    fn approves(&self, context: PrivateSettlementAuditEvaluationV1<'_>) -> bool {
        self(context)
    }
}

/// Redacted failure while producing one online local-auditor approval.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum PrivateSettlementAuditorApprovalErrorV1 {
    /// Public context, committee, policy, capsule, or delta bindings are invalid.
    #[error("private-settlement auditor view is invalid")]
    InvalidView,
    /// The requested approval height is stale or outside the active policy interval.
    #[error("private-settlement auditor approval context is stale")]
    Stale,
    /// The auditor is absent from the exact governed policy.
    #[error("private-settlement auditor is not governed by this policy")]
    UnauthorizedAuditor,
    /// The supplied signing key is not the auditor's purpose-specific governed key.
    #[error("private-settlement auditor signing key does not match policy")]
    SigningKeyMismatch,
    /// Hybrid key validation or authenticated capsule decryption failed.
    #[error("private-settlement auditor capsule could not be authenticated")]
    CapsuleAuthenticationFailed,
    /// Decrypted bytes are not the sole canonical typed audit plaintext.
    #[error("private-settlement auditor plaintext is invalid")]
    InvalidPlaintext,
    /// Dataspace-local business policy rejected the exact decrypted leg.
    #[error("private-settlement auditor policy rejected the leg")]
    PolicyRejected,
    /// Purpose-specific approval signing failed.
    #[error("private-settlement auditor approval signing failed")]
    SigningFailed,
}

/// Decrypt, validate, policy-check, and sign one local-auditor approval.
///
/// The caller must supply the exact governed hybrid decryption secret and the
/// separate governed approval-signing key. The plaintext is borrowed by the
/// evaluator only for the duration of this call and is never returned.
///
/// # Errors
///
/// Returns a redacted failure for invalid/stale context, wrong keys,
/// authenticated-decryption failure, non-canonical plaintext, policy rejection,
/// or signature failure.
pub fn approve_private_settlement_leg_v1<E: PrivateSettlementAuditPolicyEvaluatorV1 + ?Sized>(
    view: &PrivateSettlementAuditorSidecarViewV1,
    pool_governance: &PrivateSettlementPoolGovernanceV1,
    authoritative_height: u64,
    auditor_id: &AccountId,
    decryption_secret: &HybridSecretKey,
    signing_key: &KeyPair,
    evaluator: &E,
) -> Result<PrivateSettlementAuditApprovalV1, PrivateSettlementAuditorApprovalErrorV1> {
    validate_auditor_view_v1(view, pool_governance, authoritative_height)?;
    let governed_auditor = view
        .policy
        .body
        .auditors
        .iter()
        .find(|auditor| &auditor.auditor_id == auditor_id)
        .ok_or(PrivateSettlementAuditorApprovalErrorV1::UnauthorizedAuditor)?;
    if signing_key.public_key() != &governed_auditor.signing_key {
        return Err(PrivateSettlementAuditorApprovalErrorV1::SigningKeyMismatch);
    }

    let canonical_plaintext = open_private_settlement_audit_capsule_v1(
        &view.audit_capsule,
        &view.policy,
        auditor_id,
        decryption_secret,
    )
    .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::CapsuleAuthenticationFailed)?;
    let plaintext = norito::decode_canonical::<PrivateSettlementAuditPlaintextV1>(
        canonical_plaintext.as_slice(),
    )
    .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidPlaintext)?;
    pool_governance
        .validate_asset_opening(
            plaintext.route,
            plaintext.pool_id,
            &plaintext.asset_definition_id,
            plaintext.asset_binding_salt,
        )
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidPlaintext)?;
    if plaintext
        .policy_references
        .binary_search(&pool_governance.governance_digest)
        .is_err()
    {
        return Err(PrivateSettlementAuditorApprovalErrorV1::InvalidPlaintext);
    }
    validate_audit_openings_v1(&view.manifest, &view.statement, &plaintext)
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidPlaintext)?;
    if !evaluator.approves(PrivateSettlementAuditEvaluationV1 {
        plaintext: &plaintext,
        manifest: &view.manifest,
        audit_policy: &view.policy,
        authoritative_height,
    }) {
        return Err(PrivateSettlementAuditorApprovalErrorV1::PolicyRejected);
    }

    let capsule_digest = view
        .audit_capsule
        .digest()
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    let delta_digest = view
        .delta
        .digest()
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    let body = PrivateSettlementAuditApprovalBodyV1 {
        version: ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1,
        network_id: view.statement.network_id,
        bundle_id: view.statement.bundle_id,
        leg_ordinal: view.statement.leg_ordinal,
        dataspace_id: view.statement.route.dataspace_id,
        auditor_id: auditor_id.clone(),
        audit_policy_digest: view.policy.policy_digest,
        audit_key_epoch: view.policy.body.key_epoch,
        proof_digest: view.delta.proof_digest,
        capsule_digest,
        delta_digest,
        old_root: view.delta.old_root,
        new_root: view.delta.new_root,
        expiry_height: view.statement.expiry_height,
    };
    let approval = PrivateSettlementAuditApprovalV1 {
        signature: SignatureOf::try_new(signing_key.private_key(), &body)
            .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::SigningFailed)?,
        body,
    };
    approval
        .verify(&view.policy, authoritative_height)
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::SigningFailed)?;
    Ok(approval)
}

fn validate_auditor_view_v1(
    view: &PrivateSettlementAuditorSidecarViewV1,
    pool_governance: &PrivateSettlementPoolGovernanceV1,
    authoritative_height: u64,
) -> Result<(), PrivateSettlementAuditorApprovalErrorV1> {
    view.manifest
        .validate()
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    view.policy
        .validate()
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    view.authority
        .validate()
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    view.statement
        .validate()
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    view.delta
        .validate_against(&view.statement)
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    view.audit_capsule
        .validate_against(&view.policy)
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    pool_governance
        .validate_against_policy_at(&view.policy, view.manifest.authority_context_height)
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    if view.lifecycle != PrivateSettlementSidecarLifecycleV1::Collecting
        || authoritative_height < view.manifest.authority_context_height
        || authoritative_height > view.manifest.expiry_height
        || !view.policy.is_active_at(authoritative_height)
    {
        return Err(PrivateSettlementAuditorApprovalErrorV1::Stale);
    }
    let leg = view
        .manifest
        .legs
        .get(usize::from(view.statement.leg_ordinal))
        .ok_or(PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    let capsule_digest = view
        .audit_capsule
        .digest()
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    let delta_digest = view
        .delta
        .digest()
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    if view.statement.network_id != view.manifest.network_id
        || view.statement.bundle_id != view.manifest.bundle_id
        || view.statement.route != leg.route
        || view.statement.authority_context_height != view.manifest.authority_context_height
        || view.statement.pool_id != leg.pool_id
        || view.statement.asset_binding_commitment != leg.asset_binding_commitment
        || view.statement.audit_policy_digest != view.policy.policy_digest
        || view.statement.audit_key_epoch != view.policy.body.key_epoch
        || view.statement.fee_intent_digest != view.manifest.fee_intent_digest
        || view.statement.reimbursement_terms_commitment
            != view.manifest.reimbursement_terms_commitment
        || view.statement.reimbursement_leg_ordinal != view.manifest.reimbursement_leg_ordinal
        || view.statement.expiry_height != view.manifest.expiry_height
        || view.policy.body.dataspace_id != view.statement.route.dataspace_id
        || pool_governance.body.route != view.statement.route
        || pool_governance.body.pool_id != view.statement.pool_id
        || pool_governance.body.asset_binding_commitment != view.statement.asset_binding_commitment
        || pool_governance
            .body
            .lifecycle
            .retirement_height
            .is_some_and(|retirement| view.manifest.expiry_height >= retirement)
        || view.authority.route != view.statement.route
        || capsule_digest != view.statement.audit_capsule_digest
        || delta_digest != leg.delta_digest
        || view.availability.body.version != ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1
        || view.availability.body.payload_digest != leg.payload_digest
        || view.availability.body.payload_bytes == 0
        || view.availability.body.retention_until_height < view.manifest.expiry_height
    {
        return Err(PrivateSettlementAuditorApprovalErrorV1::InvalidView);
    }
    verify_private_settlement_availability_certificate_v1(&view.availability, &view.authority)
        .map_err(|_| PrivateSettlementAuditorApprovalErrorV1::InvalidView)?;
    if view
        .authority
        .validators
        .iter()
        .zip(&view.authority.validator_pops)
        .any(|(validator, pop)| {
            validator.public_key().try_algorithm().ok() != Some(Algorithm::BlsNormal)
                || iroha_crypto::bls_normal_pop_verify(validator.public_key(), pop).is_err()
        })
        || view.policy.body.auditors.iter().any(|auditor| {
            view.authority
                .validators
                .iter()
                .any(|validator| validator.public_key() == &auditor.signing_key)
        })
    {
        return Err(PrivateSettlementAuditorApprovalErrorV1::InvalidView);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::private_settlement::{
        PrivateSettlementFileSidecarStoreV1, PrivateSettlementSidecarStoreConfigV1,
        sidecar_store::tests::sidecar_fixture,
    };

    fn approve_all(_: PrivateSettlementAuditEvaluationV1<'_>) -> bool {
        true
    }

    fn reject_all(_: PrivateSettlementAuditEvaluationV1<'_>) -> bool {
        false
    }

    fn must_not_evaluate(_: PrivateSettlementAuditEvaluationV1<'_>) -> bool {
        panic!("invalid view must not reach policy")
    }

    #[test]
    fn governed_auditor_decrypts_evaluates_and_signs_exact_leg() {
        let fixture = sidecar_fixture();
        let digest = fixture.sidecar.payload_digest();
        let temp = tempfile::tempdir().expect("tempdir");
        let store = PrivateSettlementFileSidecarStoreV1::open(
            temp.path().join("auditor-sidecars"),
            PrivateSettlementSidecarStoreConfigV1::default(),
        )
        .expect("store");
        store.store(fixture.sidecar.clone()).expect("upload");
        let view = store
            .fetch_for_auditor(digest, &fixture.auditor, 12)
            .expect("auditor view");
        let expected_plaintext = fixture.plaintext.clone();
        let evaluator = move |context: PrivateSettlementAuditEvaluationV1<'_>| {
            context.plaintext == &expected_plaintext
                && context.authoritative_height == 12
                && context.audit_policy.policy_digest
                    == context.manifest.legs[0].audit_policy_digest
        };
        let approval = approve_private_settlement_leg_v1(
            &view,
            &fixture.pool_governance,
            12,
            &fixture.auditor,
            fixture.hybrid.secret(),
            &fixture.signing,
            &evaluator,
        )
        .expect("approval");
        approval
            .verify(&fixture.sidecar.policy, 12)
            .expect("signature");
        iroha_data_model::nexus::validate_private_settlement_audit_approvals_v1(
            &[approval],
            &fixture.sidecar.policy,
            &fixture.sidecar.payload,
            12,
        )
        .expect("threshold and exact bindings");
    }

    #[test]
    fn wrong_keys_policy_rejection_and_stale_height_fail_closed() {
        let fixture = sidecar_fixture();
        let digest = fixture.sidecar.payload_digest();
        let temp = tempfile::tempdir().expect("tempdir");
        let store = PrivateSettlementFileSidecarStoreV1::open(
            temp.path().join("auditor-sidecars"),
            PrivateSettlementSidecarStoreConfigV1::default(),
        )
        .expect("store");
        store.store(fixture.sidecar.clone()).expect("upload");
        let view = store
            .fetch_for_auditor(digest, &fixture.auditor, 12)
            .expect("auditor view");
        let wrong_signing = KeyPair::from_seed(vec![0xD1; 32], Algorithm::Ed25519);
        assert_eq!(
            approve_private_settlement_leg_v1(
                &view,
                &fixture.pool_governance,
                12,
                &fixture.auditor,
                fixture.hybrid.secret(),
                &wrong_signing,
                &approve_all,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::SigningKeyMismatch)
        );
        let mut wrong_rng = iroha_crypto::rng_from_seed_slice(b"wrong auditor hybrid key");
        let wrong_hybrid = iroha_crypto::HybridKeyPair::generate(&mut wrong_rng).expect("key");
        assert_eq!(
            approve_private_settlement_leg_v1(
                &view,
                &fixture.pool_governance,
                12,
                &fixture.auditor,
                wrong_hybrid.secret(),
                &fixture.signing,
                &approve_all,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::CapsuleAuthenticationFailed)
        );
        assert_eq!(
            approve_private_settlement_leg_v1(
                &view,
                &fixture.pool_governance,
                12,
                &fixture.auditor,
                fixture.hybrid.secret(),
                &fixture.signing,
                &reject_all,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::PolicyRejected)
        );
        assert_eq!(
            approve_private_settlement_leg_v1(
                &view,
                &fixture.pool_governance,
                fixture.sidecar.manifest.expiry_height + 1,
                &fixture.auditor,
                fixture.hybrid.secret(),
                &fixture.signing,
                &approve_all,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::Stale)
        );
    }

    #[test]
    fn capsule_and_statement_substitution_are_rejected_before_policy() {
        let fixture = sidecar_fixture();
        let digest = fixture.sidecar.payload_digest();
        let temp = tempfile::tempdir().expect("tempdir");
        let store = PrivateSettlementFileSidecarStoreV1::open(
            temp.path().join("auditor-sidecars"),
            PrivateSettlementSidecarStoreConfigV1::default(),
        )
        .expect("store");
        store.store(fixture.sidecar).expect("upload");
        let mut view = store
            .fetch_for_auditor(digest, &fixture.auditor, 12)
            .expect("auditor view");
        view.audit_capsule.ciphertext[0] ^= 1;
        assert_eq!(
            approve_private_settlement_leg_v1(
                &view,
                &fixture.pool_governance,
                12,
                &fixture.auditor,
                fixture.hybrid.secret(),
                &fixture.signing,
                &must_not_evaluate,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::InvalidView)
        );
    }
}
