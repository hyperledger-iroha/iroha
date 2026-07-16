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

use super::kagemusha_accumulation::{
    KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4, kagemusha_ipa_accumulator_instance_limbs_v4,
};

/// Domain separator for the fixed-shape selector-bound deferred audit.
pub(super) const KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V4: &[u8] = b"iroha:kagemusha:deferred-audit:v4";
/// Version encoded in every selector-bound deferred-audit preimage.
pub(super) const KAGEMUSHA_DEFERRED_AUDIT_VERSION_V4: u32 = 4;

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

/// Constrain the canonical Pasta compressed encoding `x || parity(y)` from
/// already canonical affine coordinates.
pub(super) fn compressed_point_bytes<F: BigPrimeField>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    x: &ProperCrtUint<F>,
    y: &ProperCrtUint<F>,
) -> [AssignedValue<F>; 32] {
    let mut encoded = proper_uint_le_bytes(ctx, range, x);
    let y_bytes = proper_uint_le_bytes(ctx, range, y);
    let y_low_bits = range.gate().num_to_bits(ctx, y_bytes[0], 8);
    let sign = range
        .gate()
        .mul(ctx, Existing(y_low_bits[0]), Constant(F::from(1_u64 << 7)));
    encoded[31] = range.gate().add(ctx, Existing(encoded[31]), Existing(sign));
    encoded
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

    /// Shared native-scalar range chip used by transcript and identity gadgets.
    pub(super) fn range(&self) -> &halo2_base::gates::RangeChip<Inner<C>> {
        self.scalar_integer.range
    }

    /// Constrain the V4 selector-bound deferred-audit preimage.
    ///
    /// `gate_tags` and `selectors` have one entry per equation in audit order.
    /// Gate tags describe the statically compiled stage while every selector is
    /// a Boolean derived from the current Step's public parent count.  Encoding
    /// both prevents the reciprocal half from accepting the same equation
    /// vector under a different enable schedule.
    pub(super) fn assigned_equation_bytes_v4(
        &self,
        ctx: &mut ScalarContext<C>,
        gate_tags: &[u32],
        selectors: &[AssignedValue<Inner<C>>],
    ) -> Result<Vec<AssignedValue<Inner<C>>>, Error> {
        fn push_constant_bytes<F: BigPrimeField>(
            ctx: &mut halo2_base::Context<F>,
            output: &mut Vec<AssignedValue<F>>,
            bytes: &[u8],
        ) {
            output.extend(
                bytes
                    .iter()
                    .map(|byte| ctx.load_constant(F::from(u64::from(*byte)))),
            );
        }

        fn push_u32<F: BigPrimeField>(
            ctx: &mut halo2_base::Context<F>,
            output: &mut Vec<AssignedValue<F>>,
            value: u32,
        ) {
            push_constant_bytes(ctx, output, &value.to_le_bytes());
        }

        let audit = self.audit();
        if gate_tags.len() != audit.equations.len() || selectors.len() != audit.equations.len() {
            return Err(Error::InvalidInstances);
        }
        let ctx = ctx.main();
        let mut output = Vec::new();
        push_constant_bytes(ctx, &mut output, KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V4);
        push_constant_bytes(ctx, &mut output, &[0]);
        push_u32(ctx, &mut output, KAGEMUSHA_DEFERRED_AUDIT_VERSION_V4);
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
        for ((equation, gate_tag), selector) in audit
            .equations
            .iter()
            .zip(gate_tags)
            .zip(selectors.iter().copied())
        {
            self.scalar.assert_bit(ctx, selector);
            push_u32(ctx, &mut output, *gate_tag);
            output.push(selector);
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
        Ok(output)
    }

    /// Constrain the exact canonical bytes of one native scalar cell.
    pub(super) fn assigned_scalar_bytes(
        &self,
        ctx: &mut ScalarContext<C>,
        scalar: AssignedValue<Inner<C>>,
    ) -> [AssignedValue<Inner<C>>; 32] {
        let scalar_integer: AssignedCoordinate<C> = self
            .scalar_integer
            .load_private(ctx.main(), *scalar.value());
        let scalar_integer: AssignedCoordinate<C> = self
            .scalar_integer
            .enforce_less_than(ctx.main(), scalar_integer)
            .into();
        ctx.main().constrain_equal(scalar_integer.native(), &scalar);
        proper_uint_le_bytes(ctx.main(), self.scalar_integer.range, &scalar_integer)
    }

    /// Constrain the exact canonical compressed bytes of one symbolic point.
    ///
    /// A derived point first emits an equation tying a fresh canonical source
    /// to its complete symbolic expression, so serialization cannot detach an
    /// accumulated output from the equations that produced it.
    pub(super) fn assigned_point_bytes(
        &self,
        ctx: &mut ScalarContext<C>,
        point: &DeferredScalarPoint<C>,
    ) -> Result<[AssignedValue<Inner<C>>; 32], Error> {
        if bool::from(point.value.is_identity()) {
            return Err(Error::Transcript(
                std::io::ErrorKind::InvalidData,
                "identity point cannot be a Kagemusha accumulated output".to_owned(),
            ));
        }
        let source_index = if let Some(source_index) = point.source_index {
            source_index
        } else {
            let source_index = self.assign_source(ctx, point.value, false);
            let mut relation = self.one_term(ctx, source_index);
            relation.extend(point.terms.iter().cloned().map(|mut term| {
                term.coefficient = <GateChip<Inner<C>> as GateInstructions<Inner<C>>>::neg(
                    &self.scalar,
                    ctx.main(),
                    Existing(term.coefficient),
                );
                term
            }));
            self.record_equation(ctx, relation);
            source_index
        };
        let state = self.state.borrow();
        let source = &state.sources[source_index];
        Ok(compressed_point_bytes(
            ctx.main(),
            self.coordinate.range,
            &source.x,
            &source.y,
        ))
    }

    /// Constrain the degree-parameterized V4 accumulator representation.
    ///
    /// The round count comes from the authenticated circuit parameters.  It is
    /// never inferred from the supplied challenge vector or public slice.
    pub(super) fn assigned_accumulator_instance_limbs_v4(
        &self,
        ctx: &mut ScalarContext<C>,
        authenticated_round_count: u32,
        round_challenges: &[AssignedValue<Inner<C>>],
        folded_generator: &DeferredScalarPoint<C>,
    ) -> Result<Vec<AssignedValue<Inner<C>>>, Error> {
        let expected_len = kagemusha_ipa_accumulator_instance_limbs_v4(authenticated_round_count)
            .map_err(Error::AssertionFailure)?;
        if usize::try_from(authenticated_round_count).ok() != Some(round_challenges.len()) {
            return Err(Error::InvalidInstances);
        }

        let mut bytes = Vec::with_capacity((round_challenges.len() + 1) * 32);
        for challenge in round_challenges {
            bytes.extend(self.assigned_scalar_bytes(ctx, *challenge));
        }
        bytes.extend(self.assigned_point_bytes(ctx, folded_generator)?);

        let gate = self.scalar.clone();
        let mut limbs = Vec::with_capacity(expected_len);
        limbs.push(ctx.main().load_constant(Inner::<C>::from(u64::from(
            KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4,
        ))));
        limbs.push(
            ctx.main()
                .load_constant(Inner::<C>::from(u64::from(authenticated_round_count))),
        );
        limbs.extend(bytes.chunks_exact(4).map(|chunk| {
            gate.inner_product(
                ctx.main(),
                chunk.iter().copied().map(Existing),
                [1_u64, 1 << 8, 1 << 16, 1 << 24]
                    .into_iter()
                    .map(|value| Constant(Inner::<C>::from(value))),
            )
        }));
        if limbs.len() != expected_len {
            return Err(Error::InvalidInstances);
        }
        Ok(limbs)
    }

    /// Select between two non-identity symbolic points using an assigned
    /// Boolean scalar.
    ///
    /// The selected host value is used only as a coordinate witness. Its fresh
    /// canonical source is tied to
    /// `selector * when_true + (1 - selector) * when_false` by a deferred
    /// equation, so changing either the selector or the coordinate witness is
    /// caught by the reciprocal point half.
    pub(super) fn select_point(
        &self,
        ctx: &mut ScalarContext<C>,
        when_true: &DeferredScalarPoint<C>,
        when_false: &DeferredScalarPoint<C>,
        selector: AssignedValue<Inner<C>>,
    ) -> DeferredScalarPoint<C> {
        assert!(
            !bool::from(when_true.value.is_identity())
                && !bool::from(when_false.value.is_identity()),
            "identity cannot enter Kagemusha accumulated-point selection"
        );
        self.scalar.assert_bit(ctx.main(), selector);
        let not_selector = self.scalar.not(ctx.main(), selector);

        let mut selected_terms = self.scale_terms(ctx, &when_true.terms, selector);
        selected_terms.extend(self.scale_terms(ctx, &when_false.terms, not_selector));
        let selected_terms = self.normalize_terms(ctx, selected_terms);

        // Arithmetic selection computes only the witness value; the deferred
        // equation below is the authority. This avoids a host Boolean branch.
        let difference = when_true.value.to_curve() - when_false.value.to_curve();
        let value = (when_false.value.to_curve() + difference * *selector.value()).to_affine();

        // Both candidates are non-identity and the selector is Boolean, so the
        // selected value is always a valid source without witness-dependent
        // circuit shape.
        let source_index = self.assign_source(ctx, value, false);
        let mut relation = self.one_term(ctx, source_index);
        relation.extend(selected_terms.into_iter().map(|mut term| {
            term.coefficient = <GateChip<Inner<C>> as GateInstructions<Inner<C>>>::neg(
                &self.scalar,
                ctx.main(),
                Existing(term.coefficient),
            );
            term
        }));
        self.record_equation(ctx, relation);
        DeferredScalarPoint {
            value,
            source_index: Some(source_index),
            terms: self.one_term(ctx, source_index),
        }
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
            term.coefficient = <GateChip<Inner<C>> as GateInstructions<Inner<C>>>::neg(
                &self.scalar,
                ctx.main(),
                Existing(term.coefficient),
            );
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
            term.coefficient = <GateChip<Inner<C>> as GateInstructions<Inner<C>>>::neg(
                &self.scalar,
                ctx.main(),
                Existing(term.coefficient),
            );
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

    /// Assign the `halo2_ecc` canonical `(0, 0)` representation of the point
    /// at infinity without passing `C::identity()` through the affine-only
    /// constant-point loader.
    fn assign_identity(&self, ctx: &mut SinglePhaseCoreManager<Outer<C>>) -> Point<C> {
        let zero = self.base.load_constant(ctx.main(), Outer::<C>::ZERO);
        AssignedEcPoint::new(zero.clone(), zero)
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

    /// Assign and canonicalize every source and coefficient, evaluate every
    /// deferred MSM, and selector-gate only its final identity constraint.
    ///
    /// There must be exactly one already-assigned Boolean selector per
    /// equation. No selector value is inspected on the host: equations with a
    /// zero selector still incur the complete source assignment,
    /// canonicalization, and curve arithmetic, while a one selector enforces
    /// the residual point to be the identity. The returned audit always
    /// contains every equation, independent of selector values.
    pub(super) fn constrain_deferred_equations_with_selectors(
        &mut self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        witness: &DeferredEquationWitness<C>,
        selectors: &[AssignedValue<Outer<C>>],
    ) -> Result<AssignedDeferredPointAudit<C>, String> {
        if witness.sources.is_empty()
            || witness.equations.is_empty()
            || selectors.len() != witness.equations.len()
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
        for (equation, selector) in witness.equations.iter().zip(selectors.iter().copied()) {
            self.base.gate().assert_bit(ctx.main(), selector);
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
            // `halo2_ecc` represents the point at infinity as `(0, 0)`. Its
            // constant-point loader accepts affine points only, so construct
            // the selector-gated identity check directly on that canonical
            // representation instead of attempting to unwrap affine
            // coordinates from `C::identity()`.
            for result_coordinate in [&result.x, &result.y] {
                let selected = <GateChip<Outer<C>> as GateInstructions<Outer<C>>>::mul(
                    self.base.gate(),
                    ctx.main(),
                    Existing(selector),
                    Existing(*result_coordinate.native()),
                );
                self.base
                    .gate()
                    .assert_is_const(ctx.main(), &selected, &Outer::<C>::ZERO);
            }
            equations.push(assigned);
        }
        Ok(AssignedDeferredPointAudit { sources, equations })
    }

    /// Constrain the reciprocal V4 selector-bound audit preimage.
    ///
    /// This is byte-for-byte identical to
    /// [`DeferredScalarEccChip::assigned_equation_bytes_v4`].  Selectors are
    /// independently derived from this circuit's public parent-count cell;
    /// they are not copied from the scalar-half witness.
    pub(super) fn assigned_equation_bytes_v4(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        audit: &AssignedDeferredPointAudit<C>,
        gate_tags: &[u32],
        selectors: &[AssignedValue<Outer<C>>],
    ) -> Result<Vec<AssignedValue<Outer<C>>>, String> {
        fn push_constant_bytes<F: BigPrimeField>(
            ctx: &mut halo2_base::Context<F>,
            output: &mut Vec<AssignedValue<F>>,
            bytes: &[u8],
        ) {
            output.extend(
                bytes
                    .iter()
                    .map(|byte| ctx.load_constant(F::from(u64::from(*byte)))),
            );
        }

        fn push_u32<F: BigPrimeField>(
            ctx: &mut halo2_base::Context<F>,
            output: &mut Vec<AssignedValue<F>>,
            value: u32,
        ) {
            push_constant_bytes(ctx, output, &value.to_le_bytes());
        }

        if gate_tags.len() != audit.equations.len() || selectors.len() != audit.equations.len() {
            return Err("Kagemusha V4 deferred-audit selector shape mismatch".to_owned());
        }
        let ctx = ctx.main();
        let mut output = Vec::new();
        push_constant_bytes(ctx, &mut output, KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V4);
        push_constant_bytes(ctx, &mut output, &[0]);
        push_u32(ctx, &mut output, KAGEMUSHA_DEFERRED_AUDIT_VERSION_V4);
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
        for ((equation, gate_tag), selector) in audit
            .equations
            .iter()
            .zip(gate_tags)
            .zip(selectors.iter().copied())
        {
            self.base.gate().assert_bit(ctx, selector);
            push_u32(ctx, &mut output, *gate_tag);
            output.push(selector);
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
        Ok(output)
    }

    /// Constrain the canonical bytes of a reciprocal non-native scalar.
    pub(super) fn assigned_scalar_bytes(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        scalar: &Integer<C>,
    ) -> [AssignedValue<Outer<C>>; 32] {
        proper_uint_le_bytes(ctx.main(), self.scalar.field.range, scalar)
    }

    /// Constrain the canonical compressed bytes of an assigned on-curve point.
    pub(super) fn assigned_point_bytes(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        point: &Point<C>,
    ) -> [AssignedValue<Outer<C>>; 32] {
        let point = self.canonical_point(ctx, point.clone());
        compressed_point_bytes(ctx.main(), self.base.range, &point.x, &point.y)
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
            return self.assign_identity(ctx);
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
            return self.assign_identity(ctx);
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

#[cfg(test)]
mod tests {
    use std::mem;

    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
    use halo2_ecc::fields::fp::FpChip;
    use halo2_proofs::{
        arithmetic::Field as _,
        dev::MockProver,
        halo2curves::{
            group::{Curve as _, Group as _},
            pasta::{EqAffine, Fp, Fq},
        },
    };
    use snark_verifier::{loader::halo2::EccInstructions, util::arithmetic::PrimeCurveAffine as _};

    use super::*;

    const TEST_K: usize = 17;

    fn reciprocal_builder(
        witness: &DeferredEquationWitness<EqAffine>,
        selectors: &[u64],
    ) -> BaseCircuitBuilder<Fq> {
        let mut builder = BaseCircuitBuilder::<Fq>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let range = builder.range_chip();
        let base = FpChip::<Fq, Fq>::new(&range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<Fq, Fp>::new(&range, LIMB_BITS, LIMBS);
        let mut chip = PastaCycleEccChip::<EqAffine>::new(&base, &scalar);
        let mut ctx = mem::take(builder.pool(0));
        let selectors = selectors
            .iter()
            .copied()
            .map(|selector| ctx.main().load_witness(Fq::from(selector)))
            .collect::<Vec<_>>();
        chip.constrain_deferred_equations_with_selectors(&mut ctx, witness, &selectors)
            .expect("fixed reciprocal witness shape");
        *builder.pool(0) = ctx;
        builder.calculate_params(Some(9));
        builder
    }

    fn assigned_preimage_bytes<F: BigPrimeField>(cells: &[AssignedValue<F>]) -> Vec<u8> {
        cells
            .iter()
            .map(|cell| {
                u8::try_from(cell.value().get_lower_64())
                    .expect("deferred-audit preimages contain exact bytes")
            })
            .collect()
    }

    #[test]
    fn selector_bound_v4_preimage_is_identical_in_both_halves_and_has_one_domain() {
        let generator = EqAffine::generator();
        let doubled = (generator.to_curve() + generator.to_curve()).to_affine();
        let gate_tags = [0x0102_0304];

        let mut scalar_builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let scalar_range = scalar_builder.range_chip();
        let coordinate = FpChip::<Fp, Fq>::new(&scalar_range, LIMB_BITS, LIMBS);
        let scalar_integer = FpChip::<Fp, Fp>::new(&scalar_range, LIMB_BITS, LIMBS);
        let scalar_chip = DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer);
        let mut scalar_ctx = mem::take(scalar_builder.pool(0));
        let when_true = scalar_chip.assign_point(&mut scalar_ctx, generator);
        let when_false = scalar_chip.assign_point(&mut scalar_ctx, doubled);
        let scalar_selector = scalar_ctx.main().load_witness(Fp::ONE);
        let _selected =
            scalar_chip.select_point(&mut scalar_ctx, &when_true, &when_false, scalar_selector);
        let witness = scalar_chip.audit().witness();
        assert_eq!(witness.equations.len(), 1);
        let scalar_preimage = scalar_chip
            .assigned_equation_bytes_v4(&mut scalar_ctx, &gate_tags, &[scalar_selector])
            .expect("canonical scalar-half V4 preimage");
        let scalar_preimage = assigned_preimage_bytes(&scalar_preimage);

        let mut point_builder = BaseCircuitBuilder::<Fq>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let point_range = point_builder.range_chip();
        let base = FpChip::<Fq, Fq>::new(&point_range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<Fq, Fp>::new(&point_range, LIMB_BITS, LIMBS);
        let mut point_chip = PastaCycleEccChip::<EqAffine>::new(&base, &scalar);
        let mut point_ctx = mem::take(point_builder.pool(0));
        let point_selector = point_ctx.main().load_witness(Fq::ONE);
        let point_audit = point_chip
            .constrain_deferred_equations_with_selectors(
                &mut point_ctx,
                &witness,
                &[point_selector],
            )
            .expect("canonical reciprocal V4 audit");
        let point_preimage = point_chip
            .assigned_equation_bytes_v4(&mut point_ctx, &point_audit, &gate_tags, &[point_selector])
            .expect("canonical reciprocal V4 preimage");
        let point_preimage = assigned_preimage_bytes(&point_preimage);

        assert_eq!(scalar_preimage, point_preimage);
        let mut expected_prefix = KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V4.to_vec();
        expected_prefix.push(0);
        expected_prefix.extend_from_slice(&KAGEMUSHA_DEFERRED_AUDIT_VERSION_V4.to_le_bytes());
        assert_eq!(
            &scalar_preimage[..expected_prefix.len()],
            expected_prefix.as_slice(),
            "V4 preimage must contain exactly domain, NUL, and version once"
        );
        let mut duplicated_prefix = KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V4.to_vec();
        duplicated_prefix.extend_from_slice(KAGEMUSHA_DEFERRED_AUDIT_DOMAIN_V4);
        assert!(!scalar_preimage.starts_with(&duplicated_prefix));
    }

    #[test]
    fn reciprocal_residual_is_gated_only_by_the_assigned_selector() {
        let generator = EqAffine::generator();
        let valid = DeferredEquationWitness {
            sources: vec![generator],
            equations: vec![vec![(0, Fp::ZERO)]],
        };
        let invalid = DeferredEquationWitness {
            sources: vec![generator],
            equations: vec![vec![(0, Fp::ONE)]],
        };

        for selector in [0, 1] {
            let builder = reciprocal_builder(&valid, &[selector]);
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("valid selector-gated residual prover")
                .assert_satisfied();
        }

        let disabled = reciprocal_builder(&invalid, &[0]);
        MockProver::run(disabled.config_params.k as u32, &disabled, vec![])
            .expect("disabled invalid residual prover")
            .assert_satisfied();

        let enabled = reciprocal_builder(&invalid, &[1]);
        assert!(
            MockProver::run(enabled.config_params.k as u32, &enabled, vec![])
                .expect("enabled invalid residual prover")
                .verify()
                .is_err(),
            "selector one must reject a non-identity deferred residual"
        );
    }

    #[test]
    fn reciprocal_equation_selectors_are_independent() {
        let generator = EqAffine::generator();
        let invalid_then_valid = DeferredEquationWitness {
            sources: vec![generator],
            equations: vec![vec![(0, Fp::ONE)], vec![(0, Fp::ZERO)]],
        };

        let disabled_invalid = reciprocal_builder(&invalid_then_valid, &[0, 1]);
        MockProver::run(
            disabled_invalid.config_params.k as u32,
            &disabled_invalid,
            vec![],
        )
        .expect("independently disabled residual prover")
        .assert_satisfied();

        let enabled_invalid = reciprocal_builder(&invalid_then_valid, &[1, 1]);
        assert!(
            MockProver::run(
                enabled_invalid.config_params.k as u32,
                &enabled_invalid,
                vec![],
            )
            .expect("independently enabled invalid residual prover")
            .verify()
            .is_err()
        );

        let invalid_then_invalid = DeferredEquationWitness {
            sources: vec![generator],
            equations: vec![vec![(0, Fp::ONE)], vec![(0, Fp::ONE)]],
        };
        let adjacent_enabled = reciprocal_builder(&invalid_then_invalid, &[0, 1]);
        assert!(
            MockProver::run(
                adjacent_enabled.config_params.k as u32,
                &adjacent_enabled,
                vec![],
            )
            .expect("adjacent enabled invalid residual prover")
            .verify()
            .is_err(),
            "disabling one equation must not disable its enabled neighbor"
        );
    }

    #[test]
    fn symbolic_point_selection_records_a_selector_bound_source_equation() {
        let generator = EqAffine::generator();
        let doubled = (generator.to_curve() + generator.to_curve()).to_affine();

        for selector_value in [0, 1] {
            let mut builder = BaseCircuitBuilder::<Fp>::new(false)
                .use_k(TEST_K)
                .use_lookup_bits(TEST_K - 1);
            let range = builder.range_chip();
            let coordinate = FpChip::<Fp, Fq>::new(&range, LIMB_BITS, LIMBS);
            let scalar_integer = FpChip::<Fp, Fp>::new(&range, LIMB_BITS, LIMBS);
            let chip = DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer);
            let mut ctx = mem::take(builder.pool(0));
            let when_true = chip.assign_point(&mut ctx, generator);
            let when_false = chip.assign_point(&mut ctx, doubled);
            let selector = ctx.main().load_witness(Fp::from(selector_value));
            let selected = chip.select_point(&mut ctx, &when_true, &when_false, selector);
            assert_eq!(
                selected.value,
                if selector_value == 1 {
                    generator
                } else {
                    doubled
                }
            );

            let witness = chip.audit().witness();
            assert_eq!(witness.equations.len(), 1);
            for equation in &witness.equations {
                let residual = equation.iter().fold(
                    EqAffine::identity().to_curve(),
                    |residual, (source, coefficient)| {
                        residual + witness.sources[*source].to_curve() * *coefficient
                    },
                );
                assert!(bool::from(residual.is_identity()));
            }

            *builder.pool(0) = ctx;
            builder.calculate_params(Some(9));
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("symbolic selector mock prover")
                .assert_satisfied();
        }
    }
}
