//! Real inner-proof preparation, compact authority export, and bootstrap key convergence.
//!
//! The online authority persists its completed checkpoint before exposure. Its fresh proof
//! randomness is not a substitute for the separate hardware-sealed device recovery protocol.

use super::*;

pub(super) struct KagemushaPreparedMintAuthorityTransportV1 {
    pub(super) eq_circuit: KagemushaMintAuthorityTransportEqCircuitV1,
    pub(super) ep_circuit: KagemushaMintAuthorityTransportEpCircuitV1,
    pub(super) eq_instances: Vec<Fp>,
    pub(super) ep_instances: Vec<Fq>,
    pub(super) eq_history: KagemushaEqAccumulatorV1,
    pub(super) ep_history: KagemushaEpAccumulatorV1,
    pub(super) eq_deferred_audit: [u8; 32],
    pub(super) ep_deferred_audit: [u8; 32],
    pub(super) semantic_digest: [u8; 32],
    pub(super) certificate_binding: [u8; 32],
    pub(super) authority_head: [u8; 32],
    pub(super) release_id: [u8; 32],
    pub(super) genesis_roster_id: [u8; 32],
    pub(super) proof_binding_digest: [u8; 32],
}

#[derive(Clone, Copy)]
struct ReleaseIdentity {
    release: [u8; 32],
    profile: [u8; 32],
    manifest: [u8; 32],
    genesis: [u8; 32],
    protocol: [u8; 32],
}

fn validate_release_identity(
    eq: ReleaseIdentity,
    ep: ReleaseIdentity,
    release: [u8; 32],
    certificate_release: [u8; 32],
    genesis: [u8; 32],
    eq_protocol: [u8; 32],
    ep_protocol: [u8; 32],
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if [
        eq.release,
        eq.profile,
        eq.manifest,
        eq.genesis,
        eq.protocol,
        ep.protocol,
    ]
    .contains(&[0; 32])
        || eq.release != ep.release
        || eq.profile != ep.profile
        || eq.manifest != ep.manifest
        || eq.genesis != ep.genesis
        || eq.protocol == ep.protocol
        || release != eq.release
        || certificate_release != eq.release
        || genesis != eq.genesis
        || eq_protocol != eq.protocol
        || ep_protocol != ep.protocol
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "MintAuthority witness and parities do not belong to one authenticated release"
                .to_owned(),
        ));
    }
    Ok(())
}

/// Produce and decide one compact bootstrap, rotation, or finalized-mint proof pair.
///
/// The inner SHA/quorum proof is verified and folded into history before proving the outer
/// decider. Only that decider is exported; the inner pair commitment is copied unchanged.
/// Callers must durably retain the resulting checkpoint before publishing an online mint.
///
/// # Errors
///
/// Rejects mixed releases, substituted protocols, wrong layouts, invalid proofs/history, or
/// any compact proof exceeding the unchanged transport limit.
pub fn prove_kagemusha_mint_authority_v1(
    eq: &KagemushaLoadedEqMintAuthorityArtifactsV1,
    ep: &KagemushaLoadedEpMintAuthorityArtifactsV1,
    witness: KagemushaMintAuthorityGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedMintAuthorityProofV1, KagemushaArtifactGenerationErrorV1> {
    validate_release_identity(
        ReleaseIdentity {
            release: eq.release_id,
            profile: eq.profile_digest,
            manifest: eq.artifact_manifest_digest,
            genesis: eq.genesis_roster_id,
            protocol: eq.protocol_digest,
        },
        ReleaseIdentity {
            release: ep.release_id,
            profile: ep.profile_digest,
            manifest: ep.artifact_manifest_digest,
            genesis: ep.genesis_roster_id,
            protocol: ep.protocol_digest,
        },
        witness.release_id,
        witness.certificate.statement.lifecycle.release_id,
        witness.genesis_roster_id,
        witness.eq_protocol_digest,
        witness.ep_protocol_digest,
    )?;
    for (parity, layout) in [
        (KagemushaPastaParityV1::Eq, &eq.circuit_params),
        (KagemushaPastaParityV1::Ep, &ep.circuit_params),
        (KagemushaPastaParityV1::Eq, &eq.inner_circuit_params),
        (KagemushaPastaParityV1::Ep, &ep.inner_circuit_params),
    ] {
        validate_recursive_profile(parity, layout)?;
    }
    let eq_protocol = compile(
        &eq.parameters,
        &eq.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    let ep_protocol = compile(
        &ep.parameters,
        &ep.verifying_key,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
    );
    validate_protocol_identity(&eq_protocol, KagemushaPastaParityV1::Eq, eq.protocol_digest)?;
    validate_protocol_identity(&ep_protocol, KagemushaPastaParityV1::Ep, ep.protocol_digest)?;
    let prepared = prepare_mint_authority_transport_v1(
        KagemushaMintAuthorizationInnerKeysV1 {
            parameters: &eq.parameters,
            proving_key: &eq.inner_proving_key,
            verifying_key: &eq.inner_verifying_key,
            circuit_params: &eq.inner_circuit_params,
        },
        KagemushaMintAuthorizationInnerKeysV1 {
            parameters: &ep.parameters,
            proving_key: &ep.inner_proving_key,
            verifying_key: &ep.inner_verifying_key,
            circuit_params: &ep.inner_circuit_params,
        },
        witness,
    )?;
    finish_transport(
        &eq.parameters,
        &ep.parameters,
        &eq.proving_key,
        &ep.proving_key,
        &eq.circuit_params,
        &ep.circuit_params,
        &eq_protocol,
        &ep_protocol,
        prepared,
    )
}

fn validate_protocol_identity<C>(
    protocol: &PlonkProtocol<C>,
    parity: KagemushaPastaParityV1,
    expected: [u8; 32],
) -> Result<(), KagemushaArtifactGenerationErrorV1>
where
    C: halo2_base::utils::CurveAffineExt,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    validate_transport_protocol_profile(parity, "compact mint authority", protocol)?;
    let actual = native_parent_protocol_digest_v1(protocol, parity)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    if actual != expected {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "MintAuthority compact protocol differs from its authenticated key".to_owned(),
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn finish_transport(
    eq: &ParamsIPA<EqAffine>,
    ep: &ParamsIPA<EpAffine>,
    eq_pk: &ProvingKey<EqAffine>,
    ep_pk: &ProvingKey<EpAffine>,
    eq_layout: &BaseCircuitParams,
    ep_layout: &BaseCircuitParams,
    eq_protocol: &PlonkProtocol<EqAffine>,
    ep_protocol: &PlonkProtocol<EpAffine>,
    prepared: KagemushaPreparedMintAuthorityTransportV1,
) -> Result<KagemushaGeneratedMintAuthorityProofV1, KagemushaArtifactGenerationErrorV1> {
    let KagemushaPreparedMintAuthorityTransportV1 {
        eq_circuit,
        ep_circuit,
        eq_instances,
        ep_instances,
        eq_history,
        ep_history,
        eq_deferred_audit,
        ep_deferred_audit,
        semantic_digest,
        certificate_binding,
        authority_head,
        release_id,
        genesis_roster_id,
        proof_binding_digest,
    } = prepared;
    if !same_base_params(&eq_circuit.params(), eq_layout) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Eq,
        ));
    }
    if !same_base_params(&ep_circuit.params(), ep_layout) {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitProfileMismatch(
            KagemushaPastaParityV1::Ep,
        ));
    }
    let eq_proof = create_mint_eq_proof(eq, eq_pk, eq_circuit, &eq_instances)?;
    let ep_proof = create_mint_ep_proof(ep, ep_pk, ep_circuit, &ep_instances)?;
    validate_recursive_proof_length(KagemushaPastaParityV1::Eq, &eq_proof)?;
    validate_recursive_proof_length(KagemushaPastaParityV1::Ep, &ep_proof)?;
    let eq_current_accumulator = KagemushaEqAccumulatorV1::from_native(
        &verify_eq_succinct_protocol(eq, eq_protocol, &eq_proof, &eq_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|e| KagemushaArtifactGenerationErrorV1::CircuitBuild(e.to_string()))?;
    let ep_current_accumulator = KagemushaEpAccumulatorV1::from_native(
        &verify_ep_succinct_protocol(ep, ep_protocol, &ep_proof, &ep_instances)
            .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
    )
    .map_err(|e| KagemushaArtifactGenerationErrorV1::CircuitBuild(e.to_string()))?;
    decide_kagemusha_eq_accumulator_v1(eq, &eq_current_accumulator)
        .and_then(|()| decide_kagemusha_eq_accumulator_v1(eq, &eq_history))
        .map_err(|e| KagemushaArtifactGenerationErrorV1::CircuitBuild(e.to_string()))?;
    decide_kagemusha_ep_accumulator_v1(ep, &ep_current_accumulator)
        .and_then(|()| decide_kagemusha_ep_accumulator_v1(ep, &ep_history))
        .map_err(|e| KagemushaArtifactGenerationErrorV1::CircuitBuild(e.to_string()))?;
    let proof = KagemushaPairedProofV1 {
        version: KAGEMUSHA_WIRE_VERSION_V1,
        eq_protocol_digest: native_parent_protocol_digest_v1(
            eq_protocol,
            KagemushaPastaParityV1::Eq,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
        ep_protocol_digest: native_parent_protocol_digest_v1(
            ep_protocol,
            KagemushaPastaParityV1::Ep,
        )
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
        semantic_digest,
        guard_eq_credential_audit: certificate_binding,
        guard_ep_credential_audit: authority_head,
        eq_deferred_audit,
        ep_deferred_audit,
        eq_proof,
        ep_proof,
        eq_history: eq_history.as_bytes().to_vec(),
        ep_history: ep_history.as_bytes().to_vec(),
    };
    proof
        .validate_shape_for_semantic_digest(semantic_digest)
        .map_err(|e| KagemushaArtifactGenerationErrorV1::CircuitBuild(e.to_string()))?;
    Ok(KagemushaGeneratedMintAuthorityProofV1 {
        eq_public_instances: eq_instances,
        ep_public_instances: ep_instances,
        proof,
        eq_current_accumulator,
        ep_current_accumulator,
        certificate_binding,
        authority_head,
        release_id,
        genesis_roster_id,
        proof_binding_digest,
    })
}

/// Fixed parser material exists only for the disabled predecessor of a real bootstrap.
struct BootstrapInputs {
    eq_history: KagemushaEqAccumulatorV1,
    ep_history: KagemushaEpAccumulatorV1,
    eq_instances: Vec<Vec<Fp>>,
    ep_instances: Vec<Vec<Fq>>,
    eq_proof: Vec<u8>,
    ep_proof: Vec<u8>,
    eq_fold: KagemushaEqFoldProofV1,
    ep_fold: KagemushaEpFoldProofV1,
}

impl BootstrapInputs {
    fn new(
        eq: &ParamsIPA<EqAffine>,
        ep: &ParamsIPA<EpAffine>,
        eq_protocol: &PlonkProtocol<EqAffine>,
        ep_protocol: &PlonkProtocol<EpAffine>,
    ) -> Result<Self, KagemushaArtifactGenerationErrorV1> {
        let eq_history = initial_kagemusha_eq_accumulator_v1(eq)
            .map_err(|e| KagemushaArtifactGenerationErrorV1::CircuitBuild(e.to_string()))?;
        let ep_history = initial_kagemusha_ep_accumulator_v1(ep)
            .map_err(|e| KagemushaArtifactGenerationErrorV1::CircuitBuild(e.to_string()))?;
        let eq_point = EqAffine::generator().to_bytes();
        let ep_point = EpAffine::generator().to_bytes();
        Ok(Self {
            eq_instances: bootstrap_parent_instances(eq_history.as_bytes()),
            ep_instances: bootstrap_parent_instances(ep_history.as_bytes()),
            eq_history,
            ep_history,
            eq_proof: dummy_ordinary_proof_bytes(
                eq_protocol,
                eq_point.as_ref(),
                KagemushaPastaParityV1::Eq,
            )?,
            ep_proof: dummy_ordinary_proof_bytes(
                ep_protocol,
                ep_point.as_ref(),
                KagemushaPastaParityV1::Ep,
            )?,
            eq_fold: KagemushaEqFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(
                eq_point.as_ref(),
            ))
            .map_err(|e| KagemushaArtifactGenerationErrorV1::CircuitBuild(e.to_string()))?,
            ep_fold: KagemushaEpFoldProofV1::try_from_bytes(&dummy_fold_proof_bytes(
                ep_point.as_ref(),
            ))
            .map_err(|e| KagemushaArtifactGenerationErrorV1::CircuitBuild(e.to_string()))?,
        })
    }

    fn witness<'a>(
        &'a self,
        template: &KagemushaMintAuthorityGenerationWitnessV1<'_>,
        eq: &'a PlonkProtocol<EqAffine>,
        ep: &'a PlonkProtocol<EpAffine>,
    ) -> Result<KagemushaMintAuthorityGenerationWitnessV1<'a>, KagemushaArtifactGenerationErrorV1>
    {
        Ok(KagemushaMintAuthorityGenerationWitnessV1 {
            step: KagemushaMintAuthorityStepV1::Bootstrap,
            release_id: template.release_id,
            genesis_roster_id: template.genesis_roster_id,
            certificate: template.certificate.clone(),
            eq_protocol_digest: native_parent_protocol_digest_v1(eq, KagemushaPastaParityV1::Eq)
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
            ep_protocol_digest: native_parent_protocol_digest_v1(ep, KagemushaPastaParityV1::Ep)
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?,
            eq_deferred_audit: [1; 32],
            ep_deferred_audit: [2; 32],
            eq_parent_protocol: eq,
            ep_parent_protocol: ep,
            eq_parent_instances: &self.eq_instances,
            ep_parent_instances: &self.ep_instances,
            eq_parent_proof: &self.eq_proof,
            ep_parent_proof: &self.ep_proof,
            eq_parent_history: &self.eq_history,
            ep_parent_history: &self.ep_history,
            eq_parent_fold_proof: &self.eq_fold,
            ep_parent_fold_proof: &self.ep_fold,
            eq_successor_history: &self.eq_history,
            ep_successor_history: &self.ep_history,
        })
    }
}

fn require_bootstrap(
    step: KagemushaMintAuthorityStepV1,
) -> Result<(), KagemushaArtifactGenerationErrorV1> {
    if step != KagemushaMintAuthorityStepV1::Bootstrap {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "MintAuthority key generation requires an actual Bootstrap witness".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn generate(
    template: KagemushaMintAuthorityGenerationWitnessV1<'_>,
) -> Result<KagemushaGeneratedMintAuthorityArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    require_bootstrap(template.step)?;
    template
        .certificate
        .validate_for_step(template.step)
        .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
    let roster = template
        .certificate
        .epoch_roster
        .finality_epoch_id()
        .map_err(|e| KagemushaArtifactGenerationErrorV1::CircuitBuild(e.to_string()))?;
    if template.release_id == [0; 32]
        || template.certificate.statement.lifecycle.release_id != template.release_id
        || roster != template.genesis_roster_id
        || template.certificate.seal_bundle.message.finality_epoch_id != roster
    {
        return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
            "MintAuthority bootstrap certificate differs from release or genesis roster".to_owned(),
        ));
    }
    let eq = ParamsIPA::<EqAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let ep = ParamsIPA::<EpAffine>::new(KAGEMUSHA_HALO2_K_V1);
    let mut eq_seed = template.eq_parent_protocol.clone();
    let mut ep_seed = template.ep_parent_protocol.clone();
    macro_rules! key {
        ($params:expr, $circuit:expr, $parity:expr, $label:literal) => {{
            let layout = $circuit.params();
            validate_recursive_profile($parity, &layout)?;
            let vk = keygen_vk_with_helper_resource_preflight_v1(
                $params,
                &$circuit,
                $parity,
                $label,
                concat!($label, " verifying key"),
            )?;
            let pk = keygen_pk($params, vk.clone(), &$circuit).map_err(|e| {
                KagemushaArtifactGenerationErrorV1::KeyGeneration {
                    parity: $parity,
                    kind: concat!($label, " proving key"),
                    reason: e.to_string(),
                }
            })?;
            (pk, vk, layout)
        }};
    }
    macro_rules! assert_vk {
        ($params:expr, $circuit:expr, $vk:expr, $parity:expr, $label:literal) => {{
            let layout = $circuit.params();
            validate_recursive_profile($parity, &layout)?;
            let rebuilt = keygen_vk_with_helper_resource_preflight_v1(
                $params,
                &$circuit,
                $parity,
                $label,
                "final mint key stability",
            )?;
            if rebuilt.to_bytes(SerdeFormat::Processed) != $vk.to_bytes(SerdeFormat::Processed) {
                return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
                    "MintAuthority verifying key changed under final compact protocol identities"
                        .to_owned(),
                ));
            }
        }};
    }
    // This bounds offline release construction only, never a monetary or recursive history.
    for _ in 0..8 {
        let input = BootstrapInputs::new(&eq, &ep, &eq_seed, &ep_seed)?;
        let witness = input.witness(&template, &eq_seed, &ep_seed)?;
        let (inner_eq, inner_ep, _, _) =
            build_mint_authority_generation_pair(&eq, &ep, witness.clone())?;
        let (inner_eq_pk, inner_eq_vk, inner_eq_layout) = key!(
            &eq,
            inner_eq,
            KagemushaPastaParityV1::Eq,
            "inner mint authority"
        );
        let (inner_ep_pk, inner_ep_vk, inner_ep_layout) = key!(
            &ep,
            inner_ep,
            KagemushaPastaParityV1::Ep,
            "inner mint authority"
        );
        drop(inner_eq);
        drop(inner_ep);
        let (_, inner_eq_proving_key, inner_eq_verifying_key) = build_generated_helper_parity(
            KagemushaPastaParityV1::Eq,
            "inner mint authority proving key",
            &eq,
            &inner_eq_pk,
            &inner_eq_vk,
        )?;
        let (_, inner_ep_proving_key, inner_ep_verifying_key) = build_generated_helper_parity(
            KagemushaPastaParityV1::Ep,
            "inner mint authority proving key",
            &ep,
            &inner_ep_pk,
            &inner_ep_vk,
        )?;
        let prepare = |witness| {
            prepare_mint_authority_transport_v1(
                KagemushaMintAuthorizationInnerKeysV1 {
                    parameters: &eq,
                    proving_key: &inner_eq_pk,
                    verifying_key: &inner_eq_vk,
                    circuit_params: &inner_eq_layout,
                },
                KagemushaMintAuthorizationInnerKeysV1 {
                    parameters: &ep,
                    proving_key: &inner_ep_pk,
                    verifying_key: &inner_ep_vk,
                    circuit_params: &inner_ep_layout,
                },
                witness,
            )
        };
        let prepared = prepare(witness)?;
        let (eq_pk, eq_vk, eq_layout) = key!(
            &eq,
            prepared.eq_circuit,
            KagemushaPastaParityV1::Eq,
            "compact mint authority"
        );
        let (ep_pk, ep_vk, ep_layout) = key!(
            &ep,
            prepared.ep_circuit,
            KagemushaPastaParityV1::Ep,
            "compact mint authority"
        );
        drop(prepared);
        let eq_protocol = compile(
            &eq,
            &eq_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let ep_protocol = compile(
            &ep,
            &ep_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![KAGEMUSHA_MINT_AUTHORITY_PUBLIC_INSTANCE_COUNT_V1]),
        );
        let eq_protocol_digest =
            native_parent_protocol_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        let ep_protocol_digest =
            native_parent_protocol_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        validate_protocol_identity(&eq_protocol, KagemushaPastaParityV1::Eq, eq_protocol_digest)?;
        validate_protocol_identity(&ep_protocol, KagemushaPastaParityV1::Ep, ep_protocol_digest)?;
        let eq_stable =
            kagemusha_protocol_structure_digest_v1(&eq_seed, KagemushaPastaParityV1::Eq)
                .and_then(|seed| {
                    kagemusha_protocol_structure_digest_v1(&eq_protocol, KagemushaPastaParityV1::Eq)
                        .map(|actual| actual == seed)
                })
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        let ep_stable =
            kagemusha_protocol_structure_digest_v1(&ep_seed, KagemushaPastaParityV1::Ep)
                .and_then(|seed| {
                    kagemusha_protocol_structure_digest_v1(&ep_protocol, KagemushaPastaParityV1::Ep)
                        .map(|actual| actual == seed)
                })
                .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)?;
        if !eq_stable || !ep_stable {
            eq_seed = eq_protocol;
            ep_seed = ep_protocol;
            continue;
        }
        // A matching Base layout/structure is insufficient: constant deduplication can alter
        // actual VK commitments. Rebuild against FINAL (unsanitized) outer protocol values.
        let final_input = BootstrapInputs::new(&eq, &ep, &eq_protocol, &ep_protocol)?;
        let final_witness = final_input.witness(&template, &eq_protocol, &ep_protocol)?;
        let (final_inner_eq, final_inner_ep, _, _) =
            build_mint_authority_generation_pair(&eq, &ep, final_witness.clone())?;
        assert_vk!(
            &eq,
            final_inner_eq,
            inner_eq_vk,
            KagemushaPastaParityV1::Eq,
            "final inner mint authority"
        );
        assert_vk!(
            &ep,
            final_inner_ep,
            inner_ep_vk,
            KagemushaPastaParityV1::Ep,
            "final inner mint authority"
        );
        drop(final_inner_eq);
        drop(final_inner_ep);
        let final_prepared = prepare(final_witness)?;
        assert_vk!(
            &eq,
            final_prepared.eq_circuit,
            eq_vk,
            KagemushaPastaParityV1::Eq,
            "final compact mint authority"
        );
        assert_vk!(
            &ep,
            final_prepared.ep_circuit,
            ep_vk,
            KagemushaPastaParityV1::Ep,
            "final compact mint authority"
        );
        // The generator itself must produce/decide a real final compact bootstrap before
        // exporting keys. Dummy or unchecked inner proofs cannot qualify this artifact path.
        finish_transport(
            &eq,
            &ep,
            &eq_pk,
            &ep_pk,
            &eq_layout,
            &ep_layout,
            &eq_protocol,
            &ep_protocol,
            final_prepared,
        )?;
        if eq_protocol_digest == ep_protocol_digest {
            return Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
                "mint parity identities alias".to_owned(),
            ));
        }
        let (eq_parameters, eq_proving_key, eq_verifying_key) =
            build_generated_mint_parity(KagemushaPastaParityV1::Eq, &eq, &eq_pk, &eq_vk)?;
        let (ep_parameters, ep_proving_key, ep_verifying_key) =
            build_generated_mint_parity(KagemushaPastaParityV1::Ep, &ep, &ep_pk, &ep_vk)?;
        return Ok(KagemushaGeneratedMintAuthorityArtifactsV1 {
            eq_parameters,
            ep_parameters,
            eq_proving_key,
            ep_proving_key,
            eq_verifying_key,
            ep_verifying_key,
            eq_circuit_params: eq_layout,
            ep_circuit_params: ep_layout,
            inner_eq_proving_key,
            inner_ep_proving_key,
            inner_eq_verifying_key,
            inner_ep_verifying_key,
            inner_eq_circuit_params: inner_eq_layout,
            inner_ep_circuit_params: inner_ep_layout,
            eq_protocol_digest,
            ep_protocol_digest,
            genesis_roster_id: template.genesis_roster_id,
            release_id: template.release_id,
        });
    }
    Err(KagemushaArtifactGenerationErrorV1::CircuitBuild(
        "MintAuthority compact recursive key structure did not converge".to_owned(),
    ))
}

macro_rules! inner_vk_reader {
    ($name:ident, $curve:ty, $circuit:ty, $parity:expr) => {
        pub(super) fn $name(
            bytes: &[u8],
            layout: BaseCircuitParams,
        ) -> Result<VerifyingKey<$curve>, KagemushaArtifactGenerationErrorV1> {
            let mut cursor = Cursor::new(bytes);
            let key = VerifyingKey::read_checked::<_, $circuit>(
                &mut cursor,
                SerdeFormat::Processed,
                KAGEMUSHA_HALO2_K_V1,
                layout,
            )
            .map_err(|e| key_decode_error($parity, "inner mint-authority verifying key", e))?;
            ensure_cursor_consumed(
                $parity,
                "inner mint-authority verifying key",
                &cursor,
                bytes.len(),
            )?;
            if key.to_bytes(SerdeFormat::Processed) != bytes {
                return Err(key_decode_message(
                    $parity,
                    "inner mint-authority verifying key",
                    "processed encoding is non-canonical",
                ));
            }
            Ok(key)
        }
    };
}
inner_vk_reader!(
    read_eq_inner_mint_vk,
    EqAffine,
    KagemushaMintAuthorityEqCircuitV1,
    KagemushaPastaParityV1::Eq
);
inner_vk_reader!(
    read_ep_inner_mint_vk,
    EpAffine,
    KagemushaMintAuthorityEpCircuitV1,
    KagemushaPastaParityV1::Ep
);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_authority_generation_never_relabels_a_funded_or_rotating_witness() {
        assert!(require_bootstrap(KagemushaMintAuthorityStepV1::Bootstrap).is_ok());
        assert!(require_bootstrap(KagemushaMintAuthorityStepV1::Rotate).is_err());
        assert!(require_bootstrap(KagemushaMintAuthorityStepV1::FinalizedMint).is_err());
    }

    #[test]
    fn mint_authority_export_rejects_every_release_identity_substitution() {
        let eq = ReleaseIdentity {
            release: [1; 32],
            profile: [2; 32],
            manifest: [3; 32],
            genesis: [4; 32],
            protocol: [5; 32],
        };
        let ep = ReleaseIdentity {
            protocol: [6; 32],
            ..eq
        };
        let check = |a, b| {
            validate_release_identity(
                a,
                b,
                eq.release,
                eq.release,
                eq.genesis,
                eq.protocol,
                ep.protocol,
            )
        };
        assert!(check(eq, ep).is_ok());
        for index in 0..5 {
            for replacement in [[0; 32], [9; 32]] {
                let mut changed = eq;
                match index {
                    0 => changed.release = replacement,
                    1 => changed.profile = replacement,
                    2 => changed.manifest = replacement,
                    3 => changed.genesis = replacement,
                    _ => changed.protocol = replacement,
                }
                assert!(check(changed, ep).is_err());
                let mut changed_ep = changed;
                if index != 4 {
                    changed_ep.protocol = ep.protocol;
                }
                assert!(check(eq, changed_ep).is_err());
            }
        }
        assert!(
            check(
                eq,
                ReleaseIdentity {
                    protocol: eq.protocol,
                    ..ep
                }
            )
            .is_err()
        );
        for index in 0..5 {
            let mut values = [eq.release, eq.release, eq.genesis, eq.protocol, ep.protocol];
            values[index] = [9; 32];
            assert!(
                validate_release_identity(
                    eq, ep, values[0], values[1], values[2], values[3], values[4]
                )
                .is_err()
            );
        }
    }
}
