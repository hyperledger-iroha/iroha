//! Deterministic key generation (task T10), following the vendored
//! `keygen_vk`/`keygen_pk`.
//!
//! 1. Synthesize the circuit without witnesses ([`crate::frontend::synthesize`]
//!    with no instances): fixed values, selector activations and copy
//!    constraints. [`keygen_from_tables`] accepts these tables directly, so an
//!    imported assignment (for example the oracle's export of a vendored
//!    circuit) needs no special hook. Imported tables get the frontend's row
//!    rule: no copy, enabled selector or nonzero fixed value at or beyond the
//!    usable rows `u` ([`KeyError::UnusableRow`]). Table import has no
//!    soundness effect (the verifier evaluates only the descriptor and the
//!    key), so it is public API, not an oracle hook (spec 6.4).
//! 2. Finalize the constraint system: selector compression or one fixed
//!    column per selector. The selector columns follow the `configure`
//!    columns, as in the vendored key.
//! 3. Build the descriptor from the finalized system and the
//!    [`KeygenConfig`] protocol choices, and bind it ([`DescriptorBinding`]).
//! 4. Commit every fixed column and every permutation polynomial in
//!    evaluation form with the vendored default blind (`... + W`).
//!    `sigma_j(omega^i) = DELTA^{j'} omega^{i'}` for `(j', i') = mapping(j, i)`
//!    of the union-find assembly ([`PermutationAssembly`]), which merges
//!    cycles exactly as halo2 does, so the commitments, and therefore the VK
//!    bytes, equal the vendored ones.
//! 5. `transcript_repr` comes from the descriptor digest and the VK bytes.
//!
//! Every step is exact arithmetic on the caller's Rayon pool: the keys do not
//! depend on the thread count.

use ff::PrimeField;
use iroha_pasta::{PastaCurve, PastaField, fft::FftDomain, msm::MemoryBudget};

use super::{
    DescriptorBinding, KeyError, check_shape,
    pk::{CosetCachePolicy, KeyConstraintSystem, ProvingKey},
    vk::VerifyingKey,
};
use crate::{
    cs::{
        CircuitDescriptorV1, CircuitDescriptorV2, ConstraintSystem, DescriptorConfig,
        InstanceModeV1, InstanceType, PermutationAssembly, ProofSuffixV1, TranscriptV1,
        TranscriptV2,
    },
    frontend::{Circuit, synthesize},
    pcs::{
        curve_v1,
        ipa::{
            PinnedParams,
            commit::{CommitmentTables, Secrecy, commit_lagrange, default_blind},
        },
    },
};

/// The protocol choices and resources of key generation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeygenConfig {
    /// The Fiat-Shamir transcript.
    pub transcript: TranscriptV1,
    /// How instance columns enter the proof.
    pub instance_mode: InstanceModeV1,
    /// Whether proofs carry the folded generator.
    pub proof_suffix: ProofSuffixV1,
    /// Whether halo2 selector compression runs.
    pub compress_selectors: bool,
    /// Whether the proving key precomputes the coset cache.
    pub coset_cache: CosetCachePolicy,
    /// Budget for the commitment-key tables of the proving key (`None`
    /// builds none).
    pub table_budget: Option<MemoryBudget>,
    /// Budget of each key-generation MSM.
    pub msm_budget: MemoryBudget,
}

impl KeygenConfig {
    /// A configuration for `transcript`: Committed instances, no suffix,
    /// selector compression, an eager coset cache, no tables and the default
    /// MSM budget.
    #[must_use]
    pub const fn new(transcript: TranscriptV1) -> Self {
        Self {
            transcript,
            instance_mode: InstanceModeV1::Committed,
            proof_suffix: ProofSuffixV1::None,
            compress_selectors: true,
            coset_cache: CosetCachePolicy::Eager,
            table_budget: None,
            msm_budget: MemoryBudget::DEFAULT,
        }
    }
}

/// Explicit V2 protocol choices and key-generation resources.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KeygenConfigV2 {
    /// Transcript profile bound into the descriptor.
    pub transcript: TranscriptV2,
    /// Public-input mode (PIPA-R requires Direct).
    pub instance_mode: InstanceModeV1,
    /// Opening suffix (PIPA-R requires `FoldedGenerator`).
    pub proof_suffix: ProofSuffixV1,
    /// One integer type per instance column.
    pub instance_types: Vec<InstanceType>,
    /// Whether selector compression runs.
    pub compress_selectors: bool,
    /// Proving-key coset policy.
    pub coset_cache: CosetCachePolicy,
    /// Optional fixed-base commitment-table budget.
    pub table_budget: Option<MemoryBudget>,
    /// Key-generation MSM budget.
    pub msm_budget: MemoryBudget,
}
impl KeygenConfigV2 {
    /// The PIPA-R profile with Direct instances and a folded-generator suffix.
    #[must_use]
    pub fn pipa_r(instance_types: Vec<InstanceType>) -> Self {
        Self {
            transcript: TranscriptV2::KagemushaPoseidonRp57Base,
            instance_mode: InstanceModeV1::Direct,
            proof_suffix: ProofSuffixV1::FoldedGenerator,
            instance_types,
            compress_selectors: true,
            coset_cache: CosetCachePolicy::Eager,
            table_budget: None,
            msm_budget: MemoryBudget::DEFAULT,
        }
    }
    // Only the common layout builder consumes this intermediate. Binding
    // below always uses the original V2 profile and the V2 domain.
    fn layout_options(&self) -> KeygenConfig {
        KeygenConfig {
            transcript: TranscriptV1::Blake2bChallenge255,
            instance_mode: self.instance_mode,
            proof_suffix: self.proof_suffix,
            compress_selectors: self.compress_selectors,
            coset_cache: self.coset_cache,
            table_budget: self.table_budget,
            msm_budget: self.msm_budget,
        }
    }
}

/// `sigma_j(omega^i) = DELTA^{j'} omega^{i'}` for every equality column `j` and
/// row `i`, with `(j', i') = mapping(j, i)`.
///
/// # Errors
///
/// [`KeyError::Shape`] when the assembly has no mapping for a cell.
pub fn permutation_values<F: PastaField>(
    assembly: &PermutationAssembly,
    omega: F,
) -> Result<Vec<Vec<F>>, KeyError> {
    let n = assembly.rows();
    let mut omega_powers = Vec::with_capacity(n);
    let mut power = F::ONE;
    for _ in 0..n {
        omega_powers.push(power);
        power *= omega;
    }
    let columns = assembly.columns().len();
    let mut delta_powers = Vec::with_capacity(columns);
    let mut delta = F::ONE;
    for _ in 0..columns {
        delta_powers.push(delta);
        delta *= <F as PrimeField>::DELTA;
    }
    (0..columns)
        .map(|column| {
            (0..n)
                .map(|row| {
                    let (target_column, target_row) =
                        assembly.mapping(column, row).ok_or(KeyError::Shape {
                            what: "permutation mapping",
                            expected: n,
                            actual: row,
                        })?;
                    Ok(omega_powers[target_row] * delta_powers[target_column])
                })
                .collect()
        })
        .collect()
}

/// Everything both keys are built from.
struct Prepared<F: PastaField> {
    binding: DescriptorBinding,
    constraint_system: KeyConstraintSystem<F>,
    fixed: Vec<Vec<F>>,
    sigma: Vec<Vec<F>>,
    copy_digest: [u8; 32],
    vk_selectors: Vec<Vec<bool>>,
}

/// Rejects a key-generation table that touches a row at or beyond the usable
/// rows `usable_rows`: a copy (a cell in a cycle of length two or more), an
/// enabled selector or a nonzero fixed value. The frontend never produces
/// one; imported tables must follow the same rule, because a copy or a gate
/// in the blinding rows constrains values the prover fills at random.
fn check_usable_rows<F: PastaField>(
    usable_rows: usize,
    fixed: &[Vec<F>],
    selectors: &[Vec<bool>],
    permutation: &PermutationAssembly,
) -> Result<(), KeyError> {
    let unusable = |what, column, row| KeyError::UnusableRow { what, column, row };
    for (column, values) in fixed.iter().enumerate() {
        if let Some(row) =
            (usable_rows..values.len()).find(|row| !bool::from(values[*row].is_zero()))
        {
            return Err(unusable("fixed", column, row));
        }
    }
    for (selector, rows) in selectors.iter().enumerate() {
        if let Some(row) = (usable_rows..rows.len()).find(|row| rows[*row]) {
            return Err(unusable("selector", selector, row));
        }
    }
    for column in 0..permutation.columns().len() {
        if let Some(row) =
            (usable_rows..permutation.rows()).find(|row| permutation.is_copied(column, *row))
        {
            return Err(unusable("copy", column, row));
        }
    }
    Ok(())
}

/// Steps 2 and 3, plus the permutation values.
fn prepare<C: PastaCurve>(
    params: &PinnedParams<C>,
    cs: ConstraintSystem<C::ScalarExt>,
    fixed: Vec<Vec<C::ScalarExt>>,
    selectors: Vec<Vec<bool>>,
    permutation: &PermutationAssembly,
    config: &KeygenConfig,
    v2: Option<&KeygenConfigV2>,
) -> Result<Prepared<C::ScalarExt>, KeyError> {
    let curve = curve_v1::<C>().ok_or(KeyError::UnknownCurve)?;
    let k = params.k();
    let domain = FftDomain::<C::ScalarExt>::new(k)?;
    let n = domain.n();
    check_shape("fixed columns", cs.num_fixed_columns(), fixed.len())?;
    check_shape("selectors", cs.num_selectors(), selectors.len())?;
    for column in &fixed {
        check_shape("fixed rows", n, column.len())?;
    }
    for rows in &selectors {
        check_shape("selector rows", n, rows.len())?;
    }
    check_shape("permutation rows", n, permutation.rows())?;
    if permutation.columns() != cs.permutation().columns() {
        return Err(KeyError::Shape {
            what: "permutation columns",
            expected: cs.permutation().columns().len(),
            actual: permutation.columns().len(),
        });
    }
    let usable_rows = cs.usable_rows(k)?;
    check_usable_rows(usable_rows, &fixed, &selectors, permutation)?;

    let finalized = cs.finalize(&selectors, config.compress_selectors)?;
    let descriptor = CircuitDescriptorV1::from_constraint_system(
        &finalized,
        DescriptorConfig {
            curve,
            k,
            transcript: config.transcript,
            instance_mode: config.instance_mode,
            proof_suffix: config.proof_suffix,
        },
    )?;
    let binding = match v2 {
        Some(profile) => DescriptorBinding::new_v2(CircuitDescriptorV2::from_layout(
            descriptor,
            profile.transcript,
            profile.instance_types.clone(),
        )?)?,
        None => DescriptorBinding::new(descriptor)?,
    };
    // The selector columns move into the fixed columns: the key keeps one
    // copy, and keeps the constraint system without them.
    let (constraint_system, selector_columns) = KeyConstraintSystem::split(finalized);
    let mut fixed = fixed;
    fixed.extend(selector_columns);
    let sigma = permutation_values(permutation, domain.omega())?;
    let copy_digest = permutation.mapping_digest();
    let vk_selectors = if config.compress_selectors {
        selectors
    } else {
        Vec::new()
    };
    Ok(Prepared {
        binding,
        constraint_system,
        fixed,
        sigma,
        copy_digest,
        vk_selectors,
    })
}

/// Step 4: the verifying key.
fn verifying_key<C: PastaCurve>(
    params: &PinnedParams<C>,
    prepared: &Prepared<C::ScalarExt>,
    budget: MemoryBudget,
) -> Result<VerifyingKey<C>, KeyError> {
    let blind = default_blind::<C::ScalarExt>();
    let commit_all = |columns: &[Vec<C::ScalarExt>]| -> Result<Vec<C::AffineExt>, KeyError> {
        columns
            .iter()
            .map(|column| {
                Ok(
                    commit_lagrange(params.params(), column, &blind, Secrecy::Public, budget)?
                        .to_affine(),
                )
            })
            .collect()
    };
    let fixed = commit_all(&prepared.fixed)?;
    let sigma = commit_all(&prepared.sigma)?;
    VerifyingKey::from_parts(
        &prepared.binding,
        fixed,
        sigma,
        prepared.vk_selectors.clone(),
    )
    .map_err(KeyError::from)
}

/// Builds the expensive key only after synthesis metadata has been released.
fn finish_pk<C: PastaCurve>(
    params: &PinnedParams<C>,
    prepared: Prepared<C::ScalarExt>,
    coset_cache: CosetCachePolicy,
    table_budget: Option<MemoryBudget>,
    msm_budget: MemoryBudget,
) -> Result<ProvingKey<C>, KeyError> {
    let vk = verifying_key(params, &prepared, msm_budget)?;
    let tables = table_budget.map_or_else(CommitmentTables::none, |budget| {
        CommitmentTables::build(params.params(), budget)
    });
    ProvingKey::new(
        vk,
        prepared.binding,
        prepared.constraint_system,
        prepared.fixed,
        prepared.sigma,
        prepared.copy_digest,
        coset_cache,
        tables,
    )
}

/// Validate imported originals against native source without generating a VK.
#[allow(clippy::too_many_arguments)]
pub(super) fn import_artifact_v2<C: PastaCurve, Ci: Circuit<C::ScalarExt>>(
    params: &PinnedParams<C>,
    circuit: &Ci,
    binding: &DescriptorBinding,
    vk: VerifyingKey<C>,
    fixed: Vec<Vec<C::ScalarExt>>,
    sigma: Vec<Vec<C::ScalarExt>>,
    copy_digest: [u8; 32],
    config: super::pk::artifact::ReadConfig,
    cancellation: Option<&iroha_pasta::CancellationToken>,
) -> Result<ProvingKey<C>, super::pk::artifact::Error> {
    use super::pk::artifact::Error;
    iroha_pasta::CancellationToken::checkpoint(cancellation)?;
    let descriptor = binding.descriptor();
    let profile = KeygenConfigV2 {
        transcript: descriptor.transcript,
        instance_mode: descriptor.instance_mode,
        proof_suffix: descriptor.proof_suffix,
        instance_types: descriptor.instance_types.clone().ok_or(Error::Profile)?,
        compress_selectors: descriptor.selectors.compress,
        coset_cache: config.coset_cache,
        table_budget: None,
        msm_budget: config.msm_budget,
    };
    let synthesized = crate::frontend::synthesize_cancellable(
        &circuit.without_witnesses(),
        params.k(),
        None,
        cancellation,
    )
    .map_err(KeyError::from)?;
    let (source_fixed, selectors, permutation) = synthesized.tables.into_keygen_parts();
    let source = prepare(
        params,
        synthesized.cs,
        source_fixed,
        selectors,
        &permutation,
        &profile.layout_options(),
        Some(&profile),
    )?;
    iroha_pasta::CancellationToken::checkpoint(cancellation)?;
    drop(permutation);
    if &source.binding != binding {
        return Err(Error::Profile);
    }
    if source.copy_digest != copy_digest
        || source.fixed != fixed
        || source.sigma != sigma
        || source.vk_selectors != vk.selectors()
    {
        return Err(Error::Source);
    }
    let blind = default_blind::<C::ScalarExt>();
    for (column, expected) in fixed.iter().chain(&sigma).zip(
        vk.fixed_commitments()
            .iter()
            .chain(vk.permutation_commitments()),
    ) {
        let actual = crate::pcs::ipa::commit::commit_lagrange_cancellable(
            params.params(),
            column,
            &blind,
            Secrecy::Public,
            config.msm_budget,
            cancellation,
        )
        .map_err(KeyError::from)?
        .to_affine();
        if &actual != expected {
            return Err(Error::Commitment);
        }
    }
    let Prepared {
        constraint_system,
        binding: source_binding,
        fixed: source_fixed,
        sigma: source_sigma,
        vk_selectors,
        ..
    } = source;
    // Retain one original table set while constructing coefficients and masks.
    drop((source_binding, source_fixed, source_sigma, vk_selectors));
    ProvingKey::new_cancellable(
        vk,
        binding.clone(),
        constraint_system,
        fixed,
        sigma,
        copy_digest,
        config.coset_cache,
        CommitmentTables::none(),
        cancellation,
    )
    .map_err(Error::from)
}

/// Generates the proving key (and its verifying key) from explicit tables:
/// the pre-substitution constraint system, the `configure` fixed columns, the
/// selector activations and the copy constraints, each over `n = 2^k` rows of
/// the parameters. This is the public assignment import (for tables produced
/// by another frontend, for example the oracle's vendored circuits); it has
/// no soundness effect.
///
/// # Errors
///
/// [`KeyError`] when a table has the wrong shape, touches a row at or beyond
/// the usable rows ([`KeyError::UnusableRow`]), the constraint system or
/// descriptor is invalid, or an MSM or FFT fails.
pub fn keygen_from_tables<C: PastaCurve>(
    params: &PinnedParams<C>,
    cs: ConstraintSystem<C::ScalarExt>,
    fixed: Vec<Vec<C::ScalarExt>>,
    selectors: Vec<Vec<bool>>,
    permutation: &PermutationAssembly,
    config: &KeygenConfig,
) -> Result<ProvingKey<C>, KeyError> {
    let prepared = prepare(params, cs, fixed, selectors, permutation, config, None)?;
    finish_pk(
        params,
        prepared,
        config.coset_cache,
        config.table_budget,
        config.msm_budget,
    )
}

/// Generates the verifying key of `circuit` at the parameters' `k`.
///
/// # Errors
///
/// As [`keygen_from_tables`], plus [`KeyError::Synthesis`].
pub fn keygen_vk<C: PastaCurve, Ci: Circuit<C::ScalarExt>>(
    params: &PinnedParams<C>,
    circuit: &Ci,
    config: &KeygenConfig,
) -> Result<VerifyingKey<C>, KeyError> {
    let synthesized = synthesize(circuit, params.k(), None)?;
    let (fixed, selectors, permutation) = synthesized.tables.into_keygen_parts();
    let prepared = prepare(
        params,
        synthesized.cs,
        fixed,
        selectors,
        &permutation,
        config,
        None,
    )?;
    drop(permutation);
    verifying_key(params, &prepared, config.msm_budget)
}

/// Generates the proving key of `circuit` at the parameters' `k`.
///
/// # Errors
///
/// As [`keygen_vk`].
pub fn keygen_pk<C: PastaCurve, Ci: Circuit<C::ScalarExt>>(
    params: &PinnedParams<C>,
    circuit: &Ci,
    config: &KeygenConfig,
) -> Result<ProvingKey<C>, KeyError> {
    let synthesized = synthesize(circuit, params.k(), None)?;
    let (fixed, selectors, permutation) = synthesized.tables.into_keygen_parts();
    let prepared = prepare(
        params,
        synthesized.cs,
        fixed,
        selectors,
        &permutation,
        config,
        None,
    )?;
    drop(permutation);
    finish_pk(
        params,
        prepared,
        config.coset_cache,
        config.table_budget,
        config.msm_budget,
    )
}

/// Generates a V2 proving key from explicit assignment tables.
///
/// # Errors
/// The ordinary key-generation errors and explicit V2 profile/type errors.
pub fn keygen_from_tables_v2<C: PastaCurve>(
    params: &PinnedParams<C>,
    cs: ConstraintSystem<C::ScalarExt>,
    fixed: Vec<Vec<C::ScalarExt>>,
    selectors: Vec<Vec<bool>>,
    permutation: &PermutationAssembly,
    config: &KeygenConfigV2,
) -> Result<ProvingKey<C>, KeyError> {
    let prepared = prepare(
        params,
        cs,
        fixed,
        selectors,
        permutation,
        &config.layout_options(),
        Some(config),
    )?;
    finish_pk(
        params,
        prepared,
        config.coset_cache,
        config.table_budget,
        config.msm_budget,
    )
}

/// Generates a V2 proving key from a circuit, using the same arithmetic engine.
///
/// # Errors
/// As [`keygen_from_tables_v2`], plus synthesis failures.
pub fn keygen_pk_v2<C: PastaCurve, Ci: Circuit<C::ScalarExt>>(
    params: &PinnedParams<C>,
    circuit: &Ci,
    config: &KeygenConfigV2,
) -> Result<ProvingKey<C>, KeyError> {
    let synthesized = synthesize(circuit, params.k(), None)?;
    let (fixed, selectors, permutation) = synthesized.tables.into_keygen_parts();
    let prepared = prepare(
        params,
        synthesized.cs,
        fixed,
        selectors,
        &permutation,
        &config.layout_options(),
        Some(config),
    )?;
    drop(permutation);
    finish_pk(
        params,
        prepared,
        config.coset_cache,
        config.table_budget,
        config.msm_budget,
    )
}

/// Generates a V2 verifying key without retaining a proving key.
///
/// # Errors
/// As [`keygen_pk_v2`].
pub fn keygen_vk_v2<C: PastaCurve, Ci: Circuit<C::ScalarExt>>(
    params: &PinnedParams<C>,
    circuit: &Ci,
    config: &KeygenConfigV2,
) -> Result<VerifyingKey<C>, KeyError> {
    keygen_vk_with_binding_v2(params, circuit, config).map(|(_, key)| key)
}

/// Generates a V2 verifier's descriptor binding and verifying key together.
///
/// Verifier-only consumers need both values to call the native verifier. This
/// path does not build a proving key, quotient cosets or commitment tables;
/// its descriptor and key are identical to [`keygen_pk_v2`]'s outputs.
///
/// # Errors
/// As [`keygen_pk_v2`].
pub fn keygen_vk_with_binding_v2<C: PastaCurve, Ci: Circuit<C::ScalarExt>>(
    params: &PinnedParams<C>,
    circuit: &Ci,
    config: &KeygenConfigV2,
) -> Result<(DescriptorBinding, VerifyingKey<C>), KeyError> {
    let synthesized = synthesize(circuit, params.k(), None)?;
    let (fixed, selectors, permutation) = synthesized.tables.into_keygen_parts();
    let prepared = prepare(
        params,
        synthesized.cs,
        fixed,
        selectors,
        &permutation,
        &config.layout_options(),
        Some(config),
    )?;
    drop(permutation);
    let key = verifying_key(params, &prepared, config.msm_budget)?;
    Ok((prepared.binding, key))
}
