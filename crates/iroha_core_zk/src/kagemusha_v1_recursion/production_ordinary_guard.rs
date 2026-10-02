//! Release-pinned ordinary Guard proving from an actual native financial holder.
//!
//! No key generation or caller-selected financial relation is admitted. The retained native
//! secret is borrowed once, bound to the actual selected Ed credential, and erased from local
//! relation copies before returning. The nonexportable app key's private scalar is never used.

use super::super::super::{
    KagemushaPlatformCredentialStatementV1,
    ordinary_guard_circuit::{
        KagemushaOrdinaryAppGuardEpCircuitV1, KagemushaOrdinaryAppGuardEqCircuitV1,
        KagemushaOrdinaryGuardCircuitParamsV1, OrdinaryGuardWitnessV1,
        build_ordinary_app_guard_pair_v1,
    },
    ordinary_guard_verifier::{OrdinaryGuardProofWireV1, public_column},
};
use super::*;
use crate::kagemusha_v1_state::{
    DigestV1, KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1,
    KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1,
    KagemushaOrdinaryEnrolledFinancialOwnerV1, KagemushaOrdinaryIdentityErrorV1,
    verify_ordinary_bootstrap_guard_v1,
};
use iroha_data_model::kagemusha::{
    KagemushaAppOperationApprovalV1, KagemushaHardwarePlatformClassV1,
    KagemushaOrdinaryAppCredentialV1,
};
use zeroize::Zeroize as _;

pub(super) struct HeldRelation(pub(super) KagemushaGuardBundleRelationWitnessV1);
impl Drop for HeldRelation {
    fn drop(&mut self) {
        self.0.predecessor_device_authority_secret.zeroize();
        self.0.successor_device_authority_secret.zeroize();
    }
}

impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Authenticate the signed ordinary Eq/Ep roles for an actual initial financial selection.
    /// Loading material grants neither a logical current owner nor financial authority.
    /// # Errors
    /// Rejects absent or substituted released keys, parameters, scope or originals.
    pub fn load_ordinary_bootstrap(
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
        let owner = Self::from_selected_ordinary_release(
            selection.authenticated_release().map_err(owner_error)?,
            profile,
            resolver,
        )?;
        owner.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        Ok(owner)
    }

    /// Prove both real ordinary Guards using the same held platform original and financial seed.
    /// The exact generated original is independently verified before it is exposed. No raw S,
    /// decoded credential, borrowed secret or user-provided proof callback creates this admission.
    /// # Errors
    /// Rejects stale original custody, an expired original attempt, mixed scope or any key/proof
    /// substitution. Failure never renews the native nonce or dispatches a platform assertion.
    pub fn prove_ordinary_bootstrap_guard(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        approval: &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
        financial: &KagemushaOrdinaryEnrolledFinancialOwnerV1,
    ) -> Result<Vec<u8>, KagemushaArtifactGenerationErrorV1> {
        let before = financial
            .trusted_time_ms()
            .map_err(ordinary_proving_error)?;
        selection
            .recheck_at_trusted_time(before)
            .map_err(owner_error)?;
        approval
            .recheck_captured_bootstrap_at_native_time(before)
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
        ) {
            return Err(proving_error("ordinary Guard actual enrollment differs"));
        }
        let mut entered = false;
        let mut result = None;
        financial
            .with_borrowed_financial_secret(selection, &mut |secret| {
                if entered {
                    return Err(KagemushaOrdinaryIdentityErrorV1::Rejected);
                }
                entered = true;
                result = Some(self.prove_original(selection, approval, secret));
                Ok(())
            })
            .map_err(ordinary_proving_error)?;
        let raw = result.ok_or_else(|| proving_error("ordinary native witness was not lent"))??;
        let after = financial
            .trusted_time_ms()
            .map_err(ordinary_proving_error)?;
        self.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        verify_ordinary_bootstrap_guard_v1(selection, approval, &raw, after)
            .map_err(owner_error)?;
        Ok(raw)
    }

    fn prove_original(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
        approval: &KagemushaAuthenticatedOrdinaryCapturedBootstrapApprovalV1<'_>,
        secret: &[u8; 32],
    ) -> Result<Vec<u8>, KagemushaArtifactGenerationErrorV1> {
        let credential: KagemushaOrdinaryAppCredentialV1 =
            norito::decode_canonical(selection.enrollment().app_credential().original())
                .map_err(ordinary_proving_error)?;
        let original: KagemushaAppOperationApprovalV1 =
            norito::decode_canonical(approval.original()).map_err(ordinary_proving_error)?;
        let integrity_lease = approval
            .original_approval_integrity_lease()
            .map(|lease| {
                if lease.original().is_empty() || lease.original().len() > 4096 {
                    return Err(proving_error(
                        "ordinary retained lease exceeds original bound",
                    ));
                }
                let raw: iroha_data_model::kagemusha::KagemushaPlayIntegrityRefreshLeaseV1 =
                    norito::decode_canonical(lease.original()).map_err(ordinary_proving_error)?;
                if raw.canonical_bytes().map_err(ordinary_proving_error)? != lease.original() {
                    return Err(proving_error(
                        "ordinary retained lease canonical original differs",
                    ));
                }
                Ok(raw)
            })
            .transpose()?;
        if credential
            .canonical_bytes()
            .map_err(ordinary_proving_error)?
            != selection.enrollment().app_credential().original()
            || norito::encode_canonical(&original).map_err(ordinary_proving_error)?
                != approval.original()
        {
            return Err(proving_error("ordinary proof original encoding differs"));
        }
        let relation = derive_relation(selection, secret)?;
        let eq_parameters = self.artifacts.load_eq_params()?;
        let ep_parameters = self.artifacts.load_ep_params()?;
        let root = self.release.provider_policy_root();
        let table = self.artifacts.ordinary_issuer_table();
        let eq_params = KagemushaOrdinaryGuardCircuitParamsV1 {
            base: self.profile.guard_eq.clone(),
            provider_policy_root: root,
            issuer_table: table.clone(),
        };
        let ep_params = KagemushaOrdinaryGuardCircuitParamsV1 {
            base: self.profile.guard_ep.clone(),
            provider_policy_root: root,
            issuer_table: table.clone(),
        };
        let eq_vk_bytes = self
            .artifacts
            .resolve(KagemushaArtifactRoleV1::OrdinaryAppGuardVkEq)?;
        let ep_vk_bytes = self
            .artifacts
            .resolve(KagemushaArtifactRoleV1::OrdinaryAppGuardVkEp)?;
        let eq_vk = native_backend::read_eq_ordinary_guard_vk(
            &eq_vk_bytes,
            self.profile.guard_eq.clone(),
            root,
            table,
        )?;
        let ep_vk = native_backend::read_ep_ordinary_guard_vk(
            &ep_vk_bytes,
            self.profile.guard_ep.clone(),
            root,
            table,
        )?;
        let eq_pk =
            load_authenticated_proving_key_v1::<EqAffine, KagemushaOrdinaryAppGuardEqCircuitV1, _>(
                &self.artifacts,
                KagemushaArtifactRoleV1::OrdinaryAppGuardPkEq,
                KagemushaPastaParityV1::Eq,
                eq_params,
            )?;
        let ep_pk =
            load_authenticated_proving_key_v1::<EpAffine, KagemushaOrdinaryAppGuardEpCircuitV1, _>(
                &self.artifacts,
                KagemushaArtifactRoleV1::OrdinaryAppGuardPkEp,
                KagemushaPastaParityV1::Ep,
                ep_params,
            )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Eq, &eq_pk, &eq_vk_bytes)?;
        ensure_embedded_vk(KagemushaPastaParityV1::Ep, &ep_pk, &ep_vk_bytes)?;
        let eq_protocol = compile(
            &eq_parameters,
            &eq_vk,
            snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![44]),
        );
        let ep_protocol = compile(
            &ep_parameters,
            &ep_vk,
            snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![44]),
        );
        let protocols = self.artifacts.ordinary_guard_protocol_digests();
        require_protocol(
            &eq_protocol,
            KagemushaPastaParityV1::Eq,
            protocols[0],
            false,
        )?;
        require_protocol(
            &ep_protocol,
            KagemushaPastaParityV1::Ep,
            protocols[1],
            false,
        )?;
        let (eq_circuit, ep_circuit) = build_ordinary_app_guard_pair_v1(
            &eq_parameters,
            &ep_parameters,
            OrdinaryGuardWitnessV1 {
                relation: &relation.0,
                credential: &credential,
                approval: &original,
                previous_app_attest_counter: approval.previous_app_attest_counter_floor(),
                integrity_lease: integrity_lease.as_ref(),
            },
            root,
            table,
        )
        .map_err(ordinary_proving_error)?;
        if !super::super::same_base_params(
            &eq_circuit.builder.config_params,
            &self.profile.guard_eq,
        ) || !super::super::same_base_params(
            &ep_circuit.builder.config_params,
            &self.profile.guard_ep,
        ) {
            return Err(proving_error("ordinary released circuit packing differs"));
        }
        let eq_history =
            initial_kagemusha_eq_accumulator_v1(&eq_parameters).map_err(ordinary_proving_error)?;
        let ep_history =
            initial_kagemusha_ep_accumulator_v1(&ep_parameters).map_err(ordinary_proving_error)?;
        let digests = [
            approval.challenge().normalized_guard_digest,
            selection.enrollment().app_credential().digest(),
            approval
                .authorization_binding_digest()
                .map_err(owner_error)?,
            approval.challenge().subject_signing_digest,
            root,
        ];
        let eq_instances = public_column::<Fp>(digests, eq_history.as_bytes());
        let ep_instances = public_column::<Fq>(digests, ep_history.as_bytes());
        let mut seed = Sha256::new();
        seed.update(b"iroha:kagemusha:v1:ordinary-guard-native-recovery-seed\0");
        seed.update(secret);
        seed.update(approval.challenge().operation_id);
        seed.update(approval.challenge().nonce);
        seed.update(
            approval
                .authorization_binding_digest()
                .map_err(owner_error)?,
        );
        let seed = KagemushaRecoverySeedV1::from_unsealed(seed.finalize().into())
            .map_err(ordinary_proving_error)?;
        let eq_proof = create_eq_proof_with_key_v1(
            &eq_parameters,
            &eq_pk,
            eq_circuit,
            &eq_instances,
            KagemushaProofRecoveryPhaseV1::OrdinaryAppGuard,
            &seed,
        )?;
        let ep_proof = create_ep_proof_with_key_v1(
            &ep_parameters,
            &ep_pk,
            ep_circuit,
            &ep_instances,
            KagemushaProofRecoveryPhaseV1::OrdinaryAppGuard,
            &seed,
        )?;
        let material = self.verifier.ordinary_guard_verifier_material();
        let wire = OrdinaryGuardProofWireV1 {
            version: 1,
            release_id: material.release_id,
            artifact_manifest_digest: material.artifact_manifest_digest,
            eq_protocol_digest: protocols[0],
            ep_protocol_digest: protocols[1],
            normalized_guard_digest: digests[0],
            credential_digest: digests[1],
            authorization_transcript_digest: digests[2],
            subject_signing_digest: digests[3],
            provider_policy_root: root,
            eq_proof,
            ep_proof,
            eq_history: *eq_history.as_bytes(),
            ep_history: *ep_history.as_bytes(),
        };
        let raw = norito::encode_canonical(&wire).map_err(ordinary_proving_error)?;
        if raw.len() > crate::kagemusha_v1_state::KAGEMUSHA_GUARD_BUNDLE_MAX_BYTES_V1 {
            return Err(proving_error("ordinary Guard original bound differs"));
        }
        Ok(raw)
    }
}

pub(super) fn derive_relation(
    selection: &KagemushaAuthenticatedOrdinaryBootstrapProvingSelectionV1<'_>,
    secret: &[u8; 32],
) -> Result<HeldRelation, KagemushaArtifactGenerationErrorV1> {
    let credential = selection.enrollment().app_credential();
    let s = credential.subject();
    if super::super::super::device_authority_commitment_v1(*secret)
        != s.financial_authority_commitment
    {
        return Err(proving_error(
            "native financial witness does not open ordinary enrollment",
        ));
    }
    let release = selection.authenticated_release().map_err(owner_error)?;
    let policy = release
        .provider_policy()
        .iter()
        .find(|p| p.hardware_profile_id == s.hardware_profile_id)
        .ok_or_else(|| proving_error("ordinary profile is not independently released"))?;
    let preview = selection.preview().map_err(owner_error)?;
    let state = &preview.state;
    let platform_tag = match s.platform_class {
        KagemushaHardwarePlatformClassV1::AppleAppAttest => 4,
        KagemushaHardwarePlatformClassV1::AndroidKeyMint => 5,
        _ => return Err(proving_error("ordinary Guard rejects OEM class")),
    };
    let projection = KagemushaPlatformCredentialStatementV1 {
        version: 1,
        protocol_version: 1,
        suite_id: s.suite_id,
        release_id: s.release_id,
        network_id: s.network_id,
        asset_id: state.lane.normalized_asset_id().map_err(owner_error)?,
        asset_incarnation: state.asset_incarnation,
        asset_scale: state.lane.scale,
        liability_pool_id: state.liability_pool_id,
        lane_id: s.lane_id,
        hardware_epoch_generation: u128::from(s.hardware_epoch),
        hardware_epoch_id: state.hardware_epoch.epoch_id,
        key_reference: s.app_key_reference,
        device_public_key: s.app_public_key,
        hardware_policy_id: release.provider_policy_root(),
        device_authority_commitment: s.financial_authority_commitment,
        hardware_profile_id: s.hardware_profile_id,
        policy_epoch: s.policy_epoch,
        platform_class: platform_tag,
        capability_mask: s.platform_class.required_guarantees(),
        provider_authority_commitment: policy.provider_authority_commitment,
        platform_attestation_digest: s.platform_evidence_digest,
        app_policy_binding_digest: credential.static_binding_digest(),
        credential_issuance_digest: credential.digest(),
        canonical_empty_effect_digest: canonical_empty_durable_effect(&release)?,
        provider_profile_index: policy.provider_profile_index,
    };
    let relation = HeldRelation(KagemushaGuardBundleRelationWitnessV1 {
        statement: preview.normalized_guard_statement.clone(),
        canonical_empty_effect_digest: projection.canonical_empty_effect_digest,
        predecessor_credential: projection,
        successor_credential: projection,
        predecessor_device_authority_secret: *secret,
        successor_device_authority_secret: *secret,
    });
    relation.0.validate().map_err(ordinary_proving_error)?;
    Ok(relation)
}

fn canonical_empty_durable_effect(
    release: &iroha_data_model::kagemusha::KagemushaAuthenticatedReleaseV1,
) -> Result<DigestV1, KagemushaArtifactGenerationErrorV1> {
    crate::kagemusha_v1_state::canonical_empty_durable_effect_digest_v1(release.release_id())
        .map_err(owner_error)
}

fn ordinary_proving_error(error: impl core::fmt::Display) -> KagemushaArtifactGenerationErrorV1 {
    proving_error(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::gates::circuit::BaseCircuitParams;

    #[test]
    fn ordinary_released_packing_checks_every_original_parameter() {
        let original = BaseCircuitParams {
            k: 16,
            num_advice_per_phase: vec![2, 1],
            num_fixed: 1,
            num_lookup_advice_per_phase: vec![1, 0],
            lookup_bits: Some(15),
            num_instance_columns: 1,
        };
        assert!(super::super::super::same_base_params(
            &original,
            &original.clone()
        ));
        let mutations: [fn(&mut BaseCircuitParams); 6] = [
            |p| p.k += 1,
            |p| p.num_advice_per_phase.swap(0, 1),
            |p| p.num_fixed += 1,
            |p| p.num_lookup_advice_per_phase.swap(0, 1),
            |p| p.lookup_bits = None,
            |p| p.num_instance_columns += 1,
        ];
        for mutate in mutations {
            let mut substituted = original.clone();
            mutate(&mut substituted);
            assert!(!super::super::super::same_base_params(
                &original,
                &substituted
            ));
        }
    }

    #[test]
    fn ordinary_proving_refusals_keep_the_canonical_typed_diagnostic() {
        let mut diagnostics = std::collections::BTreeSet::new();
        for error in [
            KagemushaOrdinaryIdentityErrorV1::Rejected,
            KagemushaOrdinaryIdentityErrorV1::Custody,
            KagemushaOrdinaryIdentityErrorV1::UnknownOutcome,
        ] {
            let KagemushaArtifactGenerationErrorV1::CircuitBuild(reason) =
                ordinary_proving_error(error)
            else {
                panic!("ordinary refusal changed its canonical error owner");
            };
            assert_eq!(reason, error.to_string());
            diagnostics.insert(reason);
        }
        assert_eq!(diagnostics.len(), 3);
        let error = crate::kagemusha_v1_recursion::KagemushaRecursionErrorV1::UnsupportedVersion;
        let expected = error.to_string();
        let KagemushaArtifactGenerationErrorV1::CircuitBuild(reason) =
            ordinary_proving_error(error)
        else {
            panic!("recursive refusal changed its canonical error owner");
        };
        assert_eq!(reason, expected);
    }
}
