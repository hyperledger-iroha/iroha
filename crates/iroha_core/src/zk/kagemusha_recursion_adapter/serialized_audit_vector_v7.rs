// Canonical vector construction for the serialized-advice V7 audit join.
//
// This module deliberately contains no standalone opening, alternative
// commitment, artifact decoder, or promotion path.

const KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_DOMAIN_V7: &[u8] =
    b"iroha:kagemusha:canonical-audit-vector:v7";
const KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_VERSION_V7: u32 = 7;
const KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_MAX_K_V7: u32 = 14;

trait KagemushaSerializedAuditCurveV7: halo2_base::utils::CurveAffineExt {
    const PARITY: KagemushaPastaCycleParityV1;
}

impl KagemushaSerializedAuditCurveV7 for halo2_proofs::halo2curves::pasta::EqAffine {
    const PARITY: KagemushaPastaCycleParityV1 = KagemushaPastaCycleParityV1::StepEq;
}

impl KagemushaSerializedAuditCurveV7 for halo2_proofs::halo2curves::pasta::EpAffine {
    const PARITY: KagemushaPastaCycleParityV1 = KagemushaPastaCycleParityV1::StepEp;
}

fn kagemusha_serialized_audit_stage_plan_v7<C>(
    output: &KagemushaScalarAuditOutputV4<C>,
    current_parent_count: u32,
) -> Result<(Vec<u32>, Vec<bool>), String>
where
    C: KagemushaSerializedAuditCurveV7,
{
    if output.identity.parity != C::PARITY
        || current_parent_count > KAGEMUSHA_PASTA_PARENT_SLOTS_V1 as u32
        || output
            .inner_parent_counts
            .iter()
            .any(|count| *count > KAGEMUSHA_PASTA_PARENT_SLOTS_V1 as u32)
    {
        return Err("Kagemusha V7 audit-vector parent count is invalid".to_owned());
    }
    scalar_lineage_v1::validate_stage_shapes_v4(&output.stages, output.audit.equations.len())
        .map_err(|error| format!("invalid Kagemusha V7 audit-vector stage plan: {error:?}"))?;
    let slot_present = [current_parent_count >= 1, current_parent_count == 2];
    let parent_has_carried = output.inner_parent_counts.map(|count| count != 0);
    let mut gate_tags = vec![0_u32; output.audit.equations.len()];
    let mut selectors = vec![false; output.audit.equations.len()];
    for stage in &output.stages {
        let enabled = match stage.gate {
            scalar_lineage_v1::DeferredEquationGateV4::ParentCurrent { slot }
            | scalar_lineage_v1::DeferredEquationGateV4::ParentLineageSelect { slot } => {
                slot_present[slot]
            }
            scalar_lineage_v1::DeferredEquationGateV4::ParentCarriedFold { slot } => {
                slot_present[slot] && parent_has_carried[slot]
            }
            scalar_lineage_v1::DeferredEquationGateV4::BranchFold => slot_present[1],
            scalar_lineage_v1::DeferredEquationGateV4::BranchSelect => slot_present[0],
        };
        for equation_index in stage.range.clone() {
            gate_tags[equation_index] = stage.gate.audit_tag();
            selectors[equation_index] = enabled;
        }
    }
    Ok((gate_tags, selectors))
}

fn kagemusha_canonical_audit_polynomial_len_v7(
    sources: usize,
    equations: usize,
    terms: usize,
    stages: usize,
    protocol_points: usize,
) -> Result<usize, String> {
    let domain = 2_usize
        .checked_add(
            KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_DOMAIN_V7
                .len()
                .div_ceil(16),
        )
        .ok_or_else(|| "Kagemusha V7 audit-vector domain length overflowed".to_owned())?;
    [
        domain,
        12,
        stages
            .checked_mul(3)
            .ok_or_else(|| "Kagemusha V7 audit-vector stage contribution overflowed".to_owned())?,
        sources
            .checked_mul(2)
            .ok_or_else(|| "Kagemusha V7 audit-vector source contribution overflowed".to_owned())?,
        equations.checked_mul(3).ok_or_else(|| {
            "Kagemusha V7 audit-vector equation contribution overflowed".to_owned()
        })?,
        terms
            .checked_mul(2)
            .ok_or_else(|| "Kagemusha V7 audit-vector term contribution overflowed".to_owned())?,
        protocol_points,
    ]
    .into_iter()
    .try_fold(0_usize, |total, contribution| {
        total
            .checked_add(contribution)
            .ok_or_else(|| "Kagemusha V7 audit-vector length overflowed".to_owned())
    })
}

/// Build the injective, canonical coefficient vector shared by both Pasta
/// halves. Protocol points are represented by their strictly increasing audit
/// source indices, avoiding the duplicate point encodings in V6 while binding
/// exactly the same points.
fn kagemusha_canonical_audit_polynomial_v7<C>(
    output: &KagemushaScalarAuditOutputV4<C>,
    current_parent_count: u32,
) -> Result<Vec<C::ScalarExt>, String>
where
    C: KagemushaSerializedAuditCurveV7,
    C::ScalarExt: halo2_base::utils::BigPrimeField,
{
    let scalar = |value: usize, role: &str| {
        u64::try_from(value)
            .map(C::ScalarExt::from)
            .map_err(|_| format!("Kagemusha V7 audit-vector {role} does not fit u64"))
    };
    let (gate_tags, selectors) =
        kagemusha_serialized_audit_stage_plan_v7(output, current_parent_count)?;
    if output.audit.sources.is_empty()
        || output.audit.equations.is_empty()
        || output.identity.preprocessed.len() != output.identity.preprocessed_source_indices.len()
    {
        return Err("Kagemusha V7 audit-vector source shape is invalid".to_owned());
    }
    let term_count = output
        .audit
        .equations
        .iter()
        .try_fold(0_usize, |total, equation| {
            if equation.is_empty() {
                return Err("Kagemusha V7 audit-vector contains an empty equation".to_owned());
            }
            total.checked_add(equation.len()).ok_or_else(|| {
                "Kagemusha V7 audit-vector equation-term count overflowed".to_owned()
            })
        })?;
    let mut elements =
        super::kagemusha_cycle_loader::kagemusha_poseidon_domain_elements::<C::ScalarExt>(
            KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_DOMAIN_V7,
            KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_VERSION_V7,
        );
    elements.extend([
        C::ScalarExt::from(u64::from(protocol_parity_tag(output.identity.parity))),
        C::ScalarExt::from(u64::from(current_parent_count)),
        C::ScalarExt::from(u64::from(output.inner_parent_counts[0])),
        C::ScalarExt::from(u64::from(output.inner_parent_counts[1])),
        scalar(output.audit.sources.len(), "source count")?,
        scalar(output.audit.equations.len(), "equation count")?,
        scalar(term_count, "term count")?,
        scalar(output.stages.len(), "stage count")?,
        scalar(
            output.identity.preprocessed.len(),
            "compiled-protocol point count",
        )?,
    ]);
    elements.extend(
        kagemusha_bytes_to_u128_chunks_v5(output.identity.structure_sha256)
            .map(C::ScalarExt::from_u128),
    );
    elements.push(output.identity.transcript_initial_state);
    for stage in &output.stages {
        elements.extend([
            C::ScalarExt::from(u64::from(stage.gate.audit_tag())),
            scalar(stage.range.start, "stage start")?,
            scalar(stage.range.end, "stage end")?,
        ]);
    }
    for source in &output.audit.sources {
        elements.extend(kagemusha_compressed_point_poseidon_elements(*source)?);
    }
    for (equation_index, equation) in output.audit.equations.iter().enumerate() {
        elements.extend([
            C::ScalarExt::from(u64::from(gate_tags[equation_index])),
            C::ScalarExt::from(u64::from(selectors[equation_index])),
            scalar(equation.len(), "equation term count")?,
        ]);
        let mut previous_source = None;
        for (source_index, coefficient) in equation {
            if *source_index >= output.audit.sources.len()
                || previous_source.is_some_and(|previous| previous >= *source_index)
            {
                return Err("Kagemusha V7 audit-vector equation source order is invalid".to_owned());
            }
            previous_source = Some(*source_index);
            elements.extend([
                scalar(*source_index, "equation source index")?,
                *coefficient,
            ]);
        }
    }
    let mut previous_source = None;
    for (point, source_index) in output
        .identity
        .preprocessed
        .iter()
        .zip(output.identity.preprocessed_source_indices.iter().copied())
    {
        if source_index >= output.audit.sources.len()
            || previous_source.is_some_and(|previous| previous >= source_index)
            || point.to_bytes().as_ref() != output.audit.sources[source_index].to_bytes().as_ref()
        {
            return Err("Kagemusha V7 compiled-protocol source map is invalid".to_owned());
        }
        previous_source = Some(source_index);
        elements.push(scalar(source_index, "compiled-protocol source index")?);
    }
    let expected_len = kagemusha_canonical_audit_polynomial_len_v7(
        output.audit.sources.len(),
        output.audit.equations.len(),
        term_count,
        output.stages.len(),
        output.identity.preprocessed.len(),
    )?;
    if elements.len() != expected_len {
        return Err(format!(
            "Kagemusha V7 canonical audit vector has {} coefficients instead of {expected_len}",
            elements.len(),
        ));
    }
    let maximum = 1_usize << KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_MAX_K_V7;
    if elements.len() > maximum {
        return Err(format!(
            "Kagemusha V7 canonical audit vector has {} coefficients, above the k{} capacity {maximum}",
            elements.len(),
            KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_MAX_K_V7,
        ));
    }
    Ok(elements)
}

fn kagemusha_audit_polynomial_evaluate_v7<F: ff::Field>(coefficients: &[F], point: F) -> F {
    coefficients
        .iter()
        .rev()
        .fold(F::ZERO, |accumulator, coefficient| {
            accumulator * point + coefficient
        })
}

fn constrain_kagemusha_native_audit_evaluation_v7<F>(
    ctx: &mut halo2_base::Context<F>,
    gate: &halo2_base::gates::GateChip<F>,
    coefficients: &[halo2_base::AssignedValue<F>],
    point: halo2_base::AssignedValue<F>,
    expected: halo2_base::AssignedValue<F>,
) -> Result<(), String>
where
    F: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{QuantumCell::Existing, gates::GateInstructions as _};

    let Some((&last, remaining)) = coefficients.split_last() else {
        return Err("Kagemusha V7 native audit polynomial is empty".to_owned());
    };
    let evaluation = remaining
        .iter()
        .rev()
        .fold(last, |accumulator, coefficient| {
            gate.mul_add(
                ctx,
                Existing(accumulator),
                Existing(point),
                Existing(*coefficient),
            )
        });
    ctx.constrain_equal(&evaluation, &expected);
    Ok(())
}

struct KagemushaReviewedNativeAuditSourceV7<F: ff::PrimeField>(Vec<halo2_base::AssignedValue<F>>);

impl<F: ff::PrimeField> super::kagemusha_serialized_audit_v7::KagemushaNativeAuditVectorSourceV7<F>
    for KagemushaReviewedNativeAuditSourceV7<F>
{
    fn into_reviewed_coefficients(self) -> Vec<halo2_base::AssignedValue<F>> {
        self.0
    }
}

fn assigned_kagemusha_native_audit_polynomial_v7<C>(
    loader: &std::rc::Rc<
        snark_verifier::loader::halo2::Halo2Loader<
            C,
            super::kagemusha_cycle_loader::DeferredScalarEccChip<'_, C>,
        >,
    >,
    current_parent_count: halo2_base::AssignedValue<C::ScalarExt>,
    inner_parent_counts: [halo2_base::AssignedValue<C::ScalarExt>; 2],
    stages: &[scalar_lineage_v1::AssignedDeferredEquationStageV4<C::ScalarExt>],
    identity: &scalar_lineage_v1::DeferredProtocolIdentityWitness<C>,
    profile: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7,
) -> Result<
    super::kagemusha_serialized_audit_v7::KagemushaFrozenNativeAuditVectorV7<C::ScalarExt>,
    String,
>
where
    C: KagemushaSerializedAuditCurveV7,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7,
{
    let audit = loader.ecc_chip().witness();
    scalar_lineage_v1::validate_stage_shapes_v4(
        &stages
            .iter()
            .map(scalar_lineage_v1::AssignedDeferredEquationStageV4::shape)
            .collect::<Vec<_>>(),
        audit.equations.len(),
    )
    .map_err(|error| format!("invalid assigned Kagemusha V7 stage plan: {error:?}"))?;
    if identity.parity != C::PARITY
        || identity.preprocessed.len() != identity.preprocessed_source_indices.len()
        || identity.preprocessed.is_empty()
    {
        return Err("Kagemusha V7 assigned audit identity shape mismatch".to_owned());
    }
    let term_count = audit
        .equations
        .iter()
        .try_fold(0_usize, |total, equation| {
            total
                .checked_add(equation.len())
                .ok_or_else(|| "Kagemusha V7 assigned audit term count overflowed".to_owned())
        })?;
    let mut gate_tags = Vec::with_capacity(audit.equations.len());
    let mut selectors = Vec::with_capacity(audit.equations.len());
    for stage in stages {
        gate_tags.extend(std::iter::repeat_n(
            stage.gate.audit_tag(),
            stage.range.len(),
        ));
        selectors.extend(std::iter::repeat_n(stage.enabled, stage.range.len()));
    }
    let v6_elements = loader
        .ecc_chip()
        .assigned_equation_poseidon_elements_v6(&mut loader.ctx_mut(), &gate_tags, &selectors)
        .map_err(|error| format!("failed to assign Kagemusha V7 audit tail: {error:?}"))?;
    let v6_prefix =
        super::kagemusha_cycle_loader::kagemusha_poseidon_domain_elements::<C::ScalarExt>(
            super::kagemusha_cycle_loader::KAGEMUSHA_DEFERRED_AUDIT_POSEIDON_DOMAIN_V6,
            super::kagemusha_cycle_loader::KAGEMUSHA_DEFERRED_AUDIT_VERSION_V6,
        )
        .len()
            + 2;
    if v6_elements.len() <= v6_prefix {
        return Err("Kagemusha V7 assigned audit tail is empty".to_owned());
    }

    let mut elements =
        super::kagemusha_cycle_loader::kagemusha_poseidon_domain_elements::<C::ScalarExt>(
            KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_DOMAIN_V7,
            KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_VERSION_V7,
        )
        .into_iter()
        .map(|value| loader.ctx_mut().main().load_constant(value))
        .collect::<Vec<_>>();
    let constant = |value: u64| {
        loader
            .ctx_mut()
            .main()
            .load_constant(C::ScalarExt::from(value))
    };
    elements.extend([
        constant(u64::from(protocol_parity_tag(identity.parity))),
        current_parent_count,
        inner_parent_counts[0],
        inner_parent_counts[1],
        constant(
            u64::try_from(audit.sources.len())
                .map_err(|_| "Kagemusha V7 assigned source count does not fit u64".to_owned())?,
        ),
        constant(
            u64::try_from(audit.equations.len())
                .map_err(|_| "Kagemusha V7 assigned equation count does not fit u64".to_owned())?,
        ),
        constant(
            u64::try_from(term_count)
                .map_err(|_| "Kagemusha V7 assigned term count does not fit u64".to_owned())?,
        ),
        constant(
            u64::try_from(stages.len())
                .map_err(|_| "Kagemusha V7 assigned stage count does not fit u64".to_owned())?,
        ),
        constant(u64::try_from(identity.preprocessed.len()).map_err(|_| {
            "Kagemusha V7 assigned protocol-point count does not fit u64".to_owned()
        })?),
    ]);
    elements.extend(
        kagemusha_bytes_to_u128_chunks_v5(identity.structure_sha256).map(|value| {
            loader
                .ctx_mut()
                .main()
                .load_constant(C::ScalarExt::from_u128(value))
        }),
    );
    elements.push(
        loader
            .ctx_mut()
            .main()
            .load_constant(identity.transcript_initial_state),
    );
    for stage in stages {
        elements.extend([
            constant(u64::from(stage.gate.audit_tag())),
            constant(
                u64::try_from(stage.range.start)
                    .map_err(|_| "Kagemusha V7 assigned stage start does not fit u64".to_owned())?,
            ),
            constant(
                u64::try_from(stage.range.end)
                    .map_err(|_| "Kagemusha V7 assigned stage end does not fit u64".to_owned())?,
            ),
        ]);
    }
    elements.extend(v6_elements.into_iter().skip(v6_prefix));
    let mut previous = None;
    for (point, source_index) in identity
        .preprocessed
        .iter()
        .zip(identity.preprocessed_source_indices.iter().copied())
    {
        if source_index >= audit.sources.len()
            || previous.is_some_and(|previous| previous >= source_index)
            || point.to_bytes().as_ref() != audit.sources[source_index].to_bytes().as_ref()
        {
            return Err("Kagemusha V7 assigned protocol-source map is invalid".to_owned());
        }
        previous = Some(source_index);
        elements.push(constant(u64::try_from(source_index).map_err(|_| {
            "Kagemusha V7 assigned protocol-source index does not fit u64".to_owned()
        })?));
    }
    let expected = kagemusha_canonical_audit_polynomial_len_v7(
        audit.sources.len(),
        audit.equations.len(),
        term_count,
        stages.len(),
        identity.preprocessed.len(),
    )?;
    if elements.len() != expected {
        return Err(format!(
            "Kagemusha V7 assigned audit vector has {} coefficients instead of {expected}",
            elements.len()
        ));
    }
    super::kagemusha_serialized_audit_v7::KagemushaFrozenNativeAuditVectorV7::from_reviewed_source(
        profile,
        KagemushaReviewedNativeAuditSourceV7(elements),
    )
}

/// Bind the actual native assigned vector to `D`, then evaluate it only after
/// the public `D/C` commitments have entered the transcript.
fn constrain_kagemusha_reciprocal_audit_evaluation_v7<C>(
    ctx: &mut halo2_base::gates::flex_gate::threads::SinglePhaseCoreManager<C::Base>,
    scalar: &halo2_ecc::fields::fp::FpChip<'_, C::Base, C::ScalarExt>,
    coefficients: &[halo2_ecc::bigint::ProperCrtUint<C::Base>],
    point: &halo2_ecc::bigint::ProperCrtUint<C::Base>,
    expected: &halo2_ecc::bigint::ProperCrtUint<C::Base>,
) -> Result<(), String>
where
    C: halo2_base::utils::CurveAffineExt,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: halo2_base::utils::BigPrimeField,
{
    use halo2_ecc::fields::FieldChip as _;

    let Some((last, remaining)) = coefficients.split_last() else {
        return Err("Kagemusha V7 reciprocal audit polynomial is empty".to_owned());
    };
    let evaluation = remaining
        .iter()
        .rev()
        .fold(last.clone(), |accumulator, coefficient| {
            let product = scalar.mul_no_carry(ctx.main(), accumulator, point);
            let sum = scalar.add_no_carry(ctx.main(), product, coefficient);
            scalar.carry_mod(ctx.main(), sum)
        });
    scalar.assert_equal(ctx.main(), evaluation, expected);
    Ok(())
}

struct KagemushaAssignedSerializedParentV7<F: ff::PrimeField> {
    present: halo2_base::AssignedValue<F>,
    instance_cells: Vec<halo2_base::AssignedValue<F>>,
    commitment_chunks: [halo2_base::AssignedValue<F>; 2],
}

struct KagemushaAssignedNativeAuditV7<C>
where
    C: KagemushaSerializedAuditCurveV7,
{
    output: KagemushaScalarAuditOutputV4<C>,
    vector: super::kagemusha_serialized_audit_v7::KagemushaFrozenNativeAuditVectorV7<C::ScalarExt>,
    parents: [KagemushaAssignedSerializedParentV7<C::ScalarExt>; 2],
}

fn assign_kagemusha_parity_native_vector_v7<C>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<C::ScalarExt>,
    sha_jobs: &mut KagemushaSha256JobsV4<C::ScalarExt>,
    public_cells: &[halo2_base::AssignedValue<C::ScalarExt>],
    params: &KagemushaStepCircuitParamsV4,
    layout: &KagemushaPastaPublicLayoutV4,
    recursion: &KagemushaStepParityRecursionV4<C>,
    audit_profile: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7,
    serialized_phase_zero_rank: usize,
) -> Result<KagemushaAssignedNativeAuditV7<C>, String>
where
    C: KagemushaSerializedAuditCurveV7,
    C::Base: halo2_base::utils::BigPrimeField,
    C::ScalarExt: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7
        + halo2_base::utils::ScalarField
        + ff::FromUniformBytes<64>,
{
    use super::kagemusha_cycle_loader::{DeferredScalarEccChip, LIMB_BITS, LIMBS};
    use halo2_base::gates::RangeInstructions as _;
    use halo2_ecc::fields::fp::FpChip;
    use snark_verifier::loader::halo2::Halo2Loader;

    let public_len = usize::try_from(layout.instance_column_limbs)
        .map_err(|_| "Kagemusha V7 public length does not fit usize".to_owned())?;
    let accumulator_limbs = usize::try_from(layout.accumulator_limbs)
        .map_err(|_| "Kagemusha V7 accumulator length does not fit usize".to_owned())?;
    if public_cells.len() != public_len
        || recursion
            .parents
            .iter()
            .any(|parent| parent.instances.len() != 1 || parent.instances[0].len() != public_len)
    {
        return Err("Kagemusha V7 fixed parent-instance shape mismatch".to_owned());
    }
    let own_protocol_offset = match C::PARITY {
        KagemushaPastaCycleParityV1::StepEq => KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5,
        KagemushaPastaCycleParityV1::StepEp => KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5,
    };
    let carried_offset = usize::try_from(match C::PARITY {
        KagemushaPastaCycleParityV1::StepEq => layout.parent_eq_accumulator_offset,
        KagemushaPastaCycleParityV1::StepEp => layout.parent_ep_accumulator_offset,
    })
    .map_err(|_| "Kagemusha V7 carried offset does not fit usize".to_owned())?;
    let max_parent_proof_bytes = usize::try_from(params.max_parent_proof_bytes)
        .map_err(|_| "Kagemusha V7 proof bound does not fit usize".to_owned())?;
    let range = builder.range_chip();
    let coordinate = FpChip::<C::ScalarExt, C::Base>::new(&range, LIMB_BITS, LIMBS);
    let scalar_integer = FpChip::<C::ScalarExt, C::ScalarExt>::new(&range, LIMB_BITS, LIMBS);
    let chip = DeferredScalarEccChip::<C>::new(&coordinate, &scalar_integer);
    let loader = Halo2Loader::new(chip, std::mem::take(builder.pool(0)));
    let loaded_protocol = scalar_lineage_v1::load_and_constrain_parent_protocol(
        &loader,
        sha_jobs,
        &recursion.compiled_parent_protocol,
        C::PARITY,
        recursion.fixed_structure_sha256,
        &public_cells[own_protocol_offset..own_protocol_offset + 2],
    )
    .map_err(|error| format!("failed to bind Kagemusha V7 parent protocol: {error:?}"))?;
    let parent_count = public_cells[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5];
    let slot_present = scalar_lineage_v1::constrain_parent_slot_selectors_v4(&loader, parent_count);
    let mut lineages = Vec::with_capacity(2);
    let mut inner_parent_counts = [0_u32; 2];
    for slot in 0..2 {
        let parent = &recursion.parents[slot];
        inner_parent_counts[slot] = scalar_field_parent_count_v4(
            parent.instances[0][KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5],
        )?;
        let bindings = [
            scalar_lineage_v1::ParentInstanceCopyBindingV4 {
                column: 0,
                source: KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_RESULT_STATE_COMMITMENT_OFFSET_V5 + 2,
                expected: &public_cells[KAGEMUSHA_COMPACT_PARENT_STATE_COMMITMENTS_OFFSET_V5
                    + slot * 2
                    ..KAGEMUSHA_COMPACT_PARENT_STATE_COMMITMENTS_OFFSET_V5 + (slot + 1) * 2],
            },
            scalar_lineage_v1::ParentInstanceCopyBindingV4 {
                column: 0,
                source: KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2,
                expected: &public_cells[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2],
            },
            scalar_lineage_v1::ParentInstanceCopyBindingV4 {
                column: 0,
                source: KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 4,
                expected: &public_cells[KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
                    ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 4],
            },
        ];
        let lineage = scalar_lineage_v1::constrain_parent_scalar_lineage_v4(
            &loader,
            &recursion.succinct_vk,
            &loaded_protocol,
            slot,
            Some(serialized_phase_zero_rank),
            slot_present[slot],
            params.k,
            max_parent_proof_bytes,
            accumulator_limbs,
            scalar_lineage_v1::ParentScalarLineageWitnessV4 {
                instances: &parent.instances,
                proof_bytes: &parent.proof_bytes,
                carried_lineage: &parent.carried_lineage,
                carried_lineage_instance_column: 0,
                carried_lineage_instance_range: carried_offset..carried_offset + accumulator_limbs,
                instance_copy_bindings: &bindings,
                external_accumulation_proof: &parent.external_accumulation_proof,
            },
        )
        .map_err(|error| {
            format!("failed to constrain Kagemusha V7 parent slot {slot}: {error:?}")
        })?;
        lineages.push(lineage);
    }
    let parents = lineages
        .iter()
        .enumerate()
        .map(|(slot, lineage)| {
            let point = lineage
                .serialized_advice_commitment
                .as_ref()
                .ok_or_else(|| {
                    format!("Kagemusha V7 parent slot {slot} omitted its serialized commitment")
                })?;
            let bytes = loader
                .ecc_chip()
                .assigned_point_bytes(&mut loader.ctx_mut(), point)
                .map_err(|error| {
                    format!("failed to bind Kagemusha V7 parent slot {slot} commitment: {error:?}")
                })?;
            let ecc_chip = loader.ecc_chip();
            let gate = ecc_chip.range().gate();
            let commitment_chunks = [
                super::kagemusha_serialized_audit_v7::pack_assigned_le_bytes_v7(
                    loader.ctx_mut().main(),
                    gate,
                    &bytes[..16],
                ),
                super::kagemusha_serialized_audit_v7::pack_assigned_le_bytes_v7(
                    loader.ctx_mut().main(),
                    gate,
                    &bytes[16..],
                ),
            ];
            Ok(KagemushaAssignedSerializedParentV7 {
                present: slot_present[slot],
                instance_cells: lineage.verified_instance_cells.clone(),
                commitment_chunks,
            })
        })
        .collect::<Result<Vec<_>, String>>()?
        .try_into()
        .map_err(|_| "Kagemusha V7 parent trace count drifted".to_owned())?;
    let branch = scalar_lineage_v1::constrain_exposed_parent_lineage_v4(
        &loader,
        &recursion.succinct_vk,
        params.k,
        accumulator_limbs,
        &lineages[0].accumulator,
        &lineages[1].accumulator,
        slot_present,
        &recursion.branch_merge_fold,
        &public_cells[carried_offset..carried_offset + accumulator_limbs],
    )
    .map_err(|error| format!("failed to constrain Kagemusha V7 branch lineage: {error:?}"))?;
    let mut all_stages = lineages
        .iter()
        .flat_map(|lineage| lineage.stages.iter().cloned())
        .collect::<Vec<_>>();
    all_stages.extend(branch.stages.iter().cloned());
    let complete_audit = loader.ecc_chip().witness();
    let complete_shapes = all_stages.iter().map(|stage| stage.shape()).collect();
    let identity = loaded_protocol.identity_witness.clone();
    let assigned_vector = assigned_kagemusha_native_audit_polynomial_v7(
        &loader,
        parent_count,
        [lineages[0].parent_count, lineages[1].parent_count],
        &all_stages,
        &identity,
        audit_profile,
    )?;
    *builder.pool(0) = loader.take_ctx();
    Ok(KagemushaAssignedNativeAuditV7 {
        output: KagemushaScalarAuditOutputV4 {
            identity,
            audit: complete_audit,
            stages: complete_shapes,
            inner_parent_counts,
        },
        vector: assigned_vector,
        parents,
    })
}

fn assigned_kagemusha_reciprocal_audit_polynomial_v7<'chip, C>(
    ctx: &mut halo2_base::gates::flex_gate::threads::SinglePhaseCoreManager<C::Base>,
    chip: &super::kagemusha_cycle_loader::PastaCycleEccChip<'chip, C>,
    scalar: &'chip halo2_ecc::fields::fp::FpChip<'chip, C::Base, C::ScalarExt>,
    witness: &super::kagemusha_cycle_loader::DeferredEquationWitness<C>,
    stages: &[scalar_lineage_v1::DeferredEquationStageShapeV4],
    current_parent_count: halo2_base::AssignedValue<C::Base>,
    inner_parent_counts: [halo2_base::AssignedValue<C::Base>; 2],
    identity: &scalar_lineage_v1::DeferredProtocolIdentityWitness<C>,
    profile: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditProfileV7,
) -> Result<
    (
        Vec<halo2_ecc::bigint::ProperCrtUint<C::Base>>,
        super::kagemusha_serialized_audit_v7::KagemushaFrozenReciprocalAuditVectorV7<C::Base>,
        super::kagemusha_cycle_loader::AssignedDeferredPointAudit<C>,
        Vec<halo2_base::AssignedValue<C::Base>>,
    ),
    String,
>
where
    C: KagemushaSerializedAuditCurveV7,
    C::Base: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7<
            ReciprocalScalar = C::ScalarExt,
        >,
    C::ScalarExt: halo2_base::utils::BigPrimeField,
{
    use halo2_base::{
        QuantumCell::Existing,
        gates::{GateInstructions as _, RangeInstructions as _},
    };
    use halo2_ecc::fields::FieldChip as _;

    scalar_lineage_v1::validate_stage_shapes_v4(stages, witness.equations.len())
        .map_err(|error| format!("invalid reciprocal Kagemusha V7 stage plan: {error:?}"))?;
    if identity.parity != C::PARITY
        || identity.preprocessed.len() != identity.preprocessed_source_indices.len()
        || identity.preprocessed.is_empty()
    {
        return Err("Kagemusha V7 reciprocal audit identity shape mismatch".to_owned());
    }
    let slot_present =
        constrain_two_parent_presence_bits(ctx.main(), scalar.range, current_parent_count);
    let parent_has_carried = inner_parent_counts.map(|parent_count| {
        constrain_two_parent_presence_bits(ctx.main(), scalar.range, parent_count)[0]
    });
    let mut gate_tags = Vec::with_capacity(witness.equations.len());
    let mut selectors = Vec::with_capacity(witness.equations.len());
    for stage in stages {
        let enabled = match stage.gate {
            scalar_lineage_v1::DeferredEquationGateV4::ParentCurrent { slot }
            | scalar_lineage_v1::DeferredEquationGateV4::ParentLineageSelect { slot } => {
                slot_present[slot]
            }
            scalar_lineage_v1::DeferredEquationGateV4::ParentCarriedFold { slot } => {
                let enabled = scalar.range.gate().mul(
                    ctx.main(),
                    Existing(slot_present[slot]),
                    Existing(parent_has_carried[slot]),
                );
                scalar.range.gate().assert_bit(ctx.main(), enabled);
                enabled
            }
            scalar_lineage_v1::DeferredEquationGateV4::BranchFold => slot_present[1],
            scalar_lineage_v1::DeferredEquationGateV4::BranchSelect => slot_present[0],
        };
        gate_tags.extend(std::iter::repeat_n(
            stage.gate.audit_tag(),
            stage.range.len(),
        ));
        selectors.extend(std::iter::repeat_n(enabled, stage.range.len()));
    }
    let audit = chip.assign_deferred_equations_with_selectors(ctx, witness, &selectors)?;
    let (v6_elements, _source_encodings) =
        chip.assigned_equation_poseidon_elements_v6(ctx, &audit, &gate_tags, &selectors)?;
    let v6_prefix =
        super::kagemusha_cycle_loader::kagemusha_poseidon_domain_elements::<C::ScalarExt>(
            super::kagemusha_cycle_loader::KAGEMUSHA_DEFERRED_AUDIT_POSEIDON_DOMAIN_V6,
            super::kagemusha_cycle_loader::KAGEMUSHA_DEFERRED_AUDIT_VERSION_V6,
        )
        .len()
            + 2;
    if v6_elements.len() <= v6_prefix {
        return Err("Kagemusha V7 reciprocal audit tail is empty".to_owned());
    }
    let term_count = witness
        .equations
        .iter()
        .try_fold(0_usize, |total, equation| {
            total
                .checked_add(equation.len())
                .ok_or_else(|| "Kagemusha V7 reciprocal audit term count overflowed".to_owned())
        })?;
    let mut elements =
        super::kagemusha_cycle_loader::kagemusha_poseidon_domain_elements::<C::ScalarExt>(
            KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_DOMAIN_V7,
            KAGEMUSHA_SERIALIZED_AUDIT_VECTOR_VERSION_V7,
        )
        .into_iter()
        .map(|value| scalar.load_constant(ctx.main(), value))
        .collect::<Vec<_>>();
    let current_parent_count =
        chip.assigned_native_as_scalar_integer(ctx, current_parent_count, 2)?;
    let inner_parent_counts = [
        chip.assigned_native_as_scalar_integer(ctx, inner_parent_counts[0], 2)?,
        chip.assigned_native_as_scalar_integer(ctx, inner_parent_counts[1], 2)?,
    ];
    macro_rules! constant {
        ($value:expr $(,)?) => {
            scalar.load_constant(ctx.main(), C::ScalarExt::from($value))
        };
    }
    elements.extend([
        constant!(u64::from(protocol_parity_tag(identity.parity))),
        current_parent_count,
        inner_parent_counts[0].clone(),
        inner_parent_counts[1].clone(),
        constant!(
            u64::try_from(witness.sources.len())
                .map_err(|_| "Kagemusha V7 reciprocal source count does not fit u64".to_owned())?,
        ),
        constant!(u64::try_from(witness.equations.len()).map_err(|_| {
            "Kagemusha V7 reciprocal equation count does not fit u64".to_owned()
        })?),
        constant!(
            u64::try_from(term_count).map_err(|_| {
                "Kagemusha V7 reciprocal term count does not fit u64".to_owned()
            })?
        ),
        constant!(
            u64::try_from(stages.len()).map_err(|_| {
                "Kagemusha V7 reciprocal stage count does not fit u64".to_owned()
            })?
        ),
        constant!(u64::try_from(identity.preprocessed.len()).map_err(|_| {
            "Kagemusha V7 reciprocal protocol-point count does not fit u64".to_owned()
        })?),
    ]);
    elements.extend(
        kagemusha_bytes_to_u128_chunks_v5(identity.structure_sha256)
            .map(|value| scalar.load_constant(ctx.main(), C::ScalarExt::from_u128(value))),
    );
    elements.push(scalar.load_constant(ctx.main(), identity.transcript_initial_state));
    for stage in stages {
        elements.extend([
            constant!(u64::from(stage.gate.audit_tag())),
            constant!(u64::try_from(stage.range.start).map_err(|_| {
                "Kagemusha V7 reciprocal stage start does not fit u64".to_owned()
            })?),
            constant!(u64::try_from(stage.range.end).map_err(|_| {
                "Kagemusha V7 reciprocal stage end does not fit u64".to_owned()
            })?),
        ]);
    }
    elements.extend(v6_elements.into_iter().skip(v6_prefix));
    let mut previous = None;
    for (point, source_index) in identity
        .preprocessed
        .iter()
        .zip(identity.preprocessed_source_indices.iter().copied())
    {
        if source_index >= witness.sources.len()
            || previous.is_some_and(|previous| previous >= source_index)
            || point.to_bytes().as_ref() != witness.sources[source_index].to_bytes().as_ref()
        {
            return Err("Kagemusha V7 reciprocal protocol-source map is invalid".to_owned());
        }
        previous = Some(source_index);
        elements.push(constant!(u64::try_from(source_index).map_err(|_| {
            "Kagemusha V7 reciprocal protocol-source index does not fit u64".to_owned()
        })?));
    }
    let expected = kagemusha_canonical_audit_polynomial_len_v7(
        witness.sources.len(),
        witness.equations.len(),
        term_count,
        stages.len(),
        identity.preprocessed.len(),
    )?;
    if elements.len() != expected {
        return Err(format!(
            "Kagemusha V7 reciprocal audit vector has {} coefficients instead of {expected}",
            elements.len()
        ));
    }
    let frozen =
        super::kagemusha_serialized_audit_v7::constrain_reciprocal_audit_vector_at_seam_v7(
            ctx.main(),
            scalar,
            profile,
            &elements,
        )?;
    Ok((elements, frozen, audit, selectors))
}
