//! Shared Terminal semantic assignment and its complete ordinary SHA queue.
//!
//! Production recursion and non-authorizing planning use the same assigned cells and ordered
//! transcript construction. The caller must still verify candidate and Guard proofs, fold all
//! histories, and constrain reciprocal audits; this module grants no proof or wallet authority.

use super::*;

/// Check both exact nested roles before assigning any shared semantic cell.
pub(super) fn validate_terminal_nested_public_shape_v1<C: CurveAffineExt>(
    candidate_protocol: &PlonkProtocol<C>,
    candidate_instances: &[Vec<C::ScalarExt>],
    terminal_guard_protocol: &PlonkProtocol<C>,
    terminal_guard_instances: &[Vec<C::ScalarExt>],
) -> Result<(), String> {
    if candidate_protocol.num_instance
        != [state_relation::PUBLIC_INSTANCE_COUNT + accumulator_limb_count()]
        || candidate_instances.len() != 1
        || candidate_instances[0].len()
            != state_relation::PUBLIC_INSTANCE_COUNT + accumulator_limb_count()
        || terminal_guard_protocol.num_instance != [GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1]
        || terminal_guard_instances.len() != 1
        || terminal_guard_instances[0].len() != GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1
    {
        return Err("terminal authorization nested proof has wrong fixed public shape".to_owned());
    }
    Ok(())
}

/// Borrowed inputs for precisely the semantic prefix of the Terminal graph.
#[derive(Clone, Copy)]
pub(super) struct TerminalSemanticInputsV1<'a, F: KagemushaPoseidonFieldV1> {
    pub(super) public: &'a KagemushaTerminalAuthorizationPublicInputsV1,
    pub(super) relation: KagemushaTerminalRelationV1,
    pub(super) private_transition: &'a KagemushaTerminalAuthorizationPrivateTransitionV1,
    pub(super) terminal_guard_relation: &'a KagemushaGuardBundleRelationWitnessV1,
    pub(super) enabled_hardware_profiles:
        &'a [DigestV1; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
    pub(super) candidate_instances: &'a [Vec<F>],
    pub(super) candidate_protocol_digest: DigestV1,
    pub(super) terminal_guard_protocol_digests: [DigestV1; 2],
    pub(super) successor_history: &'a [u8; super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
    pub(super) parity: KagemushaPastaParityV1,
}

/// Original cells retained across the move into the deferred loader.
pub(super) struct TerminalSemanticAssignmentV1<F: KagemushaPoseidonFieldV1> {
    pub(super) public_cells: Vec<AssignedValue<F>>,
    pub(super) history_cells: Vec<AssignedValue<F>>,
    pub(super) assigned_terminal_guard: KagemushaAssignedGuardBundleV1<F>,
    pub(super) candidate_instances: Vec<Vec<AssignedValue<F>>>,
    pub(super) sha_jobs: PastaSha256JobsV1<F>,
}

/// Assign the original complete semantic sequence before transferring its active pool.
///
/// The final candidate projection appends the last SHA job. Candidate cells must subsequently
/// be wrapped using `scalar_from_assigned`, never assigned a second time in the verifier.
pub(super) fn assign_terminal_semantic_pipeline_v1<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    range: &RangeChip<F>,
    inputs: TerminalSemanticInputsV1<'_, F>,
) -> Result<TerminalSemanticAssignmentV1<F>, String> {
    let public_cells = assign_public_prefix_v1(builder, range, inputs.public)?;
    let history_cells = assign_history_v1(builder, range, inputs.successor_history)?;
    builder.assigned_instances = vec![
        public_cells
            .iter()
            .copied()
            .chain(history_cells.iter().copied())
            .collect(),
    ];
    if builder.assigned_instances[0].len() != TERMINAL_AUTHORIZATION_PUBLIC_INSTANCE_COUNT_V1 {
        return Err("terminal authorization public instance has wrong fixed shape".to_owned());
    }
    let mut sha_jobs = PastaSha256JobsV1::default();
    let assigned_terminal_guard = constrain_guard_bundle_semantics_v1(
        builder,
        &mut sha_jobs,
        inputs.terminal_guard_relation,
    )?;
    constrain_terminal_relation_domain_v1(builder, range, inputs.relation);
    constrain_terminal_commit_semantics_v1(
        builder,
        &mut sha_jobs,
        &public_cells,
        inputs.private_transition,
        &assigned_terminal_guard,
    )?;
    let profile_enabled = builder.main(0).load_constant(F::ONE);
    constrain_enabled_hardware_profile_membership_v1(
        builder.main(0),
        range,
        profile_enabled,
        assigned_terminal_guard.hardware_profile_id,
        inputs.enabled_hardware_profiles,
    );
    let candidate_instances = inputs
        .candidate_instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| builder.main(0).load_witness(*value))
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let candidate_column = candidate_instances
        .first()
        .ok_or_else(|| "terminal authorization candidate public column is absent".to_owned())?;
    constrain_candidate_projection_cells_v1(
        builder.main(0),
        candidate_column,
        &public_cells,
        inputs.candidate_protocol_digest,
        inputs.parity,
        &mut sha_jobs,
    )?;
    constrain_candidate_terminal_guard_cells_v1(
        builder.main(0),
        candidate_column,
        &public_cells,
        &assigned_terminal_guard,
        inputs.terminal_guard_protocol_digests[0],
        inputs.terminal_guard_protocol_digests[1],
    )?;
    Ok(TerminalSemanticAssignmentV1 {
        public_cells,
        history_cells,
        assigned_terminal_guard,
        candidate_instances,
        sha_jobs,
    })
}

fn constrain_candidate_terminal_guard_cells_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    candidate: &[AssignedValue<F>],
    public_authorization: &[AssignedValue<F>],
    guard: &KagemushaAssignedGuardBundleV1<F>,
    terminal_guard_eq_protocol_digest: DigestV1,
    terminal_guard_ep_protocol_digest: DigestV1,
) -> Result<(), String> {
    if candidate.len() < state_relation::PUBLIC_INSTANCE_COUNT
        || public_authorization.len() != TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1
    {
        return Err("terminal Guard candidate prefix is truncated".to_owned());
    }
    for (offset, digest) in [
        (
            state_relation::public_instance::GUARD_EQ_PROTOCOL_LO,
            terminal_guard_eq_protocol_digest,
        ),
        (
            state_relation::public_instance::GUARD_EP_PROTOCOL_LO,
            terminal_guard_ep_protocol_digest,
        ),
    ] {
        let expected = crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(digest);
        for (actual, expected) in candidate[offset..offset + 2].iter().zip(expected) {
            let constant = ctx.load_constant(expected);
            ctx.constrain_equal(actual, &constant);
        }
    }
    for (index, expected) in [
        (state_relation::public_instance::OPERATION, guard.operation),
        (state_relation::public_instance::AMOUNT, guard.amount),
        (
            state_relation::public_instance::PROTOCOL_VERSION,
            guard.protocol_version,
        ),
        (
            state_relation::public_instance::POLICY_EPOCH,
            guard.policy_epoch,
        ),
        (
            state_relation::public_instance::ASSET_SCALE,
            guard.asset_scale,
        ),
    ] {
        ctx.constrain_equal(&candidate[index], &expected);
    }
    for (offset, expected) in [
        (
            state_relation::public_instance::PREDECESSOR_OUTER_LO,
            guard.predecessor_state,
        ),
        (
            state_relation::public_instance::SUCCESSOR_OUTER_LO,
            guard.successor_state,
        ),
        (
            state_relation::public_instance::RELEASE_LO,
            guard.release_id,
        ),
        (
            state_relation::public_instance::LIABILITY_POOL_LO,
            guard.liability_pool_id,
        ),
        (
            state_relation::public_instance::PEER_CREDIT_LO,
            guard.peer_credit_id,
        ),
        (
            state_relation::public_instance::RECIPIENT_ENCRYPTION_KEY_LO,
            guard.recipient_encryption_key_binding,
        ),
        (
            state_relation::public_instance::MINT_PROOF_BINDING_LO,
            guard.mint_finality_proof_binding_digest,
        ),
        (
            state_relation::public_instance::LIFECYCLE_LO,
            guard.lifecycle_binding_digest,
        ),
        (
            state_relation::public_instance::PREPARED_TRANSITION_LO,
            guard.prepared_transition_binding_digest,
        ),
        (
            state_relation::public_instance::PREDECESSOR_SUITE_LO,
            guard.predecessor_suite_id,
        ),
        (
            state_relation::public_instance::PREDECESSOR_VK_LO,
            guard.predecessor_vk_digest,
        ),
        (
            state_relation::public_instance::SUCCESSOR_SUITE_LO,
            guard.successor_suite_id,
        ),
        (
            state_relation::public_instance::SUCCESSOR_VK_LO,
            guard.successor_vk_digest,
        ),
        (
            state_relation::public_instance::ASSET_INCARNATION_LO,
            guard.asset_incarnation,
        ),
        (
            state_relation::public_instance::HARDWARE_PROFILE_LO,
            guard.hardware_profile_id,
        ),
        (
            state_relation::public_instance::NETWORK_LO,
            guard.network_id,
        ),
        (state_relation::public_instance::ASSET_LO, guard.asset_id),
    ] {
        for (actual, expected) in candidate[offset..offset + 2].iter().zip(expected) {
            ctx.constrain_equal(actual, &expected);
        }
    }

    Ok(())
}

fn constrain_candidate_projection_cells_v1<F: KagemushaPoseidonFieldV1>(
    ctx: &mut Context<F>,
    candidate: &[AssignedValue<F>],
    public_authorization: &[AssignedValue<F>],
    candidate_protocol_digest: DigestV1,
    parity: KagemushaPastaParityV1,
    sha_jobs: &mut PastaSha256JobsV1<F>,
) -> Result<(), String> {
    if candidate.len() < state_relation::PUBLIC_INSTANCE_COUNT
        || public_authorization.len() != TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1
    {
        return Err("terminal authorization candidate projection is truncated".to_owned());
    }
    let gate = halo2_base::gates::GateChip::default();
    let nullifier_low_zero = gate.is_zero(
        ctx,
        public_authorization[public_instance::TRANSITION_NULLIFIER_LO],
    );
    let nullifier_high_zero = gate.is_zero(
        ctx,
        public_authorization[public_instance::TRANSITION_NULLIFIER_LO + 1],
    );
    let nullifier_zero = gate.and(ctx, nullifier_low_zero, nullifier_high_zero);
    let terminal = gate.not(ctx, nullifier_zero);
    let scalar_bindings = [
        (
            state_relation::public_instance::OPERATION,
            public_instance::OPERATION,
        ),
        (
            state_relation::public_instance::AMOUNT,
            public_instance::AMOUNT,
        ),
        (
            state_relation::public_instance::PROTOCOL_VERSION,
            public_instance::PROTOCOL_VERSION,
        ),
    ];
    for (candidate_index, authorization_index) in scalar_bindings {
        ctx.constrain_equal(
            &candidate[candidate_index],
            &public_authorization[authorization_index],
        );
    }
    ctx.constrain_equal(
        &candidate[state_relation::public_instance::ASSET_SCALE],
        &public_authorization[public_instance::ASSET_SCALE],
    );
    ctx.constrain_equal(
        &candidate[state_relation::public_instance::POLICY_EPOCH],
        &public_authorization[public_instance::POLICY_EPOCH],
    );
    let always_digest_bindings = [
        (
            state_relation::public_instance::RELEASE_LO,
            public_instance::RELEASE_LO,
        ),
        (
            state_relation::public_instance::SUCCESSOR_SUITE_LO,
            public_instance::SUITE_LO,
        ),
        (
            state_relation::public_instance::SUCCESSOR_VK_LO,
            public_instance::VK_LO,
        ),
    ];
    for (candidate_offset, authorization_offset) in always_digest_bindings {
        for limb in 0..2 {
            ctx.constrain_equal(
                &candidate[candidate_offset + limb],
                &public_authorization[authorization_offset + limb],
            );
        }
    }
    let digest_bindings = [
        (
            state_relation::public_instance::TRANSPORT_LO,
            public_instance::SEMANTIC_LO,
        ),
        (
            state_relation::public_instance::LIFECYCLE_LO,
            public_instance::LIFECYCLE_LO,
        ),
        (
            state_relation::public_instance::ASSET_INCARNATION_LO,
            public_instance::ASSET_INCARNATION_LO,
        ),
        (
            state_relation::public_instance::LIABILITY_POOL_LO,
            public_instance::LIABILITY_POOL_LO,
        ),
        (
            state_relation::public_instance::NETWORK_LO,
            public_instance::NETWORK_LO,
        ),
        (
            state_relation::public_instance::ASSET_LO,
            public_instance::ASSET_LO,
        ),
    ];
    for (candidate_offset, authorization_offset) in digest_bindings {
        for limb in 0..2 {
            ctx.constrain_equal(
                &candidate[candidate_offset + limb],
                &public_authorization[authorization_offset + limb],
            );
        }
    }
    for limb in 0..2 {
        ctx.constrain_equal(
            &candidate[state_relation::public_instance::HARDWARE_PROFILE_LO + limb],
            &public_authorization[public_instance::HARDWARE_PROFILE_LO + limb],
        );
    }

    let protocol_offset = match parity {
        KagemushaPastaParityV1::Eq => state_relation::public_instance::EQ_PROTOCOL_LO,
        KagemushaPastaParityV1::Ep => state_relation::public_instance::EP_PROTOCOL_LO,
    };
    let expected_protocol =
        crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(candidate_protocol_digest);
    for (candidate_limb, expected) in candidate[protocol_offset..protocol_offset + 2]
        .iter()
        .zip(expected_protocol)
    {
        let constant = ctx.load_constant(expected);
        ctx.constrain_equal(candidate_limb, &constant);
    }

    let mut message = constant_bytes(CANDIDATE_BINDING_DOMAIN_V1);
    for (index, value) in candidate[..state_relation::PUBLIC_INSTANCE_COUNT]
        .iter()
        .enumerate()
    {
        if matches!(
            index,
            state_relation::public_instance::PREDECESSOR_STATE
                | state_relation::public_instance::SUCCESSOR_STATE
        ) {
            message.extend(constant_bytes(&[0_u8; 16]));
            continue;
        }
        let bits = PastaSha256BitV1::decompose(ctx, &gate, *value, 128);
        for byte_bits in bits.chunks_exact(8) {
            message.push(PastaSha256ByteV1::from_bits_le(ctx, &gate, byte_bits));
        }
    }
    let digest = hash(ctx, sha_jobs, message)?;
    let digest = digest_limbs_assigned(ctx, &digest);
    for (actual, expected) in digest
        .into_iter()
        .zip(&public_authorization[public_instance::CANDIDATE_LO..][..2])
    {
        let difference = gate.sub(ctx, actual, *expected);
        let selected = gate.mul(ctx, terminal, difference);
        gate.assert_is_const(ctx, &selected, &F::ZERO);
    }
    Ok(())
}

/// Actual compiled roles and columns required for proof-free semantic planning.
#[derive(Clone, Copy)]
pub(crate) struct TerminalSemanticPlanParityV1<'a, C: CurveAffineExt> {
    pub(crate) candidate_protocol: &'a PlonkProtocol<C>,
    pub(crate) candidate_instances: &'a [Vec<C::ScalarExt>],
    pub(crate) terminal_guard_protocol: &'a PlonkProtocol<C>,
    pub(crate) terminal_guard_instances: &'a [Vec<C::ScalarExt>],
    pub(crate) successor_history: &'a [u8; super::super::KAGEMUSHA_HISTORY_ACCUMULATOR_BYTES_V1],
}

/// Borrowed canonical material without candidate, Guard or fold proof bytes.
#[derive(Clone, Copy)]
pub(crate) struct TerminalSemanticPlanInputsV1<'a> {
    pub(crate) public: &'a KagemushaTerminalAuthorizationPublicInputsV1,
    pub(crate) private_transition: &'a KagemushaTerminalAuthorizationPrivateTransitionV1,
    pub(crate) terminal_guard_relation: &'a KagemushaGuardBundleRelationWitnessV1,
    pub(crate) enabled_hardware_profiles:
        &'a [DigestV1; TERMINAL_AUTHORIZATION_ENABLED_PROFILE_SLOTS_V1],
    pub(crate) eq: TerminalSemanticPlanParityV1<'a, EqAffine>,
    pub(crate) ep: TerminalSemanticPlanParityV1<'a, EpAffine>,
}

/// Ephemeral complete canonical queues; these messages have no proof or wallet authority.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct TerminalSemanticShaPlanV1 {
    pub(crate) eq_messages: Vec<Vec<u8>>,
    pub(crate) ep_messages: Vec<Vec<u8>>,
    pub(crate) job_block_counts: Vec<u32>,
}

/// Capture both complete production semantic queues, dropping each graph before the next.
///
/// The caller cannot choose role digests: they are derived from the actual compiled protocols.
/// This produces only the typed claim's preimages; the Terminal consumer must authenticate the
/// generated proof, both complete histories and the reciprocal carrier tail against its own queue.
pub(crate) fn plan_terminal_semantic_sha_v1(
    inputs: TerminalSemanticPlanInputsV1<'_>,
) -> Result<TerminalSemanticShaPlanV1, String> {
    inputs.public.validate()?;
    validate_enabled_hardware_profiles_v1(inputs.enabled_hardware_profiles)?;
    validate_terminal_nested_public_shape_v1(
        inputs.eq.candidate_protocol,
        inputs.eq.candidate_instances,
        inputs.eq.terminal_guard_protocol,
        inputs.eq.terminal_guard_instances,
    )?;
    validate_terminal_nested_public_shape_v1(
        inputs.ep.candidate_protocol,
        inputs.ep.candidate_instances,
        inputs.ep.terminal_guard_protocol,
        inputs.ep.terminal_guard_instances,
    )?;
    let guard_protocols = [
        native_parent_protocol_digest_v1(
            inputs.eq.terminal_guard_protocol,
            KagemushaPastaParityV1::Eq,
        )?,
        native_parent_protocol_digest_v1(
            inputs.ep.terminal_guard_protocol,
            KagemushaPastaParityV1::Ep,
        )?,
    ];
    validate_candidate_guard_protocol_binding_v1(
        inputs.eq.candidate_instances,
        guard_protocols[0],
        guard_protocols[1],
    )?;
    validate_candidate_guard_protocol_binding_v1(
        inputs.ep.candidate_instances,
        guard_protocols[0],
        guard_protocols[1],
    )?;
    validate_terminal_private_semantics_v1(
        inputs.public,
        inputs.private_transition,
        inputs.terminal_guard_relation,
        inputs.enabled_hardware_profiles,
    )?;
    fn half<C>(
        inputs: &TerminalSemanticPlanInputsV1<'_>,
        parity_inputs: &TerminalSemanticPlanParityV1<'_, C>,
        parity: KagemushaPastaParityV1,
        guard_protocols: [DigestV1; 2],
    ) -> Result<(Vec<Vec<u8>>, Vec<u32>), String>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: KagemushaPoseidonFieldV1,
    {
        let candidate_protocol_digest =
            native_parent_protocol_digest_v1(parity_inputs.candidate_protocol, parity)?;
        let candidate = &parity_inputs.candidate_instances[0];
        let candidate_digest = canonical_terminal_authorization_candidate_digest_v1(&[candidate
            [..state_relation::PUBLIC_INSTANCE_COUNT]
            .to_vec()])?;
        let protocol_offset = match parity {
            KagemushaPastaParityV1::Eq => state_relation::public_instance::EQ_PROTOCOL_LO,
            KagemushaPastaParityV1::Ep => state_relation::public_instance::EP_PROTOCOL_LO,
        };
        if candidate_digest != inputs.public.candidate_envelope_digest
            || candidate[protocol_offset..protocol_offset + 2]
                != crate::zk::kagemusha_v1_poseidon::digest_limbs::<C::ScalarExt>(
                    candidate_protocol_digest,
                )
        {
            return Err(
                "terminal SHA candidate differs from its bound digest or protocol".to_owned(),
            );
        }
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(super::super::KAGEMUSHA_RECURSION_IPA_K_V1 as usize)
            .use_lookup_bits((super::super::KAGEMUSHA_RECURSION_IPA_K_V1 - 1) as usize)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let semantic_inputs = TerminalSemanticInputsV1 {
            public: inputs.public,
            relation: KagemushaTerminalRelationV1::TerminalAuthorization,
            private_transition: inputs.private_transition,
            terminal_guard_relation: inputs.terminal_guard_relation,
            enabled_hardware_profiles: inputs.enabled_hardware_profiles,
            candidate_instances: parity_inputs.candidate_instances,
            candidate_protocol_digest,
            terminal_guard_protocol_digests: guard_protocols,
            successor_history: parity_inputs.successor_history,
            parity,
        };
        // The retained old production route is test-only and assigns neither through this
        // helper nor through the converted candidate helpers. Drop its graph before this one.
        #[cfg(test)]
        let legacy = tests::legacy_semantic_snapshot::<C>(semantic_inputs)?;
        let assignment =
            assign_terminal_semantic_pipeline_v1(&mut builder, &range, semantic_inputs)?;
        #[cfg(test)]
        if legacy
            != tests::semantic_snapshot(
                &builder,
                &assignment.sha_jobs,
                &assignment.candidate_instances,
            )?
        {
            return Err(
                "terminal shared semantic assignment differs from the retained original geometry"
                    .to_owned(),
            );
        }
        let claims = assignment.sha_jobs.claim_jobs()?;
        let messages = assignment.sha_jobs.canonical_messages()?;
        if claims.len() != 26 || messages.len() != claims.len() {
            return Err("terminal SHA queue must contain its complete 26 assigned jobs".to_owned());
        }
        let mut blocks = Vec::with_capacity(claims.len());
        let mut total_blocks = 0_usize;
        for (claim, message) in claims.iter().zip(&messages) {
            if claim.message.len() != message.len() {
                return Err("terminal SHA message differs from the assigned queue shape".to_owned());
            }
            let suffix = crate::zk::pasta_sha256_table8::canonical_padding_suffix(message.len())
                .ok_or_else(|| "terminal SHA message length is not encodable".to_owned())?;
            let padded = message
                .len()
                .checked_add(suffix.len())
                .ok_or_else(|| "terminal SHA padded length overflow".to_owned())?;
            let count = padded / crate::zk::pasta_sha256_table8::BLOCK_BYTE_SIZE;
            blocks
                .push(u32::try_from(count).map_err(|_| {
                    "terminal SHA message compression count exceeds u32".to_owned()
                })?);
            total_blocks = total_blocks
                .checked_add(count)
                .ok_or_else(|| "terminal SHA compression count overflow".to_owned())?;
        }
        if total_blocks != assignment.sha_jobs.compression_blocks()? {
            return Err(
                "terminal SHA compression inventory differs from its assigned queue".to_owned(),
            );
        }
        Ok((messages, blocks))
    }
    let (eq_messages, eq_blocks) = half(
        &inputs,
        &inputs.eq,
        KagemushaPastaParityV1::Eq,
        guard_protocols,
    )?;
    let (ep_messages, ep_blocks) = half(
        &inputs,
        &inputs.ep,
        KagemushaPastaParityV1::Ep,
        guard_protocols,
    )?;
    if eq_messages.len() != ep_messages.len()
        || eq_blocks != ep_blocks
        || eq_messages
            .iter()
            .map(Vec::len)
            .ne(ep_messages.iter().map(Vec::len))
    {
        return Err(
            "terminal paired semantic SHA queues have different job/block shapes".to_owned(),
        );
    }
    Ok(TerminalSemanticShaPlanV1 {
        eq_messages,
        ep_messages,
        job_block_counts: eq_blocks,
    })
}

#[cfg(test)]
#[path = "terminal_semantic_pipeline_tests.rs"]
mod tests;
