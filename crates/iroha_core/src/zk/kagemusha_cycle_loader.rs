//! Fixed Pasta-cycle field and curve instructions for the Kagemusha leapfrog verifier.
//!
//! This is the reviewed fixed-width loader shared by both parities. It represents
//! the proof curve's scalar field as three canonical 86-bit limbs in the opposite
//! Pasta field and constrains exact coordinate reduction for the Poseidon transcript.

use std::{cell::RefCell, collections::BTreeMap, marker::PhantomData, ops::Deref, rc::Rc};

use halo2_base::{
    AssignedValue,
    QuantumCell::{Constant, Existing},
    gates::{
        GateChip, GateInstructions, RangeInstructions, flex_gate::threads::SinglePhaseCoreManager,
    },
    halo2_proofs::{
        arithmetic::Field as _,
        halo2curves::{
            CurveAffine,
            group::{Curve as _, Group as _},
        },
    },
    utils::{
        BigPrimeField, CurveAffineExt, biguint_to_fe, decompose_biguint, fe_to_biguint, modulus,
    },
};
use halo2_ecc::{
    bigint::ProperCrtUint,
    ecc::{EcPoint as AssignedEcPoint, EccChip},
    fields::{FieldChip, Selectable, fp::FpChip},
};
use snark_verifier::{
    Error,
    loader::halo2::{EccInstructions, IntegerInstructions},
    system::halo2::transcript::halo2::NativeEncoding,
};

/// Limb width chosen so products of three-limb Pasta integers retain
/// ample native-field headroom in either parity.
pub(super) const LIMB_BITS: usize = 86;
/// Both Pasta fields fit in three 86-bit limbs.
pub(super) const LIMBS: usize = 3;

type Outer<C> = <C as CurveAffine>::Base;
type Inner<C> = <C as CurveAffine>::ScalarExt;
type Integer<C> = ProperCrtUint<Outer<C>>;
type Point<C> = AssignedEcPoint<Outer<C>, Integer<C>>;

type ScalarContext<C> = SinglePhaseCoreManager<Inner<C>>;
type AssignedCoordinate<C> = ProperCrtUint<Inner<C>>;

/// Decompose one canonical Pasta integer into its exact 32-byte little-endian
/// representation.
///
/// `ProperCrtUint` already range-constrains each 86-bit limb. This helper also
/// constrains the two unused high bits to zero and recomposes every byte from
/// those limb bits, giving both Pasta halves an identical SHA-256 preimage.
pub(super) fn proper_uint_le_bytes<F: BigPrimeField>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    value: &ProperCrtUint<F>,
) -> [AssignedValue<F>; 32] {
    let gate = range.gate();
    let mut bits = Vec::with_capacity(LIMB_BITS * LIMBS);
    for limb in value.limbs() {
        bits.extend(gate.num_to_bits(ctx, *limb, LIMB_BITS));
    }
    for bit in bits.iter().skip(256) {
        gate.assert_is_const(ctx, bit, &F::ZERO);
    }
    std::array::from_fn(|index| {
        gate.inner_product(
            ctx,
            bits[index * 8..index * 8 + 8].iter().copied().map(Existing),
            gate.pow_of_two()[..8].iter().copied().map(Constant),
        )
    })
}

/// One exact curve source consumed by the deferred point half.
///
/// Coordinates are represented in canonical non-native limbs in the scalar
/// half.  The reciprocal point half assigns the same coordinates natively and
/// equality-binds their exact byte decomposition before evaluating any MSM.
#[derive(Clone, Debug)]
pub(super) struct DeferredPointSource<C>
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField,
{
    /// Host value used only to populate the circuit witness.
    pub(super) point: C,
    /// Canonical base-field x coordinate in the scalar-half circuit.
    pub(super) x: AssignedCoordinate<C>,
    /// Canonical base-field y coordinate in the scalar-half circuit.
    pub(super) y: AssignedCoordinate<C>,
    /// Exact native-scalar residues absorbed by the Poseidon transcript.
    pub(super) transcript_encoding: [AssignedValue<Inner<C>>; 2],
}

/// One source-indexed coefficient in a constrained deferred curve equation.
#[derive(Clone, Debug)]
pub(super) struct DeferredEquationTerm<C>
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField,
{
    /// Index into [`DeferredScalarAudit::sources`].
    pub(super) source_index: usize,
    /// Coefficient derived by constrained native-scalar arithmetic.
    pub(super) coefficient: AssignedValue<Inner<C>>,
}

/// One curve equality emitted by the native-scalar verifier half.
#[derive(Clone, Debug)]
pub(super) struct DeferredEquation<C>
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField,
{
    /// Canonically source-ordered complete linear equation.
    pub(super) terms: Vec<DeferredEquationTerm<C>>,
}

/// Complete assigned output of one native-scalar verifier half.
///
/// Nothing in this object is accepted from the peer wire.  It is a snapshot of
/// cells created while `snark-verifier` executes with
/// [`DeferredScalarEccChip`].  The reciprocal point circuit must hash the same
/// source coordinates and coefficients and constrain every equation to the
/// identity.
#[derive(Clone, Debug)]
pub(super) struct DeferredScalarAudit<C>
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField,
{
    /// Transcript and authenticated fixed-key point namespace.
    pub(super) sources: Vec<DeferredPointSource<C>>,
    /// PLONK residual, IPA accumulation, and terminal equations in call order.
    pub(super) equations: Vec<DeferredEquation<C>>,
}

/// Host witness passed from a native-scalar half to the reciprocal point half.
///
/// This value has no authority by itself.  Both half circuits recompute the
/// same constrained SHA-256 join over its canonical coordinates, source
/// indices, and coefficients; the point half additionally constrains every
/// source on-curve and evaluates every equation.
#[derive(Clone, Debug)]
pub(super) struct DeferredEquationWitness<C>
where
    C: CurveAffineExt,
{
    /// Complete source namespace in scalar-verifier order.
    pub(super) sources: Vec<C>,
    /// Source-indexed scalar coefficients for every deferred equality.
    pub(super) equations: Vec<Vec<(usize, Inner<C>)>>,
}

impl<C> DeferredScalarAudit<C>
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField,
{
    /// Materialize the point-half witness from already constrained cells.
    pub(super) fn witness(&self) -> DeferredEquationWitness<C> {
        DeferredEquationWitness {
            sources: self.sources.iter().map(|source| source.point).collect(),
            equations: self
                .equations
                .iter()
                .map(|equation| {
                    equation
                        .terms
                        .iter()
                        .map(|term| (term.source_index, *term.coefficient.value()))
                        .collect()
                })
                .collect(),
        }
    }
}

/// Assigned reciprocal-point view of one deferred verifier output.
#[derive(Clone, Debug)]
pub(super) struct AssignedDeferredPointAudit<C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
{
    /// On-curve source points in canonical source order.
    pub(super) sources: Vec<Point<C>>,
    /// Canonical non-native scalar coefficients, grouped by equation.
    pub(super) equations: Vec<Vec<(usize, Integer<C>)>>,
}

#[derive(Clone, Debug)]
struct SymbolicTerm<C>
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField,
{
    source_index: usize,
    coefficient: AssignedValue<Inner<C>>,
}

/// Assigned curve value whose group expression is retained symbolically.
#[derive(Clone, Debug)]
pub(super) struct DeferredScalarPoint<C>
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField,
{
    value: C,
    source_index: Option<usize>,
    terms: Vec<SymbolicTerm<C>>,
}

#[derive(Debug)]
struct DeferredScalarState<C>
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField,
{
    sources: Vec<DeferredPointSource<C>>,
    equations: Vec<DeferredEquation<C>>,
}

impl<C> Default for DeferredScalarState<C>
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField,
{
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            equations: Vec::new(),
        }
    }
}

/// Native-scalar/symbolic-point instructions for one fixed verifier half.
///
/// Scalar arithmetic, transcript challenges, and residual coefficients are
/// constrained in `C::Scalar`.  Curve operations are retained as exact linear
/// equations over canonical point sources.  A reciprocal
/// [`PastaCycleEccChip`] circuit consumes those equations and performs the real
/// point arithmetic.  This avoids both the unsound host-receipt shortcut and
/// the multi-million-cell generic non-native verifier.
#[derive(Clone, Debug)]
pub(super) struct DeferredScalarEccChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    scalar: GateChip<Inner<C>>,
    coordinate: &'chip FpChip<'chip, Inner<C>, Outer<C>>,
    scalar_integer: &'chip FpChip<'chip, Inner<C>, Inner<C>>,
    state: Rc<RefCell<DeferredScalarState<C>>>,
}

impl<'chip, C> DeferredScalarEccChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    /// Construct a fresh fixed-verifier scalar half.
    pub(super) fn new(
        coordinate: &'chip FpChip<'chip, Inner<C>, Outer<C>>,
        scalar_integer: &'chip FpChip<'chip, Inner<C>, Inner<C>>,
    ) -> Self {
        Self {
            scalar: coordinate.range.gate().clone(),
            coordinate,
            scalar_integer,
            state: Rc::new(RefCell::new(DeferredScalarState::default())),
        }
    }

    /// Snapshot every assigned point source and deferred equation.
    pub(super) fn audit(&self) -> DeferredScalarAudit<C> {
        let state = self.state.borrow();
        DeferredScalarAudit {
            sources: state.sources.clone(),
            equations: state.equations.clone(),
        }
    }

    /// Constrain the canonical byte preimage joined to the reciprocal point
    /// half. Callers prepend the authenticated manifest/VK/schema identities
    /// and append the exact loaded transcript scalars before hashing.
    pub(super) fn assigned_equation_bytes(
        &self,
        ctx: &mut ScalarContext<C>,
    ) -> Vec<AssignedValue<Inner<C>>> {
        fn push_u32<F: BigPrimeField>(
            ctx: &mut halo2_base::Context<F>,
            output: &mut Vec<AssignedValue<F>>,
            value: u32,
        ) {
            output.extend(
                value
                    .to_le_bytes()
                    .into_iter()
                    .map(|byte| ctx.load_constant(F::from(u64::from(byte)))),
            );
        }

        let audit = self.audit();
        let ctx = ctx.main();
        let mut output = Vec::new();
        push_u32(ctx, &mut output, 1);
        push_u32(
            ctx,
            &mut output,
            u32::try_from(audit.sources.len()).expect("fixed source count fits u32"),
        );
        push_u32(
            ctx,
            &mut output,
            u32::try_from(audit.equations.len()).expect("fixed equation count fits u32"),
        );
        for source in &audit.sources {
            output.extend(proper_uint_le_bytes(ctx, self.coordinate.range, &source.x));
            output.extend(proper_uint_le_bytes(ctx, self.coordinate.range, &source.y));
        }
        for equation in &audit.equations {
            push_u32(
                ctx,
                &mut output,
                u32::try_from(equation.terms.len()).expect("fixed term count fits u32"),
            );
            for term in &equation.terms {
                push_u32(
                    ctx,
                    &mut output,
                    u32::try_from(term.source_index).expect("fixed source index fits u32"),
                );
                let scalar: AssignedCoordinate<C> = self
                    .scalar_integer
                    .load_private(ctx, *term.coefficient.value());
                let scalar: AssignedCoordinate<C> =
                    self.scalar_integer.enforce_less_than(ctx, scalar).into();
                ctx.constrain_equal(scalar.native(), &term.coefficient);
                output.extend(proper_uint_le_bytes(
                    ctx,
                    self.scalar_integer.range,
                    &scalar,
                ));
            }
        }
        output
    }

    fn coordinate_to_native_scalar(
        &self,
        ctx: &mut ScalarContext<C>,
        coordinate: AssignedCoordinate<C>,
    ) -> AssignedValue<Inner<C>> {
        let ctx = ctx.main();
        self.coordinate.enforce_less_than_p(ctx, coordinate.clone());
        let coordinate_value = self.coordinate.get_assigned_value(coordinate.as_ref());
        let coordinate_integer = fe_to_biguint(&coordinate_value);
        let scalar_modulus = modulus::<Inner<C>>();
        let quotient = &coordinate_integer / &scalar_modulus;
        assert!(
            quotient.bits() <= 1,
            "Pasta cross-field quotient is boolean"
        );
        let residue_integer = &coordinate_integer % &scalar_modulus;
        let residue_value = biguint_to_fe::<Inner<C>>(&residue_integer);
        let residue: AssignedCoordinate<C> = self.scalar_integer.load_private(ctx, residue_value);
        self.scalar_integer
            .enforce_less_than_p(ctx, residue.clone());

        let quotient_u64 = quotient.to_u64_digits().first().copied().unwrap_or(0);
        let quotient_cell = ctx.load_witness(Inner::<C>::from(quotient_u64));
        self.scalar.assert_bit(ctx, quotient_cell);

        let one = quotient.clone() - quotient.clone() + 1_u64;
        let radix_integer = &one << LIMB_BITS;
        let limb_mask = &radix_integer - &one;
        let radix = biguint_to_fe::<Inner<C>>(&radix_integer);
        let modulus_limbs = decompose_biguint::<Inner<C>>(&scalar_modulus, LIMBS, LIMB_BITS);
        let mut carry_integer = quotient.clone() - quotient.clone();
        let zero = ctx.load_zero();
        let mut carry_cell = zero;
        for index in 0..LIMBS {
            let shift = LIMB_BITS * index;
            let residue_limb = (&residue_integer >> shift) & &limb_mask;
            let modulus_limb = (&scalar_modulus >> shift) & &limb_mask;
            let coordinate_limb = (&coordinate_integer >> shift) & &limb_mask;
            let sum = residue_limb + &quotient * modulus_limb + carry_integer.clone();
            assert_eq!(&sum & &limb_mask, coordinate_limb);
            carry_integer = &sum >> LIMB_BITS;
            let carry_u64 = carry_integer.to_u64_digits().first().copied().unwrap_or(0);
            assert!(carry_u64 <= 1);
            let next_carry = ctx.load_witness(Inner::<C>::from(carry_u64));
            self.scalar.assert_bit(ctx, next_carry);

            let quotient_modulus =
                self.scalar
                    .mul(ctx, Existing(quotient_cell), Constant(modulus_limbs[index]));
            let with_residue = self.scalar.add(
                ctx,
                Existing(residue.limbs()[index]),
                Existing(quotient_modulus),
            );
            let left = self
                .scalar
                .add(ctx, Existing(with_residue), Existing(carry_cell));
            let carry_radix = self.scalar.mul(ctx, Existing(next_carry), Constant(radix));
            let right = self.scalar.add(
                ctx,
                Existing(coordinate.limbs()[index]),
                Existing(carry_radix),
            );
            ctx.constrain_equal(&left, &right);
            carry_cell = next_carry;
        }
        self.scalar
            .assert_is_const(ctx, &carry_cell, &Inner::<C>::ZERO);
        *residue.native()
    }

    fn assign_source(&self, ctx: &mut ScalarContext<C>, point: C, constant: bool) -> usize {
        assert!(
            !bool::from(point.is_identity()),
            "identity is not a point source"
        );
        let (x, y) = point.into_coordinates();
        let x = if constant {
            self.coordinate.load_constant(ctx.main(), x)
        } else {
            self.coordinate.load_private(ctx.main(), x)
        };
        let y = if constant {
            self.coordinate.load_constant(ctx.main(), y)
        } else {
            self.coordinate.load_private(ctx.main(), y)
        };
        let x: AssignedCoordinate<C> = self.coordinate.enforce_less_than(ctx.main(), x).into();
        let y: AssignedCoordinate<C> = self.coordinate.enforce_less_than(ctx.main(), y).into();
        let transcript_encoding = [
            self.coordinate_to_native_scalar(ctx, x.clone()),
            self.coordinate_to_native_scalar(ctx, y.clone()),
        ];
        let mut state = self.state.borrow_mut();
        let source_index = state.sources.len();
        state.sources.push(DeferredPointSource {
            point,
            x,
            y,
            transcript_encoding,
        });
        source_index
    }

    fn one_term(&self, ctx: &mut ScalarContext<C>, source_index: usize) -> Vec<SymbolicTerm<C>> {
        vec![SymbolicTerm {
            source_index,
            coefficient: ctx.main().load_constant(Inner::<C>::ONE),
        }]
    }

    fn normalize_terms(
        &self,
        ctx: &mut ScalarContext<C>,
        terms: impl IntoIterator<Item = SymbolicTerm<C>>,
    ) -> Vec<SymbolicTerm<C>> {
        let mut normalized = BTreeMap::<usize, AssignedValue<Inner<C>>>::new();
        for term in terms {
            normalized
                .entry(term.source_index)
                .and_modify(|coefficient| {
                    *coefficient = self.scalar.add(
                        ctx.main(),
                        Existing(*coefficient),
                        Existing(term.coefficient),
                    );
                })
                .or_insert(term.coefficient);
        }
        normalized
            .into_iter()
            .map(|(source_index, coefficient)| SymbolicTerm {
                source_index,
                coefficient,
            })
            .collect()
    }

    fn scale_terms(
        &self,
        ctx: &mut ScalarContext<C>,
        terms: &[SymbolicTerm<C>],
        scalar: AssignedValue<Inner<C>>,
    ) -> Vec<SymbolicTerm<C>> {
        terms
            .iter()
            .map(|term| SymbolicTerm {
                source_index: term.source_index,
                coefficient: self.scalar.mul(
                    ctx.main(),
                    Existing(term.coefficient),
                    Existing(scalar),
                ),
            })
            .collect()
    }

    fn record_equation(
        &self,
        ctx: &mut ScalarContext<C>,
        terms: impl IntoIterator<Item = SymbolicTerm<C>>,
    ) {
        let terms = self
            .normalize_terms(ctx, terms)
            .into_iter()
            .map(|term| DeferredEquationTerm {
                source_index: term.source_index,
                coefficient: term.coefficient,
            })
            .collect();
        self.state
            .borrow_mut()
            .equations
            .push(DeferredEquation { terms });
    }

    fn assign_derived_encoding(
        &self,
        ctx: &mut ScalarContext<C>,
        point: &DeferredScalarPoint<C>,
    ) -> Result<[AssignedValue<Inner<C>>; 2], Error> {
        if bool::from(point.value.is_identity()) {
            return Err(Error::Transcript(
                std::io::ErrorKind::InvalidData,
                "identity point cannot enter the Kagemusha Poseidon transcript".to_owned(),
            ));
        }
        if let Some(source_index) = point.source_index {
            return Ok(self.state.borrow().sources[source_index].transcript_encoding);
        }
        let source_index = self.assign_source(ctx, point.value, false);
        let mut relation = self.one_term(ctx, source_index);
        relation.extend(point.terms.iter().cloned().map(|mut term| {
            term.coefficient =
                GateInstructions::neg(&self.scalar, ctx.main(), Existing(term.coefficient));
            term
        }));
        self.record_equation(ctx, relation);
        Ok(self.state.borrow().sources[source_index].transcript_encoding)
    }
}

impl<C> EccInstructions<C> for DeferredScalarEccChip<'_, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    type Context = ScalarContext<C>;
    type ScalarChip = GateChip<Inner<C>>;
    type AssignedCell = AssignedValue<Inner<C>>;
    type AssignedScalar = AssignedValue<Inner<C>>;
    type AssignedEcPoint = DeferredScalarPoint<C>;

    fn scalar_chip(&self) -> &Self::ScalarChip {
        &self.scalar
    }

    fn assign_constant(&self, ctx: &mut Self::Context, point: C) -> Self::AssignedEcPoint {
        if bool::from(point.is_identity()) {
            return DeferredScalarPoint {
                value: point,
                source_index: None,
                terms: Vec::new(),
            };
        }
        let source_index = self.assign_source(ctx, point, true);
        DeferredScalarPoint {
            value: point,
            source_index: Some(source_index),
            terms: self.one_term(ctx, source_index),
        }
    }

    fn assign_point(&self, ctx: &mut Self::Context, point: C) -> Self::AssignedEcPoint {
        if bool::from(point.is_identity()) {
            return DeferredScalarPoint {
                value: point,
                source_index: None,
                terms: Vec::new(),
            };
        }
        let source_index = self.assign_source(ctx, point, false);
        DeferredScalarPoint {
            value: point,
            source_index: Some(source_index),
            terms: self.one_term(ctx, source_index),
        }
    }

    fn sum_with_const(
        &self,
        ctx: &mut Self::Context,
        values: &[impl Deref<Target = Self::AssignedEcPoint>],
        constant: C,
    ) -> Self::AssignedEcPoint {
        let mut value = constant.to_curve();
        let mut terms = Vec::new();
        if !bool::from(constant.is_identity()) {
            let source_index = self.assign_source(ctx, constant, true);
            terms.extend(self.one_term(ctx, source_index));
        }
        for point in values {
            value += point.value.to_curve();
            terms.extend(point.terms.iter().cloned());
        }
        DeferredScalarPoint {
            value: value.to_affine(),
            source_index: None,
            terms: self.normalize_terms(ctx, terms),
        }
    }

    fn fixed_base_msm(
        &mut self,
        ctx: &mut Self::Context,
        pairs: &[(impl Deref<Target = Self::AssignedScalar>, C)],
    ) -> Self::AssignedEcPoint {
        let mut value = C::Curve::identity();
        let mut terms = Vec::new();
        for (scalar, point) in pairs {
            if bool::from(point.is_identity()) {
                continue;
            }
            value += point.to_curve() * *scalar.value();
            let source_index = self.assign_source(ctx, *point, true);
            terms.push(SymbolicTerm {
                source_index,
                coefficient: **scalar,
            });
        }
        DeferredScalarPoint {
            value: value.to_affine(),
            source_index: None,
            terms: self.normalize_terms(ctx, terms),
        }
    }

    fn variable_base_msm(
        &mut self,
        ctx: &mut Self::Context,
        pairs: &[(
            impl Deref<Target = Self::AssignedScalar>,
            impl Deref<Target = Self::AssignedEcPoint>,
        )],
    ) -> Self::AssignedEcPoint {
        let mut value = C::Curve::identity();
        let mut terms = Vec::new();
        for (scalar, point) in pairs {
            value += point.value.to_curve() * *scalar.value();
            terms.extend(self.scale_terms(ctx, &point.terms, **scalar));
        }
        DeferredScalarPoint {
            value: value.to_affine(),
            source_index: None,
            terms: self.normalize_terms(ctx, terms),
        }
    }

    fn assert_equal(
        &self,
        ctx: &mut Self::Context,
        lhs: &Self::AssignedEcPoint,
        rhs: &Self::AssignedEcPoint,
    ) {
        let mut terms = lhs.terms.clone();
        terms.extend(rhs.terms.iter().cloned().map(|mut term| {
            term.coefficient =
                GateInstructions::neg(&self.scalar, ctx.main(), Existing(term.coefficient));
            term
        }));
        self.record_equation(ctx, terms);
    }
}

impl<C> NativeEncoding<C> for DeferredScalarEccChip<'_, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    fn encode(
        &self,
        ctx: &mut Self::Context,
        point: &Self::AssignedEcPoint,
    ) -> Result<Vec<Self::AssignedScalar>, Error> {
        Ok(self.assign_derived_encoding(ctx, point)?.to_vec())
    }
}

/// Non-native scalar instructions used by `snark-verifier`.
#[derive(Clone, Debug)]
pub(super) struct PastaCycleScalarChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    field: &'chip FpChip<'chip, Outer<C>, Inner<C>>,
    _curve: PhantomData<C>,
}

impl<'chip, C> PastaCycleScalarChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    fn new(field: &'chip FpChip<'chip, Outer<C>, Inner<C>>) -> Self {
        Self {
            field,
            _curve: PhantomData,
        }
    }

    fn canonical(&self, ctx: &mut halo2_base::Context<Outer<C>>, value: Integer<C>) -> Integer<C> {
        self.field.enforce_less_than(ctx, value).into()
    }

    fn add(
        &self,
        ctx: &mut halo2_base::Context<Outer<C>>,
        lhs: Integer<C>,
        rhs: Integer<C>,
    ) -> Integer<C> {
        let sum = self.field.add_no_carry(ctx, lhs, rhs);
        self.field.carry_mod(ctx, sum)
    }

    fn mul(
        &self,
        ctx: &mut halo2_base::Context<Outer<C>>,
        lhs: Integer<C>,
        rhs: Integer<C>,
    ) -> Integer<C> {
        self.field.mul(ctx, lhs, rhs)
    }
}

impl<C> IntegerInstructions<Inner<C>> for PastaCycleScalarChip<'_, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    type Context = SinglePhaseCoreManager<Outer<C>>;
    type AssignedCell = AssignedValue<Outer<C>>;
    type AssignedInteger = Integer<C>;

    fn assign_integer(&self, ctx: &mut Self::Context, integer: Inner<C>) -> Self::AssignedInteger {
        let value = self.field.load_private(ctx.main(), integer);
        self.canonical(ctx.main(), value)
    }

    fn assign_constant(&self, ctx: &mut Self::Context, integer: Inner<C>) -> Self::AssignedInteger {
        self.field.load_constant(ctx.main(), integer)
    }

    fn sum_with_coeff_and_const(
        &self,
        ctx: &mut Self::Context,
        values: &[(Inner<C>, impl Deref<Target = Self::AssignedInteger>)],
        constant: Inner<C>,
    ) -> Self::AssignedInteger {
        let ctx = ctx.main();
        let mut sum = self.field.load_constant(ctx, constant);
        for (coefficient, value) in values {
            let coefficient = self.field.load_constant(ctx, *coefficient);
            let term = self.mul(ctx, value.deref().clone(), coefficient);
            sum = self.add(ctx, sum, term);
        }
        sum
    }

    fn sum_products_with_coeff_and_const(
        &self,
        ctx: &mut Self::Context,
        values: &[(
            Inner<C>,
            impl Deref<Target = Self::AssignedInteger>,
            impl Deref<Target = Self::AssignedInteger>,
        )],
        constant: Inner<C>,
    ) -> Self::AssignedInteger {
        let ctx = ctx.main();
        let mut sum = self.field.load_constant(ctx, constant);
        for (coefficient, lhs, rhs) in values {
            let product = self.mul(ctx, lhs.deref().clone(), rhs.deref().clone());
            let coefficient = self.field.load_constant(ctx, *coefficient);
            let term = self.mul(ctx, product, coefficient);
            sum = self.add(ctx, sum, term);
        }
        sum
    }

    fn sub(
        &self,
        ctx: &mut Self::Context,
        lhs: &Self::AssignedInteger,
        rhs: &Self::AssignedInteger,
    ) -> Self::AssignedInteger {
        let difference = self
            .field
            .sub_no_carry(ctx.main(), lhs.clone(), rhs.clone());
        self.field.carry_mod(ctx.main(), difference)
    }

    fn neg(&self, ctx: &mut Self::Context, value: &Self::AssignedInteger) -> Self::AssignedInteger {
        self.field.negate(ctx.main(), value.clone())
    }

    fn invert(
        &self,
        ctx: &mut Self::Context,
        value: &Self::AssignedInteger,
    ) -> Self::AssignedInteger {
        let one = self.field.load_constant(ctx.main(), Inner::<C>::ONE);
        self.field.divide(ctx.main(), one, value.clone())
    }

    fn assert_equal(
        &self,
        ctx: &mut Self::Context,
        lhs: &Self::AssignedInteger,
        rhs: &Self::AssignedInteger,
    ) {
        self.field
            .assert_equal(ctx.main(), lhs.clone(), rhs.clone());
    }

    fn pow_var(
        &self,
        ctx: &mut Self::Context,
        base: &Self::AssignedInteger,
        exponent: &Self::AssignedInteger,
        max_bits: usize,
    ) -> Self::AssignedInteger {
        assert!(max_bits <= LIMB_BITS * LIMBS);
        let exponent = self.canonical(ctx.main(), exponent.clone());
        let gate = self.field.gate();
        let mut bits = Vec::with_capacity(LIMB_BITS * LIMBS);
        for limb in exponent.limbs() {
            bits.extend(gate.num_to_bits(ctx.main(), *limb, LIMB_BITS));
        }
        for bit in bits.iter().skip(max_bits) {
            gate.assert_is_const(ctx.main(), bit, &Outer::<C>::ZERO);
        }

        let mut result = self.field.load_constant(ctx.main(), Inner::<C>::ONE);
        let mut power = base.clone();
        for bit in bits.into_iter().take(max_bits) {
            let multiplied = self.mul(ctx.main(), result.clone(), power.clone());
            result = self.field.select(ctx.main(), multiplied, result, bit);
            power = self.mul(ctx.main(), power.clone(), power);
        }
        result
    }
}

/// Opposite-field curve instructions for one Pasta parity.
#[derive(Clone, Debug)]
pub(super) struct PastaCycleEccChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    base: &'chip FpChip<'chip, Outer<C>, Outer<C>>,
    scalar: PastaCycleScalarChip<'chip, C>,
}

impl<'chip, C> PastaCycleEccChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    pub(super) fn new(
        base: &'chip FpChip<'chip, Outer<C>, Outer<C>>,
        scalar: &'chip FpChip<'chip, Outer<C>, Inner<C>>,
    ) -> Self {
        Self {
            base,
            scalar: PastaCycleScalarChip::new(scalar),
        }
    }

    fn curve(&self) -> EccChip<'_, Outer<C>, FpChip<'chip, Outer<C>, Outer<C>>> {
        EccChip::new(self.base)
    }

    fn canonical_scalar(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        scalar: &Integer<C>,
    ) -> Vec<AssignedValue<Outer<C>>> {
        self.scalar
            .canonical(ctx.main(), scalar.clone())
            .limbs()
            .to_vec()
    }

    pub(super) fn canonical_point(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        point: Point<C>,
    ) -> Point<C> {
        let x = self.base.enforce_less_than(ctx.main(), point.x).into();
        let y = self.base.enforce_less_than(ctx.main(), point.y).into();
        AssignedEcPoint::new(x, y)
    }

    /// Assign the reciprocal point-half witness, constrain every source to the
    /// proof curve, and require every deferred MSM to equal the identity.
    ///
    /// The returned cells are also used to reconstruct the exact SHA join. A
    /// host-created [`DeferredEquationWitness`] cannot authorize a proof: any
    /// changed point or coefficient either violates these equations or changes
    /// the join exposed by the sibling scalar circuit.
    pub(super) fn constrain_deferred_equations(
        &mut self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        witness: &DeferredEquationWitness<C>,
    ) -> Result<AssignedDeferredPointAudit<C>, String> {
        if witness.sources.is_empty()
            || witness.equations.is_empty()
            || witness
                .sources
                .iter()
                .any(|point| bool::from(point.is_identity()))
            || witness.equations.iter().any(Vec::is_empty)
        {
            return Err("Kagemusha deferred point witness is empty or non-canonical".to_owned());
        }

        let sources = witness
            .sources
            .iter()
            .copied()
            .map(|point| {
                let point = self.curve().assign_point::<C>(ctx.main(), point);
                self.canonical_point(ctx, point)
            })
            .collect::<Vec<_>>();
        let mut equations = Vec::with_capacity(witness.equations.len());
        for equation in &witness.equations {
            let mut assigned = Vec::with_capacity(equation.len());
            let mut previous = None;
            for (source_index, coefficient) in equation {
                if *source_index >= sources.len()
                    || previous.is_some_and(|previous| previous >= *source_index)
                {
                    return Err(
                        "Kagemusha deferred point equation source order is invalid".to_owned()
                    );
                }
                previous = Some(*source_index);
                let coefficient: Integer<C> =
                    self.scalar.field.load_private(ctx.main(), *coefficient);
                let coefficient: Integer<C> = self
                    .scalar
                    .field
                    .enforce_less_than(ctx.main(), coefficient)
                    .into();
                assigned.push((*source_index, coefficient));
            }
            let pairs = assigned
                .iter()
                .map(|(source_index, coefficient)| (coefficient, &sources[*source_index]))
                .collect::<Vec<_>>();
            let result = <Self as EccInstructions<C>>::variable_base_msm(self, ctx, &pairs);
            let identity = self
                .curve()
                .assign_constant_point(ctx.main(), C::identity());
            self.curve().assert_equal(ctx.main(), result, identity);
            equations.push(assigned);
        }
        Ok(AssignedDeferredPointAudit { sources, equations })
    }

    /// Constrain the reciprocal point half's canonical SHA-join preimage.
    pub(super) fn assigned_equation_bytes(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        audit: &AssignedDeferredPointAudit<C>,
    ) -> Vec<AssignedValue<Outer<C>>> {
        fn push_u32<F: BigPrimeField>(
            ctx: &mut halo2_base::Context<F>,
            output: &mut Vec<AssignedValue<F>>,
            value: u32,
        ) {
            output.extend(
                value
                    .to_le_bytes()
                    .into_iter()
                    .map(|byte| ctx.load_constant(F::from(u64::from(byte)))),
            );
        }

        let ctx = ctx.main();
        let mut output = Vec::new();
        push_u32(ctx, &mut output, 1);
        push_u32(
            ctx,
            &mut output,
            u32::try_from(audit.sources.len()).expect("fixed source count fits u32"),
        );
        push_u32(
            ctx,
            &mut output,
            u32::try_from(audit.equations.len()).expect("fixed equation count fits u32"),
        );
        for source in &audit.sources {
            output.extend(proper_uint_le_bytes(ctx, self.base.range, &source.x));
            output.extend(proper_uint_le_bytes(ctx, self.base.range, &source.y));
        }
        for equation in &audit.equations {
            push_u32(
                ctx,
                &mut output,
                u32::try_from(equation.len()).expect("fixed term count fits u32"),
            );
            for (source_index, coefficient) in equation {
                push_u32(
                    ctx,
                    &mut output,
                    u32::try_from(*source_index).expect("fixed source index fits u32"),
                );
                output.extend(proper_uint_le_bytes(
                    ctx,
                    self.scalar.field.range,
                    coefficient,
                ));
            }
        }
        output
    }

    /// Convert a canonical base-field coordinate to the exact residue
    /// used by the native Poseidon transcript.  The quotient and every
    /// radix carry are boolean-constrained, so an outer-field wrap
    /// cannot create a second reduction witness.
    pub(super) fn coordinate_to_scalar(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        coordinate: Integer<C>,
    ) -> Integer<C> {
        let ctx = ctx.main();
        self.base.enforce_less_than_p(ctx, coordinate.clone());
        let coordinate_value = self.base.get_assigned_value(coordinate.as_ref());
        let coordinate_integer = fe_to_biguint(&coordinate_value);
        let scalar_modulus = modulus::<Inner<C>>();
        let quotient = &coordinate_integer / &scalar_modulus;
        assert!(
            quotient.bits() <= 1,
            "Pasta cross-field quotient is boolean"
        );
        let residue_integer = &coordinate_integer % &scalar_modulus;
        let residue_value = biguint_to_fe::<Inner<C>>(&residue_integer);
        let residue = self.scalar.field.load_private(ctx, residue_value);
        self.scalar.field.enforce_less_than_p(ctx, residue.clone());

        let quotient_u64 = quotient.to_u64_digits().first().copied().unwrap_or(0);
        let quotient_cell = ctx.load_witness(Outer::<C>::from(quotient_u64));
        self.base.gate().assert_bit(ctx, quotient_cell);

        let one = quotient.clone() - quotient.clone() + 1u64;
        let radix_integer = &one << LIMB_BITS;
        let limb_mask = &radix_integer - &one;
        let radix = biguint_to_fe::<Outer<C>>(&radix_integer);
        let modulus_limbs = decompose_biguint::<Outer<C>>(&scalar_modulus, LIMBS, LIMB_BITS);
        let mut carry_integer = quotient.clone() - quotient.clone();
        let zero = ctx.load_zero();
        let mut carry_cell = zero;
        for index in 0..LIMBS {
            let shift = LIMB_BITS * index;
            let residue_limb = (&residue_integer >> shift) & &limb_mask;
            let modulus_limb = (&scalar_modulus >> shift) & &limb_mask;
            let coordinate_limb = (&coordinate_integer >> shift) & &limb_mask;
            let sum = residue_limb + &quotient * modulus_limb + carry_integer.clone();
            assert_eq!(&sum & &limb_mask, coordinate_limb);
            carry_integer = &sum >> LIMB_BITS;
            let carry_u64 = carry_integer.to_u64_digits().first().copied().unwrap_or(0);
            assert!(carry_u64 <= 1);
            let next_carry = ctx.load_witness(Outer::<C>::from(carry_u64));
            self.base.gate().assert_bit(ctx, next_carry);

            let quotient_modulus =
                self.base
                    .gate()
                    .mul(ctx, Existing(quotient_cell), Constant(modulus_limbs[index]));
            let with_residue = self.base.gate().add(
                ctx,
                Existing(residue.limbs()[index]),
                Existing(quotient_modulus),
            );
            let left = self
                .base
                .gate()
                .add(ctx, Existing(with_residue), Existing(carry_cell));
            let carry_radix = self
                .base
                .gate()
                .mul(ctx, Existing(next_carry), Constant(radix));
            let right = self.base.gate().add(
                ctx,
                Existing(coordinate.limbs()[index]),
                Existing(carry_radix),
            );
            ctx.constrain_equal(&left, &right);
            carry_cell = next_carry;
        }
        self.base
            .gate()
            .assert_is_const(ctx, &carry_cell, &Outer::<C>::ZERO);
        residue
    }
}

impl<'chip, C> EccInstructions<C> for PastaCycleEccChip<'chip, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    type Context = SinglePhaseCoreManager<Outer<C>>;
    type ScalarChip = PastaCycleScalarChip<'chip, C>;
    type AssignedCell = AssignedValue<Outer<C>>;
    type AssignedScalar = Integer<C>;
    type AssignedEcPoint = Point<C>;

    fn scalar_chip(&self) -> &Self::ScalarChip {
        &self.scalar
    }

    fn assign_constant(&self, ctx: &mut Self::Context, point: C) -> Self::AssignedEcPoint {
        self.curve().assign_constant_point(ctx.main(), point)
    }

    fn assign_point(&self, ctx: &mut Self::Context, point: C) -> Self::AssignedEcPoint {
        self.curve().assign_point(ctx.main(), point)
    }

    fn sum_with_const(
        &self,
        ctx: &mut Self::Context,
        values: &[impl Deref<Target = Self::AssignedEcPoint>],
        constant: C,
    ) -> Self::AssignedEcPoint {
        let constant = (!bool::from(constant.is_identity()))
            .then(|| self.curve().assign_constant_point(ctx.main(), constant));
        self.curve().sum::<C>(
            ctx.main(),
            constant
                .into_iter()
                .chain(values.iter().map(|point| point.deref().clone())),
        )
    }

    fn fixed_base_msm(
        &mut self,
        ctx: &mut Self::Context,
        pairs: &[(impl Deref<Target = Self::AssignedScalar>, C)],
    ) -> Self::AssignedEcPoint {
        let (scalars, points): (Vec<_>, Vec<_>) = pairs
            .iter()
            .filter(|(_, point)| !bool::from(point.is_identity()))
            .map(|(scalar, point)| (self.canonical_scalar(ctx, scalar), *point))
            .unzip();
        if points.is_empty() {
            return self
                .curve()
                .assign_constant_point(ctx.main(), C::identity());
        }
        self.curve()
            .fixed_base_msm::<C>(ctx, &points, scalars, LIMB_BITS)
    }

    fn variable_base_msm(
        &mut self,
        ctx: &mut Self::Context,
        pairs: &[(
            impl Deref<Target = Self::AssignedScalar>,
            impl Deref<Target = Self::AssignedEcPoint>,
        )],
    ) -> Self::AssignedEcPoint {
        if pairs.is_empty() {
            return self
                .curve()
                .assign_constant_point(ctx.main(), C::identity());
        }
        let scalars = pairs
            .iter()
            .map(|(scalar, _)| self.canonical_scalar(ctx, scalar))
            .collect::<Vec<_>>();
        let points = pairs
            .iter()
            .map(|(_, point)| point.deref().clone())
            .collect::<Vec<_>>();
        self.curve()
            .variable_base_msm::<C>(ctx, &points, scalars, LIMB_BITS)
    }

    fn assert_equal(
        &self,
        ctx: &mut Self::Context,
        lhs: &Self::AssignedEcPoint,
        rhs: &Self::AssignedEcPoint,
    ) {
        self.curve()
            .assert_equal(ctx.main(), lhs.clone(), rhs.clone());
    }
}

impl<C> NativeEncoding<C> for PastaCycleEccChip<'_, C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    fn encode(
        &self,
        ctx: &mut Self::Context,
        point: &Self::AssignedEcPoint,
    ) -> Result<Vec<Self::AssignedScalar>, Error> {
        let point = self.canonical_point(ctx, point.clone());
        let identity = self.base.is_zero(ctx.main(), &point.y);
        self.base
            .gate()
            .assert_is_const(ctx.main(), &identity, &Outer::<C>::ZERO);
        Ok(vec![
            self.coordinate_to_scalar(ctx, point.x),
            self.coordinate_to_scalar(ctx, point.y),
        ])
    }
}
