//! Joint inner/outer State topology planning for actual artifact generation.
//! Parser-only transcripts discover geometry; no proof or Native authority is admitted here.
use super::*;

/// Value-free layouts for all four actual ordinary State roles. Protocol values are data only.
/// The caller must replan the complete SHA witness, then generate and verify final real proofs.
pub struct KagemushaOrdinaryRecursiveStateLayoutV1 {
    /// Private Eq State carrier protocol descriptor.
    pub inner_eq: PlonkProtocol<EqAffine>,
    /// Private Ep State carrier protocol descriptor.
    pub inner_ep: PlonkProtocol<EpAffine>,
    /// Public Eq State transport protocol descriptor.
    pub outer_eq: PlonkProtocol<EqAffine>,
    /// Public Ep State transport protocol descriptor.
    pub outer_ep: PlonkProtocol<EpAffine>,
}

/// Generate the ordinary State family after planning both inner and outer protocols and
/// rebuilding the exact full SHA witness. These are artifact bytes, not a signed release,
/// Native financial owner, current clock, or wallet admission.
///
/// The input is the exact zero Bootstrap before its SHA-claim merge: `hash_claim` must be
/// absent and the successor histories must be the original Guard-complete histories.
/// An initial genuine SHA claim supplies readable parser geometry. Only its shape is used
/// during discovery. The final claim is newly proved from the same pre-claim histories
/// after every selected inner/outer protocol value has been bound.
///
/// # Errors
/// Refuses a monetary witness, offered parent, premerged SHA history, incompatible helper
/// material, unqualified production relation, resource failure, malformed proof, or a final
/// key identity that differs from the selected value. Key hashes are never retried to find
/// a cryptographic fixed point. Every returned artifact has passed real paired carrier and
/// transport proof generation, native verification, and complete-history IPA decisions.
pub fn generate_kagemusha_ordinary_recursive_state_artifacts_v1(
    template: KagemushaRecursiveStateGenerationWitnessV1<'_>,
    hash_eq: &KagemushaLoadedEqMintHashArtifactsV1,
    hash_ep: &KagemushaLoadedEpMintHashArtifactsV1,
    recovery_seed: &KagemushaRecoverySeedV1,
) -> Result<KagemushaGeneratedRecursiveStateArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    generate_ordinary_state_artifacts_with_construction_v1(
        template,
        hash_eq,
        hash_ep,
        recovery_seed,
        super::super::composite::RecursiveStateConstructionV1::Production,
    )
}

fn generate_ordinary_state_artifacts_with_construction_v1(
    template: KagemushaRecursiveStateGenerationWitnessV1<'_>,
    hash_eq: &KagemushaLoadedEqMintHashArtifactsV1,
    hash_ep: &KagemushaLoadedEpMintHashArtifactsV1,
    recovery_seed: &KagemushaRecoverySeedV1,
    construction: super::super::composite::RecursiveStateConstructionV1,
) -> Result<KagemushaGeneratedRecursiveStateArtifactsV1, KagemushaArtifactGenerationErrorV1> {
    let fail = KagemushaArtifactGenerationErrorV1::CircuitBuild;
    if template.hash_claim.is_some() {
        return Err(fail(
            "ordinary artifact workflow requires the original pre-claim histories".into(),
        ));
    }
    require_zero_bootstrap_template_v1(&template).map_err(fail)?;
    validate_loaded_typed_sha_pair_v1(hash_eq, hash_ep)?;
    if template.state.successor.release_id != hash_eq.release_id {
        return Err(fail(
            "ordinary artifact SHA helpers belong to another release".into(),
        ));
    }
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    // Preserve the original authenticated Guard-complete histories. A previous SHA merge
    // may never be used as the base of a replan, or the same claim could be counted twice.
    let base_eq = template.eq_successor_history;
    let base_ep = template.ep_successor_history;
    let initial_claim = prove_kagemusha_recursive_state_hash_claim_v1_with_construction(
        hash_eq,
        hash_ep,
        template.reborrow(),
        recovery_seed,
        construction,
    )?;
    let initial_eq_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        base_eq,
        &initial_claim.eq_complete_history,
        recovery_seed,
    )
    .map_err(|e| fail(e.to_string()))?;
    let initial_ep_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        base_ep,
        &initial_claim.ep_complete_history,
        recovery_seed,
    )
    .map_err(|e| fail(e.to_string()))?;
    let mut geometry = template.reborrow();
    geometry.hash_claim = Some(initial_claim.consumer_witness(
        hash_eq,
        hash_ep,
        initial_eq_merge.proof(),
        initial_ep_merge.proof(),
    )?);
    geometry.eq_successor_history = initial_eq_merge.successor();
    geometry.ep_successor_history = initial_ep_merge.successor();
    let layout = discover_ordinary_state_layout_with_construction_v1(&geometry, construction)
        .map_err(fail)?;
    let inner_eq_padding = dummy_ordinary_proof_bytes(
        &layout.inner_eq,
        EqAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Eq,
    )?;
    let inner_ep_padding = dummy_ordinary_proof_bytes(
        &layout.inner_ep,
        EpAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Ep,
    )?;
    let outer_eq_padding = dummy_ordinary_proof_bytes(
        &layout.outer_eq,
        EqAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Eq,
    )?;
    let outer_ep_padding = dummy_ordinary_proof_bytes(
        &layout.outer_ep,
        EpAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Ep,
    )?;
    let selected_inner_eq =
        native_parent_protocol_digest_v1(&layout.inner_eq, KagemushaPastaParityV1::Eq)
            .map_err(fail)?;
    let selected_inner_ep =
        native_parent_protocol_digest_v1(&layout.inner_ep, KagemushaPastaParityV1::Ep)
            .map_err(fail)?;
    let selected_outer_eq =
        native_parent_protocol_digest_v1(&layout.outer_eq, KagemushaPastaParityV1::Eq)
            .map_err(fail)?;
    let selected_outer_ep =
        native_parent_protocol_digest_v1(&layout.outer_ep, KagemushaPastaParityV1::Ep)
            .map_err(fail)?;
    let old_outer = template
        .ordinary_selection
        .as_ref()
        .and_then(|s| s.outer_parent)
        .ok_or_else(|| fail("ordinary zero Bootstrap outer template disappeared".into()))?;
    let mut exact = template.reborrow();
    exact.eq_parent_protocol = &layout.inner_eq;
    exact.ep_parent_protocol = &layout.inner_ep;
    exact.eq_parent_proof = &inner_eq_padding;
    exact.ep_parent_proof = &inner_ep_padding;
    exact.state.eq_protocol_digest = selected_inner_eq;
    exact.state.ep_protocol_digest = selected_inner_ep;
    exact.state.guard_eq_credential_audit = selected_outer_eq;
    exact.state.guard_ep_credential_audit = selected_outer_ep;
    exact
        .ordinary_selection
        .as_mut()
        .ok_or_else(|| fail("ordinary selection disappeared".into()))?
        .outer_parent = Some(KagemushaOrdinaryRecursiveOuterParentWitnessV1 {
        eq_protocol: &layout.outer_eq,
        ep_protocol: &layout.outer_ep,
        eq_proof: &outer_eq_padding,
        ep_proof: &outer_ep_padding,
        ..old_outer
    });
    exact.hash_claim = None;
    exact.eq_successor_history = base_eq;
    exact.ep_successor_history = base_ep;
    let final_claim = prove_kagemusha_recursive_state_hash_claim_v1_with_construction(
        hash_eq,
        hash_ep,
        exact.reborrow(),
        recovery_seed,
        construction,
    )?;
    let final_eq_merge = fold_kagemusha_eq_accumulators_v1(
        &eq,
        base_eq,
        &final_claim.eq_complete_history,
        recovery_seed,
    )
    .map_err(|e| fail(e.to_string()))?;
    let final_ep_merge = fold_kagemusha_ep_accumulators_v1(
        &ep,
        base_ep,
        &final_claim.ep_complete_history,
        recovery_seed,
    )
    .map_err(|e| fail(e.to_string()))?;
    exact.hash_claim = Some(final_claim.consumer_witness(
        hash_eq,
        hash_ep,
        final_eq_merge.proof(),
        final_ep_merge.proof(),
    )?);
    exact.eq_successor_history = final_eq_merge.successor();
    exact.ep_successor_history = final_ep_merge.successor();
    let generated = generate_kagemusha_recursive_state_artifacts_v1_with_construction(
        exact,
        recovery_seed,
        construction,
    )?;
    if generated.inner_eq_protocol_digest != selected_inner_eq
        || generated.inner_ep_protocol_digest != selected_inner_ep
        || generated.eq_protocol_digest != selected_outer_eq
        || generated.ep_protocol_digest != selected_outer_ep
    {
        return Err(fail(
            "ordinary artifact family changed a protocol value after its final SHA replan".into(),
        ));
    }
    Ok(generated)
}

fn require_zero_bootstrap_template_v1(
    template: &KagemushaRecursiveStateGenerationWitnessV1<'_>,
) -> Result<(), String> {
    if template.state.operation != KagemushaOperationV1::Bootstrap
        || template.state.predecessor.is_some()
        || template.state.successor.balance != 0
    {
        return Err("ordinary layout discovery requires the exact zero Bootstrap".into());
    }
    let outer = template
        .ordinary_selection
        .as_ref()
        .and_then(|s| s.outer_parent)
        .ok_or_else(|| {
            "ordinary layout discovery lacks the mandatory outer parent template".to_owned()
        })?;
    if outer.public_original.is_some() {
        return Err("zero Bootstrap has no offered parent public original".into());
    }
    Ok(())
}
/// Discover the full ordinary Bootstrap geometry before constructing its whole SHA witness.
/// Production refusal remains effective until actual full-family qualification. The returned
/// protocols cannot authenticate a release, key, State, clock or monetary effect.
/// # Errors
/// Refuses a nonzero Bootstrap, missing exact outer operands, unqualified production relation,
/// failed resource preflight, or a topology that does not stabilize within eight geometry passes.
pub fn discover_kagemusha_ordinary_recursive_state_layout_v1(
    template: &KagemushaRecursiveStateGenerationWitnessV1<'_>,
) -> Result<KagemushaOrdinaryRecursiveStateLayoutV1, KagemushaArtifactGenerationErrorV1> {
    discover_ordinary_state_layout_with_construction_v1(
        template,
        super::super::composite::RecursiveStateConstructionV1::Production,
    )
    .map_err(KagemushaArtifactGenerationErrorV1::CircuitBuild)
}
pub(super) fn discover_ordinary_state_layout_with_construction_v1(
    template: &KagemushaRecursiveStateGenerationWitnessV1<'_>,
    construction: super::super::composite::RecursiveStateConstructionV1,
) -> Result<KagemushaOrdinaryRecursiveStateLayoutV1, String> {
    require_zero_bootstrap_template_v1(template)?;
    let outer = template
        .ordinary_selection
        .as_ref()
        .and_then(|s| s.outer_parent)
        .ok_or_else(|| {
            "ordinary layout discovery lacks the mandatory outer parent template".to_owned()
        })?;
    if outer.public_original.is_some() {
        return Err("zero Bootstrap has no offered parent public original".into());
    }
    let eq = canonical_kagemusha_eq_parameters_v1();
    let ep = canonical_kagemusha_ep_parameters_v1();
    let mut inner_eq = template.eq_parent_protocol.clone();
    let mut inner_ep = template.ep_parent_protocol.clone();
    let mut outer_eq = outer.eq_protocol.clone();
    let mut outer_ep = outer.ep_protocol.clone();
    for _ in 0..8 {
        let ip_eq = dummy_ordinary_proof_bytes(
            &inner_eq,
            EqAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Eq,
        )
        .map_err(|e| e.to_string())?;
        let ip_ep = dummy_ordinary_proof_bytes(
            &inner_ep,
            EpAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Ep,
        )
        .map_err(|e| e.to_string())?;
        let op_eq = dummy_ordinary_proof_bytes(
            &outer_eq,
            EqAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Eq,
        )
        .map_err(|e| e.to_string())?;
        let op_ep = dummy_ordinary_proof_bytes(
            &outer_ep,
            EpAffine::generator().to_bytes().as_ref(),
            KagemushaPastaParityV1::Ep,
        )
        .map_err(|e| e.to_string())?;
        // The retained claim is a readable parser operand for key geometry only. Changed
        // protocol values intentionally make its message-root constraints unsatisfied; the
        // builder adds those equalities without accepting a proof or checking satisfiability.
        // No claimed message digest is overwritten to disguise that mismatch. The concrete
        // workflow above discards this operand and proves a fresh exact claim before any
        // real carrier/transport proof is generated under the selected family.
        let mut witness = template.reborrow();
        witness.eq_parent_protocol = &inner_eq;
        witness.ep_parent_protocol = &inner_ep;
        witness.eq_parent_proof = &ip_eq;
        witness.ep_parent_proof = &ip_ep;
        witness.state.eq_protocol_digest =
            native_parent_protocol_digest_v1(&inner_eq, KagemushaPastaParityV1::Eq)?;
        witness.state.ep_protocol_digest =
            native_parent_protocol_digest_v1(&inner_ep, KagemushaPastaParityV1::Ep)?;
        witness.state.guard_eq_credential_audit =
            native_parent_protocol_digest_v1(&outer_eq, KagemushaPastaParityV1::Eq)?;
        witness.state.guard_ep_credential_audit =
            native_parent_protocol_digest_v1(&outer_ep, KagemushaPastaParityV1::Ep)?;
        witness
            .ordinary_selection
            .as_mut()
            .ok_or_else(|| "ordinary selection missing".to_owned())?
            .outer_parent = Some(KagemushaOrdinaryRecursiveOuterParentWitnessV1 {
            eq_protocol: &outer_eq,
            ep_protocol: &outer_ep,
            eq_proof: &op_eq,
            ep_proof: &op_ep,
            ..outer
        });
        let state = witness.state.clone();
        let (eq_circuit, ep_circuit, _, _) =
            build_recursive_generation_pair_v1(&eq, &ep, witness, construction)
                .map_err(|e| e.to_string())?;
        let eq_vk = keygen_vk_with_helper_resource_preflight_consuming_v1(
            &eq,
            eq_circuit,
            KagemushaPastaParityV1::Eq,
            "ordinary State layout",
            "zero State inner layout discovery",
        )
        .map_err(|e| e.to_string())?;
        let ep_vk = keygen_vk_with_helper_resource_preflight_consuming_v1(
            &ep,
            ep_circuit,
            KagemushaPastaParityV1::Ep,
            "ordinary State layout",
            "zero State inner layout discovery",
        )
        .map_err(|e| e.to_string())?;
        let next_inner_eq = compile(
            &eq,
            &eq_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![recursive_public_instance_count()]),
        );
        let next_inner_ep = compile(
            &ep,
            &ep_vk,
            snark_verifier::system::halo2::Config::ipa()
                .with_num_instance(vec![recursive_public_instance_count()]),
        );
        drop(eq_vk);
        drop(ep_vk);
        halo2_proofs::release_allocator_slack();
        let (next_outer_eq, next_outer_ep) = discover_outer_layout_v1(
            &eq,
            &ep,
            &state,
            &next_inner_eq,
            &next_inner_ep,
            template.eq_successor_history,
            template.ep_successor_history,
        )?;
        let closed = recursive_state_parent_structure_matches_v1(
            &inner_eq,
            &next_inner_eq,
            KagemushaPastaParityV1::Eq,
        )
        .map_err(|e| e.to_string())?
            && recursive_state_parent_structure_matches_v1(
                &inner_ep,
                &next_inner_ep,
                KagemushaPastaParityV1::Ep,
            )
            .map_err(|e| e.to_string())?
            && recursive_state_parent_structure_matches_v1(
                &outer_eq,
                &next_outer_eq,
                KagemushaPastaParityV1::Eq,
            )
            .map_err(|e| e.to_string())?
            && recursive_state_parent_structure_matches_v1(
                &outer_ep,
                &next_outer_ep,
                KagemushaPastaParityV1::Ep,
            )
            .map_err(|e| e.to_string())?;
        inner_eq = next_inner_eq;
        inner_ep = next_inner_ep;
        outer_eq = next_outer_eq;
        outer_ep = next_outer_ep;
        if closed {
            return Ok(KagemushaOrdinaryRecursiveStateLayoutV1 {
                inner_eq,
                inner_ep,
                outer_eq,
                outer_ep,
            });
        }
    }
    Err("ordinary full State inner/outer topology failed bounded discovery; key digests are never iterated for equality".into())
}

#[allow(clippy::too_many_arguments)]
fn discover_outer_layout_v1(
    eq: &ParamsIPA<EqAffine>,
    ep: &ParamsIPA<EpAffine>,
    state: &KagemushaStateRelationWitnessV1,
    inner_eq: &PlonkProtocol<EqAffine>,
    inner_ep: &PlonkProtocol<EpAffine>,
    eq_history: &KagemushaEqAccumulatorV1,
    ep_history: &KagemushaEpAccumulatorV1,
) -> Result<(PlonkProtocol<EqAffine>, PlonkProtocol<EpAffine>), String> {
    let proof_eq = dummy_ordinary_proof_bytes(
        inner_eq,
        EqAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Eq,
    )
    .map_err(|e| e.to_string())?;
    let proof_ep = dummy_ordinary_proof_bytes(
        inner_ep,
        EpAffine::generator().to_bytes().as_ref(),
        KagemushaPastaParityV1::Ep,
    )
    .map_err(|e| e.to_string())?;
    let fold_eq = dummy_fold_proof_bytes(EqAffine::generator().to_bytes().as_ref());
    let fold_ep = dummy_fold_proof_bytes(EpAffine::generator().to_bytes().as_ref());
    let current_eq = recursive_public_instances::<Fp>(state, eq_history.as_bytes())
        .map_err(|e| e.to_string())?;
    let current_ep = recursive_public_instances::<Fq>(state, ep_history.as_bytes())
        .map_err(|e| e.to_string())?;
    let native_eq = eq_history.to_native().map_err(|e| e.to_string())?;
    let native_ep = ep_history.to_native().map_err(|e| e.to_string())?;
    let witness = KagemushaTransportDeciderWitnessV1 {
        eq: KagemushaTransportDeciderParityWitnessV1 {
            inner_protocol: inner_eq,
            inner_instances: &current_eq,
            inner_proof: &proof_eq,
            inner_history: &native_eq,
            inner_history_fold_proof: &fold_eq,
            outer_instances: &current_eq,
        },
        ep: KagemushaTransportDeciderParityWitnessV1 {
            inner_protocol: inner_ep,
            inner_instances: &current_ep,
            inner_proof: &proof_ep,
            inner_history: &native_ep,
            inner_history_fold_proof: &fold_ep,
            outer_instances: &current_ep,
        },
    };
    let (eq_circuit, ep_circuit, _, _) =
        build_kagemusha_transport_decider_pair_v1(eq, ep, witness).map_err(|e| e.to_string())?;
    let eq_vk = keygen_vk_with_helper_resource_preflight_consuming_v1(
        eq,
        eq_circuit,
        KagemushaPastaParityV1::Eq,
        "ordinary State layout",
        "zero State outer layout discovery",
    )
    .map_err(|e| e.to_string())?;
    let ep_vk = keygen_vk_with_helper_resource_preflight_consuming_v1(
        ep,
        ep_circuit,
        KagemushaPastaParityV1::Ep,
        "ordinary State layout",
        "zero State outer layout discovery",
    )
    .map_err(|e| e.to_string())?;
    let eq_protocol = compile(
        eq,
        &eq_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    let ep_protocol = compile(
        ep,
        &ep_vk,
        snark_verifier::system::halo2::Config::ipa()
            .with_num_instance(vec![recursive_public_instance_count()]),
    );
    Ok((eq_protocol, ep_protocol))
}
