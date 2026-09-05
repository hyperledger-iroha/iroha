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
use iroha_crypto::{Algorithm, HybridPublicKey, HybridSecretKey, KeyPair, PublicKey, SignatureOf};
use iroha_data_model::{
    account::AccountId,
    nexus::{
        ATOMIC_PRIVATE_SETTLEMENT_VERSION_V1, AtomicPrivateSettlementV1,
        PrivateSettlementAuditApprovalBodyV1, PrivateSettlementAuditApprovalV1,
        PrivateSettlementAuditCapsuleV1, PrivateSettlementAuditPlaintextV1,
        PrivateSettlementAuditPolicyV1, PrivateSettlementHybridPublicKeyV1,
        PrivateSettlementPoolGovernanceV1,
    },
};
use thiserror::Error;
use zeroize::Zeroizing;

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

/// Redacted failure returned by a deployment-owned auditor credential provider.
///
/// Providers deliberately expose no backend diagnostics through this boundary:
/// a deployment-owned signer or remote decryption service may retain its
/// detailed diagnostics privately while the settlement path remains
/// indistinguishable and fail closed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
#[error("private-settlement auditor credential operation failed")]
pub struct PrivateSettlementAuditorCredentialErrorV1;

/// Deployment-owned credential boundary for one governed online auditor.
///
/// Implementations may keep the hybrid decryption secret and purpose-specific
/// approval key behind a deployment-owned credential boundary. The runtime
/// constructs every approval body and independently verifies every returned
/// signature. Decryption providers return only a zeroizing canonical plaintext
/// buffer, which is validated in full before the policy evaluator can observe
/// the decoded value.
pub trait PrivateSettlementAuditorCredentialProviderV1: Send + Sync {
    /// Public key of the purpose-specific approval signer.
    fn approval_public_key(&self) -> &PublicKey;

    /// Public counterpart of the governed hybrid capsule-decryption key.
    fn capsule_public_key(&self) -> &HybridPublicKey;

    /// Return whether this provider retains the exact governed decryption key.
    ///
    /// Providers with one key inherit the exact default comparison. Providers
    /// backed by a retained keyring override this method without exposing key
    /// count, epochs, or backend-selection details to the protocol.
    fn supports_capsule_public_key(
        &self,
        governed_key: &PrivateSettlementHybridPublicKeyV1,
    ) -> bool {
        PrivateSettlementHybridPublicKeyV1::from_hybrid(self.capsule_public_key()) == *governed_key
    }

    /// Open one exact governed capsule into its canonical plaintext bytes.
    ///
    /// # Errors
    ///
    /// Returns a redacted provider failure without leaking backend or plaintext
    /// details.
    fn open_capsule(
        &self,
        capsule: &PrivateSettlementAuditCapsuleV1,
        policy: &PrivateSettlementAuditPolicyV1,
        auditor_id: &AccountId,
    ) -> Result<Zeroizing<Vec<u8>>, PrivateSettlementAuditorCredentialErrorV1>;

    /// Sign the exact runtime-constructed, purpose-separated approval body.
    ///
    /// # Errors
    ///
    /// Returns a redacted provider failure. The runtime verifies the returned
    /// signature against [`Self::approval_public_key`] before releasing it.
    fn sign_approval(
        &self,
        body: &PrivateSettlementAuditApprovalBodyV1,
    ) -> Result<
        SignatureOf<PrivateSettlementAuditApprovalBodyV1>,
        PrivateSettlementAuditorCredentialErrorV1,
    >;
}

/// Runtime-only software adapter for an auditor's two purpose-separated keys.
///
/// Production deployments can replace this adapter with a deployment-owned
/// implementation of [`PrivateSettlementAuditorCredentialProviderV1`]. This
/// adapter borrows key material and never serializes it.
#[derive(Clone, Copy)]
pub struct SoftwarePrivateSettlementAuditorCredentialsV1<'a> {
    decryption_secret: &'a HybridSecretKey,
    signing_key: &'a KeyPair,
}

impl core::fmt::Debug for SoftwarePrivateSettlementAuditorCredentialsV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SoftwarePrivateSettlementAuditorCredentialsV1")
            .finish_non_exhaustive()
    }
}

impl<'a> SoftwarePrivateSettlementAuditorCredentialsV1<'a> {
    /// Borrow one hybrid decryption secret and a separate approval-signing key.
    #[must_use]
    pub const fn new(decryption_secret: &'a HybridSecretKey, signing_key: &'a KeyPair) -> Self {
        Self {
            decryption_secret,
            signing_key,
        }
    }
}

impl PrivateSettlementAuditorCredentialProviderV1
    for SoftwarePrivateSettlementAuditorCredentialsV1<'_>
{
    fn approval_public_key(&self) -> &PublicKey {
        self.signing_key.public_key()
    }

    fn capsule_public_key(&self) -> &HybridPublicKey {
        self.decryption_secret.public()
    }

    fn open_capsule(
        &self,
        capsule: &PrivateSettlementAuditCapsuleV1,
        policy: &PrivateSettlementAuditPolicyV1,
        auditor_id: &AccountId,
    ) -> Result<Zeroizing<Vec<u8>>, PrivateSettlementAuditorCredentialErrorV1> {
        open_private_settlement_audit_capsule_v1(
            capsule,
            policy,
            auditor_id,
            self.decryption_secret,
        )
        .map_err(|_| PrivateSettlementAuditorCredentialErrorV1)
    }

    fn sign_approval(
        &self,
        body: &PrivateSettlementAuditApprovalBodyV1,
    ) -> Result<
        SignatureOf<PrivateSettlementAuditApprovalBodyV1>,
        PrivateSettlementAuditorCredentialErrorV1,
    > {
        SignatureOf::try_new(self.signing_key.private_key(), body)
            .map_err(|_| PrivateSettlementAuditorCredentialErrorV1)
    }
}

/// Runtime-only software adapter retaining current and retired capsule keys.
///
/// The first key is the operationally current key; later entries are retained
/// only for the configured regulatory read horizon. Capsule opening selects by
/// the exact historical policy encryption key, never by trial decryption.
pub struct SoftwarePrivateSettlementAuditorKeyringCredentialsV1<'a> {
    decryption_secrets: &'a [HybridSecretKey],
    signing_key: &'a KeyPair,
}

impl core::fmt::Debug for SoftwarePrivateSettlementAuditorKeyringCredentialsV1<'_> {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SoftwarePrivateSettlementAuditorKeyringCredentialsV1")
            .finish_non_exhaustive()
    }
}

impl<'a> SoftwarePrivateSettlementAuditorKeyringCredentialsV1<'a> {
    /// Borrow a non-empty, duplicate-free current-plus-retired keyring.
    ///
    /// # Errors
    ///
    /// Returns a redacted credential error for an empty keyring or duplicate
    /// public keys.
    pub fn new(
        decryption_secrets: &'a [HybridSecretKey],
        signing_key: &'a KeyPair,
    ) -> Result<Self, PrivateSettlementAuditorCredentialErrorV1> {
        if decryption_secrets.is_empty()
            || decryption_secrets.iter().enumerate().any(|(index, key)| {
                decryption_secrets[..index].iter().any(|prior| {
                    PrivateSettlementHybridPublicKeyV1::from_hybrid(prior.public())
                        == PrivateSettlementHybridPublicKeyV1::from_hybrid(key.public())
                })
            })
        {
            return Err(PrivateSettlementAuditorCredentialErrorV1);
        }
        Ok(Self {
            decryption_secrets,
            signing_key,
        })
    }

    fn secret_for(
        &self,
        governed_key: &PrivateSettlementHybridPublicKeyV1,
    ) -> Option<&HybridSecretKey> {
        self.decryption_secrets.iter().find(|secret| {
            PrivateSettlementHybridPublicKeyV1::from_hybrid(secret.public()) == *governed_key
        })
    }
}

impl PrivateSettlementAuditorCredentialProviderV1
    for SoftwarePrivateSettlementAuditorKeyringCredentialsV1<'_>
{
    fn approval_public_key(&self) -> &PublicKey {
        self.signing_key.public_key()
    }

    fn capsule_public_key(&self) -> &HybridPublicKey {
        self.decryption_secrets[0].public()
    }

    fn supports_capsule_public_key(
        &self,
        governed_key: &PrivateSettlementHybridPublicKeyV1,
    ) -> bool {
        self.secret_for(governed_key).is_some()
    }

    fn open_capsule(
        &self,
        capsule: &PrivateSettlementAuditCapsuleV1,
        policy: &PrivateSettlementAuditPolicyV1,
        auditor_id: &AccountId,
    ) -> Result<Zeroizing<Vec<u8>>, PrivateSettlementAuditorCredentialErrorV1> {
        let governed_key = policy
            .body
            .auditors
            .iter()
            .find(|auditor| &auditor.auditor_id == auditor_id)
            .map(|auditor| &auditor.encryption_key)
            .ok_or(PrivateSettlementAuditorCredentialErrorV1)?;
        let secret = self
            .secret_for(governed_key)
            .ok_or(PrivateSettlementAuditorCredentialErrorV1)?;
        open_private_settlement_audit_capsule_v1(capsule, policy, auditor_id, secret)
            .map_err(|_| PrivateSettlementAuditorCredentialErrorV1)
    }

    fn sign_approval(
        &self,
        body: &PrivateSettlementAuditApprovalBodyV1,
    ) -> Result<
        SignatureOf<PrivateSettlementAuditApprovalBodyV1>,
        PrivateSettlementAuditorCredentialErrorV1,
    > {
        SignatureOf::try_new(self.signing_key.private_key(), body)
            .map_err(|_| PrivateSettlementAuditorCredentialErrorV1)
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
    let credentials =
        SoftwarePrivateSettlementAuditorCredentialsV1::new(decryption_secret, signing_key);
    approve_private_settlement_leg_with_provider_v1(
        view,
        pool_governance,
        authoritative_height,
        auditor_id,
        &credentials,
        evaluator,
    )
}

/// Validate, decrypt, policy-check, and sign through a deployment-owned credential provider.
///
/// The provider never chooses approval fields: this function constructs the
/// exact body after validating the complete public and restricted context, then
/// independently verifies the returned signature. Plaintext is confined to
/// this call and is never returned.
///
/// # Errors
///
/// Returns a redacted failure for invalid/stale context, provider-key mismatch,
/// authenticated-decryption failure, non-canonical plaintext, policy rejection,
/// provider failure, or an invalid returned signature.
pub fn approve_private_settlement_leg_with_provider_v1<
    P: PrivateSettlementAuditorCredentialProviderV1 + ?Sized,
    E: PrivateSettlementAuditPolicyEvaluatorV1 + ?Sized,
>(
    view: &PrivateSettlementAuditorSidecarViewV1,
    pool_governance: &PrivateSettlementPoolGovernanceV1,
    authoritative_height: u64,
    auditor_id: &AccountId,
    credentials: &P,
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
    if credentials.approval_public_key() != &governed_auditor.signing_key {
        return Err(PrivateSettlementAuditorApprovalErrorV1::SigningKeyMismatch);
    }
    if !credentials.supports_capsule_public_key(&governed_auditor.encryption_key) {
        return Err(PrivateSettlementAuditorApprovalErrorV1::CapsuleAuthenticationFailed);
    }

    let canonical_plaintext = credentials
        .open_capsule(&view.audit_capsule, &view.policy, auditor_id)
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
        signature: credentials
            .sign_approval(&body)
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
    if !matches!(
        view.lifecycle,
        PrivateSettlementSidecarLifecycleV1::Collecting
            | PrivateSettlementSidecarLifecycleV1::Audited
    ) || authoritative_height < view.manifest.authority_context_height
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

    struct TestCredentialProviderV1<'a> {
        software: SoftwarePrivateSettlementAuditorCredentialsV1<'a>,
        capsule_key_override: Option<&'a HybridPublicKey>,
        plaintext_override: Option<&'a [u8]>,
        signing_key_override: Option<&'a KeyPair>,
        fail_open: bool,
        fail_sign: bool,
    }

    impl<'a> TestCredentialProviderV1<'a> {
        fn exact(decryption_secret: &'a HybridSecretKey, signing_key: &'a KeyPair) -> Self {
            Self {
                software: SoftwarePrivateSettlementAuditorCredentialsV1::new(
                    decryption_secret,
                    signing_key,
                ),
                capsule_key_override: None,
                plaintext_override: None,
                signing_key_override: None,
                fail_open: false,
                fail_sign: false,
            }
        }
    }

    impl PrivateSettlementAuditorCredentialProviderV1 for TestCredentialProviderV1<'_> {
        fn approval_public_key(&self) -> &PublicKey {
            self.software.approval_public_key()
        }

        fn capsule_public_key(&self) -> &HybridPublicKey {
            self.capsule_key_override
                .unwrap_or_else(|| self.software.capsule_public_key())
        }

        fn open_capsule(
            &self,
            capsule: &PrivateSettlementAuditCapsuleV1,
            policy: &PrivateSettlementAuditPolicyV1,
            auditor_id: &AccountId,
        ) -> Result<Zeroizing<Vec<u8>>, PrivateSettlementAuditorCredentialErrorV1> {
            if self.fail_open {
                return Err(PrivateSettlementAuditorCredentialErrorV1);
            }
            if let Some(plaintext) = self.plaintext_override {
                return Ok(Zeroizing::new(plaintext.to_vec()));
            }
            self.software.open_capsule(capsule, policy, auditor_id)
        }

        fn sign_approval(
            &self,
            body: &PrivateSettlementAuditApprovalBodyV1,
        ) -> Result<
            SignatureOf<PrivateSettlementAuditApprovalBodyV1>,
            PrivateSettlementAuditorCredentialErrorV1,
        > {
            if self.fail_sign {
                return Err(PrivateSettlementAuditorCredentialErrorV1);
            }
            if let Some(signing_key) = self.signing_key_override {
                return SignatureOf::try_new(signing_key.private_key(), body)
                    .map_err(|_| PrivateSettlementAuditorCredentialErrorV1);
            }
            self.software.sign_approval(body)
        }
    }

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
            core::slice::from_ref(&approval),
            &fixture.sidecar.policy,
            &fixture.sidecar.payload,
            12,
        )
        .expect("threshold and exact bindings");

        let mut audited_retry_view = view;
        audited_retry_view.lifecycle = PrivateSettlementSidecarLifecycleV1::Audited;
        let retry = approve_private_settlement_leg_v1(
            &audited_retry_view,
            &fixture.pool_governance,
            12,
            &fixture.auditor,
            fixture.hybrid.secret(),
            &fixture.signing,
            &approve_all,
        )
        .expect("an uncertain submission can regenerate the same approval body");
        assert_eq!(retry.body, approval.body);
    }

    #[test]
    fn deployment_owned_provider_is_verified_at_both_credential_boundaries() {
        let fixture = sidecar_fixture();
        let digest = fixture.sidecar.payload_digest();
        let temp = tempfile::tempdir().expect("tempdir");
        let store = PrivateSettlementFileSidecarStoreV1::open(
            temp.path().join("provider-auditor-sidecars"),
            PrivateSettlementSidecarStoreConfigV1::default(),
        )
        .expect("store");
        store.store(fixture.sidecar.clone()).expect("upload");
        let view = store
            .fetch_for_auditor(digest, &fixture.auditor, 12)
            .expect("auditor view");

        let exact = TestCredentialProviderV1::exact(fixture.hybrid.secret(), &fixture.signing);
        let approval = approve_private_settlement_leg_with_provider_v1(
            &view,
            &fixture.pool_governance,
            12,
            &fixture.auditor,
            &exact,
            &approve_all,
        )
        .expect("provider approval");
        approval
            .verify(&fixture.sidecar.policy, 12)
            .expect("provider signature");

        let mut wrong_rng = iroha_crypto::rng_from_seed_slice(b"wrong provider capsule key");
        let wrong_hybrid = iroha_crypto::HybridKeyPair::generate(&mut wrong_rng).expect("key");
        let mut wrong_capsule_key =
            TestCredentialProviderV1::exact(fixture.hybrid.secret(), &fixture.signing);
        wrong_capsule_key.capsule_key_override = Some(wrong_hybrid.public());
        assert_eq!(
            approve_private_settlement_leg_with_provider_v1(
                &view,
                &fixture.pool_governance,
                12,
                &fixture.auditor,
                &wrong_capsule_key,
                &must_not_evaluate,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::CapsuleAuthenticationFailed)
        );

        let mut substituted_plaintext = fixture.plaintext.clone();
        substituted_plaintext.amount = substituted_plaintext.amount.saturating_add(1);
        let substituted_bytes = Zeroizing::new(
            norito::encode_canonical(&substituted_plaintext).expect("encode substituted plaintext"),
        );
        let mut substituted =
            TestCredentialProviderV1::exact(fixture.hybrid.secret(), &fixture.signing);
        substituted.plaintext_override = Some(substituted_bytes.as_slice());
        assert_eq!(
            approve_private_settlement_leg_with_provider_v1(
                &view,
                &fixture.pool_governance,
                12,
                &fixture.auditor,
                &substituted,
                &must_not_evaluate,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::InvalidPlaintext)
        );

        let wrong_signing = KeyPair::from_seed(vec![0xD2; 32], Algorithm::Ed25519);
        let mut invalid_signature =
            TestCredentialProviderV1::exact(fixture.hybrid.secret(), &fixture.signing);
        invalid_signature.signing_key_override = Some(&wrong_signing);
        assert_eq!(
            approve_private_settlement_leg_with_provider_v1(
                &view,
                &fixture.pool_governance,
                12,
                &fixture.auditor,
                &invalid_signature,
                &approve_all,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::SigningFailed)
        );
    }

    #[test]
    fn credential_provider_failures_remain_redacted_and_fail_closed() {
        let fixture = sidecar_fixture();
        let digest = fixture.sidecar.payload_digest();
        let temp = tempfile::tempdir().expect("tempdir");
        let store = PrivateSettlementFileSidecarStoreV1::open(
            temp.path().join("provider-failure-sidecars"),
            PrivateSettlementSidecarStoreConfigV1::default(),
        )
        .expect("store");
        store.store(fixture.sidecar.clone()).expect("upload");
        let view = store
            .fetch_for_auditor(digest, &fixture.auditor, 12)
            .expect("auditor view");

        let mut unavailable_decrypter =
            TestCredentialProviderV1::exact(fixture.hybrid.secret(), &fixture.signing);
        unavailable_decrypter.fail_open = true;
        assert_eq!(
            approve_private_settlement_leg_with_provider_v1(
                &view,
                &fixture.pool_governance,
                12,
                &fixture.auditor,
                &unavailable_decrypter,
                &must_not_evaluate,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::CapsuleAuthenticationFailed)
        );

        let mut unavailable_signer =
            TestCredentialProviderV1::exact(fixture.hybrid.secret(), &fixture.signing);
        unavailable_signer.fail_sign = true;
        assert_eq!(
            approve_private_settlement_leg_with_provider_v1(
                &view,
                &fixture.pool_governance,
                12,
                &fixture.auditor,
                &unavailable_signer,
                &approve_all,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::SigningFailed)
        );
        assert_eq!(
            PrivateSettlementAuditorCredentialErrorV1.to_string(),
            "private-settlement auditor credential operation failed"
        );
        let debug = format!(
            "{:?}",
            SoftwarePrivateSettlementAuditorCredentialsV1::new(
                fixture.hybrid.secret(),
                &fixture.signing,
            )
        );
        assert!(!debug.contains("PrivateKey"));
        assert!(!debug.contains("HybridSecretKey"));
    }

    #[test]
    fn retained_keyring_selects_the_exact_historical_decryption_key() {
        let fixture = sidecar_fixture();
        let mut current_rng =
            iroha_crypto::rng_from_seed_slice(b"rotated current auditor capsule key");
        let current =
            iroha_crypto::HybridKeyPair::generate(&mut current_rng).expect("current hybrid key");
        let current_signing = KeyPair::from_seed(vec![0xE1; 32], Algorithm::Ed25519);
        let retained = vec![current.secret().clone(), fixture.hybrid.secret().clone()];
        let keyring =
            SoftwarePrivateSettlementAuditorKeyringCredentialsV1::new(&retained, &current_signing)
                .expect("current-plus-retired keyring");
        assert!(
            keyring.supports_capsule_public_key(
                &fixture.sidecar.policy.body.auditors[0].encryption_key
            )
        );
        let opened = keyring
            .open_capsule(
                &fixture.sidecar.payload.audit_capsule,
                &fixture.sidecar.policy,
                &fixture.auditor,
            )
            .expect("retained historical key decrypts its exact old capsule");
        let plaintext = norito::decode_canonical::<PrivateSettlementAuditPlaintextV1>(&opened)
            .expect("historical capsule plaintext remains canonical");
        assert_eq!(plaintext, fixture.plaintext);

        let current_only = vec![current.secret().clone()];
        let current_only = SoftwarePrivateSettlementAuditorKeyringCredentialsV1::new(
            &current_only,
            &current_signing,
        )
        .expect("current-only keyring");
        assert_eq!(
            current_only.open_capsule(
                &fixture.sidecar.payload.audit_capsule,
                &fixture.sidecar.policy,
                &fixture.auditor,
            ),
            Err(PrivateSettlementAuditorCredentialErrorV1),
            "a current signing credential does not imply possession of a retired decryption key"
        );
        assert!(
            SoftwarePrivateSettlementAuditorKeyringCredentialsV1::new(&[], &current_signing)
                .is_err()
        );
        let duplicate = vec![current.secret().clone(), current.secret().clone()];
        assert!(
            SoftwarePrivateSettlementAuditorKeyringCredentialsV1::new(
                &duplicate,
                &current_signing,
            )
            .is_err()
        );
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
        let view = store
            .fetch_for_auditor(digest, &fixture.auditor, 12)
            .expect("auditor view");
        let mut capsule_substitution = view.clone();
        capsule_substitution.audit_capsule.ciphertext[0] ^= 1;
        assert_eq!(
            approve_private_settlement_leg_v1(
                &capsule_substitution,
                &fixture.pool_governance,
                12,
                &fixture.auditor,
                fixture.hybrid.secret(),
                &fixture.signing,
                &must_not_evaluate,
            ),
            Err(PrivateSettlementAuditorApprovalErrorV1::InvalidView)
        );

        let mut key_epoch_substitution = view;
        key_epoch_substitution.statement.audit_key_epoch = key_epoch_substitution
            .statement
            .audit_key_epoch
            .saturating_add(1);
        assert_eq!(
            approve_private_settlement_leg_v1(
                &key_epoch_substitution,
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
