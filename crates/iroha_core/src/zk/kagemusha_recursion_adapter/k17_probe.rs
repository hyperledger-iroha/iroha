#[cfg(feature = "kagemusha-generation-memory-lab")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct KagemushaK17CapturedShapeV5 {
    role: &'static str,
    k: usize,
    num_advice_per_phase: Vec<usize>,
    num_fixed: usize,
    num_lookup_advice_per_phase: Vec<usize>,
    lookup_bits: Option<usize>,
    num_instance_columns: usize,
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
impl KagemushaK17CapturedShapeV5 {
    fn widths(&self) -> Result<(u32, u32), String> {
        let advice = u32::try_from(self.num_advice_per_phase[0])
            .map_err(|_| format!("Kagemusha k17 {} advice width does not fit u32", self.role))?;
        let lookup =
            self.num_lookup_advice_per_phase
                .first()
                .copied()
                .map_or(Ok(0), |columns| {
                    u32::try_from(columns).map_err(|_| {
                        format!("Kagemusha k17 {} lookup width does not fit u32", self.role)
                    })
                })?;
        Ok((advice, lookup))
    }
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
#[derive(Clone, Debug, PartialEq, Eq)]
struct KagemushaK17ProbeIterationV5 {
    required_shapes: Vec<KagemushaK17CapturedShapeV5>,
    maximum_widths: (u32, u32),
    step_eq_proof_size_bytes: u32,
    step_ep_proof_size_bytes: u32,
    step_eq_protocol_structure_sha256: [u8; 32],
    step_ep_protocol_structure_sha256: [u8; 32],
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KagemushaK17ProbeIterationModeV5 {
    PopulatedShape,
    AuditInventory,
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
#[derive(Clone, Debug, PartialEq, Eq)]
enum KagemushaK17ProbeIterationOutcomeV5 {
    PopulatedShape(Box<KagemushaK17ProbeIterationV5>),
    AuditInventory(
        Box<[(&'static str, KagemushaK17AuditInventoryV6); KAGEMUSHA_PASTA_PARENT_SLOTS_V1]>,
    ),
}
#[cfg(any(test, feature = "kagemusha-generation-memory-lab"))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct KagemushaK17AuditCountsV6 {
    sources: u64,
    equations: u64,
    terms: u64,
    stages: u64,
    stage_equations: u64,
    max_terms_per_equation: u64,
    invalid_equation_source_indices: u64,
    protocol_points: u64,
    protocol_source_indices: u64,
    invalid_protocol_source_indices: u64,
}
#[cfg(any(test, feature = "kagemusha-generation-memory-lab"))]
#[derive(Clone, Debug, PartialEq, Eq)]
struct KagemushaK17AuditInventoryV6 {
    counts: KagemushaK17AuditCountsV6,
    usable_rows: u64,
    audit_poseidon_elements: u64,
    audit_poseidon_permutations: u64,
    protocol_poseidon_elements: u64,
    protocol_poseidon_permutations: u64,
    total_non_native_poseidon_permutations: u64,
    legacy_v5_raw_audit_bytes: u64,
    legacy_v5_raw_audit_sha256_blocks: u64,
    legacy_v5_raw_audit_rows_five_lanes: u64,
    legacy_v5_raw_audit_required_k17_lanes: u64,
    compressed_source_audit_bytes: u64,
    compressed_source_audit_sha256_blocks: u64,
    compressed_source_audit_rows_five_lanes: u64,
    compressed_source_audit_required_k17_lanes: u64,
    legacy_v1_protocol_bytes: u64,
    legacy_v1_protocol_sha256_blocks: u64,
    legacy_v5_raw_combined_sha256_blocks: u64,
    legacy_v5_raw_combined_rows_five_lanes: u64,
    legacy_v5_raw_combined_required_k17_lanes: u64,
    compressed_source_combined_sha256_blocks: u64,
    compressed_source_combined_rows_five_lanes: u64,
    compressed_source_combined_required_k17_lanes: u64,
}
#[cfg(any(test, feature = "kagemusha-generation-memory-lab"))]
impl KagemushaK17AuditInventoryV6 {
    const SHA256_ROWS_PER_BLOCK: u128 = 2_304;
    const SHA256_ROWS_PER_JOB: u128 = 64;
    const CURRENT_SHA256_LANES: u128 =
        super::kagemusha_sha256_v4::KAGEMUSHA_SHA256_LANES_V4 as u128;

    fn checked_u64(value: u128, label: &str) -> Result<u64, String> {
        u64::try_from(value)
            .map_err(|_| format!("Kagemusha k17 audit inventory {label} overflowed u64"))
    }

    fn sha256_blocks(bytes: u128) -> u128 {
        // One 0x80 byte and the 64-bit bit-length suffix, rounded to complete blocks.
        (bytes + 9).div_ceil(64)
    }

    fn sha256_rows(blocks: u128, jobs: u128) -> u128 {
        (blocks.div_ceil(Self::CURRENT_SHA256_LANES) * Self::SHA256_ROWS_PER_BLOCK
            + jobs * Self::SHA256_ROWS_PER_JOB)
            .max(
                u128::try_from(super::kagemusha_sha256_table16_v4::TABLE16_SPREAD_TABLE_ROWS)
                    .expect("fixed Table16 spread-table row count fits u128"),
            )
    }

    fn required_lanes(blocks: u128, jobs: u128, usable_rows: u128) -> Result<u128, String> {
        let job_rows = jobs
            .checked_mul(Self::SHA256_ROWS_PER_JOB)
            .ok_or_else(|| "Kagemusha k17 audit inventory SHA job rows overflowed".to_owned())?;
        let available = usable_rows.checked_sub(job_rows).ok_or_else(|| {
            "Kagemusha k17 audit inventory has no rows after SHA job overhead".to_owned()
        })?;
        let blocks_per_lane = available / Self::SHA256_ROWS_PER_BLOCK;
        if blocks_per_lane == 0 {
            return Err(
                "Kagemusha k17 audit inventory has no complete SHA block per lane".to_owned(),
            );
        }
        Ok(blocks.div_ceil(blocks_per_lane))
    }

    fn from_counts(counts: KagemushaK17AuditCountsV6, usable_rows: usize) -> Result<Self, String> {
        let sources = u128::from(counts.sources);
        let equations = u128::from(counts.equations);
        let terms = u128::from(counts.terms);
        let protocol_points = u128::from(counts.protocol_points);
        let usable_rows = u128::try_from(usable_rows).map_err(|_| {
            "Kagemusha k17 audit inventory usable-row count does not fit u128".to_owned()
        })?;

        let domain_elements = |domain_len: usize| {
            2_u128
                + u128::try_from(domain_len)
                    .expect("fixed Kagemusha domain length fits u128")
                    .div_ceil(16)
        };
        // V6 audit: domain/version elements, two counts, two elements per compressed source,
        // three per equation, and two per equation term.
        let audit_poseidon_elements = domain_elements(
            super::kagemusha_cycle_loader::KAGEMUSHA_DEFERRED_AUDIT_POSEIDON_DOMAIN_V6.len(),
        ) + 2
            + 2 * sources
            + 3 * equations
            + 2 * terms;
        let audit_poseidon_permutations = audit_poseidon_elements / 2 + 1;
        // V2 protocol: domain/version elements, parity, four structure chunks, point count,
        // two elements per compressed point, and the transcript initial state.
        let protocol_poseidon_elements =
            domain_elements(KAGEMUSHA_COMPILED_PROTOCOL_IDENTITY_POSEIDON_DOMAIN_V2.len())
                + 7
                + 2 * protocol_points;
        let protocol_poseidon_permutations = protocol_poseidon_elements / 2 + 1;

        // Historical V5 used raw x/y coordinates. The compressed-source alternative is
        // diagnostic arithmetic only; neither path is a production fallback.
        let legacy_v5_raw_audit_bytes = 46 + 64 * sources + 9 * equations + 36 * terms;
        let compressed_source_audit_bytes = 46 + 32 * sources + 9 * equations + 36 * terms;
        let legacy_v1_protocol_bytes = 122 + 32 * protocol_points;
        let legacy_v5_raw_audit_sha256_blocks = Self::sha256_blocks(legacy_v5_raw_audit_bytes);
        let compressed_source_audit_sha256_blocks =
            Self::sha256_blocks(compressed_source_audit_bytes);
        let legacy_v1_protocol_sha256_blocks = Self::sha256_blocks(legacy_v1_protocol_bytes);
        let legacy_v5_raw_combined_sha256_blocks =
            legacy_v5_raw_audit_sha256_blocks + legacy_v1_protocol_sha256_blocks;
        let compressed_source_combined_sha256_blocks =
            compressed_source_audit_sha256_blocks + legacy_v1_protocol_sha256_blocks;

        Ok(Self {
            counts,
            usable_rows: Self::checked_u64(usable_rows, "usable rows")?,
            audit_poseidon_elements: Self::checked_u64(
                audit_poseidon_elements,
                "audit Poseidon elements",
            )?,
            audit_poseidon_permutations: Self::checked_u64(
                audit_poseidon_permutations,
                "audit Poseidon permutations",
            )?,
            protocol_poseidon_elements: Self::checked_u64(
                protocol_poseidon_elements,
                "protocol Poseidon elements",
            )?,
            protocol_poseidon_permutations: Self::checked_u64(
                protocol_poseidon_permutations,
                "protocol Poseidon permutations",
            )?,
            total_non_native_poseidon_permutations: Self::checked_u64(
                audit_poseidon_permutations + protocol_poseidon_permutations,
                "total non-native Poseidon permutations",
            )?,
            legacy_v5_raw_audit_bytes: Self::checked_u64(
                legacy_v5_raw_audit_bytes,
                "legacy V5 raw-audit bytes",
            )?,
            legacy_v5_raw_audit_sha256_blocks: Self::checked_u64(
                legacy_v5_raw_audit_sha256_blocks,
                "legacy V5 raw-audit SHA blocks",
            )?,
            legacy_v5_raw_audit_rows_five_lanes: Self::checked_u64(
                Self::sha256_rows(legacy_v5_raw_audit_sha256_blocks, 1),
                "legacy V5 raw-audit rows",
            )?,
            legacy_v5_raw_audit_required_k17_lanes: Self::checked_u64(
                Self::required_lanes(legacy_v5_raw_audit_sha256_blocks, 1, usable_rows)?,
                "legacy V5 raw-audit required lanes",
            )?,
            compressed_source_audit_bytes: Self::checked_u64(
                compressed_source_audit_bytes,
                "compressed-source audit bytes",
            )?,
            compressed_source_audit_sha256_blocks: Self::checked_u64(
                compressed_source_audit_sha256_blocks,
                "compressed-source audit SHA blocks",
            )?,
            compressed_source_audit_rows_five_lanes: Self::checked_u64(
                Self::sha256_rows(compressed_source_audit_sha256_blocks, 1),
                "compressed-source audit rows",
            )?,
            compressed_source_audit_required_k17_lanes: Self::checked_u64(
                Self::required_lanes(compressed_source_audit_sha256_blocks, 1, usable_rows)?,
                "compressed-source audit required lanes",
            )?,
            legacy_v1_protocol_bytes: Self::checked_u64(
                legacy_v1_protocol_bytes,
                "legacy V1 protocol bytes",
            )?,
            legacy_v1_protocol_sha256_blocks: Self::checked_u64(
                legacy_v1_protocol_sha256_blocks,
                "legacy V1 protocol SHA blocks",
            )?,
            legacy_v5_raw_combined_sha256_blocks: Self::checked_u64(
                legacy_v5_raw_combined_sha256_blocks,
                "legacy V5 raw combined SHA blocks",
            )?,
            legacy_v5_raw_combined_rows_five_lanes: Self::checked_u64(
                Self::sha256_rows(legacy_v5_raw_combined_sha256_blocks, 2),
                "legacy V5 raw combined rows",
            )?,
            legacy_v5_raw_combined_required_k17_lanes: Self::checked_u64(
                Self::required_lanes(legacy_v5_raw_combined_sha256_blocks, 2, usable_rows)?,
                "legacy V5 raw combined required lanes",
            )?,
            compressed_source_combined_sha256_blocks: Self::checked_u64(
                compressed_source_combined_sha256_blocks,
                "compressed-source combined SHA blocks",
            )?,
            compressed_source_combined_rows_five_lanes: Self::checked_u64(
                Self::sha256_rows(compressed_source_combined_sha256_blocks, 2),
                "compressed-source combined rows",
            )?,
            compressed_source_combined_required_k17_lanes: Self::checked_u64(
                Self::required_lanes(compressed_source_combined_sha256_blocks, 2, usable_rows)?,
                "compressed-source combined required lanes",
            )?,
        })
    }

    fn validate(&self, parity: &str) -> Result<(), String> {
        if self.counts.stage_equations != self.counts.equations
            || self.counts.invalid_equation_source_indices != 0
            || self.counts.protocol_points != self.counts.protocol_source_indices
            || self.counts.invalid_protocol_source_indices != 0
        {
            return Err(format!(
                "Kagemusha k17 {parity} audit inventory is internally inconsistent: {:?}",
                self.counts
            ));
        }
        Ok(())
    }

    #[cfg(feature = "kagemusha-generation-memory-lab")]
    fn print(&self, parity: &str) {
        let counts = self.counts;
        println!(
            "k17_audit_inventory parity={parity} sources={} equations={} terms={} stages={} stage_equations={} max_terms_per_equation={} invalid_equation_source_indices={} protocol_points={} protocol_source_indices={} invalid_protocol_source_indices={} usable_rows={} audit_poseidon_elements={} audit_poseidon_permutations={} protocol_poseidon_elements={} protocol_poseidon_permutations={} total_non_native_poseidon_permutations={} legacy_v5_raw_audit_bytes={} legacy_v5_raw_audit_sha256_blocks={} legacy_v5_raw_audit_rows_five_lanes={} legacy_v5_raw_audit_required_k17_lanes={} compressed_source_audit_bytes={} compressed_source_audit_sha256_blocks={} compressed_source_audit_rows_five_lanes={} compressed_source_audit_required_k17_lanes={} legacy_v1_protocol_bytes={} legacy_v1_protocol_sha256_blocks={} legacy_v5_raw_combined_sha256_blocks={} legacy_v5_raw_combined_rows_five_lanes={} legacy_v5_raw_combined_required_k17_lanes={} compressed_source_combined_sha256_blocks={} compressed_source_combined_rows_five_lanes={} compressed_source_combined_required_k17_lanes={}",
            counts.sources,
            counts.equations,
            counts.terms,
            counts.stages,
            counts.stage_equations,
            counts.max_terms_per_equation,
            counts.invalid_equation_source_indices,
            counts.protocol_points,
            counts.protocol_source_indices,
            counts.invalid_protocol_source_indices,
            self.usable_rows,
            self.audit_poseidon_elements,
            self.audit_poseidon_permutations,
            self.protocol_poseidon_elements,
            self.protocol_poseidon_permutations,
            self.total_non_native_poseidon_permutations,
            self.legacy_v5_raw_audit_bytes,
            self.legacy_v5_raw_audit_sha256_blocks,
            self.legacy_v5_raw_audit_rows_five_lanes,
            self.legacy_v5_raw_audit_required_k17_lanes,
            self.compressed_source_audit_bytes,
            self.compressed_source_audit_sha256_blocks,
            self.compressed_source_audit_rows_five_lanes,
            self.compressed_source_audit_required_k17_lanes,
            self.legacy_v1_protocol_bytes,
            self.legacy_v1_protocol_sha256_blocks,
            self.legacy_v5_raw_combined_sha256_blocks,
            self.legacy_v5_raw_combined_rows_five_lanes,
            self.legacy_v5_raw_combined_required_k17_lanes,
            self.compressed_source_combined_sha256_blocks,
            self.compressed_source_combined_rows_five_lanes,
            self.compressed_source_combined_required_k17_lanes,
        );
    }
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
fn kagemusha_k17_audit_inventory_v6<C>(
    output: &KagemushaScalarAuditOutputV4<C>,
    params: &KagemushaStepCircuitParamsV4,
) -> Result<KagemushaK17AuditInventoryV6, String>
where
    C: halo2_base::utils::CurveAffineExt,
{
    let to_u64 = |value: usize, label: &str| {
        u64::try_from(value)
            .map_err(|_| format!("Kagemusha k17 audit inventory {label} does not fit u64"))
    };
    scalar_lineage_v1::validate_stage_shapes_v4(&output.stages, output.audit.equations.len())
        .map_err(|error| format!("invalid Kagemusha k17 audit stage plan: {error:?}"))?;
    let sources = output.audit.sources.len();
    let terms = output
        .audit
        .equations
        .iter()
        .try_fold(0_usize, |total, equation| {
            total.checked_add(equation.len()).ok_or_else(|| {
                "Kagemusha k17 audit inventory equation-term count overflowed".to_owned()
            })
        })?;
    let stage_equations = output.stages.iter().try_fold(0_usize, |total, stage| {
        total.checked_add(stage.range.len()).ok_or_else(|| {
            "Kagemusha k17 audit inventory stage-equation count overflowed".to_owned()
        })
    })?;
    let invalid_equation_source_indices = output
        .audit
        .equations
        .iter()
        .flatten()
        .filter(|(source_index, _)| *source_index >= sources)
        .count();
    let invalid_protocol_source_indices = output
        .identity
        .preprocessed_source_indices
        .iter()
        .filter(|source_index| **source_index >= sources)
        .count();
    let counts = KagemushaK17AuditCountsV6 {
        sources: to_u64(sources, "source count")?,
        equations: to_u64(output.audit.equations.len(), "equation count")?,
        terms: to_u64(terms, "term count")?,
        stages: to_u64(output.stages.len(), "stage count")?,
        stage_equations: to_u64(stage_equations, "stage-equation count")?,
        max_terms_per_equation: to_u64(
            output
                .audit
                .equations
                .iter()
                .map(Vec::len)
                .max()
                .unwrap_or(0),
            "maximum equation-term count",
        )?,
        invalid_equation_source_indices: to_u64(
            invalid_equation_source_indices,
            "invalid equation source-index count",
        )?,
        protocol_points: to_u64(output.identity.preprocessed.len(), "protocol point count")?,
        protocol_source_indices: to_u64(
            output.identity.preprocessed_source_indices.len(),
            "protocol source-index count",
        )?,
        invalid_protocol_source_indices: to_u64(
            invalid_protocol_source_indices,
            "invalid protocol source-index count",
        )?,
    };
    KagemushaK17AuditInventoryV6::from_counts(counts, kagemusha_usable_rows_v4(params)?)
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
fn kagemusha_k17_capture_required_shape_v5(
    role: &str,
    required: &halo2_base::gates::circuit::BaseCircuitParams,
) -> Result<KagemushaK17CapturedShapeV5, String> {
    let production_k = usize::try_from(KAGEMUSHA_STEP_CIRCUIT_MINIMUM_K_V4)
        .map_err(|_| "Kagemusha k17 release degree does not fit usize".to_owned())?;
    let production_lookup_bits = production_k
        .checked_sub(1)
        .ok_or_else(|| "Kagemusha k17 release lookup degree underflowed".to_owned())?;
    if required.k != production_k
        || required.lookup_bits != Some(production_lookup_bits)
        || required.num_fixed > 1
        || required.num_instance_columns > 1
        || required.num_advice_per_phase.len() != 1
        || required
            .num_lookup_advice_per_phase
            .iter()
            .skip(1)
            .any(|columns| *columns != 0)
    {
        return Err(format!(
            "Kagemusha k17 {role} populated probe returned an unsupported shape: {required:?}"
        ));
    }
    let role = match role {
        "StepEqBootstrap" => "StepEqBootstrap",
        "StepEqLive" => "StepEqLive",
        "StepEpBootstrap" => "StepEpBootstrap",
        "StepEpLive" => "StepEpLive",
        _ => return Err("Kagemusha k17 shape probe captured an unknown role".to_owned()),
    };
    Ok(KagemushaK17CapturedShapeV5 {
        role,
        k: required.k,
        num_advice_per_phase: required.num_advice_per_phase.clone(),
        num_fixed: required.num_fixed,
        num_lookup_advice_per_phase: required.num_lookup_advice_per_phase.clone(),
        lookup_bits: required.lookup_bits,
        num_instance_columns: required.num_instance_columns,
    })
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
fn kagemusha_k17_shape_probe_iteration_v5(
    iteration: usize,
    advice_columns: u32,
    lookup_columns: u32,
    mode: KagemushaK17ProbeIterationModeV5,
) -> Result<KagemushaK17ProbeIterationOutcomeV5, String> {
    use halo2_proofs::{
        halo2curves::pasta::{EpAffine, EqAffine},
        poly::{commitment::ParamsProver as _, ipa::commitment::ParamsIPA},
    };
    let mut step_eq_circuit_params =
        kagemusha_k17_shape_probe_params_v5(advice_columns, lookup_columns);
    let mut step_ep_circuit_params = step_eq_circuit_params.clone();
    validate_kagemusha_circuit_params_v4(&step_eq_circuit_params)?;
    let eq_encoding = kagemusha_artifact_encoding_sizes_v4(
        &step_eq_circuit_params,
        KagemushaPastaCycleParityV1::StepEq,
    )?;
    let ep_encoding = kagemusha_artifact_encoding_sizes_v4(
        &step_ep_circuit_params,
        KagemushaPastaCycleParityV1::StepEp,
    )?;
    let estimated_peak_bytes = estimate_kagemusha_generation_peak_bytes_v4(
        &step_eq_circuit_params,
        &step_ep_circuit_params,
    )?;
    let eq_shape = configured_kagemusha_eq_vk_wire_shape_v4(&step_eq_circuit_params)?;
    let ep_shape = configured_kagemusha_ep_vk_wire_shape_v4(&step_ep_circuit_params)?;
    println!(
        "k17_probe iteration={iteration} candidate_advice={advice_columns} candidate_lookup={lookup_columns} eq_configured_advice={} ep_configured_advice={} eq_selectors={} ep_selectors={} eq_permutation={} ep_permutation={} params_bytes={} pk_bytes={} estimated_peak_bytes={estimated_peak_bytes}",
        eq_shape.advice_columns,
        ep_shape.advice_columns,
        eq_shape.selectors,
        ep_shape.selectors,
        eq_shape.permutation_columns,
        ep_shape.permutation_columns,
        eq_encoding
            .parameters_bytes
            .max(ep_encoding.parameters_bytes),
        eq_encoding
            .proving_key_bytes
            .max(ep_encoding.proving_key_bytes),
    );
    if eq_encoding
        .parameters_bytes
        .max(ep_encoding.parameters_bytes)
        > KAGEMUSHA_COMPACT_PARAMS_IPA_MAX_BYTES_V5
    {
        return Err(format!(
            "Kagemusha k17 probe parameters exceed the production {KAGEMUSHA_COMPACT_PARAMS_IPA_MAX_BYTES_V5}-byte corridor"
        ));
    }
    if eq_encoding
        .proving_key_bytes
        .max(ep_encoding.proving_key_bytes)
        > KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5
    {
        return Err(format!(
            "Kagemusha k17 candidate {advice_columns}/{lookup_columns} would require {} PK bytes, above the {}-byte release cap",
            eq_encoding
                .proving_key_bytes
                .max(ep_encoding.proving_key_bytes),
            KAGEMUSHA_COMPACT_PROVING_KEY_MAX_BYTES_V5,
        ));
    }
    if estimated_peak_bytes > KAGEMUSHA_GENERATION_MAX_ESTIMATED_BYTES_V4 {
        return Err(format!(
            "Kagemusha k17 candidate {advice_columns}/{lookup_columns} estimates {estimated_peak_bytes} peak bytes, above the {}-byte generator cap",
            KAGEMUSHA_GENERATION_MAX_ESTIMATED_BYTES_V4,
        ));
    }
    let eq_cs = configured_kagemusha_eq_constraint_system_v4(&step_eq_circuit_params)?;
    let ep_cs = configured_kagemusha_ep_constraint_system_v4(&step_ep_circuit_params)?;
    require_kagemusha_bootstrap_constraint_system_v5(
        &configured_kagemusha_eq_bootstrap_constraint_system_v5(&step_eq_circuit_params)?,
        &eq_cs,
        "Eq k17 probe",
    )?;
    require_kagemusha_bootstrap_constraint_system_v5(
        &configured_kagemusha_ep_bootstrap_constraint_system_v5(&step_ep_circuit_params)?,
        &ep_cs,
        "Ep k17 probe",
    )?;
    let eq_proof_bytes = kagemusha_augmented_proof_size_bytes_v5(&eq_cs, step_eq_circuit_params.k)?;
    let ep_proof_bytes = kagemusha_augmented_proof_size_bytes_v5(&ep_cs, step_ep_circuit_params.k)?;
    if eq_proof_bytes > KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4
        || ep_proof_bytes > KAGEMUSHA_STEP_PROOF_ABSOLUTE_MAX_BYTES_V4
    {
        return Err(format!(
            "Kagemusha k17 configured proof sizes {eq_proof_bytes}/{ep_proof_bytes} exceed the Step cap"
        ));
    }
    step_eq_circuit_params.max_parent_proof_bytes = eq_proof_bytes;
    step_ep_circuit_params.max_parent_proof_bytes = ep_proof_bytes;
    let step_eq_params = ParamsIPA::<EqAffine>::new(step_eq_circuit_params.k);
    let step_eq_seed =
        kagemusha_k17_eq_probe_seed_v5(&step_eq_params, &step_eq_circuit_params, eq_proof_bytes)?;
    let step_ep_params = ParamsIPA::<EpAffine>::new(step_ep_circuit_params.k);
    let step_ep_seed =
        kagemusha_k17_ep_probe_seed_v5(&step_ep_params, &step_ep_circuit_params, ep_proof_bytes)?;
    let step_eq_bootstrap = kagemusha_eq_seed_bootstrap_payload_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        &step_eq_seed,
    )?;
    let step_ep_bootstrap = kagemusha_ep_seed_bootstrap_payload_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        &step_ep_seed,
    )?;
    let calibration = kagemusha_generation_calibration_v4(
        step_eq_seed.protocol_sha256,
        step_ep_seed.protocol_sha256,
    )?;
    let step_eq_recursion = kagemusha_eq_recursion_from_bootstrap_v4(
        &step_eq_params,
        &step_eq_circuit_params,
        step_eq_seed.protocol.clone(),
        step_eq_seed.structure_sha256,
        &step_eq_bootstrap,
        KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
    )?;
    let step_ep_recursion = kagemusha_ep_recursion_from_bootstrap_v4(
        &step_ep_params,
        &step_ep_circuit_params,
        step_ep_seed.protocol.clone(),
        step_ep_seed.structure_sha256,
        &step_ep_bootstrap,
        KagemushaBootstrapParentValidationV4::ProvisionalPreKeygen,
    )?;
    let witness = KagemushaStepWitnessV4 {
        public_inputs: &calibration.public_inputs,
        proof_step_count: 1,
        secure: &calibration.secure,
        output_membership: &calibration.output_membership,
        step_eq_recursion: &step_eq_recursion,
        step_ep_recursion: &step_ep_recursion,
        step_eq_bootstrap: Some(&step_eq_bootstrap),
        step_ep_bootstrap: Some(&step_ep_bootstrap),
    };
    let (eq_output, ep_output) = collect_kagemusha_step_scalar_audits_v5(
        &witness,
        &step_eq_circuit_params,
        &step_ep_circuit_params,
        false,
    )?;
    if mode == KagemushaK17ProbeIterationModeV5::AuditInventory {
        let eq_inventory = kagemusha_k17_audit_inventory_v6(&eq_output, &step_eq_circuit_params)?;
        let ep_inventory = kagemusha_k17_audit_inventory_v6(&ep_output, &step_ep_circuit_params)?;
        eq_inventory.validate("StepEq")?;
        ep_inventory.validate("StepEp")?;
        return Ok(KagemushaK17ProbeIterationOutcomeV5::AuditInventory(
            Box::new([("StepEq", eq_inventory), ("StepEp", ep_inventory)]),
        ));
    }
    KAGEMUSHA_K17_SHAPE_PROBE_REQUIRED_V5.with(|captured| captured.borrow_mut().clear());
    // Purge setup allocator slack before the fixed 64-GiB circuit-build corridor.
    halo2_proofs::release_allocator_slack();
    let step_eq = build_kagemusha_step_eq_circuit_v5(
        &witness,
        step_eq_circuit_params.clone(),
        &step_ep_circuit_params,
        &ep_output,
        KagemushaStepPublicModeV4::Bootstrap,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    drop(step_eq);
    halo2_proofs::release_allocator_slack();
    let step_eq = build_kagemusha_step_eq_circuit_v5(
        &witness,
        step_eq_circuit_params.clone(),
        &step_ep_circuit_params,
        &ep_output,
        KagemushaStepPublicModeV4::Live,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    drop(step_eq);
    halo2_proofs::release_allocator_slack();
    let step_ep = build_kagemusha_step_ep_circuit_v5(
        &witness,
        &step_eq_circuit_params,
        step_ep_circuit_params.clone(),
        &eq_output,
        KagemushaStepPublicModeV4::Bootstrap,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    drop(step_ep);
    halo2_proofs::release_allocator_slack();
    let step_ep = build_kagemusha_step_ep_circuit_v5(
        &witness,
        &step_eq_circuit_params,
        step_ep_circuit_params,
        &eq_output,
        KagemushaStepPublicModeV4::Live,
        KagemushaCircuitBuilderStageV5::Keygen,
    )?;
    drop(step_ep);
    halo2_proofs::release_allocator_slack();
    let captured = KAGEMUSHA_K17_SHAPE_PROBE_REQUIRED_V5
        .with(|required| std::mem::take(&mut *required.borrow_mut()));
    let expected_roles = [
        "StepEqBootstrap",
        "StepEqLive",
        "StepEpBootstrap",
        "StepEpLive",
    ];
    if captured.len() != expected_roles.len()
        || captured.iter().map(|(role, _)| *role).ne(expected_roles)
    {
        return Err(format!(
            "Kagemusha k17 probe captured an unexpected populated-shape sequence: {:?}",
            captured.iter().map(|(role, _)| *role).collect::<Vec<_>>()
        ));
    }
    let required_shapes = captured
        .iter()
        .map(|(role, params)| kagemusha_k17_capture_required_shape_v5(role, params))
        .collect::<Result<Vec<_>, _>>()?;
    let widths = required_shapes
        .iter()
        .map(KagemushaK17CapturedShapeV5::widths)
        .collect::<Result<Vec<_>, _>>()?;
    let maximum_widths = widths.iter().copied().fold((0, 0), |maximum, shape| {
        (maximum.0.max(shape.0), maximum.1.max(shape.1))
    });
    println!(
        "k17_probe iteration={iteration} eq_bootstrap_required={:?} eq_live_required={:?} ep_bootstrap_required={:?} ep_live_required={:?} required_advice={} required_lookup={} eq_proof_bytes={eq_proof_bytes} ep_proof_bytes={ep_proof_bytes}",
        required_shapes[0],
        required_shapes[1],
        required_shapes[2],
        required_shapes[3],
        maximum_widths.0,
        maximum_widths.1,
    );
    Ok(KagemushaK17ProbeIterationOutcomeV5::PopulatedShape(
        Box::new(KagemushaK17ProbeIterationV5 {
            required_shapes,
            maximum_widths,
            step_eq_proof_size_bytes: eq_proof_bytes,
            step_ep_proof_size_bytes: ep_proof_bytes,
            step_eq_protocol_structure_sha256: step_eq_seed.structure_sha256,
            step_ep_protocol_structure_sha256: step_ep_seed.structure_sha256,
        }),
    ))
}
/// Measure the authentic deferred-audit inventories without constructing a populated Step circuit.
///
/// This non-shipping diagnostic runs the same two witness-only scalar prepasses as the shape
/// probe, reports exact checked V6 and historical/counterfactual V5 commitment geometry, then
/// returns before either reciprocal graph is built.
#[cfg(feature = "kagemusha-generation-memory-lab")]
pub fn run_kagemusha_k17_audit_inventory_probe_v6(
    advice_columns: u32,
    lookup_columns: u32,
    memory_guard: &KagemushaGenerationMemoryGuardV4,
) -> Result<(), String> {
    if advice_columns == 0 || lookup_columns == 0 {
        return Err("Kagemusha k17 audit-inventory probe arguments must be non-zero".to_owned());
    }
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)
        .thread_name(|_| "kagemusha-v6-audit-inventory".to_owned())
        .build()
        .map_err(|error| {
            format!("failed to build bounded Kagemusha audit-inventory pool: {error}")
        })?;
    pool.install(move || {
        let _memory_guard = memory_guard;
        let _scope = KagemushaK17ShapeProbeScopeV5::enter()?;
        let outcome = kagemusha_k17_shape_probe_iteration_v5(
            1,
            advice_columns,
            lookup_columns,
            KagemushaK17ProbeIterationModeV5::AuditInventory,
        )?;
        let KagemushaK17ProbeIterationOutcomeV5::AuditInventory(inventories) = outcome else {
            return Err(
                "Kagemusha k17 audit-inventory probe unexpectedly built a populated shape"
                    .to_owned(),
            );
        };
        for (parity, inventory) in *inventories {
            inventory.print(parity);
        }
        println!("k17_audit_inventory completed=true populated_step_circuits=0");
        use std::io::Write as _;
        std::io::stdout()
            .flush()
            .map_err(|error| format!("failed to flush Kagemusha audit inventory: {error}"))?;
        halo2_proofs::release_allocator_slack();
        Ok(())
    })
}
/// Run the non-shipping compact-k17 populated-shape diagnostic with transparent IPA, empty composite VKs, parseable dummy parents, and arbitrary accumulators.
/// It only populates the graph; it creates no PK, proof, or witness bytes.
/// Errors if resource bounds, Eq/Ep derivation, or closure fails.
#[cfg(feature = "kagemusha-generation-memory-lab")]
pub fn run_kagemusha_k17_shape_probe_v5(
    initial_advice_columns: u32,
    initial_lookup_columns: u32,
    maximum_iterations: usize,
    memory_guard: &KagemushaGenerationMemoryGuardV4,
) -> Result<(), String> {
    if initial_advice_columns == 0 || initial_lookup_columns == 0 || maximum_iterations == 0 {
        return Err("Kagemusha k17 shape probe arguments must be non-zero".to_owned());
    }
    // Match production's one-worker pool to bound MSM scratch; capture state is thread-local.
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(KAGEMUSHA_GENERATION_RAYON_THREADS_V5)
        .thread_name(|_| "kagemusha-v5-shape-probe".to_owned())
        .build()
        .map_err(|error| format!("failed to build bounded Kagemusha probe pool: {error}"))?;
    pool.install(move || {
        run_kagemusha_k17_shape_probe_in_pool_v5(
            initial_advice_columns,
            initial_lookup_columns,
            maximum_iterations,
            memory_guard,
        )
    })
}
#[cfg(feature = "kagemusha-generation-memory-lab")]
fn run_kagemusha_k17_shape_probe_in_pool_v5(
    initial_advice_columns: u32,
    initial_lookup_columns: u32,
    maximum_iterations: usize,
    _memory_guard: &KagemushaGenerationMemoryGuardV4,
) -> Result<(), String> {
    let _scope = KagemushaK17ShapeProbeScopeV5::enter()?;
    let mut candidate = (initial_advice_columns, initial_lookup_columns);
    for iteration in 1..=maximum_iterations {
        let outcome = kagemusha_k17_shape_probe_iteration_v5(
            iteration,
            candidate.0,
            candidate.1,
            KagemushaK17ProbeIterationModeV5::PopulatedShape,
        )?;
        let KagemushaK17ProbeIterationOutcomeV5::PopulatedShape(required) = outcome else {
            return Err("Kagemusha k17 shape probe returned an audit inventory".to_owned());
        };
        // Return dropped iteration graphs' allocator slack before closure confirmation.
        halo2_proofs::release_allocator_slack();
        let next = (
            candidate.0.max(required.maximum_widths.0),
            candidate.1.max(required.maximum_widths.1),
        );
        if next == candidate {
            let outcome = kagemusha_k17_shape_probe_iteration_v5(
                iteration + 1,
                candidate.0,
                candidate.1,
                KagemushaK17ProbeIterationModeV5::PopulatedShape,
            )?;
            let KagemushaK17ProbeIterationOutcomeV5::PopulatedShape(confirmed) = outcome else {
                return Err(
                    "Kagemusha k17 shape confirmation returned an audit inventory".to_owned(),
                );
            };
            halo2_proofs::release_allocator_slack();
            if (
                candidate.0.max(confirmed.maximum_widths.0),
                candidate.1.max(confirmed.maximum_widths.1),
            ) != candidate
                || confirmed != required
            {
                return Err(format!(
                    "Kagemusha k17 shape closure changed on confirmation: first={required:?}, second={confirmed:?}"
                ));
            }
            let production = (
                KAGEMUSHA_GENERATION_ADVICE_COLUMNS_V4
                    .first()
                    .copied()
                    .ok_or_else(|| "Kagemusha k17 production advice profile is empty".to_owned())?,
                KAGEMUSHA_GENERATION_LOOKUP_COLUMNS_V4
                    .first()
                    .copied()
                    .ok_or_else(|| "Kagemusha k17 production lookup profile is empty".to_owned())?,
            );
            if candidate != production {
                return Err(format!(
                    "Kagemusha k17 populated closure {candidate:?} does not match the production profile {production:?}"
                ));
            }
            let populated_widths = required
                .required_shapes
                .iter()
                .map(KagemushaK17CapturedShapeV5::widths)
                .collect::<Result<Vec<_>, _>>()?;
            let expected_populated_widths: [(u32, u32); 4] =
                [(175, 19), (175, 19), (159, 19), (159, 19)];
            if populated_widths.as_slice() != expected_populated_widths.as_slice() {
                return Err(format!(
                    "Kagemusha k17 populated role widths {populated_widths:?} differ from the reviewed production widths {expected_populated_widths:?}"
                ));
            }
            if (
                required.step_eq_proof_size_bytes,
                required.step_ep_proof_size_bytes,
            ) != (
                KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4,
                KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4,
            ) {
                return Err(format!(
                    "Kagemusha k17 proof sizes ({}, {}) differ from the reviewed production size {}",
                    required.step_eq_proof_size_bytes,
                    required.step_ep_proof_size_bytes,
                    KAGEMUSHA_STEP_PROOF_RELEASE_BYTES_V4,
                ));
            }
            println!(
                "k17_probe converged=true advice_columns={} lookup_columns={} required_advice={} required_lookup={} confirmation_iterations=2",
                candidate.0, candidate.1, required.maximum_widths.0, required.maximum_widths.1,
            );
            return Ok(());
        }
        println!(
            "k17_probe iteration={iteration} converged=false next_advice={} next_lookup={}",
            next.0, next.1,
        );
        candidate = next;
    }
    Err(format!(
        "Kagemusha k17 shape probe did not converge within {maximum_iterations} iterations; last candidate={candidate:?}"
    ))
}
