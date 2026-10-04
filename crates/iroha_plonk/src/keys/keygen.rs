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
    pk::{CosetCachePolicy, ProvingKey},
    vk::VerifyingKey,
};
use crate::{
    cs::{
        CircuitDescriptorV1, ConstraintSystem, DescriptorConfig, FinalizedConstraintSystem,
        InstanceModeV1, PermutationAssembly, ProofSuffixV1, TranscriptV1,
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
    finalized: FinalizedConstraintSystem<F>,
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
    let binding = DescriptorBinding::new(descriptor)?;
    let mut fixed = fixed;
    fixed.extend(finalized.selector_columns().iter().cloned());
    let sigma = permutation_values(permutation, domain.omega())?;
    let copy_digest = permutation.mapping_digest();
    let vk_selectors = if config.compress_selectors {
        selectors
    } else {
        Vec::new()
    };
    Ok(Prepared {
        binding,
        finalized,
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
    let prepared = prepare(params, cs, fixed, selectors, permutation, config)?;
    let vk = verifying_key(params, &prepared, config.msm_budget)?;
    let tables = config
        .table_budget
        .map_or_else(CommitmentTables::none, |budget| {
            CommitmentTables::build(params.params(), budget)
        });
    ProvingKey::new(
        vk,
        prepared.binding,
        prepared.finalized,
        prepared.fixed,
        prepared.sigma,
        prepared.copy_digest,
        config.coset_cache,
        tables,
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
    let tables = synthesized.tables;
    let prepared = prepare(
        params,
        synthesized.cs,
        tables.fixed().to_vec(),
        tables.selectors().to_vec(),
        tables.permutation(),
        config,
    )?;
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
    let tables = synthesized.tables;
    keygen_from_tables(
        params,
        synthesized.cs,
        tables.fixed().to_vec(),
        tables.selectors().to_vec(),
        tables.permutation(),
        config,
    )
}
