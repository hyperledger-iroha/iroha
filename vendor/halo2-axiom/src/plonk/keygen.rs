#![allow(clippy::int_plus_one)]

use std::{fmt, ops::Range};

use ff::{Field, FromUniformBytes, WithSmallOrderMulGroup};
use group::Curve;

use super::{
    Assigned, Challenge, Error, LagrangeCoeff, Polynomial, ProvingKey, VerifyingKey,
    circuit::{
        Advice, Any, Assignment, Circuit, Column, ConstraintSystem, Fixed,
        FixedColumnModeAccumulator, FixedColumnModeCounts, FloorPlanner, Instance, Selector,
    },
    evaluation::Evaluator,
    permutation,
};
use crate::{
    arithmetic::{CurveAffine, parallelize},
    circuit::Value,
    helpers::release_allocator_slack,
    multicore::{IntoParallelIterator, ParallelIterator},
    poly::{
        EvaluationDomain, batch_invert_assigned_consuming,
        commitment::{Blind, Params},
    },
};

pub(crate) fn create_domain<C, ConcreteCircuit>(
    k: u32,
    #[cfg(feature = "circuit-params")] params: ConcreteCircuit::Params,
) -> (
    EvaluationDomain<C::Scalar>,
    ConstraintSystem<C::Scalar>,
    ConcreteCircuit::Config,
)
where
    C: CurveAffine,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    let mut cs = ConstraintSystem::default();
    #[cfg(feature = "circuit-params")]
    let config = ConcreteCircuit::configure_with_params(&mut cs, params);
    #[cfg(not(feature = "circuit-params"))]
    let config = ConcreteCircuit::configure(&mut cs);

    let degree = cs.degree();

    let domain = EvaluationDomain::new(degree as u32, k);

    (domain, cs, config)
}

/// Construct the three canonical row-mask polynomials retained by a proving key.
///
/// Keeping the formula in one helper prevents key construction and regression
/// tests from drifting on the exact inactive/blinding-row boundary.
pub(super) fn create_proving_key_masks<F>(
    domain: &EvaluationDomain<F>,
    blinding_factors: usize,
) -> (
    Polynomial<F, super::Coeff>,
    Polynomial<F, super::Coeff>,
    Polynomial<F, super::Coeff>,
)
where
    F: WithSmallOrderMulGroup<3>,
{
    let domain_rows = usize::try_from(domain.get_n()).expect("domain size fits usize");
    let last_active_row = domain_rows
        .checked_sub(blinding_factors + 1)
        .expect("constraint-system blinding rows fit the evaluation domain");

    // Compute l_0(X) directly without an FFT.
    let l0 = domain.lagrange_basis_0_coeff();

    // Compute l_blind(X), which evaluates to one for each blinding-factor row
    // and zero otherwise over the domain.
    let mut l_blind = domain.empty_lagrange();
    for evaluation in l_blind[..].iter_mut().rev().take(blinding_factors) {
        *evaluation = F::ONE;
    }

    // Compute l_last(X), which evaluates to one on the first inactive row
    // immediately before the blinding factors and zero otherwise.
    let mut l_last = domain.empty_lagrange();
    l_last[last_active_row] = F::ONE;

    // Compute l_active_row(X).
    let mut l_active_row = domain.empty_lagrange();
    parallelize(&mut l_active_row, |values, start| {
        for (i, value) in values.iter_mut().enumerate() {
            let idx = i + start;
            *value = F::ONE - (l_last[idx] + l_blind[idx]);
        }
    });
    drop(l_blind);

    let l_last = domain.lagrange_to_coeff(l_last);
    let l_active_row = domain.lagrange_to_coeff(l_active_row);
    (l0, l_last, l_active_row)
}

/// Assembly to be used in circuit synthesis.
#[derive(Debug)]
struct Assembly<F: Field> {
    k: u32,
    fixed: Vec<Polynomial<Assigned<F>, LagrangeCoeff>>,
    permutation: permutation::keygen::Assembly,
    selectors: Vec<Vec<bool>>,
    // A range of available rows for assignment and copies.
    usable_rows: Range<usize>,
    _marker: std::marker::PhantomData<F>,
}

impl<F: Field> Assignment<F> for Assembly<F> {
    fn enter_region<NR, N>(&mut self, _: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
        // Do nothing; we don't care about regions in this context.
    }

    fn exit_region(&mut self) {
        // Do nothing; we don't care about regions in this context.
    }

    fn enable_selector<A, AR>(&mut self, _: A, selector: &Selector, row: usize) -> Result<(), Error>
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        if !self.usable_rows.contains(&row) {
            return Err(Error::not_enough_rows_available(self.k));
        }

        self.selectors[selector.0][row] = true;

        Ok(())
    }

    fn query_instance(&self, _: Column<Instance>, row: usize) -> Result<Value<F>, Error> {
        if !self.usable_rows.contains(&row) {
            return Err(Error::not_enough_rows_available(self.k));
        }

        // There is no instance in this context.
        Ok(Value::unknown())
    }

    fn assign_advice<'v>(
        //<V, VR, A, AR>(
        &mut self,
        //_: A,
        _: Column<Advice>,
        _: usize,
        _: Value<Assigned<F>>,
    ) -> Value<&'v Assigned<F>> {
        Value::unknown()
    }

    fn assign_fixed(&mut self, column: Column<Fixed>, row: usize, to: Assigned<F>) {
        if !self.usable_rows.contains(&row) {
            panic!(
                "Assign Fixed {:?}",
                Error::not_enough_rows_available(self.k)
            );
        }

        *self
            .fixed
            .get_mut(column.index())
            .and_then(|v| v.get_mut(row))
            .unwrap_or_else(|| panic!("{:?}", Error::BoundsFailure)) = to;
    }

    fn copy(
        &mut self,
        left_column: Column<Any>,
        left_row: usize,
        right_column: Column<Any>,
        right_row: usize,
    ) {
        if !self.usable_rows.contains(&left_row) || !self.usable_rows.contains(&right_row) {
            panic!("{:?}", Error::not_enough_rows_available(self.k));
        }

        self.permutation
            .copy(left_column, left_row, right_column, right_row)
            .unwrap_or_else(|err| panic!("{err:?}"))
    }

    fn fill_from_row(
        &mut self,
        column: Column<Fixed>,
        from_row: usize,
        to: Value<Assigned<F>>,
    ) -> Result<(), Error> {
        if !self.usable_rows.contains(&from_row) {
            return Err(Error::not_enough_rows_available(self.k));
        }

        let col = self
            .fixed
            .get_mut(column.index())
            .ok_or(Error::BoundsFailure)?;

        let filler = to.assign()?;
        for row in self.usable_rows.clone().skip(from_row) {
            col[row] = filler;
        }

        Ok(())
    }

    fn get_challenge(&self, _: Challenge) -> Value<F> {
        Value::unknown()
    }

    fn annotate_column<A, AR>(&mut self, _annotation: A, _column: Column<Any>)
    where
        A: FnOnce() -> AR,
        AR: Into<String>,
    {
        // Do nothing
    }

    fn push_namespace<NR, N>(&mut self, _: N)
    where
        NR: Into<String>,
        N: FnOnce() -> NR,
    {
        // Do nothing; we don't care about namespaces in this context.
    }

    fn pop_namespace(&mut self, _: Option<String>) {
        // Do nothing; we don't care about namespaces in this context.
    }
}

/// Failure returned by consuming key generation with a post-synthesis extractor.
#[derive(Debug)]
pub enum KeygenWithExtractorError<E> {
    /// Circuit configuration or synthesis failed.
    Keygen(Error),
    /// The caller's post-synthesis extractor rejected the synthesized circuit.
    Extractor(E),
}

impl<E: fmt::Display> fmt::Display for KeygenWithExtractorError<E> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Keygen(error) => error.fmt(formatter),
            Self::Extractor(error) => error.fmt(formatter),
        }
    }
}

impl<E> std::error::Error for KeygenWithExtractorError<E> where E: fmt::Debug + fmt::Display {}

/// Exact column inventory for a synthesized key-generation assembly.
///
/// In particular, `materialized_selector_columns` accounts for the synthesized selector
/// activation overlap when compression is enabled. This can be inspected before selector
/// bitmaps are expanded into degree-sized field polynomials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeygenCircuitResourceProfile {
    /// Rows in the commitment domain.
    pub domain_rows: usize,
    /// Advice columns configured by the circuit.
    pub advice_columns: usize,
    /// Instance columns configured by the circuit.
    pub instance_columns: usize,
    /// Fixed columns configured before selector materialization.
    pub configured_fixed_columns: usize,
    /// Original virtual selectors retained as bitmaps in a compressed Processed key.
    pub selector_columns: usize,
    /// Fixed columns produced by the selected selector-materialization strategy.
    pub materialized_selector_columns: usize,
    /// Constant fixed columns, including configured and materialized selector columns.
    ///
    /// Constant zero, one, and other values all use one scalar payload and take precedence
    /// over the binary mode. No rational inversions or selector field expansion are required.
    pub constant_fixed_columns: usize,
    /// Nonconstant fixed columns containing only field zero and one.
    ///
    /// Each column uses `domain_rows.div_ceil(8)` payload bytes.
    pub binary_fixed_columns: usize,
    /// Remaining fixed columns, each using `domain_rows` scalar payloads.
    ///
    /// The three mode counts are disjoint and sum to `configured_fixed_columns` plus
    /// `materialized_selector_columns` for this selector strategy.
    pub raw_fixed_columns: usize,
    /// Columns participating in the permutation argument.
    pub permutation_columns: usize,
    /// Whether selector compression is selected for key construction.
    pub compress_selectors: bool,
}

/// Exact alternative selector encodings for one synthesized assembly.
///
/// Both inventories refer to the same constraint system and selector activations. Computing
/// them does not materialize selector field polynomials or construct either key.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeygenSelectorProfiles {
    /// Inventory after compression using the actual synthesized selector overlap.
    pub compressed: KeygenCircuitResourceProfile,
    /// Inventory after direct one-for-one selector conversion, without selector bitmaps.
    pub direct: KeygenCircuitResourceProfile,
}

fn keygen_circuit_resource_profile<F: Field>(
    domain_rows: usize,
    cs: &ConstraintSystem<F>,
    assembly: &Assembly<F>,
    compress_selectors: bool,
) -> KeygenCircuitResourceProfile {
    keygen_circuit_resource_profile_with_fixed_modes(
        domain_rows,
        cs,
        assembly,
        compress_selectors,
        configured_fixed_modes(assembly),
    )
}

fn configured_fixed_modes<F: Field>(assembly: &Assembly<F>) -> FixedColumnModeCounts {
    let mut modes = FixedColumnModeCounts::default();
    for polynomial in &assembly.fixed {
        let mut column = FixedColumnModeAccumulator::new();
        for &value in polynomial.iter() {
            column.observe(value, 1);
        }
        modes.add(column.finish());
    }
    modes
}

fn keygen_circuit_resource_profile_with_fixed_modes<F: Field>(
    domain_rows: usize,
    cs: &ConstraintSystem<F>,
    assembly: &Assembly<F>,
    compress_selectors: bool,
    mut fixed_modes: FixedColumnModeCounts,
) -> KeygenCircuitResourceProfile {
    let selector_modes = if compress_selectors {
        cs.compressed_selector_modes(&assembly.selectors)
    } else {
        cs.direct_selector_modes(&assembly.selectors)
    };
    let materialized_selector_columns = selector_modes.total();
    fixed_modes.add(selector_modes);
    KeygenCircuitResourceProfile {
        domain_rows,
        advice_columns: cs.num_advice_columns(),
        instance_columns: cs.num_instance_columns(),
        configured_fixed_columns: cs.num_fixed_columns(),
        selector_columns: cs.num_selectors(),
        materialized_selector_columns,
        constant_fixed_columns: fixed_modes.constant,
        binary_fixed_columns: fixed_modes.binary,
        raw_fixed_columns: fixed_modes.raw,
        permutation_columns: cs.permutation().get_columns().len(),
        compress_selectors,
    }
}

fn keygen_selector_profiles<F: Field>(
    domain_rows: usize,
    cs: &ConstraintSystem<F>,
    assembly: &Assembly<F>,
) -> KeygenSelectorProfiles {
    // Both alternatives share the same configured fixed assignments. Scan those only once.
    let fixed_modes = configured_fixed_modes(assembly);
    KeygenSelectorProfiles {
        compressed: keygen_circuit_resource_profile_with_fixed_modes(
            domain_rows,
            cs,
            assembly,
            true,
            fixed_modes,
        ),
        direct: keygen_circuit_resource_profile_with_fixed_modes(
            domain_rows,
            cs,
            assembly,
            false,
            fixed_modes,
        ),
    }
}

fn synthesize_keygen_assembly<'params, C, P, ConcreteCircuit>(
    params: &P,
    supplied_domain: Option<&EvaluationDomain<C::Scalar>>,
    circuit: &ConcreteCircuit,
) -> Result<
    (
        ConstraintSystem<C::Scalar>,
        Assembly<C::Scalar>,
        Option<EvaluationDomain<C::Scalar>>,
    ),
    Error,
>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    let mut cs = ConstraintSystem::default();
    #[cfg(feature = "circuit-params")]
    let config = ConcreteCircuit::configure_with_params(&mut cs, circuit.params());
    #[cfg(not(feature = "circuit-params"))]
    let config = ConcreteCircuit::configure(&mut cs);

    // A supplied verifier key already owns the exact evaluation domain that
    // the proving key will retain. Only key generation that must also produce
    // a verifier key needs to construct another domain.
    let generated_domain = supplied_domain
        .is_none()
        .then(|| EvaluationDomain::new(cs.degree() as u32, params.k()));
    let domain = supplied_domain.unwrap_or_else(|| {
        generated_domain
            .as_ref()
            .expect("key generation constructs an evaluation domain")
    });

    if (params.n() as usize) < cs.minimum_rows() {
        return Err(Error::not_enough_rows_available(params.k()));
    }

    let mut assembly: Assembly<C::Scalar> = Assembly {
        k: params.k(),
        fixed: vec![domain.empty_lagrange_assigned(); cs.num_fixed_columns],
        permutation: permutation::keygen::Assembly::new(params.n() as usize, &cs.permutation),
        selectors: vec![vec![false; params.n() as usize]; cs.num_selectors],
        usable_rows: 0..params.n() as usize - (cs.blinding_factors() + 1),
        _marker: std::marker::PhantomData,
    };

    ConcreteCircuit::FloorPlanner::synthesize(
        &mut assembly,
        circuit,
        config,
        cs.constants.clone(),
    )?;

    Ok((cs, assembly, generated_domain))
}

fn keygen_vk_from_assembly<'params, C, P>(
    params: &P,
    domain: EvaluationDomain<C::Scalar>,
    cs: ConstraintSystem<C::Scalar>,
    mut assembly: Assembly<C::Scalar>,
    compress_selectors: bool,
) -> VerifyingKey<C>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
{
    // The permutation verifier key only retains commitments. Build it while
    // fixed assignments and bit-packed selector activations are still in their
    // compact synthesis representation; expanding selectors into field
    // polynomials first would keep every fixed polynomial live through the
    // first degree-sized permutation commitment.
    let permutation_vk = assembly
        .permutation
        .build_vk(params, &domain, &cs.permutation);

    let mut fixed = batch_invert_assigned_consuming(
        assembly
            .fixed
            .into_iter()
            .map(|polynomial| polynomial.values)
            .collect(),
    );
    let (cs, selector_polys) = if compress_selectors {
        cs.compress_selectors(assembly.selectors.clone())
    } else {
        // The verifier does not need selectors, and keygen_pk regenerates its
        // constraint system from the circuit.
        let selectors = std::mem::take(&mut assembly.selectors);
        cs.directly_convert_selectors_to_fixed(selectors)
    };
    fixed.extend(
        selector_polys
            .into_iter()
            .map(|poly| domain.lagrange_from_vec(poly)),
    );

    // Rayon preserves canonical column order. Release each worker's completed
    // MSM scratch without serializing independent commitments; on Darwin that
    // returned scratch can otherwise remain compressed and charged across
    // hundreds of fixed columns.
    let fixed_commitments = (&fixed)
        .into_par_iter()
        .map(|poly| {
            let commitment = params.commit_lagrange(poly, Blind::default()).to_affine();
            release_allocator_slack();
            commitment
        })
        .collect();

    VerifyingKey::from_parts(
        domain,
        fixed_commitments,
        permutation_vk,
        cs,
        assembly.selectors,
        compress_selectors,
    )
}

/// Generate a `VerifyingKey` from an instance of `Circuit`.
/// By default, selector compression is turned **off**.
pub fn keygen_vk<'params, C, P, ConcreteCircuit>(
    params: &P,
    circuit: &ConcreteCircuit,
) -> Result<VerifyingKey<C>, Error>
where
    C: CurveAffine,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
    C::Scalar: FromUniformBytes<64>,
{
    keygen_vk_custom(params, circuit, false)
}

/// Generate a `VerifyingKey` from an instance of `Circuit`.
///
/// The selector compression optimization is turned on only if `compress_selectors` is `true`.
pub fn keygen_vk_custom<'params, C, P, ConcreteCircuit>(
    params: &P,
    circuit: &ConcreteCircuit,
    compress_selectors: bool,
) -> Result<VerifyingKey<C>, Error>
where
    C: CurveAffine,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
    C::Scalar: FromUniformBytes<64>,
{
    let (cs, assembly, generated_domain) =
        synthesize_keygen_assembly::<C, _, _>(params, None, circuit)?;
    let domain = generated_domain.expect("verifier-key generation constructs a domain");
    Ok(keygen_vk_from_assembly(
        params,
        domain,
        cs,
        assembly,
        compress_selectors,
    ))
}

/// Generate a verifier key while releasing an owned circuit before key assembly.
///
/// `extractor` runs immediately after successful synthesis. Its result must own
/// any data it retains. If it succeeds, `circuit` is dropped before permutation
/// polynomials and commitments are constructed. Selector compression is off,
/// matching [`keygen_vk`].
pub fn keygen_vk_consuming_with<
    'params,
    C,
    P,
    ConcreteCircuit,
    Extracted,
    ExtractError,
    Extractor,
>(
    params: &P,
    circuit: ConcreteCircuit,
    extractor: Extractor,
) -> Result<(VerifyingKey<C>, Extracted), KeygenWithExtractorError<ExtractError>>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
    Extractor: FnOnce(&ConcreteCircuit) -> Result<Extracted, ExtractError>,
{
    keygen_vk_consuming_with_profile(params, circuit, false, |circuit, _profile| {
        extractor(circuit)
    })
}

/// Generate a verifier key from an owned circuit with selectable selector compression.
///
/// `extractor` runs after successful synthesis and receives the exact synthesized column profile.
/// If it succeeds, `circuit` is dropped before permutation polynomials, selector field
/// polynomials, and commitments are constructed. This lets callers enforce serialized-resource
/// limits using actual selector overlap without first allocating the expensive key body.
pub fn keygen_vk_consuming_with_profile<
    'params,
    C,
    P,
    ConcreteCircuit,
    Extracted,
    ExtractError,
    Extractor,
>(
    params: &P,
    circuit: ConcreteCircuit,
    compress_selectors: bool,
    extractor: Extractor,
) -> Result<(VerifyingKey<C>, Extracted), KeygenWithExtractorError<ExtractError>>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
    Extractor:
        FnOnce(&ConcreteCircuit, KeygenCircuitResourceProfile) -> Result<Extracted, ExtractError>,
{
    let (cs, assembly, generated_domain) =
        synthesize_keygen_assembly::<C, _, _>(params, None, &circuit)
            .map_err(KeygenWithExtractorError::Keygen)?;
    let profile =
        keygen_circuit_resource_profile(params.n() as usize, &cs, &assembly, compress_selectors);
    let extracted = extractor(&circuit, profile).map_err(KeygenWithExtractorError::Extractor)?;
    drop(circuit);
    // The synthesized assembly is the only live owner needed below. On
    // Darwin, promptly purge pages freed with the much larger virtual circuit
    // graph before allocating permutation and MSM scratch.
    release_allocator_slack();

    let domain = generated_domain.expect("verifier-key generation constructs a domain");
    let vk = keygen_vk_from_assembly(params, domain, cs, assembly, compress_selectors);
    release_allocator_slack();
    Ok((vk, extracted))
}

/// Generate a verifier key with caller-selected encoding after one synthesis.
///
/// The callback receives both exact selector inventories before any key polynomials are
/// constructed. It returns the compression choice and extracted data, or rejects construction.
/// On success, the circuit is dropped before key expansion. Existing fixed-choice APIs retain
/// their original behavior; this API does not impose an encoding policy or resource limit.
pub fn keygen_vk_consuming_with_selector_choice<
    'params,
    C,
    P,
    ConcreteCircuit,
    Extracted,
    ExtractError,
    Extractor,
>(
    params: &P,
    circuit: ConcreteCircuit,
    extractor: Extractor,
) -> Result<(VerifyingKey<C>, Extracted), KeygenWithExtractorError<ExtractError>>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
    Extractor:
        FnOnce(&ConcreteCircuit, KeygenSelectorProfiles) -> Result<(bool, Extracted), ExtractError>,
{
    let (cs, assembly, generated_domain) =
        synthesize_keygen_assembly::<C, _, _>(params, None, &circuit)
            .map_err(KeygenWithExtractorError::Keygen)?;
    let profiles = keygen_selector_profiles(params.n() as usize, &cs, &assembly);
    let (compress_selectors, extracted) =
        extractor(&circuit, profiles).map_err(KeygenWithExtractorError::Extractor)?;
    drop(circuit);
    // The synthesized assembly is the only live owner needed below. On
    // Darwin, promptly purge pages freed with the much larger virtual circuit
    // graph before allocating permutation and MSM scratch.
    release_allocator_slack();

    let domain = generated_domain.expect("verifier-key generation constructs a domain");
    let vk = keygen_vk_from_assembly(params, domain, cs, assembly, compress_selectors);
    release_allocator_slack();
    Ok((vk, extracted))
}

/// Generate a `ProvingKey` from a `VerifyingKey` and an instance of `Circuit`.
pub fn keygen_pk<'params, C, P, ConcreteCircuit>(
    params: &P,
    vk: VerifyingKey<C>,
    circuit: &ConcreteCircuit,
) -> Result<ProvingKey<C>, Error>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    let compress_selectors = vk.compress_selectors;
    keygen_pk_impl(params, Some(vk), circuit, compress_selectors)
}

/// Generate a proving key while releasing an owned circuit before key assembly.
///
/// `extractor` runs immediately after successful synthesis. Its result must own
/// any data it retains. If it succeeds, `circuit` is dropped before fixed and
/// permutation proving-key polynomials are constructed.
pub fn keygen_pk_consuming_with<
    'params,
    C,
    P,
    ConcreteCircuit,
    Extracted,
    ExtractError,
    Extractor,
>(
    params: &P,
    vk: VerifyingKey<C>,
    circuit: ConcreteCircuit,
    extractor: Extractor,
) -> Result<(ProvingKey<C>, Extracted), KeygenWithExtractorError<ExtractError>>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
    Extractor: FnOnce(&ConcreteCircuit) -> Result<Extracted, ExtractError>,
{
    let compress_selectors = vk.compress_selectors;
    let (cs, assembly, generated_domain) =
        synthesize_keygen_assembly::<C, _, _>(params, Some(&vk.domain), &circuit)
            .map_err(KeygenWithExtractorError::Keygen)?;
    let extracted = extractor(&circuit).map_err(KeygenWithExtractorError::Extractor)?;
    drop(circuit);
    release_allocator_slack();

    let pk = keygen_pk_from_assembly(
        params,
        Some(vk),
        generated_domain,
        cs,
        assembly,
        compress_selectors,
    );
    release_allocator_slack();
    Ok((pk, extracted))
}

/// Generate a `ProvingKey` from an instance of `Circuit`. `VerifyingKey` is generated in the process.
pub fn keygen_pk2<'params, C, P, ConcreteCircuit>(
    params: &P,
    circuit: &ConcreteCircuit,
    compress_selectors: bool,
) -> Result<ProvingKey<C>, Error>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    keygen_pk_impl(params, None, circuit, compress_selectors)
}

/// Generate a `ProvingKey` from an owned circuit and release the circuit before key assembly.
///
/// This is the memory-bounded counterpart to [`keygen_pk2`]. It retains the latter's single
/// synthesis pass, but drops the generation-only circuit graph before fixed/permutation
/// polynomials and commitments are constructed. The returned proving key embeds the generated
/// verifying key exactly as `keygen_pk2` does.
pub fn keygen_pk2_consuming<'params, C, P, ConcreteCircuit>(
    params: &P,
    circuit: ConcreteCircuit,
    compress_selectors: bool,
) -> Result<ProvingKey<C>, Error>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    match keygen_pk2_consuming_with_profile(
        params,
        circuit,
        compress_selectors,
        |_circuit, _profile| Ok::<(), core::convert::Infallible>(()),
    ) {
        Ok((pk, ())) => Ok(pk),
        Err(KeygenWithExtractorError::Keygen(error)) => Err(error),
        Err(KeygenWithExtractorError::Extractor(never)) => match never {},
    }
}

/// Generate a combined proving/verifying key from an owned circuit with an exact resource guard.
///
/// The callback observes the synthesized selector overlap before any selector field polynomial or
/// proving-key polynomial is built. Returning an error aborts key assembly. On success, the owned
/// circuit is dropped before the expensive key construction starts.
pub fn keygen_pk2_consuming_with_profile<
    'params,
    C,
    P,
    ConcreteCircuit,
    Extracted,
    ExtractError,
    Extractor,
>(
    params: &P,
    circuit: ConcreteCircuit,
    compress_selectors: bool,
    extractor: Extractor,
) -> Result<(ProvingKey<C>, Extracted), KeygenWithExtractorError<ExtractError>>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
    Extractor:
        FnOnce(&ConcreteCircuit, KeygenCircuitResourceProfile) -> Result<Extracted, ExtractError>,
{
    let (cs, assembly, generated_domain) =
        synthesize_keygen_assembly::<C, _, _>(params, None, &circuit)
            .map_err(KeygenWithExtractorError::Keygen)?;
    let profile =
        keygen_circuit_resource_profile(params.n() as usize, &cs, &assembly, compress_selectors);
    let extracted = extractor(&circuit, profile).map_err(KeygenWithExtractorError::Extractor)?;
    drop(circuit);
    release_allocator_slack();
    let pk = keygen_pk_from_assembly(
        params,
        None,
        generated_domain,
        cs,
        assembly,
        compress_selectors,
    );
    release_allocator_slack();
    Ok((pk, extracted))
}

/// Generate a combined proving/verifying key with caller-selected encoding after one synthesis.
///
/// The callback receives both exact selector inventories before any key polynomials are
/// constructed. It returns the compression choice and extracted data, or rejects construction.
/// On success, the circuit is dropped before key expansion. Existing fixed-choice APIs retain
/// their original behavior; this API does not impose an encoding policy or resource limit.
pub fn keygen_pk2_consuming_with_selector_choice<
    'params,
    C,
    P,
    ConcreteCircuit,
    Extracted,
    ExtractError,
    Extractor,
>(
    params: &P,
    circuit: ConcreteCircuit,
    extractor: Extractor,
) -> Result<(ProvingKey<C>, Extracted), KeygenWithExtractorError<ExtractError>>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
    Extractor:
        FnOnce(&ConcreteCircuit, KeygenSelectorProfiles) -> Result<(bool, Extracted), ExtractError>,
{
    let (cs, assembly, generated_domain) =
        synthesize_keygen_assembly::<C, _, _>(params, None, &circuit)
            .map_err(KeygenWithExtractorError::Keygen)?;
    let profiles = keygen_selector_profiles(params.n() as usize, &cs, &assembly);
    let (compress_selectors, extracted) =
        extractor(&circuit, profiles).map_err(KeygenWithExtractorError::Extractor)?;
    drop(circuit);
    release_allocator_slack();
    let pk = keygen_pk_from_assembly(
        params,
        None,
        generated_domain,
        cs,
        assembly,
        compress_selectors,
    );
    release_allocator_slack();
    Ok((pk, extracted))
}

/// Generate a `ProvingKey` from either a precalculated `VerifyingKey` and an instance of `Circuit`, or
/// just a `Circuit`, in which case a new `VerifyingKey` is generated. The latter is more efficient because
/// it does fixed column FFTs only once.
pub fn keygen_pk_impl<'params, C, P, ConcreteCircuit>(
    params: &P,
    vk: Option<VerifyingKey<C>>,
    circuit: &ConcreteCircuit,
    compress_selectors: bool,
) -> Result<ProvingKey<C>, Error>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
    ConcreteCircuit: Circuit<C::Scalar>,
{
    let supplied_domain = vk.as_ref().map(|vk| &vk.domain);
    let (cs, assembly, generated_domain) =
        synthesize_keygen_assembly::<C, _, _>(params, supplied_domain, circuit)?;
    Ok(keygen_pk_from_assembly(
        params,
        vk,
        generated_domain,
        cs,
        assembly,
        compress_selectors,
    ))
}

fn keygen_pk_from_assembly<'params, C, P>(
    params: &P,
    mut vk: Option<VerifyingKey<C>>,
    mut generated_domain: Option<EvaluationDomain<C::Scalar>>,
    cs: ConstraintSystem<C::Scalar>,
    mut assembly: Assembly<C::Scalar>,
    compress_selectors: bool,
) -> ProvingKey<C>
where
    C: CurveAffine,
    C::Scalar: FromUniformBytes<64>,
    P: Params<'params, C> + Sync,
{
    let domain = match vk.as_ref() {
        Some(vk) => &vk.domain,
        None => generated_domain
            .as_ref()
            .expect("keygen_pk2 constructs an evaluation domain"),
    };
    let mut fixed = batch_invert_assigned_consuming(
        assembly
            .fixed
            .into_iter()
            .map(|polynomial| polynomial.values)
            .collect(),
    );
    let (cs, selector_polys) = if compress_selectors {
        if vk.is_some() {
            let selectors = std::mem::take(&mut assembly.selectors);
            cs.compress_selectors(selectors)
        } else {
            cs.compress_selectors(assembly.selectors.clone())
        }
    } else {
        let selectors = std::mem::take(&mut assembly.selectors);
        cs.directly_convert_selectors_to_fixed(selectors)
    };
    fixed.extend(
        selector_polys
            .into_iter()
            .map(|poly| domain.lagrange_from_vec(poly)),
    );

    #[cfg(not(feature = "thread-safe-region"))]
    let (permutation_pk, vk) = if let Some(vk) = vk.take() {
        {
            let permutation_pk = assembly
                .permutation
                .build_pk(params, &vk.domain, &cs.permutation);
            (permutation_pk, vk)
        }
    } else {
        {
            let domain = generated_domain
                .take()
                .expect("keygen_pk2 constructs an evaluation domain");
            let (permutation_pk, permutation_vk) =
                assembly
                    .permutation
                    .build_pk_and_vk(params, &domain, &cs.permutation);

            let fixed_commitments = (&fixed)
                .into_par_iter()
                .map(|poly| {
                    let commitment = params.commit_lagrange(poly, Blind::default()).to_affine();
                    release_allocator_slack();
                    commitment
                })
                .collect();

            let vk = VerifyingKey::from_parts(
                domain,
                fixed_commitments,
                permutation_vk,
                cs,
                assembly.selectors,
                compress_selectors,
            );
            (permutation_pk, vk)
        }
    };

    #[cfg(feature = "thread-safe-region")]
    let (permutation_pk, vk) = {
        if let Some(vk) = vk.take() {
            let permutation_pk = assembly
                .permutation
                .build_pk(params, &vk.domain, &cs.permutation);
            (permutation_pk, vk)
        } else {
            let domain = generated_domain
                .take()
                .expect("keygen_pk2 constructs an evaluation domain");
            let permutation_vk = assembly
                .permutation
                .build_vk(params, &domain, &cs.permutation);
            let permutation_pk = assembly
                .permutation
                .build_pk(params, &domain, &cs.permutation);

            let fixed_commitments = (&fixed)
                .into_par_iter()
                .map(|poly| {
                    let commitment = params.commit_lagrange(poly, Blind::default()).to_affine();
                    release_allocator_slack();
                    commitment
                })
                .collect();

            let vk = VerifyingKey::from_parts(
                domain,
                fixed_commitments,
                permutation_vk,
                cs,
                assembly.selectors,
                compress_selectors,
            );
            (permutation_pk, vk)
        }
    };

    let fixed_polys: Vec<_> = fixed
        .iter()
        .map(|poly| vk.domain.lagrange_to_coeff(poly.clone()))
        .collect();

    let (l0, l_last, l_active_row) = create_proving_key_masks(&vk.domain, vk.cs.blinding_factors());

    // Compute the optimized evaluation data structure
    let ev = Evaluator::new(&vk.cs);

    ProvingKey {
        vk,
        l0,
        l_last,
        l_active_row,
        fixed_values: fixed,
        fixed_polys,
        permutation: permutation_pk,
        ev,
    }
}

#[cfg(test)]
mod fixed_column_profile_tests {
    use super::*;
    use crate::poly::Rotation;
    use halo2curves::pasta::{Fp, Fq};

    fn materialized_modes<F: Field>(columns: &[Vec<F>]) -> (usize, usize, usize) {
        let mut result = (0, 0, 0);
        for values in columns {
            if values.iter().all(|value| value == &values[0]) {
                result.0 += 1;
            } else if values
                .iter()
                .all(|value| *value == F::ZERO || *value == F::ONE)
            {
                result.1 += 1;
            } else {
                result.2 += 1;
            }
        }
        result
    }

    fn profile_cases<F: WithSmallOrderMulGroup<3>>() {
        let rows = 8;
        let mut cs = ConstraintSystem::<F>::default();
        let advice = cs.advice_column();
        cs.instance_column();
        cs.enable_equality(advice);
        for index in 0..5 {
            let fixed = cs.fixed_column();
            if index == 0 {
                cs.enable_equality(fixed);
            }
        }
        let simple = [cs.selector(), cs.selector(), cs.selector()];
        cs.complex_selector();
        cs.set_minimum_degree(4);
        cs.create_gate("profile selectors", |meta| {
            simple.map(|selector| {
                meta.query_selector(selector) * meta.query_advice(advice, Rotation::cur())
            })
        });
        let one = F::ONE;
        let two = one + one;
        let domain = EvaluationDomain::<F>::new(cs.degree() as u32, 3);
        let values = vec![
            vec![Assigned::Rational(one, F::ZERO); rows],
            vec![Assigned::Rational(two, two); rows],
            vec![Assigned::Rational(two + two, two); rows],
            (0..rows)
                .map(|row| {
                    if row % 2 == 0 {
                        Assigned::Zero
                    } else {
                        Assigned::Rational(two, two)
                    }
                })
                .collect(),
            (0..rows)
                .map(|row| {
                    if row % 2 == 0 {
                        Assigned::Zero
                    } else {
                        Assigned::Trivial(two)
                    }
                })
                .collect(),
        ];
        let schedules = [
            (
                vec![
                    vec![false; rows],
                    vec![true; rows],
                    vec![false; rows],
                    (0..rows).map(|row| row % 2 == 0).collect(),
                ],
                (4, 2, 1),
                (6, 2, 1),
            ),
            (
                vec![
                    (0..rows).map(|row| row == 0).collect(),
                    (0..rows).map(|row| row < 2).collect(),
                    (0..rows).map(|row| row == 2).collect(),
                    vec![false; rows],
                ],
                (4, 2, 2),
                (4, 4, 1),
            ),
        ];
        for (selectors, compressed_expected, direct_expected) in schedules {
            let assembly = Assembly {
                k: 3,
                fixed: values
                    .iter()
                    .cloned()
                    .map(|column| domain.lagrange_assigned_from_vec(column))
                    .collect(),
                permutation: permutation::keygen::Assembly::new(rows, &cs.permutation),
                selectors: selectors.clone(),
                usable_rows: 0..rows,
                _marker: std::marker::PhantomData,
            };
            let profiles = keygen_selector_profiles(rows, &cs, &assembly);
            assert_eq!(
                profiles.compressed,
                keygen_circuit_resource_profile(rows, &cs, &assembly, true)
            );
            assert_eq!(
                profiles.direct,
                keygen_circuit_resource_profile(rows, &cs, &assembly, false)
            );
            for (profile, expected) in [
                (profiles.compressed, compressed_expected),
                (profiles.direct, direct_expected),
            ] {
                assert_eq!(profile.domain_rows, rows);
                assert_eq!(profile.advice_columns, 1);
                assert_eq!(profile.instance_columns, 1);
                assert_eq!(profile.configured_fixed_columns, 5);
                assert_eq!(profile.selector_columns, 4);
                assert_eq!(profile.permutation_columns, 2);
                let (_, selector_values) = if profile.compress_selectors {
                    cs.clone().compress_selectors(selectors.clone())
                } else {
                    cs.clone()
                        .directly_convert_selectors_to_fixed(selectors.clone())
                };
                assert_eq!(profile.materialized_selector_columns, selector_values.len());
                let mut materialized = values
                    .iter()
                    .map(|column| column.iter().copied().map(Assigned::evaluate).collect())
                    .collect::<Vec<_>>();
                materialized.extend(selector_values);
                let actual = (
                    profile.constant_fixed_columns,
                    profile.binary_fixed_columns,
                    profile.raw_fixed_columns,
                );
                assert_eq!(actual, expected);
                assert_eq!(actual, materialized_modes(&materialized));
                assert_eq!(
                    actual.0 + actual.1 + actual.2,
                    profile.configured_fixed_columns + profile.materialized_selector_columns
                );
            }
            // Profiling only borrows the still-owned assembly and configured constraint system.
            assert_eq!(assembly.selectors, selectors);
            for (polynomial, original) in assembly.fixed.iter().zip(&values) {
                assert_eq!(polynomial.values, *original);
            }
            assert_eq!(cs.num_fixed_columns(), 5);
            assert_eq!(cs.num_selectors(), 4);
        }
    }

    #[test]
    fn profiles_match_materialized_fixed_columns_and_preserve_assembly_in_both_fields() {
        profile_cases::<Fp>();
        profile_cases::<Fq>();
    }

    #[test]
    fn profiles_with_no_fixed_columns_or_selectors_have_zero_mode_counts() {
        let cs = ConstraintSystem::<Fp>::default();
        let assembly = Assembly {
            k: 3,
            fixed: vec![],
            permutation: permutation::keygen::Assembly::new(8, &cs.permutation),
            selectors: vec![],
            usable_rows: 0..8,
            _marker: std::marker::PhantomData,
        };
        let profiles = keygen_selector_profiles(8, &cs, &assembly);
        for profile in [profiles.compressed, profiles.direct] {
            assert_eq!(profile.materialized_selector_columns, 0);
            assert_eq!(
                (
                    profile.constant_fixed_columns,
                    profile.binary_fixed_columns,
                    profile.raw_fixed_columns
                ),
                (0, 0, 0)
            );
        }
    }
}
