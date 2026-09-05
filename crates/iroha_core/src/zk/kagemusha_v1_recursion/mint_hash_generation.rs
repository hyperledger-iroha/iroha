//! Release-key generation for the ordered KAGEMUSHA mint-hash proof chain.
//!
//! Generation qualifies a real one-block SHA proof and a real first recursive claim proof before
//! exposing any key bytes. The bounded convergence loop is release construction only; it is not a
//! payment, ancestry, fan-in, or proof-depth limit.

use super::*;

/// Complete release-construction input for the paired shard and ordered-claim keys.
#[derive(Clone, Copy)]
pub struct KagemushaMintHashArtifactGenerationWitnessV1<'a> {
    /// Nonzero release identifier embedded in both typed hash plans.
    pub release_id: [u8; 32],
    /// Real certificate whose canonical transcript qualifies the generated circuits.
    pub certificate: &'a KagemushaMintCertificateWitnessV1,
    /// Certificate transition being qualified.
    pub step: KagemushaMintAuthorityStepV1,
    /// Eq protocol with the fixed claim public ABI, used only to seed key-shape convergence.
    pub eq_claim_protocol_seed: &'a PlonkProtocol<EqAffine>,
    /// Ep protocol with the fixed claim public ABI, used only to seed key-shape convergence.
    pub ep_claim_protocol_seed: &'a PlonkProtocol<EpAffine>,
    /// Secret deterministic recovery source for the qualifying proofs.
    pub recovery_seed: &'a KagemushaRecoverySeedV1,
}

/// Canonical parameters, all eight release artifacts, layouts, and protocol identities.
#[derive(Clone, Debug)]
pub struct KagemushaGeneratedMintHashArtifactsV1 {
    /// Canonical Eq `k = 16` carrier parameters.
    pub eq_parameters: Arc<[u8]>,
    /// Canonical Ep `k = 16` carrier parameters.
    pub ep_parameters: Arc<[u8]>,
    /// Eq one-block shard proving key.
    pub eq_shard_proving_key: Arc<[u8]>,
    /// Eq one-block shard verifying key.
    pub eq_shard_verifying_key: Arc<[u8]>,
    /// Ep one-block shard proving key.
    pub ep_shard_proving_key: Arc<[u8]>,
    /// Ep one-block shard verifying key.
    pub ep_shard_verifying_key: Arc<[u8]>,
    /// Eq ordered-claim proving key.
    pub eq_claim_proving_key: Arc<[u8]>,
    /// Eq ordered-claim verifying key.
    pub eq_claim_verifying_key: Arc<[u8]>,
    /// Ep ordered-claim proving key.
    pub ep_claim_proving_key: Arc<[u8]>,
    /// Ep ordered-claim verifying key.
    pub ep_claim_verifying_key: Arc<[u8]>,
    /// Exact Eq shard layout.
    pub eq_shard_circuit_params: BaseCircuitParams,
    /// Exact Ep shard layout.
    pub ep_shard_circuit_params: BaseCircuitParams,
    /// Exact Eq claim layout.
    pub eq_claim_circuit_params: BaseCircuitParams,
    /// Exact Ep claim layout.
    pub ep_claim_circuit_params: BaseCircuitParams,
    /// Compiled Eq shard protocol identity.
    pub eq_shard_protocol_digest: [u8; 32],
    /// Compiled Ep shard protocol identity.
    pub ep_shard_protocol_digest: [u8; 32],
    /// Compiled Eq claim protocol identity.
    pub eq_claim_protocol_digest: [u8; 32],
    /// Compiled Ep claim protocol identity.
    pub ep_claim_protocol_digest: [u8; 32],
    /// Release embedded in the qualifying typed plan.
    pub release_id: [u8; 32],
}

impl KagemushaGeneratedMintHashArtifactsV1 {
    /// Return the exact eight hard-cut V1 manifest bindings in canonical role order.
    #[must_use]
    pub fn bindings(&self) -> [KagemushaArtifactBindingV1; 8] {
        [
            binding(
                KagemushaArtifactRoleV1::MintHashShardPkEq,
                &self.eq_shard_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintHashShardVkEq,
                &self.eq_shard_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintHashShardPkEp,
                &self.ep_shard_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintHashShardVkEp,
                &self.ep_shard_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintHashClaimPkEq,
                &self.eq_claim_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintHashClaimVkEq,
                &self.eq_claim_verifying_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintHashClaimPkEp,
                &self.ep_claim_proving_key,
            ),
            binding(
                KagemushaArtifactRoleV1::MintHashClaimVkEp,
                &self.ep_claim_verifying_key,
            ),
        ]
    }

    /// Install the shared carrier parameters and all eight keys in a content-addressed resolver.
    pub fn install_into(&self, resolver: &mut KagemushaMemoryArtifactResolverV1) {
        for bytes in [
            &self.eq_parameters,
            &self.ep_parameters,
            &self.eq_shard_proving_key,
            &self.eq_shard_verifying_key,
            &self.ep_shard_proving_key,
            &self.ep_shard_verifying_key,
            &self.eq_claim_proving_key,
            &self.eq_claim_verifying_key,
            &self.ep_claim_proving_key,
            &self.ep_claim_verifying_key,
        ] {
            resolver.insert(Arc::clone(bytes));
        }
    }

    /// Decode generated bytes for an in-crate guarded qualification before its final manifest
    /// exists. Production callers must use the authenticated loaders instead.
    #[cfg(any(test, feature = "kagemusha-real-proof-harness"))]
    pub(crate) fn into_loaded_for_testing(
        self,
        profile_digest: [u8; 32],
        artifact_manifest_digest: [u8; 32],
        suite_id: [u8; 32],
        vk_digest: [u8; 32],
    ) -> Result<
        (
            KagemushaLoadedEqMintHashArtifactsV1,
            KagemushaLoadedEpMintHashArtifactsV1,
        ),
        KagemushaArtifactGenerationErrorV1,
    > {
        decode_generated_for_testing_v1(
            self,
            profile_digest,
            artifact_manifest_digest,
            suite_id,
            vk_digest,
        )
    }
}

struct FirstClaimNativeWitnessV1<'a> {
    eq_carrier: &'a ParamsIPA<EqAffine>,
    ep_carrier: &'a ParamsIPA<EpAffine>,
    eq_shard_params: &'a ParamsIPA<EqAffine>,
    ep_shard_params: &'a ParamsIPA<EpAffine>,
    successor: KagemushaMintHashClaimPairStateV1,
    eq_leaf: KagemushaMintHashShardStatementV1,
    ep_leaf: KagemushaMintHashShardStatementV1,
    eq_claim_protocol: &'a PlonkProtocol<EqAffine>,
    ep_claim_protocol: &'a PlonkProtocol<EpAffine>,
    eq_shard_protocol: &'a PlonkProtocol<EqAffine>,
    ep_shard_protocol: &'a PlonkProtocol<EpAffine>,
    eq_shard_proof: &'a [u8],
    ep_shard_proof: &'a [u8],
    eq_parent_instances: Vec<Vec<Fp>>,
    ep_parent_instances: Vec<Vec<Fq>>,
    eq_parent_proof: Vec<u8>,
    ep_parent_proof: Vec<u8>,
    eq_parent_history: IpaAccumulator<EqAffine, NativeLoader>,
    ep_parent_history: IpaAccumulator<EpAffine, NativeLoader>,
    eq_parent_fold_proof: KagemushaEqFoldProofV1,
    ep_parent_fold_proof: KagemushaEpFoldProofV1,
    eq_leaf_fold_proof: KagemushaEqFoldProofV1,
    ep_leaf_fold_proof: KagemushaEpFoldProofV1,
    eq_successor_history: KagemushaEqAccumulatorV1,
    ep_successor_history: KagemushaEpAccumulatorV1,
}

impl FirstClaimNativeWitnessV1<'_> {
    fn pair_witness(
        &self,
        metadata: KagemushaMintHashClaimMetadataV1,
    ) -> KagemushaMintHashClaimPairWitnessV1<'_> {
        KagemushaMintHashClaimPairWitnessV1 {
            previous: None,
            previous_metadata: None,
            successor: self.successor,
            metadata,
            eq_leaf: self.eq_leaf.clone(),
            ep_leaf: self.ep_leaf.clone(),
            eq: KagemushaMintHashClaimParityWitnessV1 {
                parent_protocol: self.eq_claim_protocol,
                parent_instances: &self.eq_parent_instances,
                parent_proof: &self.eq_parent_proof,
                parent_history: &self.eq_parent_history,
                parent_fold_proof: self.eq_parent_fold_proof.as_bytes(),
                shard_protocol: self.eq_shard_protocol,
                shard_proof: self.eq_shard_proof,
                leaf_fold_proof: self.eq_leaf_fold_proof.as_bytes(),
                successor_history: self.eq_successor_history.as_bytes(),
            },
            ep: KagemushaMintHashClaimParityWitnessV1 {
                parent_protocol: self.ep_claim_protocol,
                parent_instances: &self.ep_parent_instances,
                parent_proof: &self.ep_parent_proof,
                parent_history: &self.ep_parent_history,
                parent_fold_proof: self.ep_parent_fold_proof.as_bytes(),
                shard_protocol: self.ep_shard_protocol,
                shard_proof: self.ep_shard_proof,
                leaf_fold_proof: self.ep_leaf_fold_proof.as_bytes(),
                successor_history: self.ep_successor_history.as_bytes(),
            },
        }
    }
}

struct PreparedFirstClaimBlueprintV1<'a> {
    native: FirstClaimNativeWitnessV1<'a>,
    audits: KagemushaMintHashClaimDeferredAuditsV1,
    metadata: KagemushaMintHashClaimMetadataV1,
    eq_instances: Vec<Vec<Fp>>,
    ep_instances: Vec<Vec<Fq>>,
}

impl PreparedFirstClaimBlueprintV1<'_> {
    fn build_eq(
        &self,
    ) -> Result<KagemushaMintHashClaimEqCircuitV1, KagemushaArtifactGenerationErrorV1> {
        let (circuit, instances) = build_kagemusha_mint_hash_claim_eq_v1(
            self.native.eq_carrier,
            self.native.ep_carrier,
            self.native.eq_shard_params,
            self.native.ep_shard_params,
            self.native.pair_witness(self.metadata),
            &self.audits,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        if instances != self.eq_instances {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
                "Eq mint-hash claim public values changed across blueprint rebuild".to_owned(),
            ));
        }
        Ok(circuit)
    }

    fn build_ep(
        &self,
    ) -> Result<KagemushaMintHashClaimEpCircuitV1, KagemushaArtifactGenerationErrorV1> {
        let (circuit, instances) = build_kagemusha_mint_hash_claim_ep_v1(
            self.native.eq_carrier,
            self.native.ep_carrier,
            self.native.eq_shard_params,
            self.native.ep_shard_params,
            self.native.pair_witness(self.metadata),
            &self.audits,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        if instances != self.ep_instances {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
                "Ep mint-hash claim public values changed across blueprint rebuild".to_owned(),
            ));
        }
        Ok(circuit)
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn prepare_first_claim_blueprint_v1<'a>(
    eq_carrier: &'a ParamsIPA<EqAffine>,
    ep_carrier: &'a ParamsIPA<EpAffine>,
    eq_shard_params: &'a ParamsIPA<EqAffine>,
    ep_shard_params: &'a ParamsIPA<EpAffine>,
    exact: &KagemushaExactMintHashPlanV1,
    eq_claim_protocol: &'a PlonkProtocol<EqAffine>,
    ep_claim_protocol: &'a PlonkProtocol<EpAffine>,
    eq_shard_protocol: &'a PlonkProtocol<EqAffine>,
    ep_shard_protocol: &'a PlonkProtocol<EpAffine>,
    eq_shard_proof: &'a [u8],
    ep_shard_proof: &'a [u8],
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<PreparedFirstClaimBlueprintV1<'a>, KagemushaArtifactGenerationErrorV1> {
    let eq_leaf = exact.eq_leaves.leaves().first().ok_or_else(|| {
        KagemushaArtifactGenerationErrorV1::CircuitBuild("empty Eq mint-hash plan".to_owned())
    })?;
    let ep_leaf = exact.ep_leaves.leaves().first().ok_or_else(|| {
        KagemushaArtifactGenerationErrorV1::CircuitBuild("empty Ep mint-hash plan".to_owned())
    })?;
    let eq_seed = initial_kagemusha_eq_accumulator_v1(eq_carrier)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_seed = initial_kagemusha_ep_accumulator_v1(ep_carrier)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_parent_instances = mint_hash_bootstrap_parent_instances_v1::<Fp>(
        eq_seed.as_bytes(),
        eq_claim_protocol.num_instance[1],
    );
    let ep_parent_instances = mint_hash_bootstrap_parent_instances_v1::<Fq>(
        ep_seed.as_bytes(),
        ep_claim_protocol.num_instance[1],
    );
    let eq_parent_proof = dummy_two_carrier_hybrid_ordinary_proof_bytes(
        eq_claim_protocol,
        EqAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Eq,
    )?;
    let ep_parent_proof = dummy_two_carrier_hybrid_ordinary_proof_bytes(
        ep_claim_protocol,
        EpAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Ep,
    )?;
    let eq_dummy_fold = KagemushaEqFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(
        EqAffine::generator().to_bytes().as_ref(),
    ))
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_dummy_fold = KagemushaEpFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(
        EpAffine::generator().to_bytes().as_ref(),
    ))
    .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let eq_shard_instances = KagemushaMintHashShardCircuitV1::<Fp>::instances(eq_leaf)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_shard_instances = KagemushaMintHashShardCircuitV1::<Fq>::instances(ep_leaf)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_shard_verified = verify_eq_succinct_protocol_with_transcript_binding(
        eq_shard_params,
        eq_shard_protocol,
        eq_shard_proof,
        &eq_shard_instances,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_shard_transcript_binding = eq_shard_verified.transcript_binding;
    let eq_lifted = lift_eq_mint_hash_shard_v1(eq_shard_verified.accumulator)?;
    let ep_shard_verified = verify_ep_succinct_protocol_with_transcript_binding(
        ep_shard_params,
        ep_shard_protocol,
        ep_shard_proof,
        &ep_shard_instances,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_shard_transcript_binding = ep_shard_verified.transcript_binding;
    let ep_lifted = lift_ep_mint_hash_shard_v1(ep_shard_verified.accumulator)?;
    let eq_leaf_fold =
        fold_kagemusha_eq_accumulators_v1(eq_carrier, &eq_lifted, &eq_seed, recovery_seed)
            .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_leaf_fold =
        fold_kagemusha_ep_accumulators_v1(ep_carrier, &ep_lifted, &ep_seed, recovery_seed)
            .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let successor = KagemushaMintHashClaimPairStateV1 {
        eq: KagemushaMintHashClaimStateV1::apply::<Fp>(exact.eq_plan, None, eq_leaf)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
        ep: KagemushaMintHashClaimStateV1::apply::<Fq>(exact.ep_plan, None, ep_leaf)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    };
    let eq_claim_digest =
        native_parent_protocol_digest_v1(eq_claim_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_claim_digest =
        native_parent_protocol_digest_v1(ep_claim_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_shard_digest =
        native_parent_protocol_digest_v1(eq_shard_protocol, KagemushaPastaParityV1::Eq)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_shard_digest =
        native_parent_protocol_digest_v1(ep_shard_protocol, KagemushaPastaParityV1::Ep)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let mut metadata = KagemushaMintHashClaimMetadataV1 {
        eq_claim_protocol: eq_claim_digest,
        ep_claim_protocol: ep_claim_digest,
        eq_shard_protocol: eq_shard_digest,
        ep_shard_protocol: ep_shard_digest,
        eq_deferred_audit: [1; 32],
        ep_deferred_audit: [2; 32],
        eq_proof_chain_root: mint_hash_proof_chain_root_v1::<Fp>(
            exact.eq_plan.release_id,
            exact.eq_plan.plan_binding,
            successor.eq.next_stage,
            None,
            Fp::from(0),
            eq_shard_transcript_binding,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
        ep_proof_chain_root: mint_hash_proof_chain_root_v1::<Fq>(
            exact.ep_plan.release_id,
            exact.ep_plan.plan_binding,
            successor.ep.next_stage,
            None,
            Fq::from(0),
            ep_shard_transcript_binding,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    };
    let eq_parent_history = eq_seed
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let ep_parent_history = ep_seed
        .to_native()
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
    let native = FirstClaimNativeWitnessV1 {
        eq_carrier,
        ep_carrier,
        eq_shard_params,
        ep_shard_params,
        successor,
        eq_leaf: eq_leaf.clone(),
        ep_leaf: ep_leaf.clone(),
        eq_claim_protocol,
        ep_claim_protocol,
        eq_shard_protocol,
        ep_shard_protocol,
        eq_shard_proof,
        ep_shard_proof,
        eq_parent_instances,
        ep_parent_instances,
        eq_parent_proof,
        ep_parent_proof,
        eq_parent_history,
        ep_parent_history,
        eq_parent_fold_proof: eq_dummy_fold,
        ep_parent_fold_proof: ep_dummy_fold,
        eq_leaf_fold_proof: eq_leaf_fold.proof().clone(),
        ep_leaf_fold_proof: ep_leaf_fold.proof().clone(),
        eq_successor_history: eq_leaf_fold.successor().clone(),
        ep_successor_history: ep_leaf_fold.successor().clone(),
    };
    let audits = derive_kagemusha_mint_hash_claim_deferred_audits_v1(
        eq_carrier,
        ep_carrier,
        eq_shard_params,
        ep_shard_params,
        native.pair_witness(metadata),
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    metadata.eq_deferred_audit = audits.eq_digest();
    metadata.ep_deferred_audit = audits.ep_digest();
    let eq_external_instances = claim_public_values_v1::<Fp>(
        KagemushaPastaParityV1::Eq,
        &successor,
        metadata,
        native.eq_successor_history.as_bytes(),
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_external_instances = claim_public_values_v1::<Fq>(
        KagemushaPastaParityV1::Ep,
        &successor,
        metadata,
        native.ep_successor_history.as_bytes(),
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_instances = audits
        .eq_inner_instances(&eq_external_instances)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_instances = audits
        .ep_inner_instances(&ep_external_instances)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    Ok(PreparedFirstClaimBlueprintV1 {
        native,
        audits,
        metadata,
        eq_instances,
        ep_instances,
    })
}

/// Generate and qualify the genuine paired shard and ordered-claim artifact set.
///
/// # Errors
///
/// Rejects an invalid certificate/release, a non-fixed public ABI, a resource-overrun, any real
/// shard or claim proof failure, or a claim key whose exact recursive protocol does not converge.
#[allow(clippy::too_many_lines)]
pub fn generate_kagemusha_mint_hash_artifacts_v1(
    witness: KagemushaMintHashArtifactGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedMintHashArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    generate_kagemusha_mint_hash_artifacts_with_limits_v1(
        witness,
        KagemushaProcessedKeyLimitsV1::release(),
    )
}

/// Exercise genuine claim keygen and proving under the external aggregate-memory guard.
///
/// This entry point exists only in unit-test builds or the dedicated non-shipping proof harness.
/// Its wider serialization envelope does not make the resulting artifacts release-eligible;
/// production generation above retains the hard V1 manifest limits and both paths retain the
/// configure-only advice-width ceiling.
#[cfg(any(test, feature = "kagemusha-real-proof-harness"))]
pub(crate) fn generate_kagemusha_mint_hash_artifacts_for_guarded_test_v1(
    witness: KagemushaMintHashArtifactGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedMintHashArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    generate_kagemusha_mint_hash_artifacts_with_limits_v1(
        witness,
        KagemushaProcessedKeyLimitsV1::guarded_real_proof(),
    )
}

#[allow(clippy::too_many_lines)]
fn generate_kagemusha_mint_hash_artifacts_with_limits_v1(
    witness: KagemushaMintHashArtifactGenerationWitnessV1<'_>,
    claim_key_limits: KagemushaProcessedKeyLimitsV1,
) -> Result<KagemushaGeneratedMintHashArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    witness
        .certificate
        .validate_for_step(witness.step)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if witness.release_id == [0; 32]
        || witness.certificate.statement.lifecycle.release_id != witness.release_id
        || witness.eq_claim_protocol_seed.num_instance.len() != 3
        || witness.ep_claim_protocol_seed.num_instance.len() != 3
        || witness.eq_claim_protocol_seed.num_instance[0]
            != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || witness.ep_claim_protocol_seed.num_instance[0]
            != KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1
        || witness.eq_claim_protocol_seed.num_instance[1]
            != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || witness.ep_claim_protocol_seed.num_instance[1]
            != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || witness.eq_claim_protocol_seed.num_instance[2]
            != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
        || witness.ep_claim_protocol_seed.num_instance[2]
            != KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "mint-hash generation input is release-unbound or has the wrong claim ABI".to_owned(),
        ));
    }
    ordinary_ipa_proof_profile_v1(witness.eq_claim_protocol_seed)
        .and_then(|_| ordinary_ipa_proof_profile_v1(witness.ep_claim_protocol_seed).map(|_| ()))
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    // Inventory the fixed dense-MSM geometry before exact-plan discovery constructs either
    // claim witness graph. This makes an accidental return to the former thousands-of-advice-
    // columns layout fail before Base assignments, synthesis, or polynomial allocation.
    preflight_helper_key_configuration_v1::<EqAffine, KagemushaMintHashClaimEqCircuitV1>(
        KAGEMUSHA_HALO2_K_V1 as usize,
        auxiliary_only_k16_base_params_v1(),
        KagemushaPastaParityV1::Eq,
        "mint-hash claim dense auxiliary geometry",
    )?;
    preflight_helper_key_configuration_v1::<EpAffine, KagemushaMintHashClaimEpCircuitV1>(
        KAGEMUSHA_HALO2_K_V1 as usize,
        auxiliary_only_k16_base_params_v1(),
        KagemushaPastaParityV1::Ep,
        "mint-hash claim dense auxiliary geometry",
    )?;
    let exact = exact_mint_hash_plan_v1(
        witness.release_id,
        mint_certificate_sha_messages_v1(witness.certificate, witness.step)?,
    )?;
    let eq_carrier = canonical_kagemusha_eq_parameters_v1();
    let ep_carrier = canonical_kagemusha_ep_parameters_v1();
    let eq_shard_params = canonical_kagemusha_eq_shard_parameters_v1();
    let ep_shard_params = canonical_kagemusha_ep_shard_parameters_v1();
    validate_mint_hash_shard_basis_prefix_v1(&eq_carrier, &eq_shard_params)
        .and_then(|()| validate_mint_hash_shard_basis_prefix_v1(&ep_carrier, &ep_shard_params))
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_leaf = exact
        .eq_leaves
        .leaves()
        .first()
        .expect("nonempty exact Eq plan");
    let ep_leaf = exact
        .ep_leaves
        .leaves()
        .first()
        .expect("nonempty exact Ep plan");
    let eq_shard_circuit = KagemushaMintHashShardCircuitV1::<Fp>::build(eq_leaf)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_shard_circuit_params = eq_shard_circuit.params();
    let eq_shard_pk = keygen_pk_with_helper_resource_preflight_consuming_v1(
        &eq_shard_params,
        eq_shard_circuit,
        KagemushaPastaParityV1::Eq,
        "mint-hash shard",
        "mint-hash shard proving key",
    )?;
    halo2_proofs::release_allocator_slack();
    let ep_shard_circuit = KagemushaMintHashShardCircuitV1::<Fq>::build(ep_leaf)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_shard_circuit_params = ep_shard_circuit.params();
    let ep_shard_pk = keygen_pk_with_helper_resource_preflight_consuming_v1(
        &ep_shard_params,
        ep_shard_circuit,
        KagemushaPastaParityV1::Ep,
        "mint-hash shard",
        "mint-hash shard proving key",
    )?;
    halo2_proofs::release_allocator_slack();
    let eq_shard_protocol = compile(
        &eq_shard_params,
        eq_shard_pk.get_vk(),
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_shard_protocol = compile(
        &ep_shard_params,
        ep_shard_pk.get_vk(),
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1]),
    );
    ordinary_ipa_proof_profile_at_k_v1(&eq_shard_protocol, KAGEMUSHA_MINT_HASH_SHARD_K_V1 as usize)
        .and_then(|_| {
            ordinary_ipa_proof_profile_at_k_v1(
                &ep_shard_protocol,
                KAGEMUSHA_MINT_HASH_SHARD_K_V1 as usize,
            )
            .map(|_| ())
        })
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_shard_instances = KagemushaMintHashShardCircuitV1::<Fp>::instances(eq_leaf)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let ep_shard_instances = KagemushaMintHashShardCircuitV1::<Fq>::instances(ep_leaf)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let eq_shard_proof_circuit = KagemushaMintHashShardCircuitV1::<Fp>::build(eq_leaf)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if !same_base_params(&eq_shard_proof_circuit.params(), &eq_shard_circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Eq,
        ));
    }
    let eq_shard_proof = create_eq_proof_with_key_v1(
        &eq_shard_params,
        &eq_shard_pk,
        eq_shard_proof_circuit,
        &eq_shard_instances,
        KagemushaProofRecoveryPhaseV1::MintHashShard,
        witness.recovery_seed,
    )?;
    halo2_proofs::release_allocator_slack();
    let ep_shard_proof_circuit = KagemushaMintHashShardCircuitV1::<Fq>::build(ep_leaf)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if !same_base_params(&ep_shard_proof_circuit.params(), &ep_shard_circuit_params) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Ep,
        ));
    }
    let ep_shard_proof = create_ep_proof_with_key_v1(
        &ep_shard_params,
        &ep_shard_pk,
        ep_shard_proof_circuit,
        &ep_shard_instances,
        KagemushaProofRecoveryPhaseV1::MintHashShard,
        witness.recovery_seed,
    )?;
    halo2_proofs::release_allocator_slack();
    verify_eq_succinct_protocol(
        &eq_shard_params,
        &eq_shard_protocol,
        &eq_shard_proof,
        &eq_shard_instances,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    verify_ep_succinct_protocol(
        &ep_shard_params,
        &ep_shard_protocol,
        &ep_shard_proof,
        &ep_shard_instances,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    // Claims need only the authenticated shard protocols/proofs. Serialize each shard key now
    // and release its native polynomial storage before building the much larger claim graphs.
    let (eq_shard_proving_key, eq_shard_verifying_key) = serialize_helper_keys_v1(
        KagemushaPastaParityV1::Eq,
        "mint-hash shard proving key",
        eq_shard_pk,
    )?;
    halo2_proofs::release_allocator_slack();
    let (ep_shard_proving_key, ep_shard_verifying_key) = serialize_helper_keys_v1(
        KagemushaPastaParityV1::Ep,
        "mint-hash shard proving key",
        ep_shard_pk,
    )?;
    halo2_proofs::release_allocator_slack();

    let mut eq_seed = witness.eq_claim_protocol_seed.clone();
    let mut ep_seed = witness.ep_claim_protocol_seed.clone();
    trim_hybrid_instance_key_v1(
        &mut eq_seed,
        KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
        "MintHashClaim seed",
    )?;
    trim_hybrid_instance_key_v1(
        &mut ep_seed,
        KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
        "MintHashClaim seed",
    )?;
    // Release construction convergence only; it does not limit payments or recursive history.
    for _ in 0..8 {
        // Each seed blueprint discovers two compact audits sequentially. Every later call builds
        // exactly one parity, so no sibling Base graph survives into polynomial expansion,
        // commitments, the fresh-VK check, or proof creation.
        halo2_proofs::release_allocator_slack();
        let seed_blueprint = prepare_first_claim_blueprint_v1(
            &eq_carrier,
            &ep_carrier,
            &eq_shard_params,
            &ep_shard_params,
            &exact,
            &eq_seed,
            &ep_seed,
            &eq_shard_protocol,
            &ep_shard_protocol,
            &eq_shard_proof,
            &ep_shard_proof,
            witness.recovery_seed,
        )?;
        let eq_circuit = seed_blueprint.build_eq()?;
        let eq_claim_circuit_params = eq_circuit.params();
        let eq_claim_vk = keygen_vk_with_key_resource_limits_consuming_v1(
            &eq_carrier,
            eq_circuit,
            KagemushaPastaParityV1::Eq,
            "mint-hash claim convergence",
            "mint-hash claim convergence verifying key",
            claim_key_limits,
        )?;
        halo2_proofs::release_allocator_slack();
        let ep_circuit = seed_blueprint.build_ep()?;
        let ep_claim_circuit_params = ep_circuit.params();
        let ep_claim_vk = keygen_vk_with_key_resource_limits_consuming_v1(
            &ep_carrier,
            ep_circuit,
            KagemushaPastaParityV1::Ep,
            "mint-hash claim convergence",
            "mint-hash claim convergence verifying key",
            claim_key_limits,
        )?;
        halo2_proofs::release_allocator_slack();
        let eq_instance_counts = seed_blueprint
            .eq_instances
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>();
        let ep_instance_counts = seed_blueprint
            .ep_instances
            .iter()
            .map(Vec::len)
            .collect::<Vec<_>>();
        let mut eq_claim_protocol = compile(
            &eq_carrier,
            &eq_claim_vk,
            snark_verifier::system::halo2::Config::ipa().with_num_instance(eq_instance_counts),
        );
        let mut ep_claim_protocol = compile(
            &ep_carrier,
            &ep_claim_vk,
            snark_verifier::system::halo2::Config::ipa().with_num_instance(ep_instance_counts),
        );
        trim_hybrid_instance_key_v1(
            &mut eq_claim_protocol,
            KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            "MintHashClaim",
        )?;
        trim_hybrid_instance_key_v1(
            &mut ep_claim_protocol,
            KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            "MintHashClaim",
        )?;
        ordinary_ipa_proof_profile_v1(&eq_claim_protocol)
            .and_then(|_| ordinary_ipa_proof_profile_v1(&ep_claim_protocol).map(|_| ()))
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        let eq_stable =
            kagemusha_protocol_structure_digest_v1(&eq_seed, KagemushaPastaParityV1::Eq)
                .and_then(|seed| {
                    kagemusha_protocol_structure_digest_v1(
                        &eq_claim_protocol,
                        KagemushaPastaParityV1::Eq,
                    )
                    .map(|actual| actual == seed)
                })
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        let ep_stable =
            kagemusha_protocol_structure_digest_v1(&ep_seed, KagemushaPastaParityV1::Ep)
                .and_then(|seed| {
                    kagemusha_protocol_structure_digest_v1(
                        &ep_claim_protocol,
                        KagemushaPastaParityV1::Ep,
                    )
                    .map(|actual| actual == seed)
                })
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        if !eq_stable || !ep_stable {
            drop(seed_blueprint);
            drop(eq_claim_vk);
            drop(ep_claim_vk);
            halo2_proofs::release_allocator_slack();
            eq_seed = eq_claim_protocol;
            ep_seed = ep_claim_protocol;
            continue;
        }
        seed_blueprint
            .audits
            .validate_release_inventory_v1()
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        drop(seed_blueprint);
        halo2_proofs::release_allocator_slack();
        let final_blueprint = prepare_first_claim_blueprint_v1(
            &eq_carrier,
            &ep_carrier,
            &eq_shard_params,
            &ep_shard_params,
            &exact,
            &eq_claim_protocol,
            &ep_claim_protocol,
            &eq_shard_protocol,
            &ep_shard_protocol,
            &eq_shard_proof,
            &ep_shard_proof,
            witness.recovery_seed,
        )?;
        final_blueprint
            .audits
            .validate_release_inventory_v1()
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        // The mandatory proving-key pass below synthesizes this exact final blueprint and embeds
        // the VK derived from that synthesis. Compare that embedded VK with the stabilized VK
        // there. Generating another standalone final VK here performed the same two full circuit
        // builds and the same comparison twice without adding an independent invariant.
        let stabilized_eq_vk_bytes = eq_claim_vk.to_bytes(SerdeFormat::Processed);
        let stabilized_ep_vk_bytes = ep_claim_vk.to_bytes(SerdeFormat::Processed);
        drop(eq_claim_vk);
        drop(ep_claim_vk);
        halo2_proofs::release_allocator_slack();

        // The proving-key pass is also the exact final-VK invariance gate. Finish Eq completely
        // and release its native PK before allocating the Ep PK so the two k=16 polynomial stores
        // never coexist.
        let eq_circuit = final_blueprint.build_eq()?;
        if !same_base_params(&eq_circuit.params(), &eq_claim_circuit_params) {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Eq,
            ));
        }
        let eq_claim_pk = keygen_pk_with_key_resource_limits_consuming_v1(
            &eq_carrier,
            eq_circuit,
            KagemushaPastaParityV1::Eq,
            "final mint-hash claim",
            "final mint-hash claim proving key",
            claim_key_limits,
        )?;
        let (eq_claim_proving_key, eq_claim_verifying_key) =
            serialize_helper_keys_streaming_with_limits_v1(
                KagemushaPastaParityV1::Eq,
                "mint-hash claim proving key",
                &eq_claim_pk,
                claim_key_limits,
            )?;
        if eq_claim_verifying_key.as_slice() != stabilized_eq_vk_bytes.as_slice() {
            drop(final_blueprint);
            drop(eq_claim_pk);
            drop(eq_claim_proving_key);
            drop(eq_claim_verifying_key);
            drop(stabilized_eq_vk_bytes);
            drop(stabilized_ep_vk_bytes);
            halo2_proofs::release_allocator_slack();
            eq_seed = eq_claim_protocol;
            ep_seed = ep_claim_protocol;
            continue;
        }
        drop(stabilized_eq_vk_bytes);
        halo2_proofs::release_allocator_slack();
        let eq_circuit = final_blueprint.build_eq()?;
        if !same_base_params(&eq_circuit.params(), &eq_claim_circuit_params) {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Eq,
            ));
        }
        let eq_claim_instances = final_blueprint.eq_instances.clone();
        let eq_claim_proof = create_eq_mint_hash_claim_hybrid_proof_consuming_key_v1(
            &eq_carrier,
            eq_claim_pk,
            eq_circuit,
            &eq_claim_instances,
            KagemushaProofRecoveryPhaseV1::MintHashClaim,
            witness.recovery_seed,
        )?;
        let eq_claim_proving_key: Arc<[u8]> = Arc::from(eq_claim_proving_key);
        let eq_claim_verifying_key: Arc<[u8]> = Arc::from(eq_claim_verifying_key);
        halo2_proofs::release_allocator_slack();
        let eq_current = KagemushaEqAccumulatorV1::from_native(
            &verify_eq_mint_hash_claim_hybrid_succinct_protocol(
                &eq_carrier,
                &eq_claim_protocol,
                &eq_claim_proof,
                &eq_claim_instances,
            )
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
        )
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
        decide_kagemusha_eq_accumulator_v1(&eq_carrier, &eq_current)
            .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
        drop(eq_claim_proof);
        drop(eq_claim_instances);
        drop(eq_current);
        halo2_proofs::release_allocator_slack();

        let ep_circuit = final_blueprint.build_ep()?;
        if !same_base_params(&ep_circuit.params(), &ep_claim_circuit_params) {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Ep,
            ));
        }
        let ep_claim_pk = keygen_pk_with_key_resource_limits_consuming_v1(
            &ep_carrier,
            ep_circuit,
            KagemushaPastaParityV1::Ep,
            "final mint-hash claim",
            "final mint-hash claim proving key",
            claim_key_limits,
        )?;
        let (ep_claim_proving_key, ep_claim_verifying_key) =
            serialize_helper_keys_streaming_with_limits_v1(
                KagemushaPastaParityV1::Ep,
                "mint-hash claim proving key",
                &ep_claim_pk,
                claim_key_limits,
            )?;
        if ep_claim_verifying_key.as_slice() != stabilized_ep_vk_bytes.as_slice() {
            drop(final_blueprint);
            drop(eq_claim_proving_key);
            drop(eq_claim_verifying_key);
            drop(ep_claim_pk);
            drop(ep_claim_proving_key);
            drop(ep_claim_verifying_key);
            drop(stabilized_ep_vk_bytes);
            halo2_proofs::release_allocator_slack();
            eq_seed = eq_claim_protocol;
            ep_seed = ep_claim_protocol;
            continue;
        }
        drop(stabilized_ep_vk_bytes);
        halo2_proofs::release_allocator_slack();
        let ep_circuit = final_blueprint.build_ep()?;
        if !same_base_params(&ep_circuit.params(), &ep_claim_circuit_params) {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
                KagemushaPastaParityV1::Ep,
            ));
        }
        let ep_claim_instances = final_blueprint.ep_instances.clone();
        let ep_claim_proof = create_ep_mint_hash_claim_hybrid_proof_consuming_key_v1(
            &ep_carrier,
            ep_claim_pk,
            ep_circuit,
            &ep_claim_instances,
            KagemushaProofRecoveryPhaseV1::MintHashClaim,
            witness.recovery_seed,
        )?;
        drop(final_blueprint);
        let ep_claim_proving_key: Arc<[u8]> = Arc::from(ep_claim_proving_key);
        let ep_claim_verifying_key: Arc<[u8]> = Arc::from(ep_claim_verifying_key);
        halo2_proofs::release_allocator_slack();
        let ep_current = KagemushaEpAccumulatorV1::from_native(
            &verify_ep_mint_hash_claim_hybrid_succinct_protocol(
                &ep_carrier,
                &ep_claim_protocol,
                &ep_claim_proof,
                &ep_claim_instances,
            )
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
        )
        .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
        decide_kagemusha_ep_accumulator_v1(&ep_carrier, &ep_current)
            .map_err(|error| KagemushaArtifactGenerationErrorV1::CircuitBuild(error.to_string()))?;
        drop(ep_claim_proof);
        drop(ep_claim_instances);
        drop(ep_current);
        halo2_proofs::release_allocator_slack();
        let eq_claim_protocol_digest =
            native_parent_protocol_digest_v1(&eq_claim_protocol, KagemushaPastaParityV1::Eq)
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        let ep_claim_protocol_digest =
            native_parent_protocol_digest_v1(&ep_claim_protocol, KagemushaPastaParityV1::Ep)
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        let eq_shard_protocol_digest =
            native_parent_protocol_digest_v1(&eq_shard_protocol, KagemushaPastaParityV1::Eq)
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        let ep_shard_protocol_digest =
            native_parent_protocol_digest_v1(&ep_shard_protocol, KagemushaPastaParityV1::Ep)
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        let identities = [
            eq_shard_protocol_digest,
            ep_shard_protocol_digest,
            eq_claim_protocol_digest,
            ep_claim_protocol_digest,
        ];
        if identities.iter().any(|digest| *digest == [0; 32])
            || identities
                .iter()
                .enumerate()
                .any(|(index, digest)| identities[index + 1..].contains(digest))
        {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
                "mint-hash generated protocol identities are absent or aliased".to_owned(),
            ));
        }
        let eq_parameters = serialize_carrier_params_v1(KagemushaPastaParityV1::Eq, &eq_carrier)?;
        let ep_parameters = serialize_carrier_params_v1(KagemushaPastaParityV1::Ep, &ep_carrier)?;
        return Ok(KagemushaGeneratedMintHashArtifactsV1 {
            eq_parameters,
            ep_parameters,
            eq_shard_proving_key,
            eq_shard_verifying_key,
            ep_shard_proving_key,
            ep_shard_verifying_key,
            eq_claim_proving_key,
            eq_claim_verifying_key,
            ep_claim_proving_key,
            ep_claim_verifying_key,
            eq_shard_circuit_params,
            ep_shard_circuit_params,
            eq_claim_circuit_params,
            ep_claim_circuit_params,
            eq_shard_protocol_digest,
            ep_shard_protocol_digest,
            eq_claim_protocol_digest,
            ep_claim_protocol_digest,
            release_id: witness.release_id,
        });
    }
    Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
        "mint-hash ordered-claim recursive key structure did not converge".to_owned(),
    ))
}

fn serialize_carrier_params_v1<C>(
    parity: KagemushaPastaParityV1,
    params: &ParamsIPA<C>,
) -> Result<Arc<[u8]>, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine,
{
    let mut bytes = Vec::new();
    params.write(&mut bytes).map_err(|error| {
        KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity,
            kind: "mint-hash carrier parameters",
            reason: error.to_string(),
        }
    })?;
    validate_length(
        parity,
        "parameters",
        bytes.len(),
        KAGEMUSHA_PARAMS_BYTES_V1,
        true,
    )?;
    Ok(Arc::from(bytes))
}

fn serialize_helper_keys_v1<C>(
    parity: KagemushaPastaParityV1,
    label: &'static str,
    proving_key: ProvingKey<C>,
) -> Result<(Arc<[u8]>, Arc<[u8]>), KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + FromUniformBytes<64>,
{
    serialize_helper_keys_with_limits_v1(
        parity,
        label,
        proving_key,
        KagemushaProcessedKeyLimitsV1::release(),
    )
}

fn serialize_helper_keys_with_limits_v1<C>(
    parity: KagemushaPastaParityV1,
    label: &'static str,
    proving_key: ProvingKey<C>,
    limits: KagemushaProcessedKeyLimitsV1,
) -> Result<(Arc<[u8]>, Arc<[u8]>), KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + FromUniformBytes<64>,
{
    let verifying = proving_key.get_vk().to_bytes(SerdeFormat::Processed);
    validate_length(
        parity,
        "mint-hash verifying key",
        verifying.len(),
        limits.verifying_key_maximum,
        false,
    )?;
    let proving = proving_key.into_bytes(SerdeFormat::Processed);
    validate_length(
        parity,
        label,
        proving.len(),
        limits.proving_key_maximum,
        false,
    )?;
    Ok((Arc::from(proving), Arc::from(verifying)))
}

fn serialize_helper_keys_streaming_with_limits_v1<C>(
    parity: KagemushaPastaParityV1,
    label: &'static str,
    proving_key: &ProvingKey<C>,
    limits: KagemushaProcessedKeyLimitsV1,
) -> Result<(Vec<u8>, Vec<u8>), KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + FromUniformBytes<64>,
{
    let verifying = proving_key.get_vk().to_bytes(SerdeFormat::Processed);
    validate_length(
        parity,
        "mint-hash verifying key",
        verifying.len(),
        limits.verifying_key_maximum,
        false,
    )?;
    let mut proving = Vec::new();
    proving_key
        .write_streaming(&mut proving, SerdeFormat::Processed)
        .map_err(|error| KagemushaArtifactGenerationErrorV1::KeyGeneration {
            parity,
            kind: label,
            reason: error.to_string(),
        })?;
    validate_length(
        parity,
        label,
        proving.len(),
        limits.proving_key_maximum,
        false,
    )?;
    Ok((proving, verifying))
}

#[cfg(any(test, feature = "kagemusha-real-proof-harness"))]
fn decode_generated_for_testing_v1(
    generated: KagemushaGeneratedMintHashArtifactsV1,
    profile_digest: [u8; 32],
    artifact_manifest_digest: [u8; 32],
    suite_id: [u8; 32],
    vk_digest: [u8; 32],
) -> Result<
    (
        KagemushaLoadedEqMintHashArtifactsV1,
        KagemushaLoadedEpMintHashArtifactsV1,
    ),
    KagemushaArtifactGenerationErrorV1,
> {
    if [
        profile_digest,
        artifact_manifest_digest,
        suite_id,
        vk_digest,
    ]
    .contains(&[0; 32])
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "test mint-hash loaded metadata must be explicit and nonzero".to_owned(),
        ));
    }
    let KagemushaGeneratedMintHashArtifactsV1 {
        eq_parameters,
        ep_parameters,
        eq_shard_proving_key,
        eq_shard_verifying_key,
        ep_shard_proving_key,
        ep_shard_verifying_key,
        eq_claim_proving_key,
        eq_claim_verifying_key,
        ep_claim_proving_key,
        ep_claim_verifying_key,
        eq_shard_circuit_params,
        ep_shard_circuit_params,
        eq_claim_circuit_params,
        ep_claim_circuit_params,
        eq_shard_protocol_digest,
        ep_shard_protocol_digest,
        eq_claim_protocol_digest,
        ep_claim_protocol_digest,
        release_id,
    } = generated;
    let eq_carrier = canonical_kagemusha_eq_parameters_v1();
    let ep_carrier = canonical_kagemusha_ep_parameters_v1();
    if serialize_carrier_params_v1(KagemushaPastaParityV1::Eq, &eq_carrier)?.as_ref()
        != eq_parameters.as_ref()
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "generated mint-hash parameters are noncanonical".to_owned(),
        ));
    }
    drop(eq_parameters);
    halo2_proofs::release_allocator_slack();
    if serialize_carrier_params_v1(KagemushaPastaParityV1::Ep, &ep_carrier)?.as_ref()
        != ep_parameters.as_ref()
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "generated mint-hash parameters are noncanonical".to_owned(),
        ));
    }
    drop(ep_parameters);
    halo2_proofs::release_allocator_slack();
    let eq_shard_params = canonical_kagemusha_eq_shard_parameters_v1();
    let ep_shard_params = canonical_kagemusha_ep_shard_parameters_v1();
    let eq_shard_vk =
        read_eq_mint_hash_shard_vk(&eq_shard_verifying_key, eq_shard_circuit_params.clone())?;
    let eq_shard_pk = read_test_proving_key_v1::<EqAffine, KagemushaMintHashShardCircuitV1<Fp>>(
        &eq_shard_proving_key,
        KagemushaPastaParityV1::Eq,
        KAGEMUSHA_MINT_HASH_SHARD_K_V1,
        eq_shard_circuit_params.clone(),
        "mint-hash shard proving key",
    )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &eq_shard_pk,
        &eq_shard_verifying_key,
    )?;
    drop(eq_shard_proving_key);
    drop(eq_shard_verifying_key);
    halo2_proofs::release_allocator_slack();

    let eq_claim_vk =
        read_eq_mint_hash_claim_vk(&eq_claim_verifying_key, eq_claim_circuit_params.clone())?;
    let eq_claim_pk = read_test_proving_key_v1::<EqAffine, KagemushaMintHashClaimEqCircuitV1>(
        &eq_claim_proving_key,
        KagemushaPastaParityV1::Eq,
        KAGEMUSHA_HALO2_K_V1,
        eq_claim_circuit_params.clone(),
        "mint-hash claim proving key",
    )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Eq,
        &eq_claim_pk,
        &eq_claim_verifying_key,
    )?;
    drop(eq_claim_proving_key);
    drop(eq_claim_verifying_key);
    halo2_proofs::release_allocator_slack();

    let ep_shard_vk =
        read_ep_mint_hash_shard_vk(&ep_shard_verifying_key, ep_shard_circuit_params.clone())?;
    let ep_shard_pk = read_test_proving_key_v1::<EpAffine, KagemushaMintHashShardCircuitV1<Fq>>(
        &ep_shard_proving_key,
        KagemushaPastaParityV1::Ep,
        KAGEMUSHA_MINT_HASH_SHARD_K_V1,
        ep_shard_circuit_params.clone(),
        "mint-hash shard proving key",
    )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &ep_shard_pk,
        &ep_shard_verifying_key,
    )?;
    drop(ep_shard_proving_key);
    drop(ep_shard_verifying_key);
    halo2_proofs::release_allocator_slack();

    let ep_claim_vk =
        read_ep_mint_hash_claim_vk(&ep_claim_verifying_key, ep_claim_circuit_params.clone())?;
    let ep_claim_pk = read_test_proving_key_v1::<EpAffine, KagemushaMintHashClaimEpCircuitV1>(
        &ep_claim_proving_key,
        KagemushaPastaParityV1::Ep,
        KAGEMUSHA_HALO2_K_V1,
        ep_claim_circuit_params.clone(),
        "mint-hash claim proving key",
    )?;
    ensure_embedded_vk(
        KagemushaPastaParityV1::Ep,
        &ep_claim_pk,
        &ep_claim_verifying_key,
    )?;
    drop(ep_claim_proving_key);
    drop(ep_claim_verifying_key);
    halo2_proofs::release_allocator_slack();

    let eq_shard_protocol = compile(
        &eq_shard_params,
        &eq_shard_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_shard_protocol = compile(
        &ep_shard_params,
        &ep_shard_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_HASH_SHARD_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let mut eq_claim_protocol = compile(
        &eq_carrier,
        &eq_claim_vk,
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![
            KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ]),
    );
    let mut ep_claim_protocol = compile(
        &ep_carrier,
        &ep_claim_vk,
        snark_verifier::system::halo2::Config::ipa().with_num_instance(vec![
            KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
            KAGEMUSHA_MINT_HASH_CLAIM_CARRIER_INSTANCE_COUNT_V1,
        ]),
    );
    trim_hybrid_instance_key_v1(
        &mut eq_claim_protocol,
        KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
        "MintHashClaim",
    )?;
    trim_hybrid_instance_key_v1(
        &mut ep_claim_protocol,
        KAGEMUSHA_MINT_HASH_CLAIM_INNER_SEMANTIC_INSTANCE_COUNT_V1,
        "MintHashClaim",
    )?;
    Ok((
        KagemushaLoadedEqMintHashArtifactsV1 {
            carrier_parameters: eq_carrier,
            shard_parameters: eq_shard_params,
            shard_proving_key: eq_shard_pk,
            shard_verifying_key: eq_shard_vk,
            shard_circuit_params: eq_shard_circuit_params,
            shard_protocol: eq_shard_protocol,
            shard_protocol_digest: eq_shard_protocol_digest,
            claim_proving_key: eq_claim_pk,
            claim_verifying_key: eq_claim_vk,
            claim_circuit_params: eq_claim_circuit_params,
            claim_protocol: eq_claim_protocol,
            claim_protocol_digest: eq_claim_protocol_digest,
            release_id,
            profile_digest,
            artifact_manifest_digest,
            suite_id,
            vk_digest,
        },
        KagemushaLoadedEpMintHashArtifactsV1 {
            carrier_parameters: ep_carrier,
            shard_parameters: ep_shard_params,
            shard_proving_key: ep_shard_pk,
            shard_verifying_key: ep_shard_vk,
            shard_circuit_params: ep_shard_circuit_params,
            shard_protocol: ep_shard_protocol,
            shard_protocol_digest: ep_shard_protocol_digest,
            claim_proving_key: ep_claim_pk,
            claim_verifying_key: ep_claim_vk,
            claim_circuit_params: ep_claim_circuit_params,
            claim_protocol: ep_claim_protocol,
            claim_protocol_digest: ep_claim_protocol_digest,
            release_id,
            profile_digest,
            artifact_manifest_digest,
            suite_id,
            vk_digest,
        },
    ))
}

#[cfg(any(test, feature = "kagemusha-real-proof-harness"))]
fn read_test_proving_key_v1<C, ConcreteCircuit>(
    bytes: &[u8],
    parity: KagemushaPastaParityV1,
    k: u32,
    circuit_params: ConcreteCircuit::Params,
    kind: &'static str,
) -> Result<ProvingKey<C>, KagemushaArtifactGenerationErrorV1>
where
    C: CurveAffine + halo2_proofs::SerdeCurveAffine,
    C::Scalar: halo2_proofs::SerdePrimeField + FromUniformBytes<64>,
    ConcreteCircuit: halo2_proofs::plonk::Circuit<C::Scalar>,
{
    let mut cursor = Cursor::new(bytes);
    let key = ProvingKey::read_checked::<_, ConcreteCircuit>(
        &mut cursor,
        SerdeFormat::Processed,
        k,
        circuit_params,
    )
    .map_err(|error| key_decode_error(parity, kind, error))?;
    ensure_cursor_consumed(parity, kind, &cursor, bytes.len())?;
    let mut canonical = ExactBytesWriterV1::new(bytes);
    key.write_streaming(&mut canonical, SerdeFormat::Processed)
        .map_err(|error| key_decode_error(parity, kind, error))?;
    if !canonical.matches() {
        return Err(key_decode_message(
            parity,
            kind,
            "processed encoding is non-canonical",
        ));
    }
    Ok(key)
}

#[cfg(any(test, feature = "kagemusha-real-proof-harness"))]
struct ExactBytesWriterV1<'a> {
    expected: &'a [u8],
    offset: usize,
    mismatch: bool,
}

#[cfg(any(test, feature = "kagemusha-real-proof-harness"))]
impl<'a> ExactBytesWriterV1<'a> {
    fn new(expected: &'a [u8]) -> Self {
        Self {
            expected,
            offset: 0,
            mismatch: false,
        }
    }

    fn matches(&self) -> bool {
        !self.mismatch && self.offset == self.expected.len()
    }
}

#[cfg(any(test, feature = "kagemusha-real-proof-harness"))]
impl std::io::Write for ExactBytesWriterV1<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        let end = self.offset.checked_add(bytes.len()).ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "canonical key length overflow",
            )
        })?;
        if end > self.expected.len() || &self.expected[self.offset..end] != bytes {
            self.mismatch = true;
        }
        self.offset = end;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_bytes_writer_rejects_mismatch_prefix_and_suffix() {
        let expected = [1, 2, 3, 4];
        let mut exact = ExactBytesWriterV1::new(&expected);
        std::io::Write::write_all(&mut exact, &expected[..2]).expect("write prefix");
        std::io::Write::write_all(&mut exact, &expected[2..]).expect("write suffix");
        assert!(exact.matches());

        let mut mismatch = ExactBytesWriterV1::new(&expected);
        std::io::Write::write_all(&mut mismatch, &[1, 2, 9, 4]).expect("write mismatch");
        assert!(!mismatch.matches());

        let mut prefix = ExactBytesWriterV1::new(&expected);
        std::io::Write::write_all(&mut prefix, &expected[..3]).expect("write short prefix");
        assert!(!prefix.matches());

        let mut suffix = ExactBytesWriterV1::new(&expected);
        std::io::Write::write_all(&mut suffix, &expected).expect("write exact bytes");
        std::io::Write::write_all(&mut suffix, &[5]).expect("write trailing byte");
        assert!(!suffix.matches());
    }

    #[test]
    fn generated_mint_hash_bindings_use_all_eight_release_roles_once() {
        let generated = KagemushaGeneratedMintHashArtifactsV1 {
            eq_parameters: Arc::from([1]),
            ep_parameters: Arc::from([2]),
            eq_shard_proving_key: Arc::from([3]),
            eq_shard_verifying_key: Arc::from([4]),
            ep_shard_proving_key: Arc::from([5]),
            ep_shard_verifying_key: Arc::from([6]),
            eq_claim_proving_key: Arc::from([7]),
            eq_claim_verifying_key: Arc::from([8]),
            ep_claim_proving_key: Arc::from([9]),
            ep_claim_verifying_key: Arc::from([10]),
            eq_shard_circuit_params: test_layout(KAGEMUSHA_MINT_HASH_SHARD_K_V1, 1),
            ep_shard_circuit_params: test_layout(KAGEMUSHA_MINT_HASH_SHARD_K_V1, 1),
            eq_claim_circuit_params: test_layout(KAGEMUSHA_HALO2_K_V1, 3),
            ep_claim_circuit_params: test_layout(KAGEMUSHA_HALO2_K_V1, 3),
            eq_shard_protocol_digest: [11; 32],
            ep_shard_protocol_digest: [12; 32],
            eq_claim_protocol_digest: [13; 32],
            ep_claim_protocol_digest: [14; 32],
            release_id: [15; 32],
        };
        assert_eq!(
            generated.bindings().map(|binding| binding.role),
            [
                KagemushaArtifactRoleV1::MintHashShardPkEq,
                KagemushaArtifactRoleV1::MintHashShardVkEq,
                KagemushaArtifactRoleV1::MintHashShardPkEp,
                KagemushaArtifactRoleV1::MintHashShardVkEp,
                KagemushaArtifactRoleV1::MintHashClaimPkEq,
                KagemushaArtifactRoleV1::MintHashClaimVkEq,
                KagemushaArtifactRoleV1::MintHashClaimPkEp,
                KagemushaArtifactRoleV1::MintHashClaimVkEp,
            ]
        );
    }

    fn test_layout(k: u32, num_instance_columns: usize) -> BaseCircuitParams {
        BaseCircuitParams {
            k: k as usize,
            num_advice_per_phase: vec![1],
            num_fixed: 1,
            num_lookup_advice_per_phase: vec![1],
            lookup_bits: Some((k - 1) as usize),
            num_instance_columns,
        }
    }
}
