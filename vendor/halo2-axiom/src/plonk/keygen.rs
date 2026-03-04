#![allow(clippy::int_plus_one)]

use std::ops::Range;

use ff::{Field, FromUniformBytes};
use group::Curve;

use super::{
    circuit::{
        Advice, Any, Assignment, Circuit, Column, ConstraintSystem, Fixed, FloorPlanner, Instance,
        Selector,
    },
    evaluation::Evaluator,
    permutation, Assigned, Challenge, Error, LagrangeCoeff, Polynomial, ProvingKey, VerifyingKey,
};
use crate::{
    arithmetic::{parallelize, CurveAffine},
    circuit::Value,
    multicore::{IntoParallelIterator, ParallelIterator},
    poly::{
        batch_invert_assigned,
        commitment::{Blind, Params},
        EvaluationDomain,
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
    let (domain, cs, config) = create_domain::<C, ConcreteCircuit>(
        params.k(),
        #[cfg(feature = "circuit-params")]
        circuit.params(),
    );

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

    // Synthesize the circuit to obtain URS
    ConcreteCircuit::FloorPlanner::synthesize(
        &mut assembly,
        circuit,
        config,
        cs.constants.clone(),
    )?;

    let mut fixed = batch_invert_assigned(assembly.fixed);
    let (cs, selector_polys) = if compress_selectors {
        cs.compress_selectors(assembly.selectors.clone())
    } else {
        // After this, the ConstraintSystem should not have any selectors: `verify` does not need them, and `keygen_pk` regenerates `cs` from scratch anyways.
        let selectors = std::mem::take(&mut assembly.selectors);
        cs.directly_convert_selectors_to_fixed(selectors)
    };
    fixed.extend(
        selector_polys
            .into_iter()
            .map(|poly| domain.lagrange_from_vec(poly)),
    );

    let permutation_vk = assembly
        .permutation
        .build_vk(params, &domain, &cs.permutation);

    let fixed_commitments = (&fixed)
        .into_par_iter()
        .map(|poly| params.commit_lagrange(poly, Blind::default()).to_affine())
        .collect();

    Ok(VerifyingKey::from_parts(
        domain,
        fixed_commitments,
        permutation_vk,
        cs,
        assembly.selectors,
        compress_selectors,
    ))
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
    let (domain, cs, config) = create_domain::<C, ConcreteCircuit>(
        params.k(),
        #[cfg(feature = "circuit-params")]
        circuit.params(),
    );

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

    // Synthesize the circuit to obtain URS
    ConcreteCircuit::FloorPlanner::synthesize(
        &mut assembly,
        circuit,
        config,
        cs.constants.clone(),
    )?;

    let mut fixed = batch_invert_assigned(assembly.fixed);
    let (cs, selector_polys) = if compress_selectors {
        cs.compress_selectors(assembly.selectors.clone())
    } else {
        let selectors = std::mem::take(&mut assembly.selectors);
        cs.directly_convert_selectors_to_fixed(selectors)
    };
    fixed.extend(
        selector_polys
            .into_iter()
            .map(|poly| domain.lagrange_from_vec(poly)),
    );

    let permutation_pk = assembly
        .permutation
        .clone()
        .build_pk(params, &domain, &cs.permutation);

    let vk = match vk {
        Some(vk) => vk,
        None => {
            let permutation_vk = assembly
                .permutation
                .build_vk(params, &domain, &cs.permutation);

            let fixed_commitments = (&fixed)
                .into_par_iter()
                .map(|poly| params.commit_lagrange(poly, Blind::default()).to_affine())
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
    };

    let fixed_polys: Vec<_> = fixed
        .iter()
        .map(|poly| vk.domain.lagrange_to_coeff(poly.clone()))
        .collect();

    // Compute l_0(X) directly without an FFT.
    let l0 = vk.domain.lagrange_basis_0_coeff();

    // Compute l_blind(X) which evaluates to 1 for each blinding factor row
    // and 0 otherwise over the domain.
    let mut l_blind = vk.domain.empty_lagrange();
    for evaluation in l_blind[..].iter_mut().rev().take(vk.cs.blinding_factors()) {
        *evaluation = C::Scalar::ONE;
    }

    // Compute l_last(X) which evaluates to 1 on the first inactive row (just
    // before the blinding factors) and 0 otherwise over the domain
    let mut l_last = vk.domain.empty_lagrange();
    l_last[params.n() as usize - vk.cs.blinding_factors() - 1] = C::Scalar::ONE;

    // Compute l_active_row(X)
    let one = C::Scalar::ONE;
    let mut l_active_row = vk.domain.empty_lagrange();
    parallelize(&mut l_active_row, |values, start| {
        for (i, value) in values.iter_mut().enumerate() {
            let idx = i + start;
            *value = one - (l_last[idx] + l_blind[idx]);
        }
    });

    let l_last = vk.domain.lagrange_to_coeff(l_last);
    let l_active_row = vk.domain.lagrange_to_coeff(l_active_row);

    // Compute the optimized evaluation data structure
    let ev = Evaluator::new(&vk.cs);

    Ok(ProvingKey {
        vk,
        l0,
        l_last,
        l_active_row,
        fixed_values: fixed,
        fixed_polys,
        permutation: permutation_pk,
        ev,
    })
}
