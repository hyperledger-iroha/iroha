//! Ordinary bootstrap State proving with retained Native financial custody.
//!
//! Public helper/padding originals are data, never a seed, a platform key, a clock or an
//! authorization callback. The actual financial owner lends its secret once. Every original
//! approval/credential/lease and both genuine Guard proofs remain the same through proving.

use super::super::super::{
    KagemushaOrdinaryAppRecursiveSelectionWitnessV1,
    ordinary_guard_verifier::{OrdinaryGuardProofWireV1, verify_ordinary_bootstrap_guard_v1},
};
use super::production_ordinary_guard::derive_relation;
use super::*;
use crate::kagemusha_v1_state::{
    KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1,
    KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1,
    KagemushaOrdinaryEnrolledFinancialOwnerV1, KagemushaOrdinaryIdentityErrorV1,
};
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalV1, KagemushaOrdinaryAppCredentialV1,
    KagemushaPlayIntegrityRefreshLeaseV1,
};
use sha2::Sha256;
use zeroize::Zeroize as _;

#[path = "production_ordinary_bootstrap_inputs.rs"]
mod bootstrap_inputs;

/// Borrow public auxiliary original proofs without exposing financial witness material.
/// These data alone authorize no wallet or proof; the Native prover replaces the financial
/// relation with the actual retained owner and checks the complete final paired proof.
pub type KagemushaOrdinaryBootstrapAuxiliaryConsumerV1<'a> =
    dyn for<'w> FnMut(KagemushaRecursiveStateGenerationWitnessV1<'w>) -> Result<(), String> + 'a;

/// Retained public helper/padding original source for an actual ordinary bootstrap.
/// No platform/OEM unsealing, arbitrary seed or clock is requested from this source.
/// Implementations retain signed helper/protocol and public padding originals; the genuine
/// Native financial owner, approval journal and authenticated release remain independent.
/// This Rust data interface has no C/JNI registration or accepting verifier callback.
pub trait KagemushaOrdinaryBootstrapAuxiliaryProofSourceV1: Send + Sync {
    /// Lend the exact immutable public helper inputs. The financial secret fields must be zero;
    /// the actual selected financial relation is reconstructed inside the Native consumer.
    /// A supplied ordered SHA claim must be retained under this same original selection.
    fn with_borrowed_bootstrap_auxiliaries(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        paired_ordinary_guard_original: &[u8],
        hash_claim: Option<&KagemushaGeneratedMintHashClaimV1>,
        consume: &mut KagemushaOrdinaryBootstrapAuxiliaryConsumerV1<'_>,
    ) -> Result<(), String>;
}

struct Originals {
    credential: KagemushaOrdinaryAppCredentialV1,
    approval: KagemushaAppOperationApprovalV1,
    lease: Option<KagemushaPlayIntegrityRefreshLeaseV1>,
    guard: OrdinaryGuardProofWireV1,
}
impl Originals {
    fn bind<'s, 'w: 's>(
        &'s self,
        witness: KagemushaRecursiveStateGenerationWitnessV1<'w>,
        floor: Option<u32>,
    ) -> KagemushaRecursiveStateGenerationWitnessV1<'s> {
        let mut witness: KagemushaRecursiveStateGenerationWitnessV1<'s> = witness;
        witness.ordinary_selection = Some(KagemushaOrdinaryAppRecursiveSelectionWitnessV1 {
            credential: &self.credential,
            approval: &self.approval,
            integrity_lease: self.lease.as_ref(),
            previous_app_attest_counter: floor,
        });
        witness
    }
}

impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Produce the actual ordered SHA claim for the same selected ordinary zero-State.
    /// This grants neither publication nor value movement; genuine State proving must consume it.
    /// # Errors
    /// Rejects stale Native custody, replaced originals, mixed helper proofs or repeated lending.
    pub fn prove_ordinary_bootstrap_state_hash_claim(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        approval: &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        paired_ordinary_guard_original: &[u8],
        auxiliaries: &dyn KagemushaOrdinaryBootstrapAuxiliaryProofSourceV1,
    ) -> Result<KagemushaGeneratedMintHashClaimV1, KagemushaArtifactGenerationErrorV1> {
        let originals = self.ordinary_state_originals(
            selection,
            approval,
            financial,
            paired_ordinary_guard_original,
        )?;
        let eq = load_kagemusha_eq_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let ep = load_kagemusha_ep_mint_hash_artifacts_v1(&self.artifacts, &self.profile)?;
        let mut consumption = WitnessConsumption::default();
        financial
            .with_borrowed_financial_secret(selection, &mut |secret| {
                auxiliaries
                    .with_borrowed_bootstrap_auxiliaries(
                        selection,
                        paired_ordinary_guard_original,
                        None,
                        &mut |witness| {
                            consumption.consume(|| {
                                self.recheck_ordinary_state(selection, approval, financial)?;
                                let witness = self.bind_ordinary_state_witness(
                                    selection, approval, secret, witness, &originals,
                                )?;
                                let seed = ordinary_state_seed(secret, approval)?;
                                let claim = prove_kagemusha_recursive_state_hash_claim_v1(
                                    &eq, &ep, witness, &seed,
                                )?;
                                self.recheck_ordinary_state(selection, approval, financial)?;
                                Ok(claim)
                            })
                        },
                    )
                    .map_err(|_| KagemushaOrdinaryIdentityErrorV1::Rejected)
            })
            .map_err(|error| proving_error(error.to_string()))?;
        self.recheck_ordinary_state(selection, approval, financial)?;
        consumption.finish()
    }

    /// Produce and independently verify both real initial ordinary State parities.
    /// The caller must publish through the exclusive current owner with the same financial
    /// owner and approval journal; proof bytes alone initialize no mutable wallet.
    /// # Errors
    /// Applies the same original custody, exact Guard, seed and complete State checks as planning.
    pub fn prove_ordinary_bootstrap_state(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        approval: &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        paired_ordinary_guard_original: &[u8],
        hash_claim: &KagemushaGeneratedMintHashClaimV1,
        auxiliaries: &dyn KagemushaOrdinaryBootstrapAuxiliaryProofSourceV1,
    ) -> Result<KagemushaGeneratedRecursiveStateProofV1, KagemushaArtifactGenerationErrorV1> {
        let originals = self.ordinary_state_originals(
            selection,
            approval,
            financial,
            paired_ordinary_guard_original,
        )?;
        let (eq, ep) = self.load_state_keys()?;
        let mut consumption = WitnessConsumption::default();
        financial
            .with_borrowed_financial_secret(selection, &mut |secret| {
                auxiliaries
                    .with_borrowed_bootstrap_auxiliaries(
                        selection,
                        paired_ordinary_guard_original,
                        Some(hash_claim),
                        &mut |witness| {
                            consumption.consume(|| {
                                self.recheck_ordinary_state(selection, approval, financial)?;
                                let witness = self.bind_ordinary_state_witness(
                                    selection, approval, secret, witness, &originals,
                                )?;
                                let seed = ordinary_state_seed(secret, approval)?;
                                let proof =
                                    prove_kagemusha_recursive_state_v1(&eq, &ep, witness, &seed)?;
                                selection
                                    .verify_state_proof(&proof.proof)
                                    .map_err(owner_error)?;
                                self.recheck_ordinary_state(selection, approval, financial)?;
                                Ok(proof)
                            })
                        },
                    )
                    .map_err(|_| KagemushaOrdinaryIdentityErrorV1::Rejected)
            })
            .map_err(|error| proving_error(error.to_string()))?;
        self.recheck_ordinary_state(selection, approval, financial)?;
        consumption.finish()
    }

    fn recheck_ordinary_state(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        approval: &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<(), KagemushaArtifactGenerationErrorV1> {
        let now = financial
            .trusted_time_ms()
            .map_err(|e| proving_error(e.to_string()))?;
        selection
            .recheck_at_trusted_time(now)
            .map_err(owner_error)?;
        approval
            .recheck_captured_bootstrap_at_native_time(now)
            .map_err(owner_error)?;
        self.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        if !core::ptr::eq(
            selection.enrollment(),
            approval.retained_enrollment().as_ref(),
        ) || !Arc::ptr_eq(financial.enrollment(), approval.retained_enrollment())
        {
            return Err(proving_error(
                "ordinary State retained enrollment owner differs",
            ));
        }
        Ok(())
    }

    fn ordinary_state_originals(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        approval: &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
        paired_guard: &[u8],
    ) -> Result<Originals, KagemushaArtifactGenerationErrorV1> {
        self.recheck_ordinary_state(selection, approval, financial)?;
        let now = financial
            .trusted_time_ms()
            .map_err(|e| proving_error(e.to_string()))?;
        verify_ordinary_bootstrap_guard_v1(selection, approval, paired_guard, now)
            .map_err(owner_error)?;
        let credential: KagemushaOrdinaryAppCredentialV1 =
            norito::decode_canonical(selection.enrollment().app_credential().original())
                .map_err(|e| proving_error(e.to_string()))?;
        let original: KagemushaAppOperationApprovalV1 =
            norito::decode_canonical(approval.original())
                .map_err(|e| proving_error(e.to_string()))?;
        let lease = approval
            .original_approval_integrity_lease()
            .map(|lease| {
                let raw: KagemushaPlayIntegrityRefreshLeaseV1 =
                    norito::decode_canonical(lease.original())
                        .map_err(|e| proving_error(e.to_string()))?;
                if raw
                    .canonical_bytes()
                    .map_err(|e| proving_error(e.to_string()))?
                    != lease.original()
                {
                    return Err(proving_error(
                        "ordinary State selected lease original differs",
                    ));
                }
                Ok(raw)
            })
            .transpose()?;
        if credential
            .canonical_bytes()
            .map_err(|e| proving_error(e.to_string()))?
            != selection.enrollment().app_credential().original()
            || norito::encode_canonical(&original).map_err(|e| proving_error(e.to_string()))?
                != approval.original()
        {
            return Err(proving_error("ordinary State selected originals differ"));
        }
        let guard =
            norito::decode_canonical(paired_guard).map_err(|e| proving_error(e.to_string()))?;
        self.recheck_ordinary_state(selection, approval, financial)?;
        Ok(Originals {
            credential,
            approval: original,
            lease,
            guard,
        })
    }

    fn bind_ordinary_state_witness<'s, 'w: 's>(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        approval: &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
        secret: &[u8; 32],
        mut witness: KagemushaRecursiveStateGenerationWitnessV1<'w>,
        originals: &'s Originals,
    ) -> Result<KagemushaRecursiveStateGenerationWitnessV1<'s>, KagemushaArtifactGenerationErrorV1>
    {
        let expected = derive_relation(selection, secret)?;
        let valid_placeholders = witness.guard_relation.predecessor_device_authority_secret
            == [0; 32]
            && witness.guard_relation.successor_device_authority_secret == [0; 32];
        witness
            .guard_relation
            .predecessor_device_authority_secret
            .zeroize();
        witness
            .guard_relation
            .successor_device_authority_secret
            .zeroize();
        let artifacts = self.artifacts.ordinary_recursion_artifacts()?;
        let state = &witness.state;
        let (eq_reserved, ep_reserved) =
            super::super::super::ordinary_state_reserved::kagemusha_ordinary_state_reserved_guard_positions_v1();
        let preview = selection.preview().map_err(owner_error)?;
        let guard = &preview.normalized_guard_statement;
        if !valid_placeholders
            || witness.hardware_selection.is_some()
            || witness.ordinary_selection.is_some()
            || witness.guard_relation.statement != expected.0.statement
            || witness.guard_relation.predecessor_credential != expected.0.predecessor_credential
            || witness.guard_relation.successor_credential != expected.0.successor_credential
            || witness.guard_relation.canonical_empty_effect_digest
                != expected.0.canonical_empty_effect_digest
            || state.operation != KagemushaOperationV1::Bootstrap
            || state.predecessor.is_some()
            || state.successor != preview.state
            || state.amount != 0
            || state.journal_revision_before != 0
            || state.journal_revision_after != 0
            || state.transport_semantic_digest != preview.transport_semantic_digest
            || state.guard_statement_digest
                != guard
                    .canonical_digest()
                    .map_err(|e| proving_error(e.to_string()))?
            || state.transition_effect_digest != guard.transition_effect_digest
            || state.lifecycle_binding_digest != guard.lifecycle_binding_digest
            || state.mint_finality_semantic_digest != [0; 32]
            || state.mint_finality_proof_binding_digest != [0; 32]
            || state.peer_credit_id != [0; 32]
            || state.recipient_encryption_key_binding != [0; 32]
            || state.receive_credit.is_some()
            || state.receive_credit_binding_digest != [0; 32]
            || state.prepared_intent.is_some()
            || state.prepared_transition_binding_digest != [0; 32]
            || state.replay_insert.is_some()
            || state.eq_protocol_digest != artifacts.eq_protocol_digest
            || state.ep_protocol_digest != artifacts.ep_protocol_digest
            || state.guard_eq_protocol_digest != artifacts.guard_bundle_eq_protocol_digest
            || state.guard_ep_protocol_digest != artifacts.guard_bundle_ep_protocol_digest
            || state.guard_eq_credential_audit != eq_reserved
            || state.guard_ep_credential_audit != ep_reserved
            || state.mint_eq_protocol_digest != artifacts.mint_finality_eq_protocol_digest
            || state.mint_ep_protocol_digest != artifacts.mint_finality_ep_protocol_digest
            || state.mint_authorization_eq_protocol_digest
                != artifacts.mint_authorization_eq_protocol_digest
            || state.mint_authorization_ep_protocol_digest
                != artifacts.mint_authorization_ep_protocol_digest
            || state.commit_wrapper_eq_protocol_digest
                != artifacts.commit_wrapper_eq_protocol_digest
            || state.commit_wrapper_ep_protocol_digest
                != artifacts.commit_wrapper_ep_protocol_digest
            || witness.eq_guard_proof != originals.guard.eq_proof
            || witness.ep_guard_proof != originals.guard.ep_proof
            || witness.eq_guard_history.as_bytes() != &originals.guard.eq_history
            || witness.ep_guard_history.as_bytes() != &originals.guard.ep_history
        {
            return Err(proving_error(
                "ordinary State auxiliary originals or exact preview differ",
            ));
        }
        witness.state.validate().map_err(proving_error)?;
        witness.guard_relation = expected.0.clone();
        let bound = originals.bind(witness, approval.previous_app_attest_counter_floor());
        Ok(bound)
    }
}

fn ordinary_state_seed(
    secret: &[u8; 32],
    approval: &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
) -> Result<KagemushaRecoverySeedV1, KagemushaArtifactGenerationErrorV1> {
    let mut hash = Sha256::new();
    hash.update(b"iroha:kagemusha:v1:ordinary-state-native-recovery-seed\0");
    hash.update(secret);
    hash.update(approval.challenge().operation_id);
    hash.update(approval.challenge().nonce);
    hash.update(
        approval
            .authorization_binding_digest()
            .map_err(owner_error)?,
    );
    KagemushaRecoverySeedV1::from_unsealed(hash.finalize().into())
        .map_err(|e| proving_error(e.to_string()))
}
