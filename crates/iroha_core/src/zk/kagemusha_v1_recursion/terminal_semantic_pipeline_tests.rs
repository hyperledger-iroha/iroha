//! Retained pre-refactor allocation oracle and exact candidate-cell regression tests.
//!
//! These tests compare Base assignments, selectors, constant/copy/lookup edges and original SHA
//! cells. Neither a matching assignment nor a Base mock supplies a nested proof or authority.

use super::*;
use halo2_base::ContextCell;
use halo2_proofs::{dev::MockProver, poly::commitment::ParamsProver as _};

/// Compact ordered assignment fingerprint; private transcript bytes never enter log output.
#[derive(PartialEq, Eq)]
pub(super) struct SemanticSnapshotV1 {
    digest: DigestV1,
}

pub(super) fn semantic_snapshot<F: KagemushaPoseidonFieldV1>(
    builder: &BaseCircuitBuilder<F>,
    jobs: &PastaSha256JobsV1<F>,
    candidate_instances: &[Vec<AssignedValue<F>>],
) -> Result<SemanticSnapshotV1, String> {
    fn bytes(hash: &mut Sha256, bytes: &[u8]) {
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    fn cell(hash: &mut Sha256, cell: ContextCell) {
        bytes(hash, cell.type_id().as_bytes());
        hash.update((cell.context_id() as u64).to_le_bytes());
        hash.update((cell.offset() as u64).to_le_bytes());
    }
    fn assigned<F: KagemushaPoseidonFieldV1>(hash: &mut Sha256, value: &AssignedValue<F>) {
        cell(hash, value.cell.expect("retained semantic assignment"));
        bytes(
            hash,
            ff::PrimeField::to_repr(&value.value.evaluate()).as_ref(),
        );
    }
    let mut hash = Sha256::new();
    bytes(&mut hash, format!("{:?}", builder.config_params).as_bytes());
    for phase in &builder.core().phase_manager {
        hash.update((phase.threads.len() as u64).to_le_bytes());
        for ctx in &phase.threads {
            bytes(&mut hash, ctx.tag().0.as_bytes());
            hash.update((ctx.tag().1 as u64).to_le_bytes());
            hash.update((ctx.advice_len() as u64).to_le_bytes());
            for offset in 0..ctx.advice_len() {
                let offset = isize::try_from(offset).expect("retained advice offset fits isize");
                let value = ctx.get(offset).value;
                bytes(
                    &mut hash,
                    ff::PrimeField::to_repr(&value.evaluate()).as_ref(),
                );
            }
            bytes(
                &mut hash,
                &ctx.selector
                    .iter()
                    .copied()
                    .map(u8::from)
                    .collect::<Vec<_>>(),
            );
        }
    }
    let manager = builder.core().copy_manager.lock().unwrap();
    hash.update((manager.advice_equalities.len() as u64).to_le_bytes());
    for (a, b) in &manager.advice_equalities {
        cell(&mut hash, *a);
        cell(&mut hash, *b);
    }
    hash.update((manager.constant_equalities.len() as u64).to_le_bytes());
    for (value, target) in manager.constant_equalities.iter() {
        bytes(&mut hash, ff::PrimeField::to_repr(value).as_ref());
        cell(&mut hash, *target);
    }
    drop(manager);
    for manager in builder.lookup_manager() {
        let lookups = manager.cells_to_lookup.lock().unwrap();
        hash.update((lookups.len() as u64).to_le_bytes());
        for (tag, rows) in lookups.iter() {
            bytes(&mut hash, tag.0.as_bytes());
            hash.update((tag.1 as u64).to_le_bytes());
            hash.update((rows.len() as u64).to_le_bytes());
            for row in rows {
                for value in row {
                    assigned(&mut hash, value);
                }
            }
        }
    }
    for columns in [&builder.assigned_instances[..], candidate_instances] {
        hash.update((columns.len() as u64).to_le_bytes());
        for column in columns {
            hash.update((column.len() as u64).to_le_bytes());
            for value in column {
                assigned(&mut hash, value);
            }
        }
    }
    let messages = jobs.canonical_messages()?;
    hash.update((messages.len() as u64).to_le_bytes());
    for (claim, message) in jobs.claim_jobs()?.iter().zip(messages) {
        bytes(&mut hash, &message);
        for byte in claim.message {
            if let Some(value) = byte.assigned() {
                hash.update([1]);
                assigned(&mut hash, &value);
            } else {
                hash.update([0]);
            }
        }
        for value in claim.output_words {
            assigned(&mut hash, value);
        }
    }
    Ok(SemanticSnapshotV1 {
        digest: hash.finalize().into(),
    })
}

/// The pre-refactor production sequence, retained as an independent assignment oracle.
/// It intentionally uses the old loader-shaped helpers below, never the shared pipeline.
pub(super) fn legacy_semantic_snapshot<C>(
    inputs: TerminalSemanticInputsV1<'_, C::ScalarExt>,
) -> Result<SemanticSnapshotV1, String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(super::super::super::KAGEMUSHA_RECURSION_IPA_K_V1 as usize)
        .use_lookup_bits((super::super::super::KAGEMUSHA_RECURSION_IPA_K_V1 - 1) as usize)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let public_cells = assign_public_prefix_v1(&mut builder, &range, inputs.public)?;
    let history_cells = assign_history_v1(&mut builder, &range, inputs.successor_history)?;
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
        &mut builder,
        &mut sha_jobs,
        inputs.terminal_guard_relation,
    )?;
    constrain_terminal_relation_domain_v1(&mut builder, &range, inputs.relation);
    constrain_terminal_commit_semantics_v1(
        &mut builder,
        &mut sha_jobs,
        &public_cells,
        inputs.private_transition,
        &assigned_terminal_guard,
    )?;
    let profile_enabled = builder.main(0).load_constant(C::ScalarExt::ONE);
    constrain_enabled_hardware_profile_membership_v1(
        builder.main(0),
        &range,
        profile_enabled,
        assigned_terminal_guard.hardware_profile_id,
        inputs.enabled_hardware_profiles,
    );
    let candidate_protocol_digest = inputs.candidate_protocol_digest;
    let (coordinate, scalar_integer) = deferred_field_chips_v1::<C>(&range);
    let loader = deferred_loader_v1::<C>(&mut builder, &coordinate, &scalar_integer);
    let candidate_instances = assign_nested_instances_v1(&loader, inputs.candidate_instances);
    let candidate_column = candidate_instances
        .first()
        .ok_or_else(|| "terminal authorization candidate public column is absent".to_owned())?;
    legacy_candidate_projection_v1(
        &loader,
        candidate_column,
        &public_cells,
        candidate_protocol_digest,
        inputs.parity,
        &mut sha_jobs,
    )?;
    legacy_candidate_terminal_guard_binding_v1(
        &loader,
        candidate_column,
        &public_cells,
        &assigned_terminal_guard,
        inputs.terminal_guard_protocol_digests[0],
        inputs.terminal_guard_protocol_digests[1],
    )?;

    let assigned_candidate_instances = candidate_instances
        .iter()
        .map(|column| {
            column
                .iter()
                .map(|value| *value.assigned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    *builder.pool(0) = loader.take_ctx();
    semantic_snapshot(&builder, &sha_jobs, &assigned_candidate_instances)
}

fn legacy_candidate_terminal_guard_binding_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    candidate: &[DeferredScalar<'chip, C>],
    public_authorization: &[AssignedValue<C::ScalarExt>],
    guard: &KagemushaAssignedGuardBundleV1<C::ScalarExt>,
    terminal_guard_eq_protocol_digest: DigestV1,
    terminal_guard_ep_protocol_digest: DigestV1,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
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
        let expected = crate::zk::kagemusha_v1_poseidon::digest_limbs::<C::ScalarExt>(digest);
        for (actual, expected) in candidate[offset..offset + 2].iter().zip(expected) {
            let constant = loader.ctx_mut().main().load_constant(expected);
            loader
                .ctx_mut()
                .main()
                .constrain_equal(&actual.assigned(), &constant);
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
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&candidate[index].assigned(), &expected);
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
            loader
                .ctx_mut()
                .main()
                .constrain_equal(&actual.assigned(), &expected);
        }
    }

    Ok(())
}

fn legacy_candidate_projection_v1<'chip, C>(
    loader: &DeferredLoader<'chip, C>,
    candidate: &[DeferredScalar<'chip, C>],
    public_authorization: &[AssignedValue<C::ScalarExt>],
    candidate_protocol_digest: DigestV1,
    parity: KagemushaPastaParityV1,
    sha_jobs: &mut PastaSha256JobsV1<C::ScalarExt>,
) -> Result<(), String>
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    if candidate.len() < state_relation::PUBLIC_INSTANCE_COUNT
        || public_authorization.len() != TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1
    {
        return Err("terminal authorization candidate projection is truncated".to_owned());
    }
    let gate = halo2_base::gates::GateChip::default();
    let nullifier_low_zero = gate.is_zero(
        loader.ctx_mut().main(),
        public_authorization[public_instance::TRANSITION_NULLIFIER_LO],
    );
    let nullifier_high_zero = gate.is_zero(
        loader.ctx_mut().main(),
        public_authorization[public_instance::TRANSITION_NULLIFIER_LO + 1],
    );
    let nullifier_zero = gate.and(
        loader.ctx_mut().main(),
        nullifier_low_zero,
        nullifier_high_zero,
    );
    let terminal = gate.not(loader.ctx_mut().main(), nullifier_zero);
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
        loader.ctx_mut().main().constrain_equal(
            &candidate[candidate_index].assigned(),
            &public_authorization[authorization_index],
        );
    }
    loader.ctx_mut().main().constrain_equal(
        &candidate[state_relation::public_instance::ASSET_SCALE].assigned(),
        &public_authorization[public_instance::ASSET_SCALE],
    );
    loader.ctx_mut().main().constrain_equal(
        &candidate[state_relation::public_instance::POLICY_EPOCH].assigned(),
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
            loader.ctx_mut().main().constrain_equal(
                &candidate[candidate_offset + limb].assigned(),
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
            loader.ctx_mut().main().constrain_equal(
                &candidate[candidate_offset + limb].assigned(),
                &public_authorization[authorization_offset + limb],
            );
        }
    }
    for limb in 0..2 {
        loader.ctx_mut().main().constrain_equal(
            &candidate[state_relation::public_instance::HARDWARE_PROFILE_LO + limb].assigned(),
            &public_authorization[public_instance::HARDWARE_PROFILE_LO + limb],
        );
    }

    let protocol_offset = match parity {
        KagemushaPastaParityV1::Eq => state_relation::public_instance::EQ_PROTOCOL_LO,
        KagemushaPastaParityV1::Ep => state_relation::public_instance::EP_PROTOCOL_LO,
    };
    let expected_protocol =
        crate::zk::kagemusha_v1_poseidon::digest_limbs::<C::ScalarExt>(candidate_protocol_digest);
    for (candidate_limb, expected) in candidate[protocol_offset..protocol_offset + 2]
        .iter()
        .zip(expected_protocol)
    {
        let constant = loader.ctx_mut().main().load_constant(expected);
        loader
            .ctx_mut()
            .main()
            .constrain_equal(&candidate_limb.assigned(), &constant);
    }

    let mut ctx = loader.ctx_mut();
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
        let bits = PastaSha256BitV1::decompose(ctx.main(), &gate, *value.assigned(), 128);
        for byte_bits in bits.chunks_exact(8) {
            message.push(PastaSha256ByteV1::from_bits_le(
                ctx.main(),
                &gate,
                byte_bits,
            ));
        }
    }
    let digest = hash(ctx.main(), sha_jobs, message)?;
    let digest = digest_limbs_assigned(ctx.main(), &digest);
    for (actual, expected) in digest
        .into_iter()
        .zip(&public_authorization[public_instance::CANDIDATE_LO..][..2])
    {
        let difference = gate.sub(ctx.main(), actual, *expected);
        let selected = gate.mul(ctx.main(), terminal, difference);
        gate.assert_is_const(ctx.main(), &selected, &C::ScalarExt::ZERO);
    }
    Ok(())
}

fn guard_cells<F: KagemushaPoseidonFieldV1>(
    builder: &mut BaseCircuitBuilder<F>,
    candidate: &[F],
) -> KagemushaAssignedGuardBundleV1<F> {
    let zero = builder.main(0).load_witness(F::ZERO);
    let mut at = |offset: usize| builder.main(0).load_witness(candidate[offset]);
    KagemushaAssignedGuardBundleV1 {
        guard_digest: constant_bytes(&[0; 32]).try_into().unwrap(),
        credential_digests: std::array::from_fn(|_| constant_bytes(&[0; 32]).try_into().unwrap()),
        credential_issuance_digests: std::array::from_fn(|_| {
            constant_bytes(&[0; 32]).try_into().unwrap()
        }),
        credential_device_public_keys: std::array::from_fn(|_| Vec::new()),
        protocol_version: at(state_relation::public_instance::PROTOCOL_VERSION),
        predecessor_suite_id: [
            at(state_relation::public_instance::PREDECESSOR_SUITE_LO),
            at(state_relation::public_instance::PREDECESSOR_SUITE_LO + 1),
        ],
        predecessor_vk_digest: [
            at(state_relation::public_instance::PREDECESSOR_VK_LO),
            at(state_relation::public_instance::PREDECESSOR_VK_LO + 1),
        ],
        successor_suite_id: [
            at(state_relation::public_instance::SUCCESSOR_SUITE_LO),
            at(state_relation::public_instance::SUCCESSOR_SUITE_LO + 1),
        ],
        successor_vk_digest: [
            at(state_relation::public_instance::SUCCESSOR_VK_LO),
            at(state_relation::public_instance::SUCCESSOR_VK_LO + 1),
        ],
        operation: at(state_relation::public_instance::OPERATION),
        amount: at(state_relation::public_instance::AMOUNT),
        peer_credit_id: [
            at(state_relation::public_instance::PEER_CREDIT_LO),
            at(state_relation::public_instance::PEER_CREDIT_LO + 1),
        ],
        recipient_encryption_key_binding: [
            at(state_relation::public_instance::RECIPIENT_ENCRYPTION_KEY_LO),
            at(state_relation::public_instance::RECIPIENT_ENCRYPTION_KEY_LO + 1),
        ],
        mint_finality_proof_binding_digest: [
            at(state_relation::public_instance::MINT_PROOF_BINDING_LO),
            at(state_relation::public_instance::MINT_PROOF_BINDING_LO + 1),
        ],
        predecessor_release_id: [zero; 2],
        release_id: [
            at(state_relation::public_instance::RELEASE_LO),
            at(state_relation::public_instance::RELEASE_LO + 1),
        ],
        network_id: [
            at(state_relation::public_instance::NETWORK_LO),
            at(state_relation::public_instance::NETWORK_LO + 1),
        ],
        asset_id: [
            at(state_relation::public_instance::ASSET_LO),
            at(state_relation::public_instance::ASSET_LO + 1),
        ],
        asset_incarnation: [
            at(state_relation::public_instance::ASSET_INCARNATION_LO),
            at(state_relation::public_instance::ASSET_INCARNATION_LO + 1),
        ],
        asset_scale: at(state_relation::public_instance::ASSET_SCALE),
        liability_pool_id: [
            at(state_relation::public_instance::LIABILITY_POOL_LO),
            at(state_relation::public_instance::LIABILITY_POOL_LO + 1),
        ],
        hardware_profile_id: [
            at(state_relation::public_instance::HARDWARE_PROFILE_LO),
            at(state_relation::public_instance::HARDWARE_PROFILE_LO + 1),
        ],
        policy_epoch: at(state_relation::public_instance::POLICY_EPOCH),
        lane_id: [zero; 2],
        predecessor_state: [
            at(state_relation::public_instance::PREDECESSOR_OUTER_LO),
            at(state_relation::public_instance::PREDECESSOR_OUTER_LO + 1),
        ],
        successor_state: [
            at(state_relation::public_instance::SUCCESSOR_OUTER_LO),
            at(state_relation::public_instance::SUCCESSOR_OUTER_LO + 1),
        ],
        predecessor_nonce: [zero; 2],
        successor_nonce: [zero; 2],
        predecessor_sequence: zero,
        successor_sequence: zero,
        predecessor_generation: zero,
        successor_generation: zero,
        predecessor_epoch: [zero; 2],
        successor_epoch: [zero; 2],
        predecessor_key: [zero; 2],
        successor_key: [zero; 2],
        predecessor_policy: [zero; 2],
        successor_policy: [zero; 2],
        journal_before: zero,
        journal_after: zero,
        lifecycle_binding_digest: [
            at(state_relation::public_instance::LIFECYCLE_LO),
            at(state_relation::public_instance::LIFECYCLE_LO + 1),
        ],
        prepared_transition_binding_digest: [
            at(state_relation::public_instance::PREPARED_TRANSITION_LO),
            at(state_relation::public_instance::PREPARED_TRANSITION_LO + 1),
        ],
        terminal_commit_binding_digest: [zero; 2],
        sender_one_time_authorization_digest: [zero; 2],
        receive_credit_binding_digest: [zero; 2],
        transition_intent: [zero; 2],
        transition_effect: [zero; 2],
        recovery_record: [zero; 2],
        durable_inbox_effect: [zero; 2],
        durable_outbox_effect: [zero; 2],
    }
}

fn projection_values<F: KagemushaPoseidonFieldV1>() -> (Vec<F>, Vec<F>) {
    // Independent projection fixture only: no funds, complete Guard relation or proof authority.
    let mut candidate = (0..state_relation::PUBLIC_INSTANCE_COUNT + accumulator_limb_count())
        .map(|index| F::from(index as u64 + 1))
        .collect::<Vec<_>>();
    for (offset, digest) in [
        (state_relation::public_instance::EQ_PROTOCOL_LO, [23; 32]),
        (state_relation::public_instance::EP_PROTOCOL_LO, [29; 32]),
        (
            state_relation::public_instance::GUARD_EQ_PROTOCOL_LO,
            [17; 32],
        ),
        (
            state_relation::public_instance::GUARD_EP_PROTOCOL_LO,
            [19; 32],
        ),
    ] {
        candidate[offset..offset + 2]
            .copy_from_slice(&crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(digest));
    }
    let mut public = vec![F::ZERO; TERMINAL_AUTHORIZATION_PUBLIC_PREFIX_COUNT_V1];
    for (source, target) in [
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
        (
            state_relation::public_instance::ASSET_SCALE,
            public_instance::ASSET_SCALE,
        ),
        (
            state_relation::public_instance::POLICY_EPOCH,
            public_instance::POLICY_EPOCH,
        ),
    ] {
        public[target] = candidate[source];
    }
    for (source, target) in [
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
        (
            state_relation::public_instance::HARDWARE_PROFILE_LO,
            public_instance::HARDWARE_PROFILE_LO,
        ),
    ] {
        public[target..target + 2].copy_from_slice(&candidate[source..source + 2]);
    }
    public[public_instance::TRANSITION_NULLIFIER_LO] = F::ONE;
    let digest = canonical_terminal_authorization_candidate_digest_v1(&[candidate
        [..state_relation::PUBLIC_INSTANCE_COUNT]
        .to_vec()])
    .unwrap();
    public[public_instance::CANDIDATE_LO..public_instance::CANDIDATE_LO + 2]
        .copy_from_slice(&crate::zk::kagemusha_v1_poseidon::digest_limbs::<F>(digest));
    (candidate, public)
}

fn projection_fixture<C>(
    mutation: u8,
    legacy: bool,
) -> (BaseCircuitBuilder<C::ScalarExt>, SemanticSnapshotV1)
where
    C: CurveAffineExt,
    C::Base: BigPrimeField,
    C::ScalarExt: KagemushaPoseidonFieldV1,
{
    let mut builder = BaseCircuitBuilder::new(false)
        .use_k(12)
        .use_lookup_bits(11)
        .use_instance_columns(1);
    let range = builder.range_chip();
    let (original, mut public) = projection_values::<C::ScalarExt>();
    let mut candidate = original.clone();
    let parity = if C::ScalarExt::IS_EQ_PARITY {
        KagemushaPastaParityV1::Eq
    } else {
        KagemushaPastaParityV1::Ep
    };
    let (role, protocol) = match parity {
        KagemushaPastaParityV1::Eq => (state_relation::public_instance::EQ_PROTOCOL_LO, [23; 32]),
        KagemushaPastaParityV1::Ep => (state_relation::public_instance::EP_PROTOCOL_LO, [29; 32]),
    };
    match mutation {
        1 => candidate[state_relation::public_instance::AMOUNT] += C::ScalarExt::ONE,
        2 => candidate[role] += C::ScalarExt::ONE,
        5 => public[public_instance::CANDIDATE_LO] += C::ScalarExt::ONE,
        _ => {}
    }
    let public = public
        .iter()
        .map(|value| builder.main(0).load_witness(*value))
        .collect::<Vec<_>>();
    let mut guard_source = original.clone();
    if mutation == 3 {
        guard_source[state_relation::public_instance::PREDECESSOR_OUTER_LO] += C::ScalarExt::ONE;
    }
    if mutation == 4 {
        guard_source[state_relation::public_instance::HARDWARE_PROFILE_LO] += C::ScalarExt::ONE;
    }
    let guard = guard_cells(&mut builder, &guard_source);
    let guard_eq_protocol = if mutation == 6 { [31; 32] } else { [17; 32] };
    let mut jobs = PastaSha256JobsV1::default();
    let assigned;
    if legacy {
        let (coordinate, integer) = deferred_field_chips_v1::<C>(&range);
        let loader = deferred_loader_v1::<C>(&mut builder, &coordinate, &integer);
        let columns = assign_nested_instances_v1(&loader, &[candidate]);
        legacy_candidate_projection_v1(&loader, &columns[0], &public, protocol, parity, &mut jobs)
            .unwrap();
        legacy_candidate_terminal_guard_binding_v1(
            &loader,
            &columns[0],
            &public,
            &guard,
            guard_eq_protocol,
            [19; 32],
        )
        .unwrap();
        assigned = columns
            .iter()
            .map(|column| {
                column
                    .iter()
                    .map(|value| *value.assigned())
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        *builder.pool(0) = loader.take_ctx();
    } else {
        assigned = vec![
            candidate
                .iter()
                .map(|value| builder.main(0).load_witness(*value))
                .collect::<Vec<_>>(),
        ];
        constrain_candidate_projection_cells_v1(
            builder.main(0),
            &assigned[0],
            &public,
            protocol,
            parity,
            &mut jobs,
        )
        .unwrap();
        constrain_candidate_terminal_guard_cells_v1(
            builder.main(0),
            &assigned[0],
            &public,
            &guard,
            guard_eq_protocol,
            [19; 32],
        )
        .unwrap();
        let before_wrap = semantic_snapshot(&builder, &jobs, &assigned).unwrap();
        let (coordinate, integer) = deferred_field_chips_v1::<C>(&range);
        let loader = deferred_loader_v1::<C>(&mut builder, &coordinate, &integer);
        let wrapped = assigned[0]
            .iter()
            .map(|value| loader.scalar_from_assigned(*value))
            .collect::<Vec<_>>();
        for (actual, original) in wrapped.iter().zip(&assigned[0]) {
            assert_eq!(
                actual.assigned().cell,
                original.cell,
                "nested verifier keeps the exact candidate cell"
            );
            assert_eq!(actual.assigned().value(), original.value());
        }
        *builder.pool(0) = loader.take_ctx();
        assert!(
            before_wrap == semantic_snapshot(&builder, &jobs, &assigned).unwrap(),
            "loader transfer/wrapping must not allocate or constrain any cell"
        );
    }
    assert_eq!(
        jobs.claim_jobs().unwrap().len(),
        1,
        "the final candidate job is present"
    );
    let messages = jobs.canonical_messages().unwrap();
    assert!(messages[0].starts_with(CANDIDATE_BINDING_DOMAIN_V1));
    let expected = canonical_terminal_authorization_candidate_digest_v1(&[original
        [..state_relation::PUBLIC_INSTANCE_COUNT]
        .to_vec()])
    .unwrap();
    if mutation == 0 {
        assert_eq!(<[u8; 32]>::from(Sha256::digest(&messages[0])), expected);
    }
    builder.calculate_params(Some(9));
    // This projection-only MockProver keeps k=12 and its original lookup/table settings.
    // The Base estimator counts virtual cells, while a gate-column break duplicates the
    // boundary cell and reserves room for four rotations. Budget that overlap separately
    // for each column so the positive and tampered witnesses reach constraint evaluation.
    // Production parameter selection and release resource limits are outside this fixture.
    let cells_per_column = (1_usize << builder.config_params.k) - 9 - 4;
    let advice_columns = builder
        .core()
        .phase_manager
        .iter()
        .map(|phase| {
            phase
                .threads
                .iter()
                .map(Context::advice_len)
                .sum::<usize>()
                .div_ceil(cells_per_column)
        })
        .collect();
    builder.config_params.num_advice_per_phase = advice_columns;
    let snapshot = semantic_snapshot(&builder, &jobs, &assigned).unwrap();
    (builder, snapshot)
}

#[test]
fn terminal_semantic_projection_preserves_legacy_cells_geometry_and_queue_in_both_fields() {
    fn check<C>()
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: KagemushaPoseidonFieldV1,
    {
        let (old, old_snapshot) = projection_fixture::<C>(0, true);
        MockProver::run(12, &old, vec![vec![]])
            .unwrap()
            .assert_satisfied();
        drop(old);
        let (current, current_snapshot) = projection_fixture::<C>(0, false);
        assert!(
            old_snapshot == current_snapshot,
            "original and shared cell assignments/constraints/queue differ"
        );
        MockProver::run(12, &current, vec![vec![]])
            .unwrap()
            .assert_satisfied();
    }
    check::<EqAffine>();
    check::<EpAffine>();
}

#[test]
fn terminal_semantic_projection_rejects_detached_candidate_guard_role_and_digest_in_both_fields() {
    fn check<C>()
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: KagemushaPoseidonFieldV1,
    {
        for mutation in 1..=6 {
            let (builder, _) = projection_fixture::<C>(mutation, false);
            assert!(
                MockProver::run(12, &builder, vec![vec![]])
                    .unwrap()
                    .verify()
                    .is_err(),
                "detached semantic case {mutation}"
            );
        }
    }
    check::<EqAffine>();
    check::<EpAffine>();
}

#[test]
fn terminal_semantic_shape_requires_exact_candidate_and_guard_columns_in_both_fields() {
    use snark_verifier::system::halo2::{Config, compile};
    fn protocol<C>(width: usize) -> PlonkProtocol<C>
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: KagemushaPoseidonFieldV1,
    {
        // Shape-only keys: this tiny Base relation conveys no candidate or Guard authority.
        let params = ParamsIPA::<C>::new(8);
        let mut builder = BaseCircuitBuilder::new(false)
            .use_k(8)
            .use_lookup_bits(7)
            .use_instance_columns(1);
        let value = builder.main(0).load_witness(C::ScalarExt::ZERO);
        builder.assigned_instances = vec![vec![value; width]];
        builder.calculate_params(Some(9));
        let vk = halo2_proofs::plonk::keygen_vk(&params, &builder).unwrap();
        compile(&params, &vk, Config::ipa().with_num_instance(vec![width]))
    }
    fn check<C>()
    where
        C: CurveAffineExt,
        C::Base: BigPrimeField,
        C::ScalarExt: KagemushaPoseidonFieldV1,
    {
        let candidate_width = state_relation::PUBLIC_INSTANCE_COUNT + accumulator_limb_count();
        let guard_width = GUARD_RECURSIVE_PUBLIC_INSTANCE_COUNT_V1;
        let candidate_protocol = protocol::<C>(candidate_width);
        let guard_protocol = protocol::<C>(guard_width);
        let candidates = vec![vec![C::ScalarExt::ZERO; candidate_width]];
        let guards = vec![vec![C::ScalarExt::ZERO; guard_width]];
        validate_terminal_nested_public_shape_v1(
            &candidate_protocol,
            &candidates,
            &guard_protocol,
            &guards,
        )
        .unwrap();
        for (candidate, guard) in [
            (vec![], guards.clone()),
            (
                vec![vec![C::ScalarExt::ZERO; candidate_width - 1]],
                guards.clone(),
            ),
            (
                vec![vec![C::ScalarExt::ZERO; candidate_width + 1]],
                guards.clone(),
            ),
            (vec![candidates[0].clone(), vec![]], guards.clone()),
            (candidates.clone(), vec![]),
            (
                candidates.clone(),
                vec![vec![C::ScalarExt::ZERO; guard_width - 1]],
            ),
            (
                candidates.clone(),
                vec![vec![C::ScalarExt::ZERO; guard_width + 1]],
            ),
            (candidates.clone(), vec![guards[0].clone(), vec![]]),
        ] {
            assert!(
                validate_terminal_nested_public_shape_v1(
                    &candidate_protocol,
                    &candidate,
                    &guard_protocol,
                    &guard
                )
                .is_err()
            );
        }
        assert!(
            validate_terminal_nested_public_shape_v1(
                &guard_protocol,
                &candidates,
                &guard_protocol,
                &guards
            )
            .is_err()
        );
        assert!(
            validate_terminal_nested_public_shape_v1(
                &candidate_protocol,
                &candidates,
                &candidate_protocol,
                &guards
            )
            .is_err()
        );
        let mut inner_role = candidate_protocol.clone();
        inner_role.num_instance = vec![state_relation::PUBLIC_INSTANCE_COUNT];
        assert!(
            validate_terminal_nested_public_shape_v1(
                &inner_role,
                &candidates,
                &guard_protocol,
                &guards
            )
            .is_err()
        );
        let mut extra_column = candidate_protocol.clone();
        extra_column.num_instance.push(0);
        assert!(
            validate_terminal_nested_public_shape_v1(
                &extra_column,
                &candidates,
                &guard_protocol,
                &guards
            )
            .is_err()
        );
    }
    check::<EqAffine>();
    check::<EpAffine>();
}
