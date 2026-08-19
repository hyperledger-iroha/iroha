// Builder integration for the review-blocked serialized-advice V7 pair.

fn constrain_kagemusha_equal_when_serialized_v7<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    enabled: halo2_base::AssignedValue<F>,
    lhs: halo2_base::AssignedValue<F>,
    rhs: halo2_base::AssignedValue<F>,
) where
    F: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{
        QuantumCell::Existing,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    range.gate().assert_bit(ctx, enabled);
    let difference = range.gate().sub(ctx, Existing(lhs), Existing(rhs));
    let selected = range
        .gate()
        .mul(ctx, Existing(enabled), Existing(difference));
    range.gate().assert_is_const(ctx, &selected, &F::ZERO);
}

fn constrain_kagemusha_serialized_parent_induction_v7<C>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::ScalarExt>,
    sha_jobs: &mut KagemushaSha256JobsV4<C::ScalarExt>,
    public: &[halo2_base::AssignedValue<C::ScalarExt>],
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    parents: &[KagemushaAssignedSerializedParentV7<C::ScalarExt>; 2],
) -> Result<(), String>
where
    C: KagemushaSerializedAuditCurveV7,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7
        + halo2_base::utils::ScalarField,
{
    use super::kagemusha_serialized_audit_v7::{
        KagemushaAssignedParentDigestContextV7, KagemushaAssignedParentSlotV7,
        constrain_kagemusha_serialized_parent_slots_digest_v7,
    };
    use halo2_base::{
        QuantumCell::Existing,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    if public.len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7 {
        return Err("Kagemusha V7 parent induction saw the wrong current column".to_owned());
    }
    let range = builder.range_chip();
    let ctx = builder.main(0);
    let commitment_offset = KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
        + match C::PARITY {
            KagemushaPastaCycleParityV1::StepEq => 0,
            KagemushaPastaCycleParityV1::StepEp => 2,
        };
    let mut slots = Vec::with_capacity(2);
    for (slot_index, parent) in parents.iter().enumerate() {
        if parent.instance_cells.len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7 {
            return Err(format!(
                "Kagemusha V7 parent slot {slot_index} has the wrong instance length"
            ));
        }
        // Presence is derived from the current child's authenticated parent
        // count, not from the parent's live/bootstrap bit.  A missing slot is
        // therefore one literal all-zero 70-cell tuple, while a bootstrap
        // proof remains a present slot with its own live cell equal to zero.
        // This removes every unused parsed-instance witness, including the
        // manifest/VK and accumulator cells that are not repeated in the
        // transitive parent digest.
        for value in &parent.instance_cells {
            let selected = range
                .gate()
                .mul(ctx, Existing(parent.present), Existing(*value));
            ctx.constrain_equal(&selected, value);
        }
        for (actual, claimed) in parent
            .commitment_chunks
            .iter()
            .zip(&parent.instance_cells[commitment_offset..commitment_offset + 2])
        {
            constrain_kagemusha_equal_when_serialized_v7(
                ctx,
                &range,
                parent.present,
                *actual,
                *claimed,
            );
        }
        slots.push(KagemushaAssignedParentSlotV7 {
            present: parent.present,
            // Pass the raw verified-parent cells.  The digest gadget itself
            // constrains each value to `present * value`, so an absent slot is
            // literally the canonical all-zero tuple rather than merely
            // hashing a selected zero while retaining hidden witness data.
            profile: parent.instance_cells[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5],
            proof_step_count: parent.instance_cells[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5],
            parent_count: parent.instance_cells[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5],
            parent_live: parent.instance_cells[KAGEMUSHA_SERIALIZED_LIVE_SELECTOR_OFFSET_V7],
            frozen_core_statement: [
                parent.instance_cells[KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5],
                parent.instance_cells[KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5 + 1],
            ],
            current_join: std::array::from_fn(|index| {
                parent.instance_cells[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + index]
            }),
            parent_slots_digest: std::array::from_fn(|index| {
                parent.instance_cells[KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7 + index]
            }),
        });
    }
    let assigned_context = KagemushaAssignedParentDigestContextV7 {
        profile_sha256_exact_chunks: public[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2]
            .try_into()
            .expect("V7 profile digest has two chunks"),
        step_eq_vk_sha256_word_chunks: public[KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 2]
            .try_into()
            .expect("V7 Eq VK digest has two chunks"),
        step_ep_vk_sha256_word_chunks: public[KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5 + 2]
            .try_into()
            .expect("V7 Ep VK digest has two chunks"),
        eq_coefficient_count: manifest.eq_coefficient_count,
        ep_coefficient_count: manifest.ep_coefficient_count,
    };
    let expected = public[KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7
        ..KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7 + 2]
        .try_into()
        .expect("V7 parent digest has two chunks");
    constrain_kagemusha_serialized_parent_slots_digest_v7(
        ctx,
        &range,
        sha_jobs,
        assigned_context,
        slots
            .try_into()
            .map_err(|_| "Kagemusha V7 parent slot count drifted".to_owned())?,
        expected,
    )?;
    Ok(())
}

fn constrain_kagemusha_serialized_challenge_and_native_evaluation_v7<C>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::ScalarExt>,
    sha_jobs: &mut KagemushaSha256JobsV4<C::ScalarExt>,
    public: &[halo2_base::AssignedValue<C::ScalarExt>],
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    vector: &super::kagemusha_serialized_audit_v7::KagemushaFrozenNativeAuditVectorV7<C::ScalarExt>,
) -> Result<halo2_base::AssignedValue<C::ScalarExt>, String>
where
    C: KagemushaSerializedAuditCurveV7,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7
        + halo2_base::utils::ScalarField,
{
    use super::kagemusha_serialized_audit_v7::{
        KagemushaAssignedAuditChallengeContextV7, constrain_kagemusha_serialized_audit_challenge_v7,
    };
    use halo2_base::gates::RangeInstructions as _;

    let range = builder.range_chip();
    let ctx = builder.main(0);
    let context = KagemushaAssignedAuditChallengeContextV7 {
        profile_sha256_exact_chunks: public[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2]
            .try_into()
            .expect("V7 profile digest has two chunks"),
        step_eq_vk_sha256_word_chunks: public[KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 2]
            .try_into()
            .expect("V7 Eq VK digest has two chunks"),
        step_ep_vk_sha256_word_chunks: public[KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5 + 2]
            .try_into()
            .expect("V7 Ep VK digest has two chunks"),
        frozen_core_statement_sha256_exact_chunks: public
            [KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5
                ..KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5 + 2]
            .try_into()
            .expect("V7 frozen statement has two chunks"),
        eq_coefficient_count: manifest.eq_coefficient_count,
        ep_coefficient_count: manifest.ep_coefficient_count,
    };
    let step_eq_commitment = public[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
        ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 2]
        .try_into()
        .expect("V7 Eq commitment has two chunks");
    let step_ep_commitment = public[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 2
        ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 4]
        .try_into()
        .expect("V7 Ep commitment has two chunks");
    let expected_challenge = public[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 4
        ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 6]
        .try_into()
        .expect("V7 challenge has two chunks");
    let challenge = constrain_kagemusha_serialized_audit_challenge_v7(
        ctx,
        &range,
        sha_jobs,
        context,
        step_eq_commitment,
        step_ep_commitment,
        expected_challenge,
    )?;
    let evaluation_offset = KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
        + match C::PARITY {
            KagemushaPastaCycleParityV1::StepEq => 6,
            KagemushaPastaCycleParityV1::StepEp => 8,
        };
    let expected = constrain_kagemusha_serialized_scalar_chunks_v7(
        ctx,
        &range,
        public[evaluation_offset..evaluation_offset + 2]
            .try_into()
            .expect("V7 native evaluation has two chunks"),
    );
    constrain_kagemusha_native_audit_evaluation_v7(
        ctx,
        range.gate(),
        vector.coefficients(),
        challenge,
        expected,
    )?;
    Ok(challenge)
}

fn assign_kagemusha_serialized_reciprocal_scalar_v7<F>(
    ctx: &mut halo2_base::gates::flex_gate::threads::SinglePhaseCoreManager<F>,
    scalar: &halo2_ecc::fields::fp::FpChip<'_, F, F::ReciprocalScalar>,
    chunks: [halo2_base::AssignedValue<F>; 2],
    value: F::ReciprocalScalar,
) -> Result<halo2_ecc::bigint::ProperCrtUint<F>, String>
where
    F: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7,
{
    use halo2_ecc::fields::FieldChip as _;

    let assigned: halo2_ecc::bigint::ProperCrtUint<F> =
        scalar.load_private(ctx.main(), value).into();
    let profile = super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7::for_captured_coefficient_count(1)?;
    let frozen =
        super::kagemusha_serialized_audit_v7::constrain_reciprocal_audit_vector_at_seam_v7(
            ctx.main(),
            scalar,
            profile,
            std::slice::from_ref(&assigned),
        )?;
    for (actual, expected) in frozen.chunks()[0].into_iter().zip(chunks) {
        ctx.main().constrain_equal(&actual, &expected);
    }
    Ok(assigned)
}

#[allow(clippy::too_many_arguments)]
fn constrain_kagemusha_serialized_reciprocal_v7<C>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::Base>,
    sha_jobs: &mut KagemushaSha256JobsV4<C::Base>,
    dense_jobs: &mut KagemushaDenseMsmJobsV5<C>,
    serialized_jobs: &mut super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditJobsV7<
        C::Base,
    >,
    public: &[halo2_base::AssignedValue<C::Base>],
    params: &KagemushaStepCircuitParamsV7,
    output: &KagemushaScalarAuditOutputV4<C>,
    native_profile: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7,
    native_vector: &super::kagemusha_serialized_audit_v7::KagemushaFrozenNativeAuditVectorV7<
        C::Base,
    >,
    reciprocal_profile: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7,
    join: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditPublicJoinV7,
) -> Result<(), String>
where
    C: KagemushaSerializedAuditCurveV7,
    C::Base: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7<
            ReciprocalScalar = C::ScalarExt,
        > + halo2_base::utils::ScalarField,
    C::ScalarExt: halo2_base::utils::BigPrimeField,
{
    use super::kagemusha_cycle_loader::{LIMB_BITS, LIMBS, PastaCycleEccChip};
    use halo2_ecc::fields::fp::FpChip;

    if public.len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
        || output.identity.parity != C::PARITY
    {
        return Err("Kagemusha V7 reciprocal bridge shape/parity mismatch".to_owned());
    }
    let range = builder.range_chip();
    let base = FpChip::<C::Base, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar = FpChip::<C::Base, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let mut ctx = std::mem::take(builder.pool(0));
    let inner_parent_counts = output
        .inner_parent_counts
        .map(|count| ctx.main().load_witness(C::Base::from(u64::from(count))));
    let mut chip = PastaCycleEccChip::<C>::new(&base, &scalar);
    let (coefficients, frozen, audit, selectors) =
        assigned_kagemusha_reciprocal_audit_polynomial_v7(
            &mut ctx,
            &chip,
            &scalar,
            &output.audit,
            &output.stages,
            public[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5],
            inner_parent_counts,
            &output.identity,
            reciprocal_profile,
        )?;
    let challenge_value =
        kagemusha_serialized_scalar_from_chunks_v7::<C::ScalarExt>(join.challenge)
            .ok_or_else(|| "Kagemusha V7 reciprocal challenge is noncanonical".to_owned())?;
    let evaluation_chunks = match C::PARITY {
        KagemushaPastaCycleParityV1::StepEq => public[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
            + 6
            ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 8]
            .try_into()
            .expect("V7 Eq evaluation has two chunks"),
        KagemushaPastaCycleParityV1::StepEp => public[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
            + 8
            ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 10]
            .try_into()
            .expect("V7 Ep evaluation has two chunks"),
    };
    let evaluation_value = match C::PARITY {
        KagemushaPastaCycleParityV1::StepEq => {
            kagemusha_serialized_scalar_from_chunks_v7::<C::ScalarExt>(join.eq_evaluation)
        }
        KagemushaPastaCycleParityV1::StepEp => {
            kagemusha_serialized_scalar_from_chunks_v7::<C::ScalarExt>(join.ep_evaluation)
        }
    }
    .ok_or_else(|| "Kagemusha V7 reciprocal evaluation is noncanonical".to_owned())?;
    let challenge_chunks = public[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 4
        ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 6]
        .try_into()
        .expect("V7 challenge has two chunks");
    let challenge = assign_kagemusha_serialized_reciprocal_scalar_v7(
        &mut ctx,
        &scalar,
        challenge_chunks,
        challenge_value,
    )?;
    let evaluation = assign_kagemusha_serialized_reciprocal_scalar_v7(
        &mut ctx,
        &scalar,
        evaluation_chunks,
        evaluation_value,
    )?;
    constrain_kagemusha_reciprocal_audit_evaluation_v7::<C>(
        &mut ctx,
        &scalar,
        &coefficients,
        &challenge,
        &evaluation,
    )?;
    let mut batch_bytes = KAGEMUSHA_SERIALIZED_DENSE_BATCH_DOMAIN_V7
        .iter()
        .copied()
        .map(KagemushaSha256ByteV4::constant)
        .collect::<Vec<_>>();
    batch_bytes.push(KagemushaSha256ByteV4::constant(0));
    batch_bytes.extend(
        KAGEMUSHA_SERIALIZED_PROFILE_VERSION_V7
            .to_le_bytes()
            .into_iter()
            .map(KagemushaSha256ByteV4::constant),
    );
    batch_bytes.push(KagemushaSha256ByteV4::constant(
        u8::try_from(protocol_parity_tag(output.identity.parity))
            .expect("fixed Kagemusha parity tag fits u8"),
    ));
    batch_bytes.extend(chip.assigned_scalar_bytes(&mut ctx, &challenge));
    let batch_digest: [halo2_base::AssignedValue<C::Base>; 8] = sha_jobs
        .digest_constrained(ctx.main(), &batch_bytes)?
        .try_into()
        .expect("SHA-256 digest has eight words");
    chip.constrain_deferred_equation_batch_v5(
        &mut ctx,
        &audit,
        &selectors,
        &batch_digest,
        dense_jobs,
    )?;
    serialized_jobs.queue_physical_proof(
        ctx.main(),
        native_profile,
        native_vector,
        reciprocal_profile,
        &frozen,
        kagemusha_usable_rows_v4(&params.base)?,
    )?;
    *builder.pool(0) = ctx;
    Ok(())
}

fn kagemusha_serialized_core_projection_v7<F>(
    ctx: &mut halo2_base::Context<F>,
    public: &[halo2_base::AssignedValue<F>],
) -> Result<Vec<halo2_base::AssignedValue<F>>, String>
where
    F: halo2_base::utils::BigPrimeField,
{
    if public.len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7 {
        return Err("Kagemusha V7 core projection saw the wrong public length".to_owned());
    }
    let mut core = Vec::with_capacity(66);
    core.push(ctx.load_constant(F::from(u64::from(KAGEMUSHA_COMPACT_PROFILE_VERSION_V5))));
    core.extend_from_slice(&public[1..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7]);
    core.extend(std::iter::repeat_with(|| ctx.load_zero()).take(8));
    core.push(public[KAGEMUSHA_SERIALIZED_LIVE_SELECTOR_OFFSET_V7]);
    if core.len() != 66 {
        return Err("Kagemusha V7 frozen core projection length drifted".to_owned());
    }
    Ok(core)
}

fn validate_kagemusha_serialized_public_identity_v7(
    public_inputs: &KagemushaSerializedPublicInputsV7<'_>,
    params: &KagemushaStepCircuitParamsV7,
) -> Result<(), String> {
    use super::kagemusha_serialized_audit_v7::{
        kagemusha_serialized_exact_chunks_to_bytes_v7,
        kagemusha_serialized_sha256_word_chunks_to_bytes_v7,
    };

    let profile = params.manifest.sha256()?;
    let profile_chunks = kagemusha_serialized_exact_chunks_to_bytes_v7(
        kagemusha_u32_words_to_u128_chunks_v5(&public_inputs.core.manifest_sha256),
    );
    let eq_chunks = kagemusha_serialized_sha256_word_chunks_to_bytes_v7(
        kagemusha_u32_words_to_u128_chunks_v5(&public_inputs.core.step_eq_compiled_protocol_sha256),
    );
    let ep_chunks = kagemusha_serialized_sha256_word_chunks_to_bytes_v7(
        kagemusha_u32_words_to_u128_chunks_v5(&public_inputs.core.step_ep_compiled_protocol_sha256),
    );
    if profile_chunks != profile
        || eq_chunks != params.manifest.step_eq_vk_sha256
        || ep_chunks != params.manifest.step_ep_vk_sha256
    {
        return Err("Kagemusha V7 public profile/VK identity differs from the manifest".to_owned());
    }
    Ok(())
}

fn validate_kagemusha_serialized_auxiliary_capacity_v7<F, C>(
    sha_jobs: &KagemushaSha256JobsV4<F>,
    dense_jobs: &KagemushaDenseMsmJobsV5<C>,
    serialized_jobs: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditJobsV7<F>,
    params: &KagemushaStepCircuitParamsV7,
    role: &str,
) -> Result<(), String>
where
    F: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7,
    C: halo2_base::utils::CurveAffineExt<Base = F>,
    C::ScalarExt: halo2_base::utils::BigPrimeField + ff::WithSmallOrderMulGroup<3>,
{
    validate_kagemusha_auxiliary_capacity_v5(sha_jobs, dense_jobs, &params.base, role)?;
    let (_, native, reciprocal, rows) = serialized_jobs.capacity_profile()?;
    let expected = match F::NATIVE_TARGET {
        KagemushaPastaCycleParityV1::StepEq => (
            usize::try_from(params.manifest.eq_coefficient_count).ok(),
            usize::try_from(params.manifest.ep_coefficient_count).ok(),
        ),
        KagemushaPastaCycleParityV1::StepEp => (
            usize::try_from(params.manifest.ep_coefficient_count).ok(),
            usize::try_from(params.manifest.eq_coefficient_count).ok(),
        ),
    };
    if Some(native) != expected.0
        || Some(reciprocal) != expected.1
        || rows != 10 + native + 2 * reciprocal
        || rows > kagemusha_usable_rows_v4(&params.base)?
    {
        return Err(format!("Kagemusha V7 {role} serialized capacity drifted"));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_kagemusha_step_eq_circuit_serialized_v7(
    witness: &KagemushaStepWitnessV4<'_>,
    params: KagemushaStepCircuitParamsV7,
    public_inputs: &KagemushaSerializedPublicInputsV7<'_>,
    ep_output: &KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EpAffine>,
    mode: KagemushaStepPublicModeV4,
    stage: KagemushaCircuitBuilderStageV5<'_>,
) -> Result<KagemushaStepEqCircuitV7, String> {
    use halo2_proofs::halo2curves::pasta::{EqAffine, Fp};

    let layout = params.validate()?;
    validate_kagemusha_serialized_public_identity_v7(public_inputs, &params)?;
    let base_layout = validate_kagemusha_circuit_params_v4(&params.base)?;
    let recursive_layout = layout.recursive_parent_layout(&base_layout);
    let mut step = kagemusha_base_builder_for_stage_v5::<Fp>(&params.base, stage)?;
    let values = public_inputs.instance_column::<Fp>(
        witness.proof_step_count,
        &params.base,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let public = assign_kagemusha_serialized_public_mode_v7(&mut step, values, layout, mode)?;
    let range = step.range_chip();
    let mut sha_jobs = KagemushaSha256JobsV4::default();
    let mut dense_jobs = KagemushaDenseMsmJobsV5::default();
    let mut serialized_jobs =
        super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditJobsV7::default();
    let semantic_values = witness.public_inputs.private_semantic_column::<Fp>();
    let semantic_len = semantic_values.len();
    let semantic = step.main(0).assign_witnesses(semantic_values);
    let bindings = constrain_kagemusha_common_transition(
        step.main(0),
        &range,
        &mut sha_jobs,
        &semantic,
        semantic_len,
    )?;
    let core = kagemusha_serialized_core_projection_v7(step.main(0), &public)?;
    constrain_kagemusha_compact_eq_header_v5(step.main(0), &range, &core, &semantic, &base_layout)?;
    constrain_kagemusha_eq_secure_relations_v4(
        step.main(0),
        &range,
        &bindings,
        witness.secure,
        witness.output_membership,
    )?;
    let native_profile =
        super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7::for_captured_coefficient_count(
            usize::try_from(params.manifest.eq_coefficient_count)
                .map_err(|_| "Kagemusha V7 Eq coefficient count does not fit usize")?,
        )?;
    let assigned = assign_kagemusha_parity_native_vector_v7::<EqAffine>(
        &mut step,
        &mut sha_jobs,
        &public,
        &params.base,
        &recursive_layout,
        witness.step_eq_recursion,
        native_profile,
        usize::try_from(params.manifest.step_eq_phase_zero_rank)
            .map_err(|_| "Kagemusha V7 Eq phase-zero rank does not fit usize")?,
    )?;
    constrain_kagemusha_serialized_parent_induction_v7::<EqAffine>(
        &mut step,
        &mut sha_jobs,
        &public,
        &params.manifest,
        &assigned.parents,
    )?;
    constrain_kagemusha_serialized_challenge_and_native_evaluation_v7::<EqAffine>(
        &mut step,
        &mut sha_jobs,
        &public,
        &params.manifest,
        &assigned.vector,
    )?;
    let reciprocal_profile =
        super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7::for_captured_coefficient_count(
            usize::try_from(params.manifest.ep_coefficient_count)
                .map_err(|_| "Kagemusha V7 Ep coefficient count does not fit usize")?,
        )?;
    constrain_kagemusha_serialized_reciprocal_v7::<halo2_proofs::halo2curves::pasta::EpAffine>(
        &mut step,
        &mut sha_jobs,
        &mut dense_jobs,
        &mut serialized_jobs,
        &public,
        &params,
        ep_output,
        native_profile,
        &assigned.vector,
        reciprocal_profile,
        &public_inputs.current_join,
    )?;
    validate_kagemusha_serialized_auxiliary_capacity_v7(
        &sha_jobs,
        &dense_jobs,
        &serialized_jobs,
        &params,
        "StepEq",
    )?;
    match stage {
        KagemushaCircuitBuilderStageV5::Keygen => {
            let role = if mode == KagemushaStepPublicModeV4::Bootstrap {
                "StepEqBootstrap"
            } else {
                "StepEqLive"
            };
            validate_kagemusha_populated_builder_fit_v5(&mut step, &params.base, role)?;
        }
        KagemushaCircuitBuilderStageV5::Prover(break_points) => {
            validate_kagemusha_witness_builder_break_points_v5(
                &step,
                &params.base,
                break_points,
                "StepEqV7",
            )?;
        }
    }
    Ok(KagemushaStepEqCircuitV7 {
        params,
        builder: step,
        sha_jobs,
        dense_jobs,
        serialized_jobs,
    })
}

#[allow(clippy::too_many_arguments)]
fn build_kagemusha_step_ep_circuit_serialized_v7(
    witness: &KagemushaStepWitnessV4<'_>,
    params: KagemushaStepCircuitParamsV7,
    public_inputs: &KagemushaSerializedPublicInputsV7<'_>,
    eq_output: &KagemushaScalarAuditOutputV4<halo2_proofs::halo2curves::pasta::EqAffine>,
    mode: KagemushaStepPublicModeV4,
    stage: KagemushaCircuitBuilderStageV5<'_>,
) -> Result<KagemushaStepEpCircuitV7, String> {
    use halo2_proofs::halo2curves::pasta::{EpAffine, Fq};

    let layout = params.validate()?;
    validate_kagemusha_serialized_public_identity_v7(public_inputs, &params)?;
    let base_layout = validate_kagemusha_circuit_params_v4(&params.base)?;
    let recursive_layout = layout.recursive_parent_layout(&base_layout);
    let mut step = kagemusha_base_builder_for_stage_v5::<Fq>(&params.base, stage)?;
    let values = public_inputs.instance_column::<Fq>(
        witness.proof_step_count,
        &params.base,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let public = assign_kagemusha_serialized_public_mode_v7(&mut step, values, layout, mode)?;
    let mut sha_jobs = KagemushaSha256JobsV4::default();
    let mut dense_jobs = KagemushaDenseMsmJobsV5::default();
    let mut serialized_jobs =
        super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditJobsV7::default();
    let native_profile =
        super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7::for_captured_coefficient_count(
            usize::try_from(params.manifest.ep_coefficient_count)
                .map_err(|_| "Kagemusha V7 Ep coefficient count does not fit usize")?,
        )?;
    let assigned = assign_kagemusha_parity_native_vector_v7::<EpAffine>(
        &mut step,
        &mut sha_jobs,
        &public,
        &params.base,
        &recursive_layout,
        witness.step_ep_recursion,
        native_profile,
        usize::try_from(params.manifest.step_ep_phase_zero_rank)
            .map_err(|_| "Kagemusha V7 Ep phase-zero rank does not fit usize")?,
    )?;
    constrain_kagemusha_serialized_parent_induction_v7::<EpAffine>(
        &mut step,
        &mut sha_jobs,
        &public,
        &params.manifest,
        &assigned.parents,
    )?;
    constrain_kagemusha_serialized_challenge_and_native_evaluation_v7::<EpAffine>(
        &mut step,
        &mut sha_jobs,
        &public,
        &params.manifest,
        &assigned.vector,
    )?;
    let reciprocal_profile =
        super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7::for_captured_coefficient_count(
            usize::try_from(params.manifest.eq_coefficient_count)
                .map_err(|_| "Kagemusha V7 Eq coefficient count does not fit usize")?,
        )?;
    constrain_kagemusha_serialized_reciprocal_v7::<halo2_proofs::halo2curves::pasta::EqAffine>(
        &mut step,
        &mut sha_jobs,
        &mut dense_jobs,
        &mut serialized_jobs,
        &public,
        &params,
        eq_output,
        native_profile,
        &assigned.vector,
        reciprocal_profile,
        &public_inputs.current_join,
    )?;
    validate_kagemusha_serialized_auxiliary_capacity_v7(
        &sha_jobs,
        &dense_jobs,
        &serialized_jobs,
        &params,
        "StepEp",
    )?;
    match stage {
        KagemushaCircuitBuilderStageV5::Keygen => {
            let role = if mode == KagemushaStepPublicModeV4::Bootstrap {
                "StepEpBootstrap"
            } else {
                "StepEpLive"
            };
            validate_kagemusha_populated_builder_fit_v5(&mut step, &params.base, role)?;
        }
        KagemushaCircuitBuilderStageV5::Prover(break_points) => {
            validate_kagemusha_witness_builder_break_points_v5(
                &step,
                &params.base,
                break_points,
                "StepEpV7",
            )?;
        }
    }
    Ok(KagemushaStepEpCircuitV7 {
        params,
        builder: step,
        sha_jobs,
        dense_jobs,
        serialized_jobs,
    })
}

struct KagemushaSerializedVerifiedPairV7 {
    step_eq: snark_verifier::pcs::ipa::IpaAccumulator<
        halo2_proofs::halo2curves::pasta::EqAffine,
        snark_verifier::loader::native::NativeLoader,
    >,
    step_ep: snark_verifier::pcs::ipa::IpaAccumulator<
        halo2_proofs::halo2curves::pasta::EpAffine,
        snark_verifier::loader::native::NativeLoader,
    >,
}

fn kagemusha_serialized_params_sha256_v7<C>(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<C>,
) -> Result<[u8; 32], String>
where
    C: KagemushaSerializedAuditCurveV7,
{
    use halo2_proofs::poly::commitment::Params as _;
    use sha2::Digest as _;

    let mut encoded = Vec::new();
    params
        .write(&mut encoded)
        .map_err(|error| format!("failed to encode Kagemusha V7 ParamsIPA: {error}"))?;
    Ok(sha2::Sha256::digest(encoded).into())
}

fn succinct_verify_kagemusha_serialized_eq_v7(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    proof: &[u8],
    instances: &[Vec<Fp>],
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    max_proof_bytes: usize,
) -> Result<
    (
        snark_verifier::pcs::ipa::IpaAccumulator<
            halo2_proofs::halo2curves::pasta::EqAffine,
            snark_verifier::loader::native::NativeLoader,
        >,
        [u8; 32],
    ),
    String,
> {
    use halo2_proofs::{
        halo2curves::{
            CurveExt as _,
            group::Curve as _,
            pasta::{Eq, EqAffine},
        },
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        loader::native::NativeLoader,
        pcs::ipa::{Bgh19, IpaAs, IpaSuccinctVerifyingKey},
        system::halo2::{compile, transcript::halo2::PoseidonTranscript},
        util::arithmetic::{Domain, root_of_unity},
        verifier::{SnarkVerifier as _, plonk::PlonkSuccinctVerifier},
    };

    let authenticated_proof_bytes = usize::try_from(manifest.step_eq_proof_bytes)
        .map_err(|_| "Kagemusha V7 Eq proof size does not fit usize".to_owned())?;
    if max_proof_bytes != authenticated_proof_bytes
        || proof.len() != authenticated_proof_bytes
        || instances.len() != 1
        || instances[0].len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
    {
        return Err("Kagemusha V7 Eq proof/instance shape is invalid".to_owned());
    }
    type Scheme = IpaAs<EqAffine, Bgh19>;
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let hash_to_curve = Eq::hash_to_curve("Halo2-Parameters");
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(
            usize::try_from(params.k()).map_err(|_| "V7 Eq degree does not fit usize")?,
            root_of_unity(
                usize::try_from(params.k()).map_err(|_| "V7 Eq degree does not fit usize")?,
            ),
        ),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    let protocol = compile(
        params,
        verifying_key,
        kagemusha_ipa_compile_config_v4(KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7),
    );
    if kagemusha_serialized_params_sha256_v7(params)? != manifest.step_eq_params_sha256
        || kagemusha_compiled_protocol_identity_sha256(
            &protocol,
            KagemushaPastaCycleParityV1::StepEq,
        )? != manifest.step_eq_vk_sha256
    {
        return Err(
            "Kagemusha V7 Eq ParamsIPA or compiled-protocol identity differs from the manifest"
                .to_owned(),
        );
    }
    let mut cursor = std::io::Cursor::new(proof);
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(&mut cursor);
    let parsed =
        PlonkSuccinctVerifier::<Scheme>::read_proof(&svk, &protocol, instances, &mut transcript)
            .map_err(|error| format!("failed to parse Kagemusha V7 Eq proof: {error:?}"))?;
    let (_, commitment) =
        super::kagemusha_serialized_audit_v7::kagemusha_selected_advice_commitment_v7(
            &parsed.witnesses,
            &manifest.step_eq_advice_phases,
            usize::try_from(manifest.step_eq_serialized_column)
                .map_err(|_| "V7 Eq serialized column does not fit usize")?,
        )?;
    let accumulators = PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, instances, &parsed)
        .map_err(|error| format!("Kagemusha V7 Eq proof verification failed: {error:?}"))?;
    if cursor.position()
        != u64::try_from(proof.len()).map_err(|_| "V7 Eq proof length does not fit u64")?
    {
        return Err("Kagemusha V7 Eq proof has trailing bytes".to_owned());
    }
    let [accumulator]: [_; 1] = accumulators.try_into().map_err(|values: Vec<_>| {
        format!(
            "Kagemusha V7 Eq proof emitted {} accumulators instead of one",
            values.len()
        )
    })?;
    Ok((accumulator, commitment))
}

fn succinct_verify_kagemusha_serialized_ep_v7(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    verifying_key: &halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    proof: &[u8],
    instances: &[Vec<Fq>],
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    max_proof_bytes: usize,
) -> Result<
    (
        snark_verifier::pcs::ipa::IpaAccumulator<
            halo2_proofs::halo2curves::pasta::EpAffine,
            snark_verifier::loader::native::NativeLoader,
        >,
        [u8; 32],
    ),
    String,
> {
    use halo2_proofs::{
        halo2curves::{
            CurveExt as _,
            group::Curve as _,
            pasta::{Ep, EpAffine},
        },
        poly::commitment::{Params as _, ParamsProver as _},
    };
    use snark_verifier::{
        loader::native::NativeLoader,
        pcs::ipa::{Bgh19, IpaAs, IpaSuccinctVerifyingKey},
        system::halo2::{compile, transcript::halo2::PoseidonTranscript},
        util::arithmetic::{Domain, root_of_unity},
        verifier::{SnarkVerifier as _, plonk::PlonkSuccinctVerifier},
    };

    let authenticated_proof_bytes = usize::try_from(manifest.step_ep_proof_bytes)
        .map_err(|_| "Kagemusha V7 Ep proof size does not fit usize".to_owned())?;
    if max_proof_bytes != authenticated_proof_bytes
        || proof.len() != authenticated_proof_bytes
        || instances.len() != 1
        || instances[0].len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
    {
        return Err("Kagemusha V7 Ep proof/instance shape is invalid".to_owned());
    }
    type Scheme = IpaAs<EpAffine, Bgh19>;
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let hash_to_curve = Ep::hash_to_curve("Halo2-Parameters");
    let svk = IpaSuccinctVerifyingKey::new(
        Domain::new(
            usize::try_from(params.k()).map_err(|_| "V7 Ep degree does not fit usize")?,
            root_of_unity(
                usize::try_from(params.k()).map_err(|_| "V7 Ep degree does not fit usize")?,
            ),
        ),
        params.get_g()[0],
        hash_to_curve(&[2]).to_affine(),
        Some(hash_to_curve(&[1]).to_affine()),
    );
    let protocol = compile(
        params,
        verifying_key,
        kagemusha_ipa_compile_config_v4(KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7),
    );
    if kagemusha_serialized_params_sha256_v7(params)? != manifest.step_ep_params_sha256
        || kagemusha_compiled_protocol_identity_sha256(
            &protocol,
            KagemushaPastaCycleParityV1::StepEp,
        )? != manifest.step_ep_vk_sha256
    {
        return Err(
            "Kagemusha V7 Ep ParamsIPA or compiled-protocol identity differs from the manifest"
                .to_owned(),
        );
    }
    let mut cursor = std::io::Cursor::new(proof);
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(&mut cursor);
    let parsed =
        PlonkSuccinctVerifier::<Scheme>::read_proof(&svk, &protocol, instances, &mut transcript)
            .map_err(|error| format!("failed to parse Kagemusha V7 Ep proof: {error:?}"))?;
    let (_, commitment) =
        super::kagemusha_serialized_audit_v7::kagemusha_selected_advice_commitment_v7(
            &parsed.witnesses,
            &manifest.step_ep_advice_phases,
            usize::try_from(manifest.step_ep_serialized_column)
                .map_err(|_| "V7 Ep serialized column does not fit usize")?,
        )?;
    let accumulators = PlonkSuccinctVerifier::<Scheme>::verify(&svk, &protocol, instances, &parsed)
        .map_err(|error| format!("Kagemusha V7 Ep proof verification failed: {error:?}"))?;
    if cursor.position()
        != u64::try_from(proof.len()).map_err(|_| "V7 Ep proof length does not fit u64")?
    {
        return Err("Kagemusha V7 Ep proof has trailing bytes".to_owned());
    }
    let [accumulator]: [_; 1] = accumulators.try_into().map_err(|values: Vec<_>| {
        format!(
            "Kagemusha V7 Ep proof emitted {} accumulators instead of one",
            values.len()
        )
    })?;
    Ok((accumulator, commitment))
}

fn kagemusha_serialized_instance_u128_v7<F: ff::PrimeField + halo2_base::utils::ScalarField>(
    instances: &[Vec<F>],
) -> Result<[u128; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7], String> {
    if instances.len() != 1 || instances[0].len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7 {
        return Err("Kagemusha V7 pair instance shape is invalid".to_owned());
    }
    instances[0]
        .iter()
        .map(|value| {
            u128::try_from(halo2_base::utils::fe_to_biguint(value))
                .map_err(|_| "Kagemusha V7 public cell does not fit u128".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| "Kagemusha V7 public cell count drifted".to_owned())
}

#[allow(clippy::too_many_arguments)]
fn verify_kagemusha_serialized_atomic_pair_v7(
    step_eq_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_eq_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_ep_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    step_eq_proof: &[u8],
    step_ep_proof: &[u8],
    step_eq_instances: &[Vec<Fp>],
    step_ep_instances: &[Vec<Fq>],
    max_proof_bytes: usize,
    max_pair_bytes: usize,
) -> Result<KagemushaSerializedVerifiedPairV7, String> {
    use super::kagemusha_serialized_audit_v7::{
        KagemushaSerializedAuditChallengeContextV7, KagemushaSerializedAuditPublicJoinV7,
        kagemusha_serialized_audit_challenge_v7, kagemusha_serialized_bytes_to_chunks_v7,
        kagemusha_serialized_exact_chunks_to_bytes_v7,
        kagemusha_serialized_sha256_word_chunks_to_bytes_v7,
    };

    manifest.validate()?;
    let step_eq_proof_bytes = usize::try_from(manifest.step_eq_proof_bytes)
        .map_err(|_| "Kagemusha V7 Eq proof size does not fit usize".to_owned())?;
    let step_ep_proof_bytes = usize::try_from(manifest.step_ep_proof_bytes)
        .map_err(|_| "Kagemusha V7 Ep proof size does not fit usize".to_owned())?;
    if max_proof_bytes != step_eq_proof_bytes
        || max_proof_bytes != step_ep_proof_bytes
        || step_eq_proof.len() != step_eq_proof_bytes
        || step_ep_proof.len() != step_ep_proof_bytes
        || step_eq_proof
            .len()
            .checked_add(step_ep_proof.len())
            .filter(|length| *length <= max_pair_bytes)
            .is_none()
    {
        return Err("Kagemusha V7 atomic proof/pair byte profile mismatch".to_owned());
    }
    let eq_cells = kagemusha_serialized_instance_u128_v7(step_eq_instances)?;
    let ep_cells = kagemusha_serialized_instance_u128_v7(step_ep_instances)?;
    if eq_cells[..KAGEMUSHA_SERIALIZED_COMMON_HEADER_CELLS_V7]
        != ep_cells[..KAGEMUSHA_SERIALIZED_COMMON_HEADER_CELLS_V7]
        || eq_cells[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7..]
            != ep_cells[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7..]
        || eq_cells[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5]
            != u128::from(KAGEMUSHA_SERIALIZED_PROFILE_VERSION_V7)
    {
        return Err("Kagemusha V7 Eq/Ep public pair does not cross-match".to_owned());
    }
    let join_cells: [u128; KAGEMUSHA_SERIALIZED_CURRENT_JOIN_CELLS_V7] = eq_cells
        [KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
            ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
                + KAGEMUSHA_SERIALIZED_CURRENT_JOIN_CELLS_V7]
        .try_into()
        .expect("V7 join has ten cells");
    let join = KagemushaSerializedAuditPublicJoinV7 {
        step_eq_commitment: join_cells[..2].try_into().expect("Eq commitment chunks"),
        step_ep_commitment: join_cells[2..4].try_into().expect("Ep commitment chunks"),
        challenge: join_cells[4..6].try_into().expect("challenge chunks"),
        eq_evaluation: join_cells[6..8].try_into().expect("Eq evaluation chunks"),
        ep_evaluation: join_cells[8..10].try_into().expect("Ep evaluation chunks"),
    };
    validate_kagemusha_serialized_public_join_v7(&join)?;
    let manifest_digest = manifest.sha256()?;
    let identity_chunks = |offset| {
        eq_cells[offset..offset + 2]
            .try_into()
            .expect("V7 identity has two chunks")
    };
    if kagemusha_serialized_exact_chunks_to_bytes_v7(identity_chunks(
        KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5,
    )) != manifest_digest
        || kagemusha_serialized_sha256_word_chunks_to_bytes_v7(identity_chunks(
            KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5,
        )) != manifest.step_eq_vk_sha256
        || kagemusha_serialized_sha256_word_chunks_to_bytes_v7(identity_chunks(
            KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5,
        )) != manifest.step_ep_vk_sha256
    {
        return Err("Kagemusha V7 public identity does not match its manifest".to_owned());
    }
    let (step_eq, actual_eq_commitment) = succinct_verify_kagemusha_serialized_eq_v7(
        step_eq_params,
        step_eq_verifying_key,
        step_eq_proof,
        step_eq_instances,
        manifest,
        max_proof_bytes,
    )?;
    let (step_ep, actual_ep_commitment) = succinct_verify_kagemusha_serialized_ep_v7(
        step_ep_params,
        step_ep_verifying_key,
        step_ep_proof,
        step_ep_instances,
        manifest,
        max_proof_bytes,
    )?;
    if kagemusha_serialized_bytes_to_chunks_v7(actual_eq_commitment) != join.step_eq_commitment
        || kagemusha_serialized_bytes_to_chunks_v7(actual_ep_commitment) != join.step_ep_commitment
    {
        return Err(
            "Kagemusha V7 selected advice commitment differs from the public pair".to_owned(),
        );
    }
    let frozen_statement = kagemusha_serialized_exact_chunks_to_bytes_v7(
        eq_cells[KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5 + 2]
            .try_into()
            .expect("V7 statement has two chunks"),
    );
    let context =
        KagemushaSerializedAuditChallengeContextV7::from_manifest(manifest, frozen_statement)?;
    let challenge = kagemusha_serialized_audit_challenge_v7(
        &context,
        actual_eq_commitment,
        actual_ep_commitment,
    )?;
    if kagemusha_serialized_bytes_to_chunks_v7(challenge) != join.challenge {
        return Err("Kagemusha V7 public challenge is not commitment-derived".to_owned());
    }
    // Succinct verification only reduces each PLONK proof to an IPA opening
    // accumulator.  Atomic acceptance must also decide both opening claims;
    // returning the undecided accumulators as "verified" would leave the
    // polynomial-commitment relation unchecked at the terminal boundary.
    let step_eq = super::kagemusha_accumulation::verify_and_decide_eq_accumulation_v4(
        step_eq_params,
        manifest.k,
        step_eq,
        None,
        &KagemushaIpaAccumulationProofV4::initialization(manifest.k)?,
    )?;
    let step_ep = super::kagemusha_accumulation::verify_and_decide_ep_accumulation_v4(
        step_ep_params,
        manifest.k,
        step_ep,
        None,
        &KagemushaIpaAccumulationProofV4::initialization(manifest.k)?,
    )?;
    Ok(KagemushaSerializedVerifiedPairV7 { step_eq, step_ep })
}

#[allow(clippy::too_many_arguments)]
fn create_kagemusha_serialized_eq_proof_v7(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    circuit: KagemushaStepEqCircuitV7,
    instances: &[Vec<Fp>],
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    role_seed: &[u8; 32],
    expected_commitment: [u8; 32],
    max_proof_bytes: usize,
) -> Result<
    (
        Vec<u8>,
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    ),
    String,
> {
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EqAffine},
        plonk::{create_proof_consuming, verify_proof},
        poly::ipa::commitment::IPACommitmentScheme,
    };
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    };

    if role_seed.iter().all(|byte| *byte == 0)
        || circuit.params.manifest != *manifest
        || instances.len() != 1
        || instances[0].len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
    {
        return Err("Kagemusha V7 Eq one-proof input/profile is invalid".to_owned());
    }
    let selected = usize::try_from(manifest.step_eq_serialized_column)
        .map_err(|_| "Kagemusha V7 Eq serialized column does not fit usize".to_owned())?;
    let predicted = circuit.serialized_jobs.precommitment(
        params,
        proving_key.get_vk(),
        selected,
        iroha_crypto::rng_from_seed_slice(role_seed),
    )?;
    let predicted_bytes: [u8; 32] = predicted
        .to_bytes()
        .as_ref()
        .try_into()
        .expect("Pasta commitment encoding is 32 bytes");
    if predicted_bytes != expected_commitment {
        return Err("Kagemusha V7 Eq precommitment differs from the public join".to_owned());
    }
    type Transcript<S> = PoseidonTranscript<
        EqAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let columns: [&[Fp]; 1] = [&instances[0]];
    let proof_instances: [&[&[Fp]]; 1] = [&columns];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    let verifying_key = create_proof_consuming::<
        IPACommitmentScheme<EqAffine>,
        KagemushaDirectInstanceProverIpa<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        circuit,
        &proof_instances,
        iroha_crypto::rng_from_seed_slice(role_seed),
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha V7 Eq proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EqAffine>,
        KagemushaDirectInstanceVerifierIpa<'_, EqAffine>,
        ChallengeScalar<EqAffine>,
        _,
        _,
    >(
        params,
        &verifying_key,
        KagemushaDirectInstanceSingleStrategy::from_params(params),
        &proof_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha V7 Eq generator: {error}"))?;
    halo2_proofs::release_allocator_slack();
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    if proof.len()
        != usize::try_from(manifest.step_eq_proof_bytes)
            .map_err(|_| "Kagemusha V7 Eq proof size does not fit usize".to_owned())?
    {
        return Err("Kagemusha V7 Eq augmented proof size drifted".to_owned());
    }
    let (_, actual_commitment) = succinct_verify_kagemusha_serialized_eq_v7(
        params,
        &verifying_key,
        &proof,
        instances,
        manifest,
        max_proof_bytes,
    )?;
    if actual_commitment != expected_commitment {
        return Err("Kagemusha V7 Eq final commitment drifted from its precommitment".to_owned());
    }
    Ok((proof, verifying_key))
}

#[allow(clippy::too_many_arguments)]
fn create_kagemusha_serialized_ep_proof_v7(
    params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    proving_key: halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    circuit: KagemushaStepEpCircuitV7,
    instances: &[Vec<Fq>],
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    role_seed: &[u8; 32],
    expected_commitment: [u8; 32],
    max_proof_bytes: usize,
) -> Result<
    (
        Vec<u8>,
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    ),
    String,
> {
    use halo2_proofs::{
        halo2curves::{group::GroupEncoding as _, pasta::EpAffine},
        plonk::{create_proof_consuming, verify_proof},
        poly::ipa::commitment::IPACommitmentScheme,
    };
    use snark_verifier::{
        loader::native::NativeLoader,
        system::halo2::transcript::halo2::{ChallengeScalar, PoseidonTranscript},
    };

    if role_seed.iter().all(|byte| *byte == 0)
        || circuit.params.manifest != *manifest
        || instances.len() != 1
        || instances[0].len() != KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
    {
        return Err("Kagemusha V7 Ep one-proof input/profile is invalid".to_owned());
    }
    let selected = usize::try_from(manifest.step_ep_serialized_column)
        .map_err(|_| "Kagemusha V7 Ep serialized column does not fit usize".to_owned())?;
    let predicted = circuit.serialized_jobs.precommitment(
        params,
        proving_key.get_vk(),
        selected,
        iroha_crypto::rng_from_seed_slice(role_seed),
    )?;
    let predicted_bytes: [u8; 32] = predicted
        .to_bytes()
        .as_ref()
        .try_into()
        .expect("Pasta commitment encoding is 32 bytes");
    if predicted_bytes != expected_commitment {
        return Err("Kagemusha V7 Ep precommitment differs from the public join".to_owned());
    }
    type Transcript<S> = PoseidonTranscript<
        EpAffine,
        NativeLoader,
        S,
        KAGEMUSHA_POSEIDON_WIDTH,
        KAGEMUSHA_POSEIDON_RATE,
        KAGEMUSHA_POSEIDON_FULL_ROUNDS,
        KAGEMUSHA_POSEIDON_PARTIAL_ROUNDS,
    >;
    let columns: [&[Fq]; 1] = [&instances[0]];
    let proof_instances: [&[&[Fq]]; 1] = [&columns];
    let mut transcript = Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(Vec::new());
    let verifying_key = create_proof_consuming::<
        IPACommitmentScheme<EpAffine>,
        KagemushaDirectInstanceProverIpa<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
        _,
    >(
        params,
        proving_key,
        circuit,
        &proof_instances,
        iroha_crypto::rng_from_seed_slice(role_seed),
        &mut transcript,
    )
    .map_err(|error| format!("failed to create Kagemusha V7 Ep proof: {error}"))?;
    let mut proof = transcript.finalize();
    let mut verification_transcript =
        Transcript::new::<KAGEMUSHA_POSEIDON_SECURE_MDS>(proof.as_slice());
    let folded_generator = verify_proof::<
        IPACommitmentScheme<EpAffine>,
        KagemushaDirectInstanceVerifierIpa<'_, EpAffine>,
        ChallengeScalar<EpAffine>,
        _,
        _,
    >(
        params,
        &verifying_key,
        KagemushaDirectInstanceSingleStrategy::from_params(params),
        &proof_instances,
        &mut verification_transcript,
    )
    .map_err(|error| format!("failed to derive Kagemusha V7 Ep generator: {error}"))?;
    halo2_proofs::release_allocator_slack();
    proof.extend_from_slice(folded_generator.to_bytes().as_ref());
    if proof.len()
        != usize::try_from(manifest.step_ep_proof_bytes)
            .map_err(|_| "Kagemusha V7 Ep proof size does not fit usize".to_owned())?
    {
        return Err("Kagemusha V7 Ep augmented proof size drifted".to_owned());
    }
    let (_, actual_commitment) = succinct_verify_kagemusha_serialized_ep_v7(
        params,
        &verifying_key,
        &proof,
        instances,
        manifest,
        max_proof_bytes,
    )?;
    if actual_commitment != expected_commitment {
        return Err("Kagemusha V7 Ep final commitment drifted from its precommitment".to_owned());
    }
    Ok((proof, verifying_key))
}

struct KagemushaSerializedSecretSeedV7([u8; 32]);

impl Drop for KagemushaSerializedSecretSeedV7 {
    fn drop(&mut self) {
        use zeroize::Zeroize as _;
        self.0.zeroize();
    }
}

fn kagemusha_fresh_serialized_role_seeds_v7() -> Result<
    (
        KagemushaSerializedSecretSeedV7,
        KagemushaSerializedSecretSeedV7,
    ),
    String,
> {
    use rand_core_06::RngCore as _;

    let mut step_eq = KagemushaSerializedSecretSeedV7([0; 32]);
    let mut step_ep = KagemushaSerializedSecretSeedV7([0; 32]);
    rand_core_06::OsRng.fill_bytes(&mut step_eq.0);
    rand_core_06::OsRng.fill_bytes(&mut step_ep.0);
    if step_eq.0 == [0; 32] || step_ep.0 == [0; 32] || step_eq.0 == step_ep.0 {
        return Err("Kagemusha V7 role-local proving seeds are invalid".to_owned());
    }
    Ok((step_eq, step_ep))
}

struct KagemushaSerializedCreatedPairV7 {
    step_eq_proof: Vec<u8>,
    step_ep_proof: Vec<u8>,
    step_eq_instances: Vec<Vec<Fp>>,
    step_ep_instances: Vec<Vec<Fq>>,
    step_eq_verifying_key:
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
    step_ep_verifying_key:
        halo2_proofs::plonk::VerifyingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
    verified: KagemushaSerializedVerifiedPairV7,
}

#[allow(clippy::too_many_arguments, clippy::type_complexity)]
fn create_kagemusha_serialized_atomic_pair_once_v7<LoadEqPk, LoadEpPk, BuildEq, BuildEp>(
    step_eq_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_params: &halo2_proofs::poly::ipa::commitment::ParamsIPA<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    step_eq_precommit_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EqAffine,
    >,
    step_ep_precommit_verifying_key: &halo2_proofs::plonk::VerifyingKey<
        halo2_proofs::halo2curves::pasta::EpAffine,
    >,
    manifest: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
    frozen_core_statement_sha256: [u8; 32],
    eq_coefficients: &[Fp],
    ep_coefficients: &[Fq],
    mut load_step_eq_proving_key: LoadEqPk,
    mut load_step_ep_proving_key: LoadEpPk,
    mut build_step_eq: BuildEq,
    mut build_step_ep: BuildEp,
    max_proof_bytes: usize,
    max_pair_bytes: usize,
) -> Result<KagemushaSerializedCreatedPairV7, String>
where
    LoadEqPk: FnMut() -> Result<
        halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EqAffine>,
        String,
    >,
    LoadEpPk: FnMut() -> Result<
        halo2_proofs::plonk::ProvingKey<halo2_proofs::halo2curves::pasta::EpAffine>,
        String,
    >,
    BuildEq: FnMut(
        super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditPublicJoinV7,
    ) -> Result<(KagemushaStepEqCircuitV7, Vec<Vec<Fp>>), String>,
    BuildEp: FnMut(
        super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditPublicJoinV7,
    ) -> Result<(KagemushaStepEpCircuitV7, Vec<Vec<Fq>>), String>,
{
    use super::kagemusha_serialized_audit_v7::{
        KagemushaSerializedAuditChallengeContextV7, KagemushaSerializedAuditPublicJoinV7,
        kagemusha_serialized_audit_challenge_v7, kagemusha_serialized_bytes_to_chunks_v7,
    };
    use halo2_proofs::halo2curves::{
        group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine},
    };
    use snark_verifier::system::halo2::compile;

    manifest.validate()?;
    let eq_protocol = compile(
        step_eq_params,
        step_eq_precommit_verifying_key,
        kagemusha_ipa_compile_config_v4(KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7),
    );
    let ep_protocol = compile(
        step_ep_params,
        step_ep_precommit_verifying_key,
        kagemusha_ipa_compile_config_v4(KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7),
    );
    if kagemusha_serialized_params_sha256_v7(step_eq_params)? != manifest.step_eq_params_sha256
        || kagemusha_serialized_params_sha256_v7(step_ep_params)? != manifest.step_ep_params_sha256
        || kagemusha_compiled_protocol_identity_sha256(
            &eq_protocol,
            KagemushaPastaCycleParityV1::StepEq,
        )? != manifest.step_eq_vk_sha256
        || kagemusha_compiled_protocol_identity_sha256(
            &ep_protocol,
            KagemushaPastaCycleParityV1::StepEp,
        )? != manifest.step_ep_vk_sha256
    {
        return Err("Kagemusha V7 precommit ParamsIPA/VK identity mismatch".to_owned());
    }
    drop(eq_protocol);
    drop(ep_protocol);
    if eq_coefficients.len()
        != usize::try_from(manifest.eq_coefficient_count)
            .map_err(|_| "Kagemusha V7 Eq coefficient count does not fit usize".to_owned())?
        || ep_coefficients.len()
            != usize::try_from(manifest.ep_coefficient_count)
                .map_err(|_| "Kagemusha V7 Ep coefficient count does not fit usize".to_owned())?
    {
        return Err("Kagemusha V7 host coefficient counts differ from the manifest".to_owned());
    }
    let (step_eq_seed, step_ep_seed) = kagemusha_fresh_serialized_role_seeds_v7()?;
    let placeholder = KagemushaSerializedAuditPublicJoinV7 {
        step_eq_commitment: kagemusha_serialized_bytes_to_chunks_v7(
            EqAffine::generator()
                .to_bytes()
                .as_ref()
                .try_into()
                .expect("Pasta point encoding is 32 bytes"),
        ),
        step_ep_commitment: kagemusha_serialized_bytes_to_chunks_v7(
            EpAffine::generator()
                .to_bytes()
                .as_ref()
                .try_into()
                .expect("Pasta point encoding is 32 bytes"),
        ),
        challenge: [2, 0],
        eq_evaluation: [0, 0],
        ep_evaluation: [0, 0],
    };

    // Precommit Eq and Ep in separate populated-builder passes using only the
    // small authenticated VKs. No multi-GiB proving key is loaded until its
    // parity's final one-pass proof is ready to consume it.
    let step_eq_commitment = {
        let (circuit, _) = build_step_eq(placeholder)?;
        if circuit.params.manifest != *manifest {
            return Err("Kagemusha V7 Eq prepass manifest drifted".to_owned());
        }
        let selected = usize::try_from(manifest.step_eq_serialized_column)
            .map_err(|_| "Kagemusha V7 Eq serialized column does not fit usize".to_owned())?;
        let point = circuit.serialized_jobs.precommitment(
            step_eq_params,
            step_eq_precommit_verifying_key,
            selected,
            iroha_crypto::rng_from_seed_slice(&step_eq_seed.0),
        )?;
        let bytes = point.to_bytes();
        let bytes: [u8; 32] = bytes
            .as_ref()
            .try_into()
            .expect("Pasta point encoding is 32 bytes");
        drop(circuit);
        halo2_proofs::release_allocator_slack();
        bytes
    };
    let step_ep_commitment = {
        let (circuit, _) = build_step_ep(placeholder)?;
        if circuit.params.manifest != *manifest {
            return Err("Kagemusha V7 Ep prepass manifest drifted".to_owned());
        }
        let selected = usize::try_from(manifest.step_ep_serialized_column)
            .map_err(|_| "Kagemusha V7 Ep serialized column does not fit usize".to_owned())?;
        let point = circuit.serialized_jobs.precommitment(
            step_ep_params,
            step_ep_precommit_verifying_key,
            selected,
            iroha_crypto::rng_from_seed_slice(&step_ep_seed.0),
        )?;
        let bytes = point.to_bytes();
        let bytes: [u8; 32] = bytes
            .as_ref()
            .try_into()
            .expect("Pasta point encoding is 32 bytes");
        drop(circuit);
        halo2_proofs::release_allocator_slack();
        bytes
    };
    let challenge_context = KagemushaSerializedAuditChallengeContextV7::from_manifest(
        manifest,
        frozen_core_statement_sha256,
    )?;
    let challenge_bytes = kagemusha_serialized_audit_challenge_v7(
        &challenge_context,
        step_eq_commitment,
        step_ep_commitment,
    )?;
    let challenge_chunks = kagemusha_serialized_bytes_to_chunks_v7(challenge_bytes);
    let challenge_eq = kagemusha_serialized_scalar_from_chunks_v7::<Fp>(challenge_chunks)
        .ok_or_else(|| "Kagemusha V7 Eq challenge is noncanonical".to_owned())?;
    let challenge_ep = kagemusha_serialized_scalar_from_chunks_v7::<Fq>(challenge_chunks)
        .ok_or_else(|| "Kagemusha V7 Ep challenge is noncanonical".to_owned())?;
    let eq_evaluation =
        kagemusha_audit_polynomial_evaluate_v7(eq_coefficients, challenge_eq).to_repr();
    let ep_evaluation =
        kagemusha_audit_polynomial_evaluate_v7(ep_coefficients, challenge_ep).to_repr();
    let join = KagemushaSerializedAuditPublicJoinV7 {
        step_eq_commitment: kagemusha_serialized_bytes_to_chunks_v7(step_eq_commitment),
        step_ep_commitment: kagemusha_serialized_bytes_to_chunks_v7(step_ep_commitment),
        challenge: challenge_chunks,
        eq_evaluation: kagemusha_serialized_bytes_to_chunks_v7(
            eq_evaluation
                .as_ref()
                .try_into()
                .expect("Pasta scalar encoding is 32 bytes"),
        ),
        ep_evaluation: kagemusha_serialized_bytes_to_chunks_v7(
            ep_evaluation
                .as_ref()
                .try_into()
                .expect("Pasta scalar encoding is 32 bytes"),
        ),
    };
    validate_kagemusha_serialized_public_join_v7(&join)?;

    let step_eq_proving_key = load_step_eq_proving_key()?;
    let (step_eq_circuit, step_eq_instances) = build_step_eq(join)?;
    let (step_eq_proof, step_eq_verifying_key) = create_kagemusha_serialized_eq_proof_v7(
        step_eq_params,
        step_eq_proving_key,
        step_eq_circuit,
        &step_eq_instances,
        manifest,
        &step_eq_seed.0,
        step_eq_commitment,
        max_proof_bytes,
    )?;
    halo2_proofs::release_allocator_slack();

    let step_ep_proving_key = load_step_ep_proving_key()?;
    let (step_ep_circuit, step_ep_instances) = build_step_ep(join)?;
    let (step_ep_proof, step_ep_verifying_key) = create_kagemusha_serialized_ep_proof_v7(
        step_ep_params,
        step_ep_proving_key,
        step_ep_circuit,
        &step_ep_instances,
        manifest,
        &step_ep_seed.0,
        step_ep_commitment,
        max_proof_bytes,
    )?;
    halo2_proofs::release_allocator_slack();

    let verified = verify_kagemusha_serialized_atomic_pair_v7(
        step_eq_params,
        &step_eq_verifying_key,
        step_ep_params,
        &step_ep_verifying_key,
        manifest,
        &step_eq_proof,
        &step_ep_proof,
        &step_eq_instances,
        &step_ep_instances,
        max_proof_bytes,
        max_pair_bytes,
    )?;
    Ok(KagemushaSerializedCreatedPairV7 {
        step_eq_proof,
        step_ep_proof,
        step_eq_instances,
        step_ep_instances,
        step_eq_verifying_key,
        step_ep_verifying_key,
        verified,
    })
}
