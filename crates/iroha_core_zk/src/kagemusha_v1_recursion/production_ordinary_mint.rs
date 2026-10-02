//! Genuine released ordinary Mint113 proving from the actual Main captured funding attempt.
//! Both concrete released parity keys are used sequentially; no key generation, caller secret,
//! software platform key, fabricated finalized source or account-debit authority enters here.
use super::super::super::{
    ordinary_mint_circuit::{
        KagemushaOrdinaryMintCircuitParamsV1, KagemushaOrdinaryMintEpCircuitV1,
        KagemushaOrdinaryMintEqCircuitV1, ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1,
        OrdinaryMintWitnessV1, build_ordinary_mint_ep_v1, build_ordinary_mint_eq_v1,
    },
    ordinary_mint_public::{ordinary_mint_public_column_v1, ordinary_mint_public_data_v1},
    ordinary_mint_verifier::{
        KagemushaVerifiedOrdinaryMintAuthorizationV1, verify_ordinary_mint_authorization_v1,
    },
};
use super::*;
use crate::kagemusha_v1_state::KagemushaAuthenticatedOrdinaryMintApprovalSelectionV1;
use iroha_data_model::kagemusha::*;

impl<R: KagemushaArtifactByteResolverV1> KagemushaProductionProverV1<R> {
    /// Load only the actual captured Native Mint's same authenticated release and ordinary family.
    pub(crate) fn load_ordinary_mint(
        selection: &KagemushaAuthenticatedOrdinaryMintApprovalSelectionV1<'_>,
        profile: KagemushaRecursiveVerifierProfileV1,
        resolver: R,
    ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
        selection.recheck().map_err(owner_error)?;
        let owner = Self::from_selected_ordinary_release(
            selection.authenticated_release().map_err(owner_error)?,
            profile,
            resolver,
        )?;
        owner
            .verifier
            .ordinary_mint_material()
            .map_err(|e| proving_error(e.to_string()))?;
        owner.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        selection.recheck().map_err(owner_error)?;
        Ok(owner)
    }
    /// Produce and verify the sole complete unsigned TopUp original. This is pre-debit proof
    /// authority only. The actual account consent, current FI, exclusive DATA pending head and
    /// finalized Node source must be independently supplied by their actual owners afterward.
    pub(crate) fn prove_ordinary_mint_authorization(
        &self,
        selection: &KagemushaAuthenticatedOrdinaryMintApprovalSelectionV1<'_>,
    ) -> Result<KagemushaVerifiedOrdinaryMintAuthorizationV1, KagemushaArtifactGenerationErrorV1>
    {
        selection.recheck().map_err(owner_error)?;
        self.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        let statement = selection.statement().map_err(owner_error)?.clone();
        let raw_approval = selection.approval_original().map_err(owner_error)?;
        let approval: KagemushaOrdinaryMintApprovalV1 = norito::decode_canonical_with_limits(
            raw_approval,
            norito::canonical_decode_limits(raw_approval.len()),
        )
        .map_err(|e| proving_error(e.to_string()))?;
        if approval
            .canonical_bytes()
            .map_err(|e| proving_error(e.to_string()))?
            != raw_approval
        {
            return Err(proving_error(
                "retained Native Mint approval original differs",
            ));
        }
        let c = selection
            .enrollment()
            .map_err(owner_error)?
            .app_credential();
        let credential = KagemushaOrdinaryAppCredentialV1::decode_canonical_exact(c.original())
            .map_err(|e| proving_error(e.to_string()))?;
        let actual_lease = selection.selected_integrity_lease().map_err(owner_error)?;
        let lease = actual_lease
            .map(|held| {
                let v: KagemushaPlayIntegrityRefreshLeaseV1 = norito::decode_canonical_with_limits(
                    held.original(),
                    norito::canonical_decode_limits(held.original().len()),
                )
                .map_err(|e| proving_error(e.to_string()))?;
                if v.canonical_bytes()
                    .map_err(|e| proving_error(e.to_string()))?
                    != held.original()
                {
                    return Err(proving_error("retained Native Mint PI original differs"));
                }
                Ok(v)
            })
            .transpose()?;
        let encrypted = selection.encrypted_credit().map_err(owner_error)?;
        let floor = selection
            .previous_app_attest_counter()
            .map_err(owner_error)?;
        let mut result = None;
        let mut entered = false;
        selection
            .with_borrowed_mint_secrets(&mut |secret, opening| {
                if entered {
                    return Err(KagemushaStateErrorV1::SnapshotIntegrity);
                }
                entered = true;
                result = Some(self.prove_selected_mint(OrdinaryMintWitnessV1 {
                    statement: &statement,
                    approval: &approval,
                    credential: &credential,
                    previous_app_attest_counter: floor,
                    integrity_lease: lease.as_ref(),
                    financial_secret: secret,
                    credit_opening: opening,
                    encrypted_credit: encrypted,
                }));
                Ok(())
            })
            .map_err(owner_error)?;
        let proof = result
            .ok_or_else(|| proving_error("actual Mint financial/credit secrets were not lent"))??;
        let request = KagemushaOrdinaryTopUpRequestV1 {
            version: 1,
            authorization: KagemushaOrdinaryMintAuthorizationV1 {
                version: 1,
                statement,
                approval,
                proof,
            },
            encrypted_credit: encrypted.to_vec(),
        };
        let original = request
            .canonical_bytes()
            .map_err(|e| proving_error(e.to_string()))?;
        if original.len() > selection.request_capacity().map_err(owner_error)? {
            return Err(proving_error(
                "actual Mint full original exceeds the pre-invocation Native slot",
            ));
        }
        let mut verified = None;
        selection
            .with_verified_preparation_clock(&mut |clock| {
                verified = Some(verify_ordinary_mint_authorization_v1(
                    &self.verifier,
                    &original,
                    c,
                    actual_lease,
                    clock,
                ));
                Ok(())
            })
            .map_err(owner_error)?;
        let verified = verified
            .ok_or_else(|| proving_error("actual Native Mint clock original was not lent"))?
            .map_err(|e| proving_error(e.to_string()))?;
        selection.recheck().map_err(owner_error)?;
        self.require_release_binding(
            selection
                .authenticated_release()
                .map_err(owner_error)?
                .as_ref(),
        )?;
        Ok(verified)
    }
    fn prove_selected_mint(
        &self,
        witness: OrdinaryMintWitnessV1<'_>,
    ) -> Result<KagemushaOrdinaryMintPairedProofV1, KagemushaArtifactGenerationErrorV1> {
        let material = self
            .verifier
            .ordinary_mint_material()
            .map_err(|e| proving_error(e.to_string()))?;
        let eq_parameters = self.artifacts.load_eq_params()?;
        let ep_parameters = self.artifacts.load_ep_params()?;
        let root = self.release.provider_policy_root();
        let table = self.artifacts.ordinary_issuer_table();
        let public = ordinary_mint_public_data_v1(
            witness.statement,
            witness.approval,
            witness.credential,
            witness.integrity_lease,
            root,
        )
        .map_err(|e| proving_error(e.to_string()))?;
        let eq_history = initial_kagemusha_eq_accumulator_v1(&eq_parameters)
            .map_err(|e| proving_error(e.to_string()))?;
        let ep_history = initial_kagemusha_ep_accumulator_v1(&ep_parameters)
            .map_err(|e| proving_error(e.to_string()))?;
        let eq_instances = ordinary_mint_public_column_v1::<Fp>(&public, eq_history.as_bytes());
        let ep_instances = ordinary_mint_public_column_v1::<Fq>(&public, ep_history.as_bytes());
        let mut h = Sha256::new();
        h.update(b"iroha:kagemusha:v1:ordinary-native-mint-proof-recovery\0");
        h.update(witness.financial_secret);
        h.update(witness.approval.challenge.operation_id);
        h.update(witness.approval.challenge.nonce);
        h.update(
            witness
                .statement
                .binding_digest()
                .map_err(|e| proving_error(e.to_string()))?,
        );
        h.update(
            witness
                .approval
                .binding_digest()
                .map_err(|e| proving_error(e.to_string()))?,
        );
        let seed = KagemushaRecoverySeedV1::from_unsealed(h.finalize().into())
            .map_err(|e| proving_error(e.to_string()))?;
        // Eq's graph/PK are dropped before Ep allocation. The signed roles/profile must describe
        // the actual same relation packing; a mismatched OEM key or old84 column is refused.
        let eq_vk_bytes = self
            .artifacts
            .resolve(KagemushaArtifactRoleV1::MintAuthorizationVkEq)?;
        let eq_vk = native_backend::read_eq_ordinary_mint_authorization_vk(
            &eq_vk_bytes,
            self.profile.mint_authorization_eq.clone(),
            root,
            table,
        )?;
        let eq_pk =
            load_authenticated_proving_key_v1::<EqAffine, KagemushaOrdinaryMintEqCircuitV1, _>(
                &self.artifacts,
                KagemushaArtifactRoleV1::MintAuthorizationPkEq,
                KagemushaPastaParityV1::Eq,
                KagemushaOrdinaryMintCircuitParamsV1 {
                    base: self.profile.mint_authorization_eq.clone(),
                    provider_policy_root: root,
                    issuer_table: table.clone(),
                },
            )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Eq, &eq_pk, &eq_vk_bytes)?;
        let eq_protocol = compile(
            &eq_parameters,
            &eq_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1]),
        );
        require_protocol(
            &eq_protocol,
            KagemushaPastaParityV1::Eq,
            material.eq_protocol_digest,
            false,
        )?;
        let eq = build_ordinary_mint_eq_v1(&eq_parameters, &witness, root, table)
            .map_err(|e| proving_error(e.to_string()))?;
        if !super::super::same_base_params(
            &eq.builder.config_params,
            &self.profile.mint_authorization_eq,
        ) || eq.builder.assigned_instances[0]
            .iter()
            .map(|v| *v.value())
            .collect::<Vec<_>>()
            != eq_instances
        {
            return Err(proving_error(
                "ordinary Mint released Eq packing/public original differs",
            ));
        }
        let eq_proof = create_eq_proof_with_key_v1(
            &eq_parameters,
            &eq_pk,
            eq,
            &eq_instances,
            KagemushaProofRecoveryPhaseV1::MintAuthorization,
            &seed,
        )?;
        drop(eq_pk);
        halo2_proofs::release_allocator_slack();
        let ep_vk_bytes = self
            .artifacts
            .resolve(KagemushaArtifactRoleV1::MintAuthorizationVkEp)?;
        let ep_vk = native_backend::read_ep_ordinary_mint_authorization_vk(
            &ep_vk_bytes,
            self.profile.mint_authorization_ep.clone(),
            root,
            table,
        )?;
        let ep_pk =
            load_authenticated_proving_key_v1::<EpAffine, KagemushaOrdinaryMintEpCircuitV1, _>(
                &self.artifacts,
                KagemushaArtifactRoleV1::MintAuthorizationPkEp,
                KagemushaPastaParityV1::Ep,
                KagemushaOrdinaryMintCircuitParamsV1 {
                    base: self.profile.mint_authorization_ep.clone(),
                    provider_policy_root: root,
                    issuer_table: table.clone(),
                },
            )?;
        ensure_embedded_vk(KagemushaPastaParityV1::Ep, &ep_pk, &ep_vk_bytes)?;
        let ep_protocol = compile(
            &ep_parameters,
            &ep_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![ORDINARY_MINT_PUBLIC_INSTANCE_COUNT_V1]),
        );
        require_protocol(
            &ep_protocol,
            KagemushaPastaParityV1::Ep,
            material.ep_protocol_digest,
            false,
        )?;
        let ep = build_ordinary_mint_ep_v1(&ep_parameters, &witness, root, table)
            .map_err(|e| proving_error(e.to_string()))?;
        if !super::super::same_base_params(
            &ep.builder.config_params,
            &self.profile.mint_authorization_ep,
        ) || ep.builder.assigned_instances[0]
            .iter()
            .map(|v| *v.value())
            .collect::<Vec<_>>()
            != ep_instances
        {
            return Err(proving_error(
                "ordinary Mint released Ep packing/public original differs",
            ));
        }
        let ep_proof = create_ep_proof_with_key_v1(
            &ep_parameters,
            &ep_pk,
            ep,
            &ep_instances,
            KagemushaProofRecoveryPhaseV1::MintAuthorization,
            &seed,
        )?;
        drop(ep_pk);
        halo2_proofs::release_allocator_slack();
        let proof = KagemushaOrdinaryMintPairedProofV1 {
            version: 1,
            eq_protocol_digest: material.eq_protocol_digest,
            ep_protocol_digest: material.ep_protocol_digest,
            statement_digest: witness
                .statement
                .binding_digest()
                .map_err(|e| proving_error(e.to_string()))?,
            approval_original_digest: witness
                .approval
                .binding_digest()
                .map_err(|e| proving_error(e.to_string()))?,
            eq_proof,
            ep_proof,
            eq_history: eq_history.as_bytes().to_vec(),
            ep_history: ep_history.as_bytes().to_vec(),
        };
        proof
            .validate_shape_for_authorization(
                proof.statement_digest,
                proof.approval_original_digest,
            )
            .map_err(|e| proving_error(e.to_string()))?;
        Ok(proof)
    }
}
