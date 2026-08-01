#![allow(clippy::int_plus_one)]

use std::{fmt, ops::Range};

use ff::{Field, FromUniformBytes, WithSmallOrderMulGroup};
use group::Curve;

use super::{
    Assigned, Challenge, Error, LagrangeCoeff, Polynomial, ProvingKey, VerifyingKey,
    circuit::{
        Advice, Any, Assignment, Circuit, Column, ConstraintSystem, Fixed, FloorPlanner, Instance,
        Selector,
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
    let (cs, assembly, generated_domain) =
        synthesize_keygen_assembly::<C, _, _>(params, None, &circuit)
            .map_err(KeygenWithExtractorError::Keygen)?;
    let extracted = extractor(&circuit).map_err(KeygenWithExtractorError::Extractor)?;
    drop(circuit);
    // The synthesized assembly is the only live owner needed below. On
    // Darwin, promptly purge pages freed with the much larger virtual circuit
    // graph before allocating permutation and MSM scratch.
    release_allocator_slack();

    let domain = generated_domain.expect("verifier-key generation constructs a domain");
    let vk = keygen_vk_from_assembly(params, domain, cs, assembly, false);
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

    let (l0, l_last, l_active_row) =
        create_proving_key_masks(&vk.domain, vk.cs.blinding_factors());

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
