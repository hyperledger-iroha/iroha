// Sound serialized-advice commitment candidate for the non-shipping V7 lab.
//
// The accepted V5 circuit and wire remain untouched.  This file owns the new
// 70-cell instance layout and the circuit configuration that retains the dense
// group-identity machine while adding exactly one equality-enabled phase-zero
// serialization column.

const KAGEMUSHA_SERIALIZED_PROFILE_VERSION_V7: u16 = 7;
const KAGEMUSHA_SERIALIZED_COMMON_HEADER_CELLS_V7: usize = 19;
const KAGEMUSHA_SERIALIZED_CURRENT_JOIN_CELLS_V7: usize = 10;
const KAGEMUSHA_SERIALIZED_PARENT_DIGEST_CELLS_V7: usize = 2;
const KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7: usize = 70;
const KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7: usize = 57;
const KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7: usize = 67;
const KAGEMUSHA_SERIALIZED_LIVE_SELECTOR_OFFSET_V7: usize = 69;
const KAGEMUSHA_SERIALIZED_BRIDGE_REVIEWED_V7: bool = false;
const KAGEMUSHA_SERIALIZED_DENSE_BATCH_DOMAIN_V7: &[u8] =
    b"iroha:kagemusha:v7:serialized-dense-msm-batch";

/// Fail closed until the selected V7 graph, artifacts, and pair API are reviewed.
fn require_kagemusha_serialized_bridge_release_review_v7() -> Result<(), String> {
    // TODO: Flip only with candidate-bound independent review and measured
    // four-role artifacts under every exact release envelope.
    if !KAGEMUSHA_SERIALIZED_BRIDGE_REVIEWED_V7 {
        return Err(
            "Kagemusha V7 serialized-advice bridge is review-blocked and non-shipping".to_owned(),
        );
    }
    Ok(())
}

/// New typed public layout; it does not reinterpret the retired V5 digest slots.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KagemushaSerializedPublicLayoutV7 {
    accumulator_limbs: usize,
    accumulator_offset: usize,
    current_join_offset: usize,
    parent_digest_offset: usize,
    live_selector_offset: usize,
    instance_column_cells: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KagemushaSerializedPublicModeV7 {
    Live,
    NullParent,
}

impl KagemushaSerializedPublicLayoutV7 {
    fn for_k17(base: &KagemushaPastaPublicLayoutV4) -> Result<Self, String> {
        let accumulator_limbs = usize::try_from(base.accumulator_limbs)
            .map_err(|_| "Kagemusha V7 accumulator length does not fit usize".to_owned())?;
        let accumulator_offset = usize::try_from(base.parent_eq_accumulator_offset)
            .map_err(|_| "Kagemusha V7 accumulator offset does not fit usize".to_owned())?;
        let layout = Self {
            accumulator_limbs,
            accumulator_offset,
            current_join_offset: KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7,
            parent_digest_offset: KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7,
            live_selector_offset: KAGEMUSHA_SERIALIZED_LIVE_SELECTOR_OFFSET_V7,
            instance_column_cells: KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7,
        };
        if base.ipa_round_count != KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4
            || accumulator_offset != KAGEMUSHA_SERIALIZED_COMMON_HEADER_CELLS_V7
            || accumulator_offset.checked_add(accumulator_limbs) != Some(layout.current_join_offset)
            || layout
                .current_join_offset
                .checked_add(KAGEMUSHA_SERIALIZED_CURRENT_JOIN_CELLS_V7)
                != Some(layout.parent_digest_offset)
            || layout
                .parent_digest_offset
                .checked_add(KAGEMUSHA_SERIALIZED_PARENT_DIGEST_CELLS_V7)
                != Some(layout.live_selector_offset)
            || layout.live_selector_offset.checked_add(1) != Some(layout.instance_column_cells)
            || base.instance_column_limbs != 66
        {
            return Err("Kagemusha V7 public layout derivation drifted".to_owned());
        }
        Ok(layout)
    }

    fn recursive_parent_layout(
        self,
        base: &KagemushaPastaPublicLayoutV4,
    ) -> KagemushaPastaPublicLayoutV4 {
        let mut layout = base.clone();
        layout.live_selector_offset = u32::try_from(self.live_selector_offset)
            .expect("fixed V7 live-selector offset fits u32");
        layout.instance_column_limbs =
            u32::try_from(self.instance_column_cells).expect("fixed V7 public length fits u32");
        layout
    }
}

fn assign_kagemusha_serialized_public_mode_v7<F>(
    builder: &mut halo2_base::gates::circuit::builder::BaseCircuitBuilder<F>,
    semantic_values: Vec<F>,
    layout: KagemushaSerializedPublicLayoutV7,
    mode: KagemushaSerializedPublicModeV7,
) -> Result<Vec<halo2_base::AssignedValue<F>>, String>
where
    F: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7
        + halo2_base::utils::ScalarField,
{
    use halo2_base::{
        QuantumCell::Existing,
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    if semantic_values.len() != layout.instance_column_cells {
        return Err("Kagemusha V7 semantic public column has the wrong length".to_owned());
    }
    // Mode chooses witness values only.  Both modes execute the identical
    // constraint graph and therefore share one final VK: a live proof exposes
    // all 70 semantic cells, while the dedicated absent-slot carrier exposes
    // the literal all-zero column required by parent induction.
    let exposed_values = match mode {
        KagemushaSerializedPublicModeV7::Live => semantic_values.clone(),
        KagemushaSerializedPublicModeV7::NullParent => {
            vec![F::ZERO; layout.instance_column_cells]
        }
    };
    let exposed = builder.main(0).assign_witnesses(exposed_values);
    let semantic = builder.main(0).assign_witnesses(semantic_values);
    builder.assigned_instances = vec![exposed.clone()];
    let range = builder.range_chip();
    let ctx = builder.main(0);
    for cell in &semantic {
        range.range_check(ctx, *cell, 128);
    }
    range.gate().assert_is_const(
        ctx,
        &semantic[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5],
        &F::from(u64::from(KAGEMUSHA_SERIALIZED_PROFILE_VERSION_V7)),
    );
    range.range_check(ctx, semantic[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5], 2);
    let invalid_parent_count = range.gate().is_equal(
        ctx,
        semantic[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5],
        halo2_base::QuantumCell::Constant(F::from(3)),
    );
    range
        .gate()
        .assert_is_const(ctx, &invalid_parent_count, &F::ZERO);
    range.range_check(
        ctx,
        semantic[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5],
        32,
    );
    range
        .gate()
        .assert_is_const(ctx, &semantic[layout.live_selector_offset], &F::ONE);
    let live = exposed[layout.live_selector_offset];
    range.gate().assert_bit(ctx, live);
    let not_live = range.gate().not(ctx, live);
    for index in 0..layout.instance_column_cells {
        let null_value = range
            .gate()
            .mul(ctx, Existing(not_live), Existing(exposed[index]));
        range.gate().assert_is_const(ctx, &null_value, &F::ZERO);
        constrain_equal_if_v4(ctx, &range, live, exposed[index], semantic[index]);
    }
    Ok(semantic)
}

fn constrain_kagemusha_serialized_scalar_chunks_v7<F>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    chunks: [halo2_base::AssignedValue<F>; 2],
) -> halo2_base::AssignedValue<F>
where
    F: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditFieldV7,
{
    use halo2_base::{
        QuantumCell::{Constant, Existing},
        gates::{GateInstructions as _, RangeInstructions as _},
    };

    let [low, high] = chunks;
    range.range_check(ctx, low, 128);
    range.range_check(ctx, high, 128);
    let high_less =
        range.is_less_than(ctx, high, Constant(F::from_u128(F::MODULUS_HIGH_U128)), 128);
    let high_equal = range
        .gate()
        .is_equal(ctx, high, Constant(F::from_u128(F::MODULUS_HIGH_U128)));
    let low_less = range.is_less_than(ctx, low, Constant(F::from_u128(F::MODULUS_LOW_U128)), 128);
    let equal_and_low = range
        .gate()
        .mul(ctx, Existing(high_equal), Existing(low_less));
    let canonical = range
        .gate()
        .add(ctx, Existing(high_less), Existing(equal_and_low));
    range.gate().assert_is_const(ctx, &canonical, &F::ONE);
    range.gate().mul_add(
        ctx,
        Existing(high),
        Constant(F::from_u128(1_u128 << 127).double()),
        Existing(low),
    )
}

fn kagemusha_serialized_scalar_from_chunks_v7<F>(chunks: [u128; 2]) -> Option<F>
where
    F: ff::PrimeField,
{
    let bytes =
        super::kagemusha_serialized_audit_v7::kagemusha_serialized_exact_chunks_to_bytes_v7(chunks);
    let mut repr = F::Repr::default();
    (repr.as_mut().len() == bytes.len()).then(|| repr.as_mut().copy_from_slice(&bytes))?;
    Option::<F>::from(F::from_repr(repr))
}

fn validate_kagemusha_serialized_public_join_v7(
    join: &super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditPublicJoinV7,
) -> Result<(), String> {
    use halo2_proofs::halo2curves::{
        group::{GroupEncoding as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine, Fp, Fq},
    };

    let eq_bytes =
        super::kagemusha_serialized_audit_v7::kagemusha_serialized_exact_chunks_to_bytes_v7(
            join.step_eq_commitment,
        );
    let ep_bytes =
        super::kagemusha_serialized_audit_v7::kagemusha_serialized_exact_chunks_to_bytes_v7(
            join.step_ep_commitment,
        );
    let mut eq_repr =
        <EqAffine as halo2_proofs::halo2curves::group::GroupEncoding>::Repr::default();
    let mut ep_repr =
        <EpAffine as halo2_proofs::halo2curves::group::GroupEncoding>::Repr::default();
    eq_repr.as_mut().copy_from_slice(&eq_bytes);
    ep_repr.as_mut().copy_from_slice(&ep_bytes);
    let eq = Option::<EqAffine>::from(EqAffine::from_bytes(&eq_repr))
        .filter(|point| !bool::from(point.is_identity()))
        .ok_or_else(|| "Kagemusha V7 Eq serialization commitment is invalid".to_owned())?;
    let ep = Option::<EpAffine>::from(EpAffine::from_bytes(&ep_repr))
        .filter(|point| !bool::from(point.is_identity()))
        .ok_or_else(|| "Kagemusha V7 Ep serialization commitment is invalid".to_owned())?;
    if eq.to_bytes().as_ref() != eq_bytes || ep.to_bytes().as_ref() != ep_bytes {
        return Err("Kagemusha V7 serialization commitment is noncanonical".to_owned());
    }
    if join.challenge[1] >= (1_u128 << 126)
        || (join.challenge[1] == 0 && join.challenge[0] <= 1)
        || kagemusha_serialized_scalar_from_chunks_v7::<Fp>(join.eq_evaluation).is_none()
        || kagemusha_serialized_scalar_from_chunks_v7::<Fq>(join.ep_evaluation).is_none()
    {
        return Err("Kagemusha V7 challenge/evaluation encoding is invalid".to_owned());
    }
    Ok(())
}

/// Host carrier for the new shared current and transitive-parent bindings.
struct KagemushaSerializedPublicInputsV7<'a> {
    core: &'a KagemushaPastaCyclePublicInputsV4,
    current_join: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditPublicJoinV7,
    parent_slots_digest: [u128; KAGEMUSHA_SERIALIZED_PARENT_DIGEST_CELLS_V7],
}

impl KagemushaSerializedPublicInputsV7<'_> {
    fn validate(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
    ) -> Result<KagemushaSerializedPublicLayoutV7, String> {
        let base = self
            .core
            .validate_for_audit_derivation_prepass(proof_step_count, params)?;
        let layout = KagemushaSerializedPublicLayoutV7::for_k17(&base)?;
        validate_kagemusha_serialized_public_join_v7(&self.current_join)?;
        if self.parent_slots_digest == [0; 2] {
            return Err("Kagemusha V7 ordered-parent digest is zero".to_owned());
        }
        Ok(layout)
    }

    fn instance_column<F>(
        &self,
        proof_step_count: u32,
        params: &KagemushaStepCircuitParamsV4,
        parity: KagemushaPastaCycleParityV1,
    ) -> Result<Vec<F>, String>
    where
        F: ff::PrimeField + From<u64>,
    {
        let layout = self.validate(proof_step_count, params)?;
        let mut cells = self.core.compact_header_chunks_v5(proof_step_count);
        cells[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5] =
            u128::from(KAGEMUSHA_SERIALIZED_PROFILE_VERSION_V7);
        let accumulator = match parity {
            KagemushaPastaCycleParityV1::StepEq => &self.core.parent_eq_lineage_accumulator,
            KagemushaPastaCycleParityV1::StepEp => &self.core.parent_ep_lineage_accumulator,
        };
        match accumulator {
            Some(accumulator) => cells.extend(accumulator.instance_limbs(params.k)?),
            None => cells.resize(cells.len() + layout.accumulator_limbs, 0),
        }
        cells.extend(self.current_join.cells());
        cells.extend(self.parent_slots_digest);
        cells.push(u128::from(self.core.live_selector));
        if cells.len() != layout.instance_column_cells {
            return Err("Kagemusha V7 instance-column length drifted".to_owned());
        }
        Ok(cells.into_iter().map(F::from_u128).collect())
    }
}

/// New circuit identity coupling the existing Base geometry to the V7 manifest.
#[derive(Clone, Debug, Default)]
struct KagemushaStepCircuitParamsV7 {
    base: KagemushaStepCircuitParamsV4,
    manifest: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
}

impl KagemushaStepCircuitParamsV7 {
    fn validate(&self) -> Result<KagemushaSerializedPublicLayoutV7, String> {
        let base = validate_kagemusha_circuit_params_v4(&self.base)?;
        let layout = KagemushaSerializedPublicLayoutV7::for_k17(&base)?;
        self.manifest.validate()?;
        if self.manifest.k != self.base.k
            || self.base.minimum_unusable_rows != self.manifest.minimum_unusable_rows
            || self.base.max_parent_proof_bytes != self.manifest.step_eq_proof_bytes
            || self.base.max_parent_proof_bytes != self.manifest.step_ep_proof_bytes
            || usize::try_from(self.manifest.public_instance_cells).ok()
                != Some(layout.instance_column_cells)
            || usize::try_from(self.manifest.current_join_offset).ok()
                != Some(layout.current_join_offset)
            || usize::try_from(self.manifest.parent_digest_offset).ok()
                != Some(layout.parent_digest_offset)
            || usize::try_from(self.manifest.live_selector_offset).ok()
                != Some(layout.live_selector_offset)
        {
            return Err("Kagemusha V7 manifest/circuit layout mismatch".to_owned());
        }
        Ok(layout)
    }
}

#[derive(Clone, Debug)]
struct KagemushaStepCompositeConfigV7<F: halo2_base::utils::ScalarField> {
    base: halo2_base::gates::circuit::BaseConfig<F>,
    sha: KagemushaSha256ConfigV4,
    dense: KagemushaDenseMsmConfigV5,
    serialized: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditConfigV7,
}

fn configure_kagemusha_step_eq_composite_v7(
    meta: &mut halo2_proofs::plonk::ConstraintSystem<Fp>,
    params: &KagemushaStepCircuitParamsV7,
) -> KagemushaStepCompositeConfigV7<Fp> {
    params
        .validate()
        .expect("authenticated Kagemusha V7 Eq profile");
    let base = kagemusha_base_circuit_params_v4(&params.base)
        .expect("authenticated Kagemusha V7 Eq Base parameters");
    let usable_rows =
        kagemusha_usable_rows_v4(&params.base).expect("authenticated Kagemusha V7 Eq usable rows");
    let mut base = halo2_base::gates::circuit::BaseConfig::configure(meta, base);
    base.set_usable_rows(usable_rows);
    KagemushaStepCompositeConfigV7 {
        base,
        sha: KagemushaSha256ConfigV4::configure(meta),
        dense: KagemushaDenseMsmConfigV5::configure::<halo2_proofs::halo2curves::pasta::EpAffine>(
            meta,
        ),
        serialized:
            super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditConfigV7::configure(meta),
    }
}

fn configure_kagemusha_step_ep_composite_v7(
    meta: &mut halo2_proofs::plonk::ConstraintSystem<Fq>,
    params: &KagemushaStepCircuitParamsV7,
) -> KagemushaStepCompositeConfigV7<Fq> {
    params
        .validate()
        .expect("authenticated Kagemusha V7 Ep profile");
    let base = kagemusha_base_circuit_params_v4(&params.base)
        .expect("authenticated Kagemusha V7 Ep Base parameters");
    let usable_rows =
        kagemusha_usable_rows_v4(&params.base).expect("authenticated Kagemusha V7 Ep usable rows");
    let mut base = halo2_base::gates::circuit::BaseConfig::configure(meta, base);
    base.set_usable_rows(usable_rows);
    KagemushaStepCompositeConfigV7 {
        base,
        sha: KagemushaSha256ConfigV4::configure(meta),
        dense: KagemushaDenseMsmConfigV5::configure::<halo2_proofs::halo2curves::pasta::EqAffine>(
            meta,
        ),
        serialized:
            super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditConfigV7::configure(meta),
    }
}

#[derive(Clone)]
struct KagemushaStepEqCircuitV7 {
    params: KagemushaStepCircuitParamsV7,
    builder: halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp>,
    sha_jobs: KagemushaSha256JobsV4<Fp>,
    dense_jobs: KagemushaDenseMsmJobsV5<halo2_proofs::halo2curves::pasta::EpAffine>,
    serialized_jobs: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditJobsV7<Fp>,
}

impl halo2_proofs::plonk::Circuit<Fp> for KagemushaStepEqCircuitV7 {
    type Config = KagemushaStepCompositeConfigV7<Fp>;
    type FloorPlanner = halo2_proofs::circuit::V1;
    type Params = KagemushaStepCircuitParamsV7;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            params: self.params.clone(),
            builder: kagemusha_builder_without_witnesses_v4(&self.builder),
            sha_jobs: self.sha_jobs.unknown(),
            dense_jobs: self.dense_jobs.unknown(),
            serialized_jobs: self.serialized_jobs.unknown(),
        }
    }

    fn configure_with_params(
        meta: &mut halo2_proofs::plonk::ConstraintSystem<Fp>,
        params: Self::Params,
    ) -> Self::Config {
        configure_kagemusha_step_eq_composite_v7(meta, &params)
    }

    fn configure(_: &mut halo2_proofs::plonk::ConstraintSystem<Fp>) -> Self::Config {
        unreachable!("Kagemusha StepEq V7 requires authenticated circuit parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<Fp>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let usable_rows = kagemusha_usable_rows_v4(&self.params.base)
            .map_err(|_| halo2_proofs::plonk::Error::Synthesis)?;
        <halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fp> as halo2_proofs::plonk::Circuit<
            Fp,
        >>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "Kagemusha StepEq V7 Base"),
        )?;
        self.sha_jobs.synthesize(
            &config.sha,
            &mut layouter,
            &self.builder.core().copy_manager,
            usable_rows,
        )?;
        self.dense_jobs.synthesize(
            &config.dense,
            &mut layouter,
            &self.builder.core().copy_manager,
            self.builder.witness_gen_only(),
            usable_rows,
        )?;
        self.serialized_jobs.synthesize(
            &config.serialized,
            &mut layouter,
            &self.builder.core().copy_manager,
            usable_rows,
        )
    }
}

#[derive(Clone)]
struct KagemushaStepEpCircuitV7 {
    params: KagemushaStepCircuitParamsV7,
    builder: halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fq>,
    sha_jobs: KagemushaSha256JobsV4<Fq>,
    dense_jobs: KagemushaDenseMsmJobsV5<halo2_proofs::halo2curves::pasta::EqAffine>,
    serialized_jobs: super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditJobsV7<Fq>,
}

impl halo2_proofs::plonk::Circuit<Fq> for KagemushaStepEpCircuitV7 {
    type Config = KagemushaStepCompositeConfigV7<Fq>;
    type FloorPlanner = halo2_proofs::circuit::V1;
    type Params = KagemushaStepCircuitParamsV7;

    fn params(&self) -> Self::Params {
        self.params.clone()
    }

    fn without_witnesses(&self) -> Self {
        Self {
            params: self.params.clone(),
            builder: kagemusha_builder_without_witnesses_v4(&self.builder),
            sha_jobs: self.sha_jobs.unknown(),
            dense_jobs: self.dense_jobs.unknown(),
            serialized_jobs: self.serialized_jobs.unknown(),
        }
    }

    fn configure_with_params(
        meta: &mut halo2_proofs::plonk::ConstraintSystem<Fq>,
        params: Self::Params,
    ) -> Self::Config {
        configure_kagemusha_step_ep_composite_v7(meta, &params)
    }

    fn configure(_: &mut halo2_proofs::plonk::ConstraintSystem<Fq>) -> Self::Config {
        unreachable!("Kagemusha StepEp V7 requires authenticated circuit parameters")
    }

    fn synthesize(
        &self,
        config: Self::Config,
        mut layouter: impl halo2_proofs::circuit::Layouter<Fq>,
    ) -> Result<(), halo2_proofs::plonk::Error> {
        let usable_rows = kagemusha_usable_rows_v4(&self.params.base)
            .map_err(|_| halo2_proofs::plonk::Error::Synthesis)?;
        <halo2_base::gates::circuit::builder::BaseCircuitBuilder<Fq> as halo2_proofs::plonk::Circuit<
            Fq,
        >>::synthesize(
            &self.builder,
            config.base,
            layouter.namespace(|| "Kagemusha StepEp V7 Base"),
        )?;
        self.sha_jobs.synthesize(
            &config.sha,
            &mut layouter,
            &self.builder.core().copy_manager,
            usable_rows,
        )?;
        self.dense_jobs.synthesize(
            &config.dense,
            &mut layouter,
            &self.builder.core().copy_manager,
            self.builder.witness_gen_only(),
            usable_rows,
        )?;
        self.serialized_jobs.synthesize(
            &config.serialized,
            &mut layouter,
            &self.builder.core().copy_manager,
            usable_rows,
        )
    }
}

fn kagemusha_serialized_generation_peak_bound_v7(
    params: &KagemushaStepCircuitParamsV7,
) -> Result<u64, String> {
    params.validate()?;
    kagemusha_serialized_generation_peak_bound_from_base_v7(&params.base)
}

fn kagemusha_serialized_generation_peak_bound_from_base_v7(
    base: &KagemushaStepCircuitParamsV4,
) -> Result<u64, String> {
    validate_kagemusha_circuit_params_v4(base)?;
    let base_bound = estimate_kagemusha_generation_peak_bytes_v4(base, base)?;
    let domain_rows = 1_u64
        .checked_shl(base.k)
        .ok_or_else(|| "Kagemusha V7 generation domain-row bound overflowed".to_owned())?;
    // The Base graph widths are unchanged.  Relative to V5, the sole custom
    // column adds one physical advice column and one copy-permutation
    // polynomial; it adds no selector, fixed column, or virtual Base slot.
    let polynomial_delta = checked_kagemusha_generation_product_v4(
        &[
            domain_rows,
            KAGEMUSHA_GENERATION_FIELD_BYTES_V4,
            KAGEMUSHA_GENERATION_LIVE_COLUMN_COPIES_V4,
        ],
        "serialized V7 polynomial delta",
    )?;
    let keygen_delta = checked_kagemusha_generation_product_v4(
        &[
            KAGEMUSHA_GENERATION_PERMUTATION_ASSEMBLY_COPIES_V5,
            domain_rows,
            KAGEMUSHA_GENERATION_PERMUTATION_CELL_BYTES_V5,
        ],
        "serialized V7 keygen delta",
    )?;
    let physical_advice_delta = checked_kagemusha_generation_product_v4(
        &[
            domain_rows,
            KAGEMUSHA_GENERATION_PHYSICAL_COLUMN_CELL_BYTES_V5,
        ],
        "serialized V7 physical-advice delta",
    )?;
    let processed_key_delta = checked_kagemusha_generation_product_v4(
        &[
            KAGEMUSHA_GENERATION_PROCESSED_KEY_POLYNOMIAL_COPIES_V5,
            domain_rows,
            KAGEMUSHA_GENERATION_FIELD_BYTES_V4,
        ],
        "serialized V7 processed-key delta",
    )?;
    let prover_delta = physical_advice_delta
        .checked_add(processed_key_delta)
        .ok_or_else(|| "Kagemusha V7 prover delta overflowed".to_owned())?;
    base_bound
        .checked_add(polynomial_delta.max(keygen_delta).max(prover_delta))
        .ok_or_else(|| "Kagemusha V7 generation peak bound overflowed".to_owned())
}

include!("serialized_audit_builder_v7.rs");
include!("serialized_audit_release_proof_v7.rs");

#[cfg(test)]
mod kagemusha_serialized_bridge_v7_tests {
    use super::*;
    use halo2_proofs::halo2curves::{
        group::{Curve as _, GroupEncoding as _, prime::PrimeCurveAffine as _},
        pasta::{EpAffine, EqAffine},
    };

    fn k17_geometry_manifest()
    -> super::super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7 {
        // The composite has 411 existing advice columns.  The serialized
        // phase-zero column is configured last and is therefore column/rank
        // 411 in the 412-column phase vector.
        let phases = vec![0; 412];
        super::super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7 {
            k: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
            step_eq_params_sha256: [0x11; 32],
            step_ep_params_sha256: [0x12; 32],
            step_eq_vk_sha256: [0x22; 32],
            step_ep_vk_sha256: [0x33; 32],
            step_eq_advice_phases: phases.clone(),
            step_ep_advice_phases: phases,
            step_eq_serialized_column: 411,
            step_ep_serialized_column: 411,
            step_eq_phase_zero_rank: 411,
            step_ep_phase_zero_rank: 411,
            step_eq_constraint_degree: 9,
            step_ep_constraint_degree: 9,
            step_eq_fixed_columns: 339,
            step_ep_fixed_columns: 339,
            step_eq_permutation_columns: 298,
            step_ep_permutation_columns: 298,
            step_eq_blinding_factors: 8,
            step_ep_blinding_factors: 8,
            minimum_unusable_rows:
                super::super::kagemusha_serialized_audit_v7::SERIALIZED_EXPECTED_UNUSABLE_ROWS_V7,
            step_eq_proof_bytes:
                super::super::kagemusha_serialized_audit_v7::SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7,
            step_ep_proof_bytes:
                super::super::kagemusha_serialized_audit_v7::SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7,
            eq_coefficient_count: 10_111,
            ep_coefficient_count: 10_111,
            public_instance_cells: 70,
            current_join_offset: 57,
            parent_digest_offset: 67,
            live_selector_offset: 69,
        }
    }

    fn k17_geometry_params() -> KagemushaStepCircuitParamsV7 {
        let mut base = KagemushaStepCircuitParamsV4::reviewed_first_release_generation_profile()
            .expect("reviewed k17 Base profile");
        base.max_parent_proof_bytes =
            super::super::kagemusha_serialized_audit_v7::SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7;
        KagemushaStepCircuitParamsV7 {
            base,
            manifest: k17_geometry_manifest(),
        }
    }

    #[allow(clippy::type_complexity)]
    fn atomic_envelope_fixture() -> (
        super::super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditManifestV7,
        Vec<u8>,
        Vec<u8>,
        Vec<Vec<Fp>>,
        Vec<Vec<Fq>>,
    ) {
        use super::super::kagemusha_serialized_audit_v7::{
            KagemushaSerializedAuditPublicJoinV7, kagemusha_serialized_bytes_to_chunks_v7,
            kagemusha_serialized_digest_word_chunks_v7,
        };

        let manifest = k17_geometry_manifest();
        let eq = (EqAffine::generator() * Fp::from(3)).to_affine().to_bytes();
        let ep = (EpAffine::generator() * Fq::from(5)).to_affine().to_bytes();
        let join = KagemushaSerializedAuditPublicJoinV7 {
            step_eq_commitment: kagemusha_serialized_bytes_to_chunks_v7(
                eq.as_ref().try_into().expect("Eq point encoding"),
            ),
            step_ep_commitment: kagemusha_serialized_bytes_to_chunks_v7(
                ep.as_ref().try_into().expect("Ep point encoding"),
            ),
            challenge: [2, 0],
            eq_evaluation: [7, 0],
            ep_evaluation: [11, 0],
        };
        let mut cells = [0_u128; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7];
        cells[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5] =
            u128::from(KAGEMUSHA_SERIALIZED_PROFILE_VERSION_V7);
        cells[KAGEMUSHA_COMPACT_PROOF_STEP_COUNT_OFFSET_V5] = 1;
        cells[KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5 + 2]
            .copy_from_slice(&kagemusha_serialized_bytes_to_chunks_v7([0x42; 32]));
        cells[KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5 + 2]
            .copy_from_slice(&kagemusha_serialized_bytes_to_chunks_v7(
                manifest.sha256().expect("valid fixture manifest"),
            ));
        cells[KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5 + 2]
            .copy_from_slice(&kagemusha_serialized_digest_word_chunks_v7(
                manifest.step_eq_vk_sha256,
            ));
        cells[KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5
            ..KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5 + 2]
            .copy_from_slice(&kagemusha_serialized_digest_word_chunks_v7(
                manifest.step_ep_vk_sha256,
            ));
        cells[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
            ..KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
                + KAGEMUSHA_SERIALIZED_CURRENT_JOIN_CELLS_V7]
            .copy_from_slice(&join.cells());
        cells[KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7
            ..KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7
                + KAGEMUSHA_SERIALIZED_PARENT_DIGEST_CELLS_V7]
            .copy_from_slice(&[71, 73]);
        cells[KAGEMUSHA_SERIALIZED_LIVE_SELECTOR_OFFSET_V7] = 1;
        let eq_instances = vec![cells.into_iter().map(Fp::from_u128).collect()];
        let ep_instances = vec![cells.into_iter().map(Fq::from_u128).collect()];
        let proof_bytes = manifest.step_eq_proof_bytes as usize;
        (
            manifest,
            vec![0; proof_bytes],
            vec![0; proof_bytes],
            eq_instances,
            ep_instances,
        )
    }

    fn assert_k17_serialized_geometry<F: ff::Field>(
        constraints: &halo2_proofs::plonk::ConstraintSystem<F>,
        serialized_column: usize,
    ) {
        assert_eq!(constraints.degree(), 9);
        assert_eq!(constraints.num_advice_columns(), 412);
        assert_eq!(constraints.num_fixed_columns(), 9);
        assert_eq!(constraints.num_selectors(), 330);
        assert_eq!(
            constraints.num_fixed_columns() + constraints.num_selectors(),
            339
        );
        assert_eq!(constraints.permutation().get_columns().len(), 298);
        assert_eq!(constraints.blinding_factors(), 8);
        assert_eq!(constraints.num_instance_columns(), 1);
        assert_eq!(constraints.advice_column_phase(), vec![0; 412]);
        assert_eq!(serialized_column, 411);
        assert_eq!(
            kagemusha_augmented_proof_size_bytes_v5(
                constraints,
                KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
            )
            .expect("exact serialized V7 proof size"),
            93_184
        );

        let processed = KagemushaProcessedKeyShapeV4 {
            k: KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
            domain_rows: 1 << KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
            fixed_polynomials: 339,
            permutation_polynomials: 298,
            point_bytes: 32,
            scalar_bytes: 32,
        };
        assert_eq!(
            processed
                .proving_key_bytes("serialized V7")
                .expect("exact serialized V7 proving-key size"),
            5_356_151_726
        );
        assert!(
            processed
                .proving_key_bytes("serialized V7")
                .expect("bounded serialized V7 proving-key size")
                <= KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5
        );
        assert_eq!(
            processed
                .verifier_key_bytes("serialized V7")
                .expect("exact serialized V7 verifier-key size"),
            20_394
        );
        assert_eq!(
            kagemusha_params_encoded_bytes_v4::<EqAffine>(
                KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
                "serialized V7 Eq",
            )
            .expect("exact serialized V7 parameter size"),
            8_388_676
        );
        assert!(
            kagemusha_params_encoded_bytes_v4::<EqAffine>(
                KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4,
                "serialized V7 Eq",
            )
            .expect("bounded serialized V7 parameter size")
                <= KAGEMUSHA_COMPACT_PARAMS_IPA_MAX_BYTES_V5
        );
        let raw_pair_bytes = 2_u64
            * u64::from(
                super::super::kagemusha_serialized_audit_v7::SERIALIZED_EXPECTED_STEP_PROOF_BYTES_V7,
            );
        assert_eq!(raw_pair_bytes, 186_368);
        assert!(
            raw_pair_bytes
                <= u64::from(
                    iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
                )
        );
    }

    #[test]
    fn typed_layout_is_seventy_cells_and_does_not_reuse_v5_digest_slots() {
        let base = KagemushaPastaPublicLayoutV4::for_ipa_round_count(17).expect("V5 layout");
        let layout = KagemushaSerializedPublicLayoutV7::for_k17(&base).expect("V7 layout");
        assert_eq!(layout.current_join_offset, 57);
        assert_eq!(layout.parent_digest_offset, 67);
        assert_eq!(layout.live_selector_offset, 69);
        assert_eq!(layout.instance_column_cells, 70);
        assert_eq!(base.parent_eq_deferred_offset, 57);
        assert_eq!(base.parent_ep_deferred_offset, 61);
        assert_eq!(base.live_selector_offset, 65);
    }

    #[test]
    fn typed_instance_column_recaptures_all_seventy_live_cells_in_both_fields() {
        use super::super::kagemusha_serialized_audit_v7::{
            KagemushaSerializedAuditPublicJoinV7, kagemusha_serialized_bytes_to_chunks_v7,
        };

        let params = k17_geometry_params();
        let calibration = kagemusha_generation_calibration_v4([0x22; 32], [0x33; 32])
            .expect("satisfying initialization calibration");
        let eq = (EqAffine::generator() * Fp::from(3)).to_affine().to_bytes();
        let ep = (EpAffine::generator() * Fq::from(5)).to_affine().to_bytes();
        let join = KagemushaSerializedAuditPublicJoinV7 {
            step_eq_commitment: kagemusha_serialized_bytes_to_chunks_v7(
                eq.as_ref().try_into().expect("Eq point encoding"),
            ),
            step_ep_commitment: kagemusha_serialized_bytes_to_chunks_v7(
                ep.as_ref().try_into().expect("Ep point encoding"),
            ),
            challenge: [2, 0],
            eq_evaluation: [7, 0],
            ep_evaluation: [11, 0],
        };
        let parent_slots_digest = [71, 73];
        let public = KagemushaSerializedPublicInputsV7 {
            core: &calibration.public_inputs,
            current_join: join,
            parent_slots_digest,
        };
        let eq_cells = public
            .instance_column::<Fp>(1, &params.base, KagemushaPastaCycleParityV1::StepEq)
            .expect("typed Eq column");
        let ep_cells = public
            .instance_column::<Fq>(1, &params.base, KagemushaPastaCycleParityV1::StepEp)
            .expect("typed Ep column");
        let to_u128 = |cells: &[Fp]| {
            cells
                .iter()
                .map(|value| {
                    u128::try_from(halo2_base::utils::fe_to_biguint(value))
                        .expect("V7 public cell fits u128")
                })
                .collect::<Vec<_>>()
        };
        let eq_u128 = to_u128(&eq_cells);
        let ep_u128 = ep_cells
            .iter()
            .map(|value| {
                u128::try_from(halo2_base::utils::fe_to_biguint(value))
                    .expect("V7 public cell fits u128")
            })
            .collect::<Vec<_>>();
        assert_eq!(eq_u128.len(), KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7);
        assert_eq!(eq_u128, ep_u128);
        assert_eq!(
            eq_u128[KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5],
            u128::from(KAGEMUSHA_SERIALIZED_PROFILE_VERSION_V7)
        );
        assert_eq!(
            &eq_u128[KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
                ..KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7],
            &join.cells()
        );
        assert_eq!(
            &eq_u128[KAGEMUSHA_SERIALIZED_PARENT_DIGEST_OFFSET_V7
                ..KAGEMUSHA_SERIALIZED_LIVE_SELECTOR_OFFSET_V7],
            &parent_slots_digest
        );
        assert_eq!(eq_u128[KAGEMUSHA_SERIALIZED_LIVE_SELECTOR_OFFSET_V7], 1);
    }

    #[test]
    fn compact_header_preserves_exact_values_and_sha_word_identities() {
        use super::super::kagemusha_serialized_audit_v7::{
            kagemusha_serialized_exact_chunks_to_bytes_v7,
            kagemusha_serialized_sha256_word_chunks_to_bytes_v7,
        };

        let statement = std::array::from_fn(|index| 0x10_u8.wrapping_add(7 * index as u8));
        let profile = std::array::from_fn(|index| 0x81_u8.wrapping_sub(3 * index as u8));
        let step_eq = std::array::from_fn(|index| 0x21_u8.wrapping_add(5 * index as u8));
        let step_ep = std::array::from_fn(|index| 0xe3_u8.wrapping_sub(6 * index as u8));
        let public = KagemushaPastaCyclePublicInputsV4 {
            public_statement_digest: kagemusha_exact_u32_public_limbs(statement),
            operation: KagemushaStepOperationVectorV4::default(),
            parent_count: 0,
            parent_states: std::array::from_fn(|_| Vec::new()),
            result_state: Vec::new(),
            manifest_sha256: kagemusha_exact_u32_public_limbs(profile),
            step_eq_compiled_protocol_sha256: kagemusha_sha256_public_words(step_eq),
            step_ep_compiled_protocol_sha256: kagemusha_sha256_public_words(step_ep),
            parent_eq_lineage_accumulator: None,
            parent_ep_lineage_accumulator: None,
            parent_eq_deferred_sha256: [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
            parent_ep_deferred_sha256: [[0; 8]; KAGEMUSHA_PASTA_PARENT_SLOTS_V1],
            live_selector: KAGEMUSHA_PASTA_PUBLIC_LIVE_SELECTOR_V4,
        };
        let header = public.compact_header_chunks_v5(1);
        let chunks = |offset| {
            header[offset..offset + 2]
                .try_into()
                .expect("digest has two compact chunks")
        };
        assert_eq!(
            kagemusha_serialized_exact_chunks_to_bytes_v7(chunks(
                KAGEMUSHA_COMPACT_STATEMENT_DIGEST_OFFSET_V5,
            )),
            statement
        );
        assert_eq!(
            kagemusha_serialized_exact_chunks_to_bytes_v7(chunks(
                KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5,
            )),
            profile
        );
        assert_eq!(
            kagemusha_serialized_sha256_word_chunks_to_bytes_v7(chunks(
                KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5,
            )),
            step_eq
        );
        assert_eq!(
            kagemusha_serialized_sha256_word_chunks_to_bytes_v7(chunks(
                KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5,
            )),
            step_ep
        );
        assert_ne!(
            kagemusha_serialized_exact_chunks_to_bytes_v7(chunks(
                KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5,
            )),
            step_eq,
            "protocol SHA words must not be decoded as exact LE chunks"
        );
    }

    #[test]
    fn null_parent_mode_is_distinct_from_the_live_base_pair() {
        assert_ne!(
            KagemushaSerializedPublicModeV7::Live,
            KagemushaSerializedPublicModeV7::NullParent
        );
    }

    #[test]
    fn compact_canonical_carrier_schemas_fit_the_unchanged_release_corridor() {
        let release_max =
            usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4)
                .expect("V7 release carrier cap fits usize");
        let mut null_payload =
            Vec::with_capacity(KAGEMUSHA_SERIALIZED_NULL_CARRIER_PAYLOAD_BYTES_V7);
        for (sentinel, length) in [
            (0x61, KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7),
            (0x62, KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7),
            (0x63, KAGEMUSHA_SERIALIZED_FOLD_PROOF_BYTES_V7),
            (0x64, KAGEMUSHA_SERIALIZED_FOLD_PROOF_BYTES_V7),
            (0x65, KAGEMUSHA_SERIALIZED_FOLD_PROOF_BYTES_V7),
            (0x66, KAGEMUSHA_SERIALIZED_FOLD_PROOF_BYTES_V7),
        ] {
            null_payload.extend(std::iter::repeat_n(sentinel, length));
        }
        let null = KagemushaSerializedNullCarrierWireV7 {
            manifest_sha256: [0x51; 32],
            payload: null_payload,
        };
        let null_bytes = norito::encode_canonical(&null).expect("encode compact NullParent");
        assert_eq!(null_bytes.len(), KAGEMUSHA_SERIALIZED_NULL_CARRIER_BYTES_V7);
        let decoded_null: KagemushaSerializedNullCarrierWireV7 =
            norito::decode_canonical_with_limits(
                &null_bytes,
                norito::canonical_decode_limits(null_bytes.len()),
            )
            .expect("decode compact NullParent");
        assert_eq!(decoded_null, null);
        let null_parts = decoded_null.parts().expect("split compact NullParent");
        assert_eq!(
            null_parts.step_eq_proof.len(),
            KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
        );
        assert_eq!(
            null_parts.step_ep_branch_merge_fold.len(),
            KAGEMUSHA_SERIALIZED_FOLD_PROOF_BYTES_V7
        );
        for (part, sentinel) in [
            (null_parts.step_eq_proof, 0x61),
            (null_parts.step_ep_proof, 0x62),
            (null_parts.step_eq_post_proof_fold, 0x63),
            (null_parts.step_ep_post_proof_fold, 0x64),
            (null_parts.step_eq_branch_merge_fold, 0x65),
            (null_parts.step_ep_branch_merge_fold, 0x66),
        ] {
            assert!(part.iter().all(|byte| *byte == sentinel));
        }
        require_distinct_kagemusha_serialized_null_fold_transcripts_v7(&null_parts)
            .expect("separately generated NullParent folds");
        let mut duplicate_null_fold = null.clone();
        let eq_post_start = KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7;
        let eq_branch_start = eq_post_start + 2 * KAGEMUSHA_SERIALIZED_FOLD_PROOF_BYTES_V7;
        duplicate_null_fold.payload.copy_within(
            eq_post_start..eq_post_start + KAGEMUSHA_SERIALIZED_FOLD_PROOF_BYTES_V7,
            eq_branch_start,
        );
        let duplicate_parts = duplicate_null_fold
            .parts()
            .expect("split copied NullParent fold");
        assert!(
            require_distinct_kagemusha_serialized_null_fold_transcripts_v7(&duplicate_parts)
                .is_err(),
            "a copied NullParent fold transcript must fail closed"
        );
        assert!(
            matches!(
                norito::decode_canonical_with_limits::<KagemushaSerializedReleaseCarrierWireV7>(
                    &null_bytes,
                    norito::canonical_decode_limits(null_bytes.len()),
                ),
                Err(norito::Error::SchemaMismatch)
            ),
            "the NullParent schema must not decode as a live carrier"
        );
        assert!(null_bytes.len() > KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7);
        assert!(
            null_bytes.len() <= release_max,
            "compact NullParent encoded to {} bytes under the {release_max}-byte release cap",
            null_bytes.len()
        );

        let base = KagemushaSerializedReleaseCarrierWireV7 {
            manifest_sha256: [0x52; 32],
            step_eq_instances: [0; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7],
            step_ep_instances: [0; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7],
            payload: [
                vec![0x71; KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7],
                vec![0x72; KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7],
            ]
            .concat(),
        };
        let base_bytes = norito::encode_canonical(&base).expect("encode compact base carrier");
        assert_eq!(base_bytes.len(), KAGEMUSHA_SERIALIZED_BASE_CARRIER_BYTES_V7);
        let decoded_base: KagemushaSerializedReleaseCarrierWireV7 =
            norito::decode_canonical_with_limits(
                &base_bytes,
                norito::canonical_decode_limits(base_bytes.len()),
            )
            .expect("decode compact base carrier");
        assert_eq!(decoded_base, base);
        let base_parts = decoded_base.parts().expect("split compact base carrier");
        assert_eq!(
            base_parts.step_eq_proof.len() + base_parts.step_ep_proof.len(),
            KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
        );
        assert!(base_parts.step_eq_proof.iter().all(|byte| *byte == 0x71));
        assert!(base_parts.step_ep_proof.iter().all(|byte| *byte == 0x72));
        assert!(base_parts.step_eq_post_proof_fold.is_empty());
        assert!(base_parts.step_ep_post_proof_fold.is_empty());

        let mut step_eq_instances = [0_u128; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7];
        let mut step_ep_instances = [0_u128; KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7];
        step_eq_instances[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5] = 2;
        step_ep_instances[KAGEMUSHA_COMPACT_PARENT_COUNT_OFFSET_V5] = 2;
        let live = KagemushaSerializedReleaseCarrierWireV7 {
            manifest_sha256: [0x53; 32],
            step_eq_instances,
            step_ep_instances,
            payload: [
                vec![0x81; KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7],
                vec![0x82; KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7],
                vec![0x83; KAGEMUSHA_SERIALIZED_FOLD_PROOF_BYTES_V7],
                vec![0x84; KAGEMUSHA_SERIALIZED_FOLD_PROOF_BYTES_V7],
            ]
            .concat(),
        };
        let live_bytes = norito::encode_canonical(&live).expect("encode compact live carrier");
        assert_eq!(
            live_bytes.len(),
            KAGEMUSHA_SERIALIZED_RECURSIVE_CARRIER_BYTES_V7
        );
        let decoded_live: KagemushaSerializedReleaseCarrierWireV7 =
            norito::decode_canonical_with_limits(
                &live_bytes,
                norito::canonical_decode_limits(live_bytes.len()),
            )
            .expect("decode compact live carrier");
        assert_eq!(decoded_live, live);
        assert!(
            matches!(
                norito::decode_canonical_with_limits::<KagemushaSerializedNullCarrierWireV7>(
                    &live_bytes,
                    norito::canonical_decode_limits(live_bytes.len()),
                ),
                Err(norito::Error::SchemaMismatch)
            ),
            "the live schema must not decode as a NullParent carrier"
        );
        let live_parts = decoded_live.parts().expect("split compact live carrier");
        assert_eq!(
            live_parts.step_eq_proof.len() + live_parts.step_ep_proof.len(),
            KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
        );
        assert_eq!(
            live_parts.step_eq_post_proof_fold.len() + live_parts.step_ep_post_proof_fold.len(),
            2 * KAGEMUSHA_SERIALIZED_FOLD_PROOF_BYTES_V7
        );
        for (part, sentinel) in [
            (live_parts.step_eq_proof, 0x81),
            (live_parts.step_ep_proof, 0x82),
            (live_parts.step_eq_post_proof_fold, 0x83),
            (live_parts.step_ep_post_proof_fold, 0x84),
        ] {
            assert!(part.iter().all(|byte| *byte == sentinel));
        }
        assert!(live_bytes.len() > KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7);
        assert!(
            live_bytes.len() <= release_max,
            "compact recursive carrier encoded to {} bytes under the {release_max}-byte release cap",
            live_bytes.len()
        );
    }

    #[test]
    #[ignore = "non-shipping k17 proof generation requires the guarded 56-GiB release lane"]
    fn genuine_four_node_serialized_v7_release_proof_remains_gate_blocked() {
        assert!(!KAGEMUSHA_SERIALIZED_BRIDGE_REVIEWED_V7);
        let measurement = execute_kagemusha_serialized_release_proof_v7()
            .expect("complete genuine V7 release proof");
        eprintln!("Kagemusha V7 exact non-shipping release proof: {measurement:#?}");
        assert_eq!(
            measurement.params_bytes,
            [KAGEMUSHA_SERIALIZED_PARAMS_BYTES_V7; 2]
        );
        assert_eq!(
            measurement.verifying_key_bytes,
            [KAGEMUSHA_SERIALIZED_VERIFYING_KEY_BYTES_V7; 2]
        );
        assert_eq!(
            measurement.proving_key_bytes,
            [KAGEMUSHA_SERIALIZED_PROVING_KEY_BYTES_V7; 2]
        );
        assert!(
            measurement
                .proving_key_sha256
                .iter()
                .all(|sha256| *sha256 != [0; 32])
        );
        assert_eq!(
            measurement.conservative_peak_bytes,
            KAGEMUSHA_SERIALIZED_REVIEWED_PEAK_BYTES_V7
        );
        assert!(measurement.conservative_peak_bytes <= measurement.active_memory_limit_bytes);
        assert!(
            measurement.active_memory_limit_bytes
                <= KAGEMUSHA_GENERATION_REVIEWED_MAX_ESTIMATED_BYTES_V5
        );
        assert_eq!(measurement.mutation_rejections, 40);
        assert_eq!(
            measurement.terminal_ipa_decisions,
            KAGEMUSHA_SERIALIZED_REQUIRED_TERMINAL_IPA_DECISIONS_V7
        );
        assert_ne!(measurement.null_carrier_sha256, [0; 32]);
        assert_eq!(
            measurement.null_carrier_bytes,
            KAGEMUSHA_SERIALIZED_NULL_CARRIER_BYTES_V7
        );
        assert_eq!(measurement.cases.len(), 4);
        assert_eq!(measurement.cases[0].label, "initialization");
        assert_eq!(measurement.cases[3].label, "two-parent-merge");
        let mut sibling_labels = [measurement.cases[1].label, measurement.cases[2].label];
        sibling_labels.sort_unstable();
        assert_eq!(sibling_labels, ["change-child", "recipient-child"]);
        assert_eq!(
            measurement.cases[0].canonical_carrier_bytes,
            KAGEMUSHA_SERIALIZED_BASE_CARRIER_BYTES_V7
        );
        assert!(measurement.cases[1..].iter().all(|case| {
            case.canonical_carrier_bytes == KAGEMUSHA_SERIALIZED_RECURSIVE_CARRIER_BYTES_V7
        }));
        assert_eq!(
            measurement
                .cases
                .iter()
                .map(|case| (case.proof_step_count, case.parent_count))
                .collect::<Vec<_>>(),
            vec![(1, 0), (2, 1), (2, 1), (3, 2)]
        );
        assert!(measurement.cases.iter().all(|case| {
            case.step_eq_proof_bytes == KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
                && case.step_ep_proof_bytes == KAGEMUSHA_SERIALIZED_STEP_PROOF_BYTES_V7
                && case.raw_proof_pair_bytes == KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7
                && case.canonical_carrier_bytes > case.raw_proof_pair_bytes
                && case.canonical_carrier_bytes
                    <= usize::try_from(KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4)
                        .expect("V7 release carrier cap fits usize")
                && case.canonical_carrier_sha256 != [0; 32]
                && case.public_cells == KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
                && case.eq_coefficients == 10_111
                && case.ep_coefficients == 10_111
        }));
        assert_eq!(
            measurement.maximum_canonical_carrier_bytes,
            measurement
                .cases
                .iter()
                .map(|case| case.canonical_carrier_bytes)
                .max()
                .expect("four carrier measurements")
                .max(measurement.null_carrier_bytes)
        );
        assert!(
            measurement.cases[0].canonical_carrier_bytes
                < measurement.cases[1].canonical_carrier_bytes
        );
        assert!(measurement.cases[1..].iter().all(
            |case| case.canonical_carrier_bytes == measurement.cases[1].canonical_carrier_bytes
        ));
        assert!(
            measurement.null_carrier_bytes > measurement.cases[1].canonical_carrier_bytes,
            "NullParent carries two additional independently randomized branch-fold transcripts"
        );
        assert_eq!(
            measurement.canonical_carriers_fit_current_release_max,
            u32::try_from(measurement.maximum_canonical_carrier_bytes).is_ok_and(|bytes| {
                bytes <= KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_RELEASE_MAX_BYTES_V4
            })
        );
        assert!(measurement.canonical_carriers_fit_current_release_max);
        assert!(
            measurement.null_carrier_bytes > KAGEMUSHA_SERIALIZED_RAW_PROOF_PAIR_BYTES_V7,
            "the compact NullParent carrier must still account for all four fold transcripts"
        );
        for (index, case) in measurement.cases.iter().enumerate() {
            assert_ne!(
                case.canonical_carrier_sha256,
                measurement.null_carrier_sha256
            );
            assert!(
                measurement.cases[..index]
                    .iter()
                    .all(|prior| prior.canonical_carrier_sha256 != case.canonical_carrier_sha256)
            );
        }
        assert!(!KAGEMUSHA_SERIALIZED_BRIDGE_REVIEWED_V7);
        assert!(require_kagemusha_serialized_bridge_release_review_v7().is_err());
    }

    #[test]
    fn public_join_rejects_identity_bad_challenge_and_noncanonical_evaluations() {
        let eq = (EqAffine::generator() * halo2_proofs::halo2curves::pasta::Fp::from(3))
            .to_affine()
            .to_bytes();
        let ep = (EpAffine::generator() * halo2_proofs::halo2curves::pasta::Fq::from(5))
            .to_affine()
            .to_bytes();
        let chunks = |bytes: &[u8]| {
            let bytes: [u8; 32] = bytes.try_into().expect("Pasta point encoding");
            super::super::kagemusha_serialized_audit_v7::kagemusha_serialized_bytes_to_chunks_v7(
                bytes,
            )
        };
        let valid =
            super::super::kagemusha_serialized_audit_v7::KagemushaSerializedAuditPublicJoinV7 {
                step_eq_commitment: chunks(eq.as_ref()),
                step_ep_commitment: chunks(ep.as_ref()),
                challenge: [2, 0],
                eq_evaluation: [7, 0],
                ep_evaluation: [11, 0],
            };
        validate_kagemusha_serialized_public_join_v7(&valid).expect("valid join");
        let mut identity = valid;
        identity.step_eq_commitment = [0; 2];
        assert!(validate_kagemusha_serialized_public_join_v7(&identity).is_err());
        let mut bad_challenge = valid;
        bad_challenge.challenge = [0, 1_u128 << 126];
        assert!(validate_kagemusha_serialized_public_join_v7(&bad_challenge).is_err());
        let mut noncanonical = valid;
        noncanonical.eq_evaluation = [u128::MAX; 2];
        assert!(validate_kagemusha_serialized_public_join_v7(&noncanonical).is_err());
    }

    #[test]
    fn atomic_envelope_rejects_shape_identity_and_shared_cell_mutations() {
        let (manifest, eq_proof, ep_proof, eq_instances, ep_instances) = atomic_envelope_fixture();
        let proof_bound = manifest.step_eq_proof_bytes as usize;
        let pair_bound =
            iroha_data_model::offline::KAGEMUSHA_RECURSIVE_SPEND_PROOF_PAIR_ABSOLUTE_MAX_BYTES_V4
                as usize;
        validate_kagemusha_serialized_atomic_envelope_v7(
            &manifest,
            &eq_proof,
            &ep_proof,
            &eq_instances,
            &ep_instances,
            proof_bound,
            pair_bound,
        )
        .expect("valid atomic envelope");

        let mut short_eq = eq_proof.clone();
        short_eq.pop();
        assert!(
            validate_kagemusha_serialized_atomic_envelope_v7(
                &manifest,
                &short_eq,
                &ep_proof,
                &eq_instances,
                &ep_instances,
                proof_bound,
                pair_bound,
            )
            .is_err()
        );
        for (proof_limit, pair_limit) in [
            (proof_bound - 1, pair_bound),
            (proof_bound, eq_proof.len() + ep_proof.len() - 1),
            (proof_bound, pair_bound + 1),
        ] {
            assert!(
                validate_kagemusha_serialized_atomic_envelope_v7(
                    &manifest,
                    &eq_proof,
                    &ep_proof,
                    &eq_instances,
                    &ep_instances,
                    proof_limit,
                    pair_limit,
                )
                .is_err()
            );
        }

        let mut wrong_shape = eq_instances.clone();
        wrong_shape[0].pop();
        assert!(
            validate_kagemusha_serialized_atomic_envelope_v7(
                &manifest,
                &eq_proof,
                &ep_proof,
                &wrong_shape,
                &ep_instances,
                proof_bound,
                pair_bound,
            )
            .is_err()
        );

        let assert_cell_mutation_fails = |index: usize, mutate_both: bool| {
            let mut changed_eq = eq_instances.clone();
            let mut changed_ep = ep_instances.clone();
            changed_eq[0][index] += Fp::ONE;
            if mutate_both {
                changed_ep[0][index] += Fq::ONE;
            }
            assert!(
                validate_kagemusha_serialized_atomic_envelope_v7(
                    &manifest,
                    &eq_proof,
                    &ep_proof,
                    &changed_eq,
                    &changed_ep,
                    proof_bound,
                    pair_bound,
                )
                .is_err(),
                "cell {index} mutation must fail"
            );
        };
        assert_cell_mutation_fails(KAGEMUSHA_COMPACT_OPERATION_COMMITMENT_OFFSET_V5, false);
        for index in KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7
            ..KAGEMUSHA_SERIALIZED_PUBLIC_INSTANCE_CELLS_V7
        {
            assert_cell_mutation_fails(index, false);
        }
        for index in [
            KAGEMUSHA_COMPACT_PROFILE_OFFSET_V5,
            KAGEMUSHA_COMPACT_MANIFEST_SHA256_OFFSET_V5,
            KAGEMUSHA_COMPACT_STEP_EQ_PROTOCOL_SHA256_OFFSET_V5,
            KAGEMUSHA_COMPACT_STEP_EP_PROTOCOL_SHA256_OFFSET_V5,
        ] {
            assert_cell_mutation_fails(index, true);
        }

        let mut bad_join = eq_instances.clone();
        let mut bad_join_ep = ep_instances.clone();
        bad_join[0][KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7] = Fp::ZERO;
        bad_join[0][KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 1] = Fp::ZERO;
        bad_join_ep[0][KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7] = Fq::ZERO;
        bad_join_ep[0][KAGEMUSHA_SERIALIZED_CURRENT_JOIN_OFFSET_V7 + 1] = Fq::ZERO;
        assert!(
            validate_kagemusha_serialized_atomic_envelope_v7(
                &manifest,
                &eq_proof,
                &ep_proof,
                &bad_join,
                &bad_join_ep,
                proof_bound,
                pair_bound,
            )
            .is_err()
        );
    }

    #[test]
    fn release_gate_remains_closed() {
        assert!(require_kagemusha_serialized_bridge_release_review_v7().is_err());
    }

    #[test]
    fn configured_k17_geometry_pins_the_single_new_permutation_column() {
        let params = k17_geometry_params();
        assert_eq!(params.base.num_advice_per_phase, [220]);
        assert_eq!(params.base.num_lookup_advice_per_phase, [25, 0, 0]);
        assert_eq!(params.base.public_input_limbs, 66);
        assert_eq!(10 + 10_111 + 2 * 10_111, 30_343);
        assert!(30_343 <= kagemusha_usable_rows_v4(&params.base).expect("k17 usable rows"));
        params.validate().expect("valid typed V7 geometry");

        let mut step_eq = halo2_proofs::plonk::ConstraintSystem::<Fp>::default();
        let eq_config = configure_kagemusha_step_eq_composite_v7(&mut step_eq, &params);
        assert_k17_serialized_geometry(&step_eq, eq_config.serialized.advice_column_index());

        let mut step_ep = halo2_proofs::plonk::ConstraintSystem::<Fq>::default();
        let ep_config = configure_kagemusha_step_ep_composite_v7(&mut step_ep, &params);
        assert_k17_serialized_geometry(&step_ep, ep_config.serialized.advice_column_index());
        assert_eq!(
            format!("{:?}", step_eq.pinned()),
            format!("{:?}", step_ep.pinned())
        );
        let peak = kagemusha_serialized_generation_peak_bound_v7(&params)
            .expect("checked serialized V7 peak bound");
        assert_eq!(peak, 53_126_388_928);
        assert!(peak <= KAGEMUSHA_GENERATION_MAX_ESTIMATED_BYTES_V4);
    }
}
