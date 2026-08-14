//! Fixed Pasta-cycle field and curve instructions for the Kagemusha leapfrog verifier.
//!
//! This is the reviewed fixed-width loader shared by both parities. Curve-base
//! arithmetic stays native to the containing Pasta circuit, while the proof
//! curve's scalar field uses three canonical 86-bit limbs. Exact CRT bridges at
//! transcript and audit boundaries preserve canonical coordinate encodings.
use super::kagemusha_accumulation::{
    KAGEMUSHA_IPA_ACCUMULATION_WIRE_VERSION_V4, kagemusha_ipa_accumulator_instance_limbs_v4,
};
use super::kagemusha_dense_msm::{KagemushaDenseMsmJobsV5, KagemushaDenseMsmSourceV5};
use super::kagemusha_sha256_v4::KagemushaSha256ByteV4;
use der_parser::num_bigint::BigUint;
use ff::PrimeField as _;
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
use std::{cell::RefCell, collections::BTreeMap, marker::PhantomData, ops::Deref, rc::Rc};
/// Domain absorbed by the compact deferred-audit Poseidon sponge.
pub(super) const KAGEMUSHA_DEFERRED_AUDIT_POSEIDON_DOMAIN_V6: &[u8] =
    b"iroha:kagemusha:deferred-audit-poseidon:v6";
/// Domain of the short SHA-256 wrapper around the deferred-audit Poseidon digest.
pub(super) const KAGEMUSHA_DEFERRED_AUDIT_SHA256_DOMAIN_V6: &[u8] = b"kg-audit-sha-v6!";
/// Version encoded in both compact deferred-audit commitment layers.
pub(super) const KAGEMUSHA_DEFERRED_AUDIT_VERSION_V6: u32 = 6;
/// Encode a Poseidon domain and version injectively as native field elements.
pub(super) fn kagemusha_poseidon_domain_elements<F: ff::PrimeField>(
    domain: &[u8],
    version: u32,
) -> Vec<F> {
    let mut elements = Vec::with_capacity(domain.len().div_ceil(16) + 2);
    elements.push(F::from(
        u64::try_from(domain.len()).expect("fixed Kagemusha Poseidon domain length fits u64"),
    ));
    elements.extend(domain.chunks(16).map(|chunk| {
        let mut packed = [0_u8; 16];
        packed[..chunk.len()].copy_from_slice(chunk);
        F::from_u128(u128::from_le_bytes(packed))
    }));
    elements.push(F::from(u64::from(version)));
    elements
}
/// Limb width chosen so products of three-limb Pasta integers retain
/// ample native-field headroom in either parity.
pub(super) const LIMB_BITS: usize = 86;
/// Both Pasta fields fit in three 86-bit limbs.
pub(super) const LIMBS: usize = 3;
type Outer<C> = <C as CurveAffine>::Base;
type Inner<C> = <C as CurveAffine>::ScalarExt;
type Integer<C> = ProperCrtUint<Outer<C>>;
type Point<C> = AssignedEcPoint<Outer<C>, NativePastaFieldPoint<Outer<C>>>;
type ScalarContext<C> = SinglePhaseCoreManager<Inner<C>>;
type AssignedCoordinate<C> = ProperCrtUint<Inner<C>>;
/// Canonical non-native coordinate whose integer limbs are known to be below
/// the represented Pasta modulus.
///
/// Witness coordinates acquire this invariant through
/// [`FpChip::enforce_less_than`]. Constant coordinates acquire it directly
/// from [`FpChip::load_constant`], which decomposes the canonical host field
/// element into fixed circuit constants and therefore needs no prover-facing
/// comparison.
#[derive(Clone, Debug)]
pub(super) struct CanonicalCoordinate<C>(AssignedCoordinate<C>)
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField;
impl<C> CanonicalCoordinate<C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    fn load_private(
        chip: &FpChip<'_, Inner<C>, Outer<C>>,
        ctx: &mut halo2_base::Context<Inner<C>>,
        coordinate: Outer<C>,
    ) -> Self {
        let coordinate = chip.load_private(ctx, coordinate);
        Self(chip.enforce_less_than(ctx, coordinate).into())
    }
    fn load_constant(
        chip: &FpChip<'_, Inner<C>, Outer<C>>,
        ctx: &mut halo2_base::Context<Inner<C>>,
        coordinate: Outer<C>,
    ) -> Self {
        Self(chip.load_constant(ctx, coordinate))
    }
    fn integer(&self) -> &AssignedCoordinate<C> {
        &self.0
    }
}
/// One cell in a Pasta base field that is native to the containing circuit.
///
/// `halo2_ecc` is parameterized over a `FieldChip`, including when the curve
/// base field and the circuit field are identical.  Wrapping the assigned cell
/// keeps that native case distinct from `ProperCrtUint`: curve arithmetic must
/// not pay for, or rely on, a three-limb emulation of its own field.
#[derive(Clone, Copy, Debug)]
pub(super) struct NativePastaFieldPoint<F: BigPrimeField>(AssignedValue<F>);
impl<F: BigPrimeField> From<&NativePastaFieldPoint<F>> for NativePastaFieldPoint<F> {
    fn from(value: &NativePastaFieldPoint<F>) -> Self {
        *value
    }
}
impl<F: BigPrimeField> NativePastaFieldPoint<F> {
    fn assigned(self) -> AssignedValue<F> {
        self.0
    }
}
/// Native-field adapter used only for reciprocal Pasta curve arithmetic.
///
/// Every operation is an ordinary gate operation modulo `F`.  Canonical byte
/// encodings are deliberately handled by the existing fixed-width CRT bridge
/// at the audit boundary, where integer uniqueness rather than field
/// arithmetic is required.
#[derive(Clone, Debug)]
struct NativePastaFieldChip<'range, F: BigPrimeField> {
    range: &'range halo2_base::gates::RangeChip<F>,
    native_modulus: BigUint,
}
impl<'range, F: BigPrimeField> NativePastaFieldChip<'range, F> {
    fn new(range: &'range halo2_base::gates::RangeChip<F>) -> Self {
        Self {
            range,
            native_modulus: modulus::<F>(),
        }
    }
    fn signed_constant(value: i64) -> F {
        let magnitude = F::from(value.unsigned_abs());
        if value.is_negative() {
            -magnitude
        } else {
            magnitude
        }
    }
}
impl<F: BigPrimeField> FieldChip<F> for NativePastaFieldChip<'_, F> {
    const PRIME_FIELD_NUM_BITS: u32 = F::NUM_BITS;
    type UnsafeFieldPoint = NativePastaFieldPoint<F>;
    type FieldPoint = NativePastaFieldPoint<F>;
    type ReducedFieldPoint = NativePastaFieldPoint<F>;
    type FieldType = F;
    type RangeChip = halo2_base::gates::RangeChip<F>;
    fn native_modulus(&self) -> &BigUint {
        &self.native_modulus
    }
    fn range(&self) -> &Self::RangeChip {
        self.range
    }
    fn limb_bits(&self) -> usize {
        F::NUM_BITS as usize
    }
    fn get_assigned_value(&self, value: &Self::UnsafeFieldPoint) -> Self::FieldType {
        *value.0.value()
    }
    fn load_private(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: Self::FieldType,
    ) -> Self::FieldPoint {
        NativePastaFieldPoint(ctx.load_witness(value))
    }
    fn load_constant(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: Self::FieldType,
    ) -> Self::FieldPoint {
        NativePastaFieldPoint(ctx.load_constant(value))
    }
    fn add_no_carry(
        &self,
        ctx: &mut halo2_base::Context<F>,
        lhs: impl Into<Self::UnsafeFieldPoint>,
        rhs: impl Into<Self::UnsafeFieldPoint>,
    ) -> Self::UnsafeFieldPoint {
        let lhs = lhs.into();
        let rhs = rhs.into();
        NativePastaFieldPoint(self.gate().add(ctx, Existing(lhs.0), Existing(rhs.0)))
    }
    fn add_constant_no_carry(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: impl Into<Self::UnsafeFieldPoint>,
        constant: Self::FieldType,
    ) -> Self::UnsafeFieldPoint {
        let value = value.into();
        NativePastaFieldPoint(self.gate().add(ctx, Existing(value.0), Constant(constant)))
    }
    fn sub_no_carry(
        &self,
        ctx: &mut halo2_base::Context<F>,
        lhs: impl Into<Self::UnsafeFieldPoint>,
        rhs: impl Into<Self::UnsafeFieldPoint>,
    ) -> Self::UnsafeFieldPoint {
        let lhs = lhs.into();
        let rhs = rhs.into();
        NativePastaFieldPoint(<GateChip<F> as GateInstructions<F>>::sub(
            self.gate(),
            ctx,
            Existing(lhs.0),
            Existing(rhs.0),
        ))
    }
    fn negate(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: Self::FieldPoint,
    ) -> Self::FieldPoint {
        NativePastaFieldPoint(<GateChip<F> as GateInstructions<F>>::neg(
            self.gate(),
            ctx,
            Existing(value.0),
        ))
    }
    fn scalar_mul_no_carry(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: impl Into<Self::UnsafeFieldPoint>,
        constant: i64,
    ) -> Self::UnsafeFieldPoint {
        let value = value.into();
        NativePastaFieldPoint(self.gate().mul(
            ctx,
            Existing(value.0),
            Constant(Self::signed_constant(constant)),
        ))
    }
    fn scalar_mul_and_add_no_carry(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: impl Into<Self::UnsafeFieldPoint>,
        addend: impl Into<Self::UnsafeFieldPoint>,
        constant: i64,
    ) -> Self::UnsafeFieldPoint {
        let value = value.into();
        let addend = addend.into();
        NativePastaFieldPoint(self.gate().mul_add(
            ctx,
            Existing(value.0),
            Constant(Self::signed_constant(constant)),
            Existing(addend.0),
        ))
    }
    fn mul_no_carry(
        &self,
        ctx: &mut halo2_base::Context<F>,
        lhs: impl Into<Self::UnsafeFieldPoint>,
        rhs: impl Into<Self::UnsafeFieldPoint>,
    ) -> Self::UnsafeFieldPoint {
        let lhs = lhs.into();
        let rhs = rhs.into();
        NativePastaFieldPoint(self.gate().mul(ctx, Existing(lhs.0), Existing(rhs.0)))
    }
    fn check_carry_mod_to_zero(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: Self::UnsafeFieldPoint,
    ) {
        self.gate().assert_is_const(ctx, &value.0, &F::ZERO);
    }
    fn carry_mod(
        &self,
        _ctx: &mut halo2_base::Context<F>,
        value: Self::UnsafeFieldPoint,
    ) -> Self::FieldPoint {
        value
    }
    fn range_check(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: impl Into<Self::FieldPoint>,
        max_bits: usize,
    ) {
        assert!(
            max_bits <= F::NUM_BITS as usize,
            "native Pasta range check exceeds the field width"
        );
        if max_bits < F::NUM_BITS as usize {
            self.range.range_check(ctx, value.into().0, max_bits);
        }
    }
    fn enforce_less_than(
        &self,
        _ctx: &mut halo2_base::Context<F>,
        value: Self::FieldPoint,
    ) -> Self::ReducedFieldPoint {
        // An assigned native-field cell is already a unique element modulo F.
        value
    }
    fn is_soft_zero(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: impl Into<Self::FieldPoint>,
    ) -> AssignedValue<F> {
        self.gate().is_zero(ctx, value.into().0)
    }
    fn is_soft_nonzero(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: impl Into<Self::FieldPoint>,
    ) -> AssignedValue<F> {
        let is_zero = self.gate().is_zero(ctx, value.into().0);
        self.gate().not(ctx, is_zero)
    }
    fn is_zero(
        &self,
        ctx: &mut halo2_base::Context<F>,
        value: impl Into<Self::FieldPoint>,
    ) -> AssignedValue<F> {
        self.gate().is_zero(ctx, value.into().0)
    }
    fn is_equal_unenforced(
        &self,
        ctx: &mut halo2_base::Context<F>,
        lhs: Self::ReducedFieldPoint,
        rhs: Self::ReducedFieldPoint,
    ) -> AssignedValue<F> {
        self.gate().is_equal(ctx, Existing(lhs.0), Existing(rhs.0))
    }
    fn assert_equal(
        &self,
        ctx: &mut halo2_base::Context<F>,
        lhs: impl Into<Self::FieldPoint>,
        rhs: impl Into<Self::FieldPoint>,
    ) {
        ctx.constrain_equal(&lhs.into().0, &rhs.into().0);
    }
}
impl<F: BigPrimeField> Selectable<F, NativePastaFieldPoint<F>> for NativePastaFieldChip<'_, F> {
    fn select(
        &self,
        ctx: &mut halo2_base::Context<F>,
        when_true: NativePastaFieldPoint<F>,
        when_false: NativePastaFieldPoint<F>,
        selector: AssignedValue<F>,
    ) -> NativePastaFieldPoint<F> {
        NativePastaFieldPoint(<GateChip<F> as GateInstructions<F>>::select(
            self.gate(),
            ctx,
            when_true.0,
            when_false.0,
            selector,
        ))
    }
    fn select_by_indicator(
        &self,
        ctx: &mut halo2_base::Context<F>,
        values: &impl AsRef<[NativePastaFieldPoint<F>]>,
        coefficients: &[AssignedValue<F>],
    ) -> NativePastaFieldPoint<F> {
        let values = values.as_ref();
        assert_eq!(
            values.len(),
            coefficients.len(),
            "native Pasta indicator shape mismatch"
        );
        NativePastaFieldPoint(self.gate().inner_product(
            ctx,
            values.iter().map(|value| Existing(value.0)),
            coefficients.iter().copied().map(Existing),
        ))
    }
}
/// Decompose one canonical Pasta integer into its exact 32-byte little-endian
/// representation.
///
/// The 32 byte witnesses are range-checked once, split exactly where byte
/// boundaries cross the three 86-bit limbs, and linearly recomposed back to
/// those limbs. The final limb has no source for bits 256 and 257, constraining
/// both to zero without a 258-bit Boolean decomposition.
pub(super) fn proper_uint_le_bytes<F: BigPrimeField>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    value: &ProperCrtUint<F>,
) -> [KagemushaSha256ByteV4<F>; 32] {
    let gate = range.gate();
    let host_encoding = value.value().to_bytes_le();
    let host_bytes: [u8; 32] =
        std::array::from_fn(|index| host_encoding.get(index).copied().unwrap_or(0));
    let assigned_bytes: [AssignedValue<F>; 32] = ctx
        .assign_witnesses(host_bytes.into_iter().map(|byte| F::from(u64::from(byte))))
        .try_into()
        .expect("canonical Pasta encoding has 32 bytes");
    let bytes = std::array::from_fn(|index| {
        KagemushaSha256ByteV4::range_checked(ctx, range, assigned_bytes[index])
    });
    fn split_byte<F: BigPrimeField>(
        ctx: &mut halo2_base::Context<F>,
        range: &halo2_base::gates::RangeChip<F>,
        byte: AssignedValue<F>,
        low_bits: usize,
    ) -> (AssignedValue<F>, AssignedValue<F>) {
        debug_assert!(low_bits > 0 && low_bits < 8);
        let byte_value = u8::try_from(fe_to_biguint(byte.value()))
            .expect("proper integer byte witness is canonical");
        let low_mask = (1_u16 << low_bits) - 1;
        let low = ctx.load_witness(F::from(u64::from(u16::from(byte_value) & low_mask)));
        let high = ctx.load_witness(F::from(u64::from(byte_value >> low_bits)));
        range.range_check(ctx, low, low_bits);
        range.range_check(ctx, high, 8 - low_bits);
        let recomposed = range.gate().mul_add(
            ctx,
            Existing(high),
            Constant(F::from(1_u64 << low_bits)),
            Existing(low),
        );
        ctx.constrain_equal(&recomposed, &byte);
        (low, high)
    }
    let (byte_10_low_6, byte_10_high_2) = split_byte(ctx, range, assigned_bytes[10], 6);
    let (byte_21_low_4, byte_21_high_4) = split_byte(ctx, range, assigned_bytes[21], 4);
    let limb_0 = gate.inner_product(
        ctx,
        assigned_bytes[..10]
            .iter()
            .copied()
            .chain(std::iter::once(byte_10_low_6))
            .map(Existing),
        (0..10)
            .map(|index| Constant(F::from_u128(1_u128 << (8 * index))))
            .chain(std::iter::once(Constant(F::from_u128(1_u128 << 80)))),
    );
    let limb_1 = gate.inner_product(
        ctx,
        std::iter::once(byte_10_high_2)
            .chain(assigned_bytes[11..21].iter().copied())
            .chain(std::iter::once(byte_21_low_4))
            .map(Existing),
        std::iter::once(Constant(F::ONE))
            .chain((0..10).map(|index| Constant(F::from_u128(1_u128 << (2 + 8 * index)))))
            .chain(std::iter::once(Constant(F::from_u128(1_u128 << 82)))),
    );
    let limb_2 = gate.inner_product(
        ctx,
        std::iter::once(byte_21_high_4)
            .chain(assigned_bytes[22..].iter().copied())
            .map(Existing),
        std::iter::once(Constant(F::ONE))
            .chain((0..10).map(|index| Constant(F::from_u128(1_u128 << (4 + 8 * index))))),
    );
    for (recomposed, expected) in [limb_0, limb_1, limb_2].into_iter().zip(value.limbs()) {
        ctx.constrain_equal(&recomposed, expected);
    }
    bytes
}
/// Constrain the canonical Pasta compressed encoding `x || parity(y)` from
/// already canonical affine coordinates.
pub(super) fn compressed_point_bytes<F: BigPrimeField>(
    ctx: &mut halo2_base::Context<F>,
    range: &halo2_base::gates::RangeChip<F>,
    x: &ProperCrtUint<F>,
    y: &ProperCrtUint<F>,
) -> [KagemushaSha256ByteV4<F>; 32] {
    let mut encoded = proper_uint_le_bytes(ctx, range, x);
    let y_bytes = proper_uint_le_bytes(ctx, range, y);
    let gate = range.gate();
    let mut high_bits = encoded[31].decompose_bits_le(ctx, gate);
    let y_low_bits = y_bytes[0].decompose_bits_le(ctx, gate);
    high_bits[7].assert_zero(ctx, gate);
    high_bits[7] = y_low_bits[0];
    encoded[31] = KagemushaSha256ByteV4::from_bits_le(ctx, gate, &high_bits);
    encoded
}
/// Pack at most sixteen proven little-endian bytes into one native field cell.
fn pack_constrained_bytes_u128<F: BigPrimeField>(
    ctx: &mut halo2_base::Context<F>,
    gate: &GateChip<F>,
    bytes: &[KagemushaSha256ByteV4<F>],
) -> AssignedValue<F> {
    assert!(
        bytes.len() <= 16,
        "u128 packing accepts at most sixteen bytes"
    );
    gate.inner_product(
        ctx,
        bytes
            .iter()
            .copied()
            .map(KagemushaSha256ByteV4::quantum_cell),
        (0..bytes.len()).map(|index| Constant(F::from_u128(1_u128 << (8 * index)))),
    )
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
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    /// Host value used only to populate the circuit witness.
    pub(super) point: C,
    /// Canonical base-field x coordinate in the scalar-half circuit.
    pub(super) x: CanonicalCoordinate<C>,
    /// Canonical base-field y coordinate in the scalar-half circuit.
    pub(super) y: CanonicalCoordinate<C>,
    /// Lazily assigned native-scalar residues absorbed by Poseidon.
    ///
    /// The cache is indexed only by this source's fixed namespace position;
    /// witness values are never deduplicated.
    pub(super) transcript_encoding: Option<[AssignedValue<Inner<C>>; 2]>,
    /// Lazily assigned injective compressed-point chunks used by compact
    /// protocol and deferred-audit commitments.
    pub(super) commitment_encoding: Option<[AssignedValue<Inner<C>>; 2]>,
}
/// One source-indexed coefficient in a constrained deferred curve equation.
#[derive(Clone, Debug)]
pub(super) struct DeferredEquationTerm<C>
where
    C: CurveAffineExt,
    Inner<C>: BigPrimeField,
{
    /// Index into the shared deferred point-source namespace.
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
/// Host witness passed from a native-scalar half to the reciprocal point half.
///
/// This value has no authority by itself.  Both half circuits recompute the
/// same compact Poseidon-then-SHA commitment over injective compressed
/// sources, source indices, and coefficients; the point half additionally
/// constrains every source on-curve and evaluates every equation.
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
/// Assigned reciprocal-point view of one deferred verifier output.
#[derive(Clone, Debug)]
pub(super) struct AssignedDeferredPointAudit<C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
{
    /// Canonical host points used only to populate the dense trace witness.
    source_values: Vec<C>,
    /// On-curve source points in canonical source order.
    pub(super) sources: Vec<Point<C>>,
    /// Canonical non-native scalar coefficients, grouped by equation.
    pub(super) equations: Vec<Vec<(usize, Integer<C>)>>,
}
/// Source-indexed reciprocal encodings shared by V6 audit and V2 protocol identity.
#[derive(Clone, Debug)]
pub(super) struct AssignedDeferredSourceEncodingsV6<C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
{
    source_values: Vec<C>,
    pub(super) poseidon_elements: Vec<[Integer<C>; 2]>,
}
impl<C> AssignedDeferredSourceEncodingsV6<C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    /// Select exact cached point chunks by a strictly increasing source map.
    pub(super) fn mapped_poseidon_elements_v2(
        &self,
        points: &[C],
        source_indices: &[usize],
    ) -> Result<Vec<Integer<C>>, String> {
        if points.len() != source_indices.len()
            || self.source_values.len() != self.poseidon_elements.len()
        {
            return Err("Kagemusha V6-to-V2 source map shape mismatch".to_owned());
        }
        let mut previous = None;
        let mut mapped = Vec::with_capacity(points.len() * 2);
        for (point, source_index) in points.iter().zip(source_indices.iter().copied()) {
            if source_index >= self.source_values.len()
                || previous.is_some_and(|previous| previous >= source_index)
                || point.to_bytes().as_ref() != self.source_values[source_index].to_bytes().as_ref()
            {
                return Err("Kagemusha V6-to-V2 source map is invalid".to_owned());
            }
            previous = Some(source_index);
            mapped.extend(self.poseidon_elements[source_index].iter().cloned());
        }
        Ok(mapped)
    }
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
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    sources: Vec<DeferredPointSource<C>>,
    equations: Vec<DeferredEquation<C>>,
    constant_sources: BTreeMap<Vec<u8>, usize>,
}
impl<C> Default for DeferredScalarState<C>
where
    C: CurveAffineExt,
    Outer<C>: BigPrimeField,
    Inner<C>: BigPrimeField,
{
    fn default() -> Self {
        Self {
            sources: Vec::new(),
            equations: Vec::new(),
            constant_sources: BTreeMap::new(),
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
    /// Return the current deferred-equation count without cloning the audit.
    pub(super) fn equation_count(&self) -> usize {
        self.state.borrow().equations.len()
    }
    /// Materialize the reciprocal point-half witness directly from the shared
    /// audit state without first cloning every assigned source and equation.
    pub(super) fn witness(&self) -> DeferredEquationWitness<C> {
        let state = self.state.borrow();
        DeferredEquationWitness {
            sources: state.sources.iter().map(|source| source.point).collect(),
            equations: state
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
    /// Shared native-scalar range chip used by transcript and identity gadgets.
    pub(super) fn range(&self) -> &halo2_base::gates::RangeChip<Inner<C>> {
        self.scalar_integer.range
    }
    /// Constrain the complete V6 selector-bound deferred audit for Poseidon.
    pub(super) fn assigned_equation_poseidon_elements_v6(
        &self,
        ctx: &mut ScalarContext<C>,
        gate_tags: &[u32],
        selectors: &[AssignedValue<Inner<C>>],
    ) -> Result<Vec<AssignedValue<Inner<C>>>, Error> {
        let (source_count, equations) = {
            let state = self.state.borrow();
            if gate_tags.len() != state.equations.len() || selectors.len() != state.equations.len()
            {
                return Err(Error::InvalidInstances);
            }
            (state.sources.len(), state.equations.clone())
        };
        let mut elements = kagemusha_poseidon_domain_elements::<Inner<C>>(
            KAGEMUSHA_DEFERRED_AUDIT_POSEIDON_DOMAIN_V6,
            KAGEMUSHA_DEFERRED_AUDIT_VERSION_V6,
        )
        .into_iter()
        .map(|value| ctx.main().load_constant(value))
        .collect::<Vec<_>>();
        elements.push(ctx.main().load_constant(Inner::<C>::from(
            u64::try_from(source_count).expect("fixed source count fits u64"),
        )));
        elements.push(ctx.main().load_constant(Inner::<C>::from(
            u64::try_from(equations.len()).expect("fixed equation count fits u64"),
        )));
        for source_index in 0..source_count {
            elements.extend(self.source_commitment_encoding(ctx, source_index));
        }
        for (((equation, gate_tag), selector), equation_index) in equations
            .iter()
            .zip(gate_tags)
            .zip(selectors.iter().copied())
            .zip(0_usize..)
        {
            self.scalar.assert_bit(ctx.main(), selector);
            elements.push(
                ctx.main()
                    .load_constant(Inner::<C>::from(u64::from(*gate_tag))),
            );
            elements.push(selector);
            elements.push(
                ctx.main().load_constant(Inner::<C>::from(
                    u64::try_from(equation.terms.len())
                        .expect("fixed deferred equation term count fits u64"),
                )),
            );
            for term in &equation.terms {
                if term.source_index >= source_count {
                    return Err(Error::Transcript(
                        std::io::ErrorKind::InvalidData,
                        format!("Kagemusha V6 equation {equation_index} source index is invalid"),
                    ));
                }
                elements.push(ctx.main().load_constant(Inner::<C>::from(
                    u64::try_from(term.source_index).expect("fixed deferred source index fits u64"),
                )));
                elements.push(term.coefficient);
            }
        }
        Ok(elements)
    }
    /// Constrain the exact canonical bytes of one native scalar cell.
    pub(super) fn assigned_scalar_bytes(
        &self,
        ctx: &mut ScalarContext<C>,
        scalar: AssignedValue<Inner<C>>,
    ) -> [KagemushaSha256ByteV4<Inner<C>>; 32] {
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
    /// Return the stable deferred-source index carried by one assigned point.
    pub(super) fn assigned_point_source_index(
        &self,
        point: &DeferredScalarPoint<C>,
    ) -> Result<usize, Error> {
        if bool::from(point.value.is_identity()) {
            return Err(Error::InvalidInstances);
        }
        point.source_index.ok_or(Error::InvalidInstances)
    }
    /// Constrain the injective two-`u128` encoding of one symbolic point.
    pub(super) fn assigned_point_poseidon_elements_v2(
        &self,
        ctx: &mut ScalarContext<C>,
        point: &DeferredScalarPoint<C>,
    ) -> Result<[AssignedValue<Inner<C>>; 2], Error> {
        if bool::from(point.value.is_identity()) {
            return Err(Error::Transcript(
                std::io::ErrorKind::InvalidData,
                "identity point cannot enter a Kagemusha commitment".to_owned(),
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
        Ok(self.source_commitment_encoding(ctx, source_index))
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
    ) -> Result<[KagemushaSha256ByteV4<Inner<C>>; 32], Error> {
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
            source.x.integer(),
            source.y.integer(),
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
        limbs.extend(bytes.chunks_exact(16).map(|chunk| {
            gate.inner_product(
                ctx.main(),
                chunk.iter().map(|byte| {
                    Existing(
                        byte.assigned()
                            .expect("canonical scalar and point bytes are assigned"),
                    )
                }),
                (0..16).map(|index| Constant(Inner::<C>::from_u128(1_u128 << (8 * index)))),
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
        _ctx: &mut ScalarContext<C>,
        coordinate: &CanonicalCoordinate<C>,
    ) -> AssignedValue<Inner<C>> {
        // `ProperCrtUint` constrains `native` to the exact limb integer modulo
        // the containing field. `CanonicalCoordinate` additionally proves that
        // those limbs name the unique base-field coordinate. Consequently this
        // is exactly the `fe_to_fe` reduction absorbed by Halo2's native
        // transcript; assigning a second same-field CRT integer and reproving
        // the same reduction would add no invariant.
        *coordinate.integer().native()
    }
    fn assign_source(&self, ctx: &mut ScalarContext<C>, point: C, constant: bool) -> usize {
        assert!(
            !bool::from(point.is_identity()),
            "identity is not a point source"
        );
        let constant_key = constant.then(|| point.to_bytes().as_ref().to_vec());
        if let Some(source_index) = constant_key
            .as_ref()
            .and_then(|key| self.state.borrow().constant_sources.get(key).copied())
        {
            return source_index;
        }
        let (x, y) = point.into_coordinates();
        let x = if constant {
            CanonicalCoordinate::load_constant(self.coordinate, ctx.main(), x)
        } else {
            CanonicalCoordinate::load_private(self.coordinate, ctx.main(), x)
        };
        let y = if constant {
            CanonicalCoordinate::load_constant(self.coordinate, ctx.main(), y)
        } else {
            CanonicalCoordinate::load_private(self.coordinate, ctx.main(), y)
        };
        let mut state = self.state.borrow_mut();
        let source_index = state.sources.len();
        state.sources.push(DeferredPointSource {
            point,
            x,
            y,
            transcript_encoding: None,
            commitment_encoding: None,
        });
        if let Some(key) = constant_key {
            let previous = state.constant_sources.insert(key, source_index);
            assert!(
                previous.is_none(),
                "constant source was checked before assignment"
            );
        }
        source_index
    }
    /// Return the exact Poseidon encoding for one fixed source, assigning its
    /// cross-field residues only on first use.
    fn source_transcript_encoding(
        &self,
        ctx: &mut ScalarContext<C>,
        source_index: usize,
    ) -> [AssignedValue<Inner<C>>; 2] {
        if let Some(encoding) = self
            .state
            .borrow()
            .sources
            .get(source_index)
            .and_then(|source| source.transcript_encoding)
        {
            return encoding;
        }
        let (x, y) = {
            let state = self.state.borrow();
            let source = state
                .sources
                .get(source_index)
                .expect("deferred source index was assigned by this chip");
            (source.x.clone(), source.y.clone())
        };
        let encoding = [
            self.coordinate_to_native_scalar(ctx, &x),
            self.coordinate_to_native_scalar(ctx, &y),
        ];
        let previous = self
            .state
            .borrow_mut()
            .sources
            .get_mut(source_index)
            .expect("deferred source index was assigned by this chip")
            .transcript_encoding
            .replace(encoding);
        debug_assert!(
            previous.is_none(),
            "single-threaded deferred encoding cache was populated twice"
        );
        encoding
    }
    /// Return the exact compressed-point chunks for one fixed source,
    /// assigning and caching them only on first use.
    fn source_commitment_encoding(
        &self,
        ctx: &mut ScalarContext<C>,
        source_index: usize,
    ) -> [AssignedValue<Inner<C>>; 2] {
        if let Some(encoding) = self
            .state
            .borrow()
            .sources
            .get(source_index)
            .and_then(|source| source.commitment_encoding)
        {
            return encoding;
        }
        let (x, y) = {
            let state = self.state.borrow();
            let source = state
                .sources
                .get(source_index)
                .expect("deferred source index was assigned by this chip");
            (source.x.clone(), source.y.clone())
        };
        let bytes =
            compressed_point_bytes(ctx.main(), self.coordinate.range, x.integer(), y.integer());
        let encoding = std::array::from_fn(|half| {
            pack_constrained_bytes_u128(
                ctx.main(),
                &self.scalar,
                &bytes[half * 16..(half + 1) * 16],
            )
        });
        let previous = self
            .state
            .borrow_mut()
            .sources
            .get_mut(source_index)
            .expect("deferred source index was assigned by this chip")
            .commitment_encoding
            .replace(encoding);
        debug_assert!(
            previous.is_none(),
            "single-threaded deferred commitment cache was populated twice"
        );
        encoding
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
            return Ok(self.source_transcript_encoding(ctx, source_index));
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
        Ok(self.source_transcript_encoding(ctx, source_index))
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
    native_base: NativePastaFieldChip<'chip, Outer<C>>,
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
            native_base: NativePastaFieldChip::new(base.range),
            base,
            scalar: PastaCycleScalarChip::new(scalar),
        }
    }
    fn curve(&self) -> EccChip<'_, Outer<C>, NativePastaFieldChip<'chip, Outer<C>>> {
        EccChip::new(&self.native_base)
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
        let zero = self.native_base.load_constant(ctx.main(), Outer::<C>::ZERO);
        AssignedEcPoint::new(zero, zero)
    }
    fn canonical_coordinate(
        &self,
        ctx: &mut halo2_base::Context<Outer<C>>,
        coordinate: NativePastaFieldPoint<Outer<C>>,
    ) -> Integer<C> {
        let canonical = self.base.load_private(ctx, *coordinate.0.value());
        let canonical: Integer<C> = self.base.enforce_less_than(ctx, canonical).into();
        ctx.constrain_equal(canonical.native(), &coordinate.0);
        canonical
    }
    /// Assign and canonicalize every deferred source and coefficient.
    ///
    /// Assignment is deliberately separate from enforcement. The complete
    /// assigned audit must first be committed with the compact V6
    /// Poseidon-then-SHA construction; only then may
    /// [`Self::constrain_deferred_equation_batch_v5`] derive its unpredictable
    /// batching challenge from that digest.
    pub(super) fn assign_deferred_equations_with_selectors(
        &self,
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
            .map(|point| self.curve().assign_point::<C>(ctx.main(), point))
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
            equations.push(assigned);
        }
        Ok(AssignedDeferredPointAudit {
            source_values: witness.sources.clone(),
            sources,
            equations,
        })
    }
    /// Derive the V5 batch challenge from seven complete SHA-256 words.
    ///
    /// Words are packed little-endian by word into a 224-bit integer and one
    /// is added. The result is at most `2^224`, below both Pasta moduli. Its
    /// 225-bit non-native representation is equality-bound to the same native
    /// residue, so neither field can silently reduce a different integer.
    fn deferred_batch_challenge_v5(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        digest_words: &[AssignedValue<Outer<C>>; 8],
    ) -> Result<Integer<C>, String> {
        let ctx = ctx.main();
        let radix = Outer::<C>::from(1_u64 << 32);
        let mut weight = Outer::<C>::ONE;
        let mut weights = Vec::with_capacity(7);
        let mut challenge_integer = BigUint::from(1_u8);
        for (index, word) in digest_words[..7].iter().copied().enumerate() {
            self.base.range.range_check(ctx, word, 32);
            weights.push(Constant(weight));
            weight *= radix;
            let digits = fe_to_biguint(word.value()).to_u32_digits();
            if digits.len() > 1 {
                return Err(format!(
                    "Kagemusha V5 batch-challenge digest word {index} exceeds u32"
                ));
            }
            challenge_integer +=
                BigUint::from(digits.first().copied().unwrap_or(0)) << (index * 32);
        }
        let packed = self.base.gate().inner_product(
            ctx,
            digest_words[..7].iter().copied().map(Existing),
            weights,
        );
        let native_challenge =
            self.base
                .gate()
                .add(ctx, Existing(packed), Constant(Outer::<C>::ONE));
        self.base.range.range_check(ctx, native_challenge, 225);
        let maximum = BigUint::from(1_u8) << 224;
        if challenge_integer > maximum {
            return Err("Kagemusha V5 batch challenge exceeds 2^224".to_owned());
        }
        let challenge = self
            .scalar
            .field
            .load_private(ctx, biguint_to_fe::<Inner<C>>(&challenge_integer));
        self.scalar.field.range_check(ctx, challenge.clone(), 225);
        ctx.constrain_equal(challenge.native(), &native_challenge);
        Ok(challenge)
    }
    /// Accumulate all selector-gated equations by stable source index.
    fn deferred_equation_batch_coefficients_v5(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        audit: &AssignedDeferredPointAudit<C>,
        selectors: &[AssignedValue<Outer<C>>],
        digest_words: &[AssignedValue<Outer<C>>; 8],
    ) -> Result<BTreeMap<usize, Integer<C>>, String> {
        if audit.sources.is_empty()
            || audit.source_values.len() != audit.sources.len()
            || audit.equations.is_empty()
            || selectors.len() != audit.equations.len()
            || audit.equations.iter().any(Vec::is_empty)
        {
            return Err("Kagemusha V5 deferred-equation batch shape is invalid".to_owned());
        }
        let challenge = self.deferred_batch_challenge_v5(ctx, digest_words)?;
        let mut power = self.scalar.field.load_constant(ctx.main(), Inner::<C>::ONE);
        let zero = self
            .scalar
            .field
            .load_constant(ctx.main(), Inner::<C>::ZERO);
        let mut by_source = BTreeMap::<usize, Integer<C>>::new();
        for (equation, selector) in audit.equations.iter().zip(selectors.iter().copied()) {
            self.base.gate().assert_bit(ctx.main(), selector);
            for (source_index, coefficient) in equation {
                if *source_index >= audit.sources.len() {
                    return Err("Kagemusha V5 deferred-equation source index is invalid".to_owned());
                }
                let weighted = self
                    .scalar
                    .mul(ctx.main(), coefficient.clone(), power.clone());
                let gated = self
                    .scalar
                    .field
                    .select(ctx.main(), weighted, zero.clone(), selector);
                if let Some(previous) = by_source.remove(source_index) {
                    let combined = self.scalar.add(ctx.main(), previous, gated);
                    by_source.insert(*source_index, combined);
                } else {
                    by_source.insert(*source_index, gated);
                }
            }
            power = self.scalar.mul(ctx.main(), power, challenge.clone());
        }
        Ok(by_source)
    }
    /// Enforce all selector-gated equations with one dense normalized-GLV MSM.
    ///
    /// The power advances for every equation, including disabled equations.
    /// Coefficients are accumulated by stable source index, constrained as
    /// canonical scalars in the Base graph, and copied once into the dedicated
    /// source-major machine. The dense machine requires their aggregate to be
    /// the group identity without allocating the generic variable-base MSM.
    pub(super) fn constrain_deferred_equation_batch_v5(
        &mut self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        audit: &AssignedDeferredPointAudit<C>,
        selectors: &[AssignedValue<Outer<C>>],
        digest_words: &[AssignedValue<Outer<C>>; 8],
        dense_jobs: &mut KagemushaDenseMsmJobsV5<C>,
    ) -> Result<(), String>
    where
        Outer<C>: ff::WithSmallOrderMulGroup<3>,
        Inner<C>: ff::WithSmallOrderMulGroup<3>,
    {
        let by_source =
            self.deferred_equation_batch_coefficients_v5(ctx, audit, selectors, digest_words)?;
        let sources = by_source
            .iter()
            .map(|(source_index, coefficient)| {
                let source = &audit.sources[*source_index];
                KagemushaDenseMsmSourceV5 {
                    point: audit.source_values[*source_index],
                    x: source.x.assigned(),
                    y: source.y.assigned(),
                    coefficient: coefficient.clone(),
                }
            })
            .collect::<Vec<_>>();
        dense_jobs.queue_constrained(ctx.main(), self.scalar.field, &sources)
    }
    /// Retain the former generic MSM only for focused legacy-equivalence tests.
    #[cfg(test)]
    pub(super) fn constrain_deferred_equation_batch_generic_v5(
        &mut self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        audit: &AssignedDeferredPointAudit<C>,
        selectors: &[AssignedValue<Outer<C>>],
        digest_words: &[AssignedValue<Outer<C>>; 8],
    ) -> Result<(), String> {
        let by_source =
            self.deferred_equation_batch_coefficients_v5(ctx, audit, selectors, digest_words)?;
        let pairs = by_source
            .iter()
            .map(|(source_index, coefficient)| (coefficient, &audit.sources[*source_index]))
            .collect::<Vec<_>>();
        let aggregate = <Self as EccInstructions<C>>::variable_base_msm(self, ctx, &pairs);
        self.native_base.gate().assert_is_const(
            ctx.main(),
            &aggregate.x.assigned(),
            &Outer::<C>::ZERO,
        );
        self.native_base.gate().assert_is_const(
            ctx.main(),
            &aggregate.y.assigned(),
            &Outer::<C>::ZERO,
        );
        Ok(())
    }
    /// Convert one assigned outer-field value below `2^bit_len` into the exact
    /// non-native scalar integer used by the reciprocal Poseidon sponge.
    fn assigned_native_as_scalar_integer(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        value: AssignedValue<Outer<C>>,
        bit_len: usize,
    ) -> Result<Integer<C>, String> {
        let integer = fe_to_biguint(value.value());
        if integer.bits() > u64::try_from(bit_len).expect("fixed bit length fits u64") {
            return Err("Kagemusha compact Poseidon element exceeds its bound".to_owned());
        }
        let assigned = self
            .scalar
            .field
            .load_private(ctx.main(), biguint_to_fe::<Inner<C>>(&integer));
        self.scalar
            .field
            .range_check(ctx.main(), assigned.clone(), bit_len);
        ctx.main().constrain_equal(assigned.native(), &value);
        Ok(assigned)
    }
    /// Constrain the injective two-`u128` compressed encoding of one point.
    pub(super) fn assigned_point_poseidon_elements_v2(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        point: &Point<C>,
    ) -> Result<[Integer<C>; 2], String> {
        let bytes = self.assigned_point_bytes(ctx, point);
        let packed: [AssignedValue<Outer<C>>; 2] = std::array::from_fn(|half| {
            pack_constrained_bytes_u128(
                ctx.main(),
                self.base.gate(),
                &bytes[half * 16..(half + 1) * 16],
            )
        });
        Ok([
            self.assigned_native_as_scalar_integer(ctx, packed[0], 128)?,
            self.assigned_native_as_scalar_integer(ctx, packed[1], 128)?,
        ])
    }
    /// Constrain the complete reciprocal V6 audit Poseidon input and retain
    /// its source encodings for the V2 protocol-identity commitment.
    pub(super) fn assigned_equation_poseidon_elements_v6(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        audit: &AssignedDeferredPointAudit<C>,
        gate_tags: &[u32],
        selectors: &[AssignedValue<Outer<C>>],
    ) -> Result<(Vec<Integer<C>>, AssignedDeferredSourceEncodingsV6<C>), String> {
        if audit.sources.is_empty()
            || audit.source_values.len() != audit.sources.len()
            || audit.equations.is_empty()
            || gate_tags.len() != audit.equations.len()
            || selectors.len() != audit.equations.len()
        {
            return Err("Kagemusha V6 deferred-audit selector shape mismatch".to_owned());
        }
        let mut elements = kagemusha_poseidon_domain_elements::<Inner<C>>(
            KAGEMUSHA_DEFERRED_AUDIT_POSEIDON_DOMAIN_V6,
            KAGEMUSHA_DEFERRED_AUDIT_VERSION_V6,
        )
        .into_iter()
        .map(|value| self.scalar.field.load_constant(ctx.main(), value))
        .collect::<Vec<_>>();
        elements.push(self.scalar.field.load_constant(
            ctx.main(),
            Inner::<C>::from(
                u64::try_from(audit.sources.len()).expect("fixed source count fits u64"),
            ),
        ));
        elements.push(self.scalar.field.load_constant(
            ctx.main(),
            Inner::<C>::from(
                u64::try_from(audit.equations.len()).expect("fixed equation count fits u64"),
            ),
        ));
        let mut poseidon_elements = Vec::with_capacity(audit.sources.len());
        for source in &audit.sources {
            let encoding = self.assigned_point_poseidon_elements_v2(ctx, source)?;
            elements.extend(encoding.iter().cloned());
            poseidon_elements.push(encoding);
        }
        for ((equation, gate_tag), selector) in audit
            .equations
            .iter()
            .zip(gate_tags)
            .zip(selectors.iter().copied())
        {
            self.base.gate().assert_bit(ctx.main(), selector);
            elements.push(
                self.scalar
                    .field
                    .load_constant(ctx.main(), Inner::<C>::from(u64::from(*gate_tag))),
            );
            elements.push(self.assigned_native_as_scalar_integer(ctx, selector, 1)?);
            elements.push(self.scalar.field.load_constant(
                ctx.main(),
                Inner::<C>::from(u64::try_from(equation.len()).expect("fixed term count fits u64")),
            ));
            for (source_index, coefficient) in equation {
                if *source_index >= audit.sources.len() {
                    return Err("Kagemusha V6 deferred-audit source index is invalid".to_owned());
                }
                elements.push(self.scalar.field.load_constant(
                    ctx.main(),
                    Inner::<C>::from(
                        u64::try_from(*source_index).expect("fixed source index fits u64"),
                    ),
                ));
                elements.push(coefficient.clone());
            }
        }
        Ok((
            elements,
            AssignedDeferredSourceEncodingsV6 {
                source_values: audit.source_values.clone(),
                poseidon_elements,
            },
        ))
    }
    /// Constrain the canonical bytes of a reciprocal non-native scalar.
    pub(super) fn assigned_scalar_bytes(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        scalar: &Integer<C>,
    ) -> [KagemushaSha256ByteV4<Outer<C>>; 32] {
        proper_uint_le_bytes(ctx.main(), self.scalar.field.range, scalar)
    }
    /// Constrain the canonical compressed bytes of an assigned on-curve point.
    pub(super) fn assigned_point_bytes(
        &self,
        ctx: &mut SinglePhaseCoreManager<Outer<C>>,
        point: &Point<C>,
    ) -> [KagemushaSha256ByteV4<Outer<C>>; 32] {
        let x = self.canonical_coordinate(ctx.main(), point.x);
        let y = self.canonical_coordinate(ctx.main(), point.y);
        compressed_point_bytes(ctx.main(), self.base.range, &x, &y)
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
        let identity = self.native_base.is_zero(ctx.main(), point.y);
        self.base
            .gate()
            .assert_is_const(ctx.main(), &identity, &Outer::<C>::ZERO);
        let x = self.canonical_coordinate(ctx.main(), point.x);
        let y = self.canonical_coordinate(ctx.main(), point.y);
        Ok(vec![
            self.coordinate_to_scalar(ctx, x),
            self.coordinate_to_scalar(ctx, y),
        ])
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::zk::{
        kagemusha_recursion_adapter::constrain_reciprocal_poseidon_v6,
        kagemusha_sha256_v4::KagemushaSha256JobsV4,
    };
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
    use halo2_ecc::fields::fp::FpChip;
    use halo2_proofs::{
        arithmetic::Field as _,
        dev::MockProver,
        halo2curves::{
            group::{Curve as _, Group as _, GroupEncoding as _},
            pasta::{EpAffine, EqAffine, Fp, Fq},
        },
    };
    use snark_verifier::{loader::halo2::EccInstructions, util::arithmetic::PrimeCurveAffine as _};
    use std::mem;
    const TEST_K: usize = 17;
    fn assigned_preimage_bytes<F: BigPrimeField>(bytes: &[KagemushaSha256ByteV4<F>]) -> Vec<u8> {
        bytes
            .iter()
            .copied()
            .map(KagemushaSha256ByteV4::test_value)
            .collect()
    }
    fn reciprocal_builder<C>(
        witness: &DeferredEquationWitness<C>,
        selectors: &[u64],
    ) -> BaseCircuitBuilder<Outer<C>>
    where
        C: CurveAffineExt,
        Outer<C>: BigPrimeField,
        Inner<C>: BigPrimeField,
    {
        let mut builder = BaseCircuitBuilder::<Outer<C>>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let range = builder.range_chip();
        let base = FpChip::<Outer<C>, Outer<C>>::new(&range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<Outer<C>, Inner<C>>::new(&range, LIMB_BITS, LIMBS);
        let mut chip = PastaCycleEccChip::<C>::new(&base, &scalar);
        let mut ctx = mem::take(builder.pool(0));
        let selectors = selectors
            .iter()
            .copied()
            .map(|selector| ctx.main().load_witness(Outer::<C>::from(selector)))
            .collect::<Vec<_>>();
        let audit = chip
            .assign_deferred_equations_with_selectors(&mut ctx, witness, &selectors)
            .expect("fixed reciprocal witness shape");
        let gate_tags = vec![0_u32; witness.equations.len()];
        let (elements, _) = chip
            .assigned_equation_poseidon_elements_v6(&mut ctx, &audit, &gate_tags, &selectors)
            .expect("fixed reciprocal V6 audit");
        let poseidon = constrain_reciprocal_poseidon_v6::<C>(&mut ctx, &base, &scalar, elements);
        let mut bytes = KAGEMUSHA_DEFERRED_AUDIT_SHA256_DOMAIN_V6
            .iter()
            .copied()
            .map(KagemushaSha256ByteV4::constant)
            .collect::<Vec<_>>();
        bytes.push(KagemushaSha256ByteV4::constant(0));
        bytes.extend(
            KAGEMUSHA_DEFERRED_AUDIT_VERSION_V6
                .to_le_bytes()
                .into_iter()
                .map(KagemushaSha256ByteV4::constant),
        );
        bytes.extend(chip.assigned_scalar_bytes(&mut ctx, &poseidon));
        let digest_words = KagemushaSha256JobsV4::default()
            .digest_constrained(ctx.main(), &bytes)
            .expect("fixed reciprocal V6 digest");
        chip.constrain_deferred_equation_batch_generic_v5(
            &mut ctx,
            &audit,
            &selectors,
            &digest_words,
        )
        .expect("fixed reciprocal V5 batch");
        *builder.pool(0) = ctx;
        builder.calculate_params(Some(9));
        builder
    }
    fn reciprocal_source_encoding_fixture<C>(
        sources: Vec<C>,
    ) -> (
        BaseCircuitBuilder<Outer<C>>,
        AssignedDeferredSourceEncodingsV6<C>,
    )
    where
        C: CurveAffineExt,
        Outer<C>: BigPrimeField,
        Inner<C>: BigPrimeField,
    {
        let mut builder = BaseCircuitBuilder::<Outer<C>>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let range = builder.range_chip();
        let base = FpChip::<Outer<C>, Outer<C>>::new(&range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<Outer<C>, Inner<C>>::new(&range, LIMB_BITS, LIMBS);
        let chip = PastaCycleEccChip::<C>::new(&base, &scalar);
        let mut ctx = mem::take(builder.pool(0));
        let witness = DeferredEquationWitness {
            sources,
            equations: vec![vec![(0, Inner::<C>::ZERO)]],
        };
        let selector = ctx.main().load_witness(Outer::<C>::ONE);
        let audit = chip
            .assign_deferred_equations_with_selectors(&mut ctx, &witness, &[selector])
            .expect("fixed reciprocal source assignment");
        let (_, source_encodings) = chip
            .assigned_equation_poseidon_elements_v6(&mut ctx, &audit, &[0], &[selector])
            .expect("fixed reciprocal V6 source encodings");
        *builder.pool(0) = ctx;
        builder.calculate_params(Some(9));
        (builder, source_encodings)
    }
    fn native_curve_arithmetic_builder<C>() -> BaseCircuitBuilder<Outer<C>>
    where
        C: CurveAffineExt,
        Outer<C>: BigPrimeField,
        Inner<C>: BigPrimeField,
    {
        let mut builder = BaseCircuitBuilder::<Outer<C>>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let range = builder.range_chip();
        let native = NativePastaFieldChip::<Outer<C>>::new(&range);
        let curve = EccChip::new(&native);
        let mut ctx = mem::take(builder.pool(0));
        let generator = C::generator();
        let doubled = (generator.to_curve() + generator.to_curve()).to_affine();
        let tripled = (doubled.to_curve() + generator.to_curve()).to_affine();
        let assigned_generator = curve.assign_point::<C>(ctx.main(), generator);
        let assigned_doubled = curve.assign_point::<C>(ctx.main(), doubled);
        let sum = curve.sum::<C>(
            ctx.main(),
            [assigned_generator.clone(), assigned_doubled.clone()],
        );
        let expected = curve.assign_constant_point(ctx.main(), tripled);
        curve.assert_equal(ctx.main(), sum, expected);
        let negated = curve.negate(ctx.main(), assigned_generator.clone());
        let identity = curve.sum::<C>(ctx.main(), [assigned_generator.clone(), negated]);
        native
            .gate()
            .assert_is_const(ctx.main(), &identity.x.assigned(), &Outer::<C>::ZERO);
        native
            .gate()
            .assert_is_const(ctx.main(), &identity.y.assigned(), &Outer::<C>::ZERO);
        let zero = native.load_constant(ctx.main(), Outer::<C>::ZERO);
        let encoded_identity = AssignedEcPoint::new(zero, zero);
        let identity_ignored =
            curve.sum::<C>(ctx.main(), [encoded_identity, assigned_generator.clone()]);
        curve.assert_equal(ctx.main(), identity_ignored, assigned_generator);
        *builder.pool(0) = ctx;
        builder.calculate_params(Some(9));
        builder
    }
    fn reciprocal_msm_builder<C>() -> BaseCircuitBuilder<Outer<C>>
    where
        C: CurveAffineExt,
        Outer<C>: BigPrimeField,
        Inner<C>: BigPrimeField,
    {
        let mut builder = BaseCircuitBuilder::<Outer<C>>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let range = builder.range_chip();
        let base = FpChip::<Outer<C>, Outer<C>>::new(&range, LIMB_BITS, LIMBS);
        let scalar = FpChip::<Outer<C>, Inner<C>>::new(&range, LIMB_BITS, LIMBS);
        let mut chip = PastaCycleEccChip::<C>::new(&base, &scalar);
        let mut ctx = mem::take(builder.pool(0));
        let generator = C::generator();
        let doubled = (generator.to_curve() + generator.to_curve()).to_affine();
        let scalar_three = chip.scalar.assign_integer(&mut ctx, Inner::<C>::from(3));
        let scalar_five = chip.scalar.assign_integer(&mut ctx, Inner::<C>::from(5));
        let assigned_generator = chip.assign_point(&mut ctx, generator);
        let assigned_doubled = chip.assign_point(&mut ctx, doubled);
        let variable = chip.variable_base_msm(
            &mut ctx,
            &[
                (&scalar_three, &assigned_generator),
                (&scalar_five, &assigned_doubled),
            ],
        );
        let fixed = chip.fixed_base_msm(
            &mut ctx,
            &[(&scalar_three, generator), (&scalar_five, doubled)],
        );
        let expected = (generator.to_curve() * Inner::<C>::from(3)
            + doubled.to_curve() * Inner::<C>::from(5))
        .to_affine();
        let expected = chip.assign_constant(&mut ctx, expected);
        chip.assert_equal(&mut ctx, &variable, &expected);
        chip.assert_equal(&mut ctx, &fixed, &expected);
        *builder.pool(0) = ctx;
        builder.calculate_params(Some(9));
        builder
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
    fn native_reciprocal_curve_arithmetic_matches_both_pasta_host_groups() {
        fn check<C>()
        where
            C: CurveAffineExt,
            Outer<C>: BigPrimeField,
            Inner<C>: BigPrimeField,
        {
            let builder = native_curve_arithmetic_builder::<C>();
            assert!(
                builder
                    .config_params
                    .num_lookup_advice_per_phase
                    .iter()
                    .all(|columns| *columns == 0),
                "native base-field curve arithmetic must not allocate CRT lookup columns"
            );
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("native Pasta curve arithmetic mock prover")
                .assert_satisfied();
        }
        check::<EqAffine>();
        check::<EpAffine>();
    }
    #[test]
    fn native_reciprocal_msm_matches_both_pasta_host_groups() {
        fn check<C>()
        where
            C: CurveAffineExt,
            Outer<C>: BigPrimeField,
            Inner<C>: BigPrimeField,
        {
            let builder = reciprocal_msm_builder::<C>();
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("native Pasta reciprocal MSM mock prover")
                .assert_satisfied();
        }
        check::<EqAffine>();
        check::<EpAffine>();
    }
    #[test]
    fn native_reciprocal_point_encoding_matches_both_pasta_host_encodings() {
        fn check<C>()
        where
            C: CurveAffineExt,
            Outer<C>: BigPrimeField,
            Inner<C>: BigPrimeField,
        {
            let mut builder = BaseCircuitBuilder::<Outer<C>>::new(false)
                .use_k(TEST_K)
                .use_lookup_bits(TEST_K - 1);
            let range = builder.range_chip();
            let base = FpChip::<Outer<C>, Outer<C>>::new(&range, LIMB_BITS, LIMBS);
            let scalar = FpChip::<Outer<C>, Inner<C>>::new(&range, LIMB_BITS, LIMBS);
            let chip = PastaCycleEccChip::<C>::new(&base, &scalar);
            let mut ctx = mem::take(builder.pool(0));
            let generator = C::generator();
            let assigned = chip.assign_point(&mut ctx, generator);
            let actual = assigned_preimage_bytes(&chip.assigned_point_bytes(&mut ctx, &assigned));
            let expected = generator.to_bytes();
            assert_eq!(actual.as_slice(), expected.as_ref());
            *builder.pool(0) = ctx;
            builder.calculate_params(Some(9));
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("native Pasta canonical-point mock prover")
                .assert_satisfied();
        }
        check::<EqAffine>();
        check::<EpAffine>();
    }
    #[test]
    fn reciprocal_v6_source_map_reuses_native_chunks_in_both_pasta_parities() {
        fn check<C>()
        where
            C: CurveAffineExt,
            Outer<C>: BigPrimeField,
            Inner<C>: BigPrimeField,
        {
            let generator = C::generator();
            let doubled = (generator.to_curve() + generator.to_curve()).to_affine();
            let tripled = (doubled.to_curve() + generator.to_curve()).to_affine();
            let (builder, source_encodings) =
                reciprocal_source_encoding_fixture(vec![generator, doubled, tripled]);
            let mapped = source_encodings
                .mapped_poseidon_elements_v2(&[generator, tripled], &[0, 2])
                .expect("strict V6-to-V2 source map");
            assert_eq!(mapped.len(), 4);
            for (mapped_point, (source_index, point)) in
                mapped.chunks_exact(2).zip([(0, generator), (2, tripled)])
            {
                let encoded = point.to_bytes();
                for (half, actual) in mapped_point.iter().enumerate() {
                    let expected = u128::from_le_bytes(
                        encoded.as_ref()[half * 16..(half + 1) * 16]
                            .try_into()
                            .expect("compressed point half has sixteen bytes"),
                    );
                    assert_eq!(actual.value(), BigUint::from(expected));
                    assert_eq!(
                        actual.native().cell,
                        source_encodings.poseidon_elements[source_index][half]
                            .native()
                            .cell,
                        "V2 must retain the exact V6-assigned chunk cell"
                    );
                }
            }
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("V6-to-V2 source reuse mock prover")
                .assert_satisfied();
        }
        check::<EqAffine>();
        check::<EpAffine>();
    }
    #[test]
    fn reciprocal_v6_source_map_rejects_ambiguous_or_mismatched_indices() {
        let generator = EqAffine::generator();
        let doubled = (generator.to_curve() + generator.to_curve()).to_affine();
        let tripled = (doubled.to_curve() + generator.to_curve()).to_affine();
        let (_, source_encodings) =
            reciprocal_source_encoding_fixture(vec![generator, doubled, tripled]);
        assert!(
            source_encodings
                .mapped_poseidon_elements_v2(&[tripled, generator], &[2, 0])
                .is_err(),
            "reordered source indices must fail"
        );
        assert!(
            source_encodings
                .mapped_poseidon_elements_v2(&[generator, generator], &[0, 0])
                .is_err(),
            "duplicate source indices must fail"
        );
        assert!(
            source_encodings
                .mapped_poseidon_elements_v2(&[generator, tripled], &[0, 3])
                .is_err(),
            "out-of-range source indices must fail"
        );
        assert!(
            source_encodings
                .mapped_poseidon_elements_v2(&[generator, doubled], &[0, 2])
                .is_err(),
            "host points must match their V6 audit sources"
        );
        assert!(
            source_encodings
                .mapped_poseidon_elements_v2(&[generator, tripled], &[0])
                .is_err(),
            "point/index shape mismatches must fail"
        );
    }
    #[test]
    fn byte_oriented_proper_uint_matches_both_pasta_canonical_encodings() {
        fn check<F, P>(value: P)
        where
            F: BigPrimeField,
            P: BigPrimeField,
        {
            let mut builder = BaseCircuitBuilder::<F>::new(false)
                .use_k(TEST_K)
                .use_lookup_bits(TEST_K - 1);
            let range = builder.range_chip();
            let chip = FpChip::<F, P>::new(&range, LIMB_BITS, LIMBS);
            let mut ctx = mem::take(builder.pool(0));
            let assigned = chip.load_private(ctx.main(), value);
            let assigned: ProperCrtUint<F> = chip.enforce_less_than(ctx.main(), assigned).into();
            let lookup_rows_before = builder
                .lookup_manager()
                .iter()
                .map(|manager| manager.total_rows())
                .sum::<usize>();
            let bytes = proper_uint_le_bytes(ctx.main(), &range, &assigned);
            let lookup_rows_after = builder
                .lookup_manager()
                .iter()
                .map(|manager| manager.total_rows())
                .sum::<usize>();
            let host_encoding = fe_to_biguint(&value).to_bytes_le();
            let expected: [u8; 32] =
                std::array::from_fn(|index| host_encoding.get(index).copied().unwrap_or(0));
            assert_eq!(assigned_preimage_bytes(&bytes), expected);
            assert_eq!(
                lookup_rows_after - lookup_rows_before,
                72,
                "32 Range8 bytes and four split pieces each consume two lookup rows"
            );
            *builder.pool(0) = ctx;
            builder.calculate_params(Some(9));
            MockProver::run(builder.config_params.k as u32, &builder, vec![])
                .expect("byte-oriented canonical Pasta integer mock prover")
                .assert_satisfied();
        }
        check::<Fp, Fq>(-Fq::ONE);
        check::<Fq, Fp>(-Fp::ONE);
    }
    #[test]
    fn reciprocal_residual_enforcement_supports_both_pasta_parities() {
        let eq_generator = EqAffine::generator();
        let eq_valid = DeferredEquationWitness {
            sources: vec![eq_generator],
            equations: vec![vec![(0, Fp::ZERO)]],
        };
        let eq_builder = reciprocal_builder(&eq_valid, &[1]);
        MockProver::run(eq_builder.config_params.k as u32, &eq_builder, vec![])
            .expect("Eq reciprocal residual mock prover")
            .assert_satisfied();
        let eq_invalid = DeferredEquationWitness {
            sources: vec![eq_generator],
            equations: vec![vec![(0, Fp::ONE)]],
        };
        let eq_disabled = reciprocal_builder(&eq_invalid, &[0]);
        MockProver::run(eq_disabled.config_params.k as u32, &eq_disabled, vec![])
            .expect("disabled invalid Eq reciprocal residual mock prover")
            .assert_satisfied();
        let eq_enabled = reciprocal_builder(&eq_invalid, &[1]);
        assert!(
            MockProver::run(eq_enabled.config_params.k as u32, &eq_enabled, vec![])
                .expect("enabled invalid Eq reciprocal residual mock prover")
                .verify()
                .is_err()
        );
        let ep_generator = EpAffine::generator();
        let ep_valid = DeferredEquationWitness {
            sources: vec![ep_generator],
            equations: vec![vec![(0, Fq::ZERO)]],
        };
        let ep_builder = reciprocal_builder(&ep_valid, &[1]);
        MockProver::run(ep_builder.config_params.k as u32, &ep_builder, vec![])
            .expect("Ep reciprocal residual mock prover")
            .assert_satisfied();
        let ep_invalid = DeferredEquationWitness {
            sources: vec![ep_generator],
            equations: vec![vec![(0, Fq::ONE)]],
        };
        let ep_disabled = reciprocal_builder(&ep_invalid, &[0]);
        MockProver::run(ep_disabled.config_params.k as u32, &ep_disabled, vec![])
            .expect("disabled invalid Ep reciprocal residual mock prover")
            .assert_satisfied();
        let ep_enabled = reciprocal_builder(&ep_invalid, &[1]);
        assert!(
            MockProver::run(ep_enabled.config_params.k as u32, &ep_enabled, vec![])
                .expect("enabled invalid Ep reciprocal residual mock prover")
                .verify()
                .is_err()
        );
    }
    #[test]
    fn transcript_challenge_rejects_fixed_sum_equation_cancellation() {
        fn check<C>()
        where
            C: CurveAffineExt,
            Outer<C>: BigPrimeField,
            Inner<C>: BigPrimeField,
        {
            let generator = C::generator();
            let witness = DeferredEquationWitness {
                sources: vec![generator],
                equations: vec![vec![(0, Inner::<C>::ONE)], vec![(0, -Inner::<C>::ONE)]],
            };
            let builder = reciprocal_builder(&witness, &[1, 1]);
            assert!(
                MockProver::run(builder.config_params.k as u32, &builder, vec![])
                    .expect("fixed-sum cancellation reciprocal mock prover")
                    .verify()
                    .is_err(),
                "distinct transcript powers must prevent fixed-sum cancellation"
            );
        }
        check::<EqAffine>();
        check::<EpAffine>();
    }
    #[test]
    fn constant_sources_are_interned_but_equal_witness_sources_remain_distinct() {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let range = builder.range_chip();
        let coordinate = FpChip::<Fp, Fq>::new(&range, LIMB_BITS, LIMBS);
        let scalar_integer = FpChip::<Fp, Fp>::new(&range, LIMB_BITS, LIMBS);
        let chip = DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer);
        let mut ctx = mem::take(builder.pool(0));
        let generator = EqAffine::generator();
        let first_constant = chip.assign_constant(&mut ctx, generator);
        let second_constant = chip.assign_constant(&mut ctx, generator);
        assert_eq!(first_constant.source_index, second_constant.source_index);
        assert_eq!(chip.witness().sources.len(), 1);
        let first_witness = chip.assign_point(&mut ctx, generator);
        let second_witness = chip.assign_point(&mut ctx, generator);
        assert_ne!(first_witness.source_index, second_witness.source_index);
        assert_eq!(chip.witness().sources.len(), 3);
        *builder.pool(0) = ctx;
        builder.calculate_params(Some(9));
        MockProver::run(builder.config_params.k as u32, &builder, vec![])
            .expect("constant-source interning mock prover")
            .assert_satisfied();
    }
    #[test]
    fn constant_source_byte_serialization_keeps_transcript_encoding_lazy() {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let range = builder.range_chip();
        let coordinate = FpChip::<Fp, Fq>::new(&range, LIMB_BITS, LIMBS);
        let scalar_integer = FpChip::<Fp, Fp>::new(&range, LIMB_BITS, LIMBS);
        let chip = DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer);
        let mut ctx = mem::take(builder.pool(0));
        let point = chip.assign_constant(&mut ctx, EqAffine::generator());
        let encoded = chip
            .assigned_point_bytes(&mut ctx, &point)
            .expect("constant point has a canonical compressed encoding");
        assert_eq!(
            assigned_preimage_bytes(&encoded),
            EqAffine::generator().to_bytes().as_ref()
        );
        assert!(
            chip.state.borrow().sources[0].transcript_encoding.is_none(),
            "canonical byte serialization must not construct Poseidon residues"
        );
        *builder.pool(0) = ctx;
        assert_eq!(
            builder
                .lookup_manager()
                .iter()
                .map(|manager| manager.total_rows())
                .sum::<usize>(),
            144,
            "compressed coordinates serialize two canonical 32-byte integers"
        );
        builder.calculate_params(Some(9));
        MockProver::run(builder.config_params.k as u32, &builder, vec![])
            .expect("constant canonical-coordinate mock prover")
            .assert_satisfied();
    }
    #[test]
    fn transcript_encoding_is_lazy_and_cached_by_source_index() {
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K)
            .use_lookup_bits(TEST_K - 1);
        let range = builder.range_chip();
        let coordinate = FpChip::<Fp, Fq>::new(&range, LIMB_BITS, LIMBS);
        let scalar_integer = FpChip::<Fp, Fp>::new(&range, LIMB_BITS, LIMBS);
        let chip = DeferredScalarEccChip::<EqAffine>::new(&coordinate, &scalar_integer);
        let mut ctx = mem::take(builder.pool(0));
        let generator = chip.assign_point(&mut ctx, EqAffine::generator());
        let doubled_value =
            (EqAffine::generator().to_curve() + EqAffine::generator().to_curve()).to_affine();
        let doubled = chip.assign_point(&mut ctx, doubled_value);
        assert!(
            chip.state
                .borrow()
                .sources
                .iter()
                .all(|source| source.transcript_encoding.is_none()),
            "source assignment must not eagerly build cross-field residues"
        );
        let first = chip
            .assign_derived_encoding(&mut ctx, &generator)
            .expect("generator transcript encoding");
        assert!(
            chip.state.borrow().sources[generator.source_index.expect("assigned generator source")]
                .transcript_encoding
                .is_some()
        );
        assert!(
            chip.state.borrow().sources[doubled.source_index.expect("assigned doubled source")]
                .transcript_encoding
                .is_none(),
            "encoding one source must not materialize an equal-shaped neighbor"
        );
        let cells_after_first = ctx.main().advice.len();
        let repeated = chip
            .assign_derived_encoding(&mut ctx, &generator)
            .expect("cached generator transcript encoding");
        assert_eq!(
            first.map(|value| value.cell),
            repeated.map(|value| value.cell),
            "the cache must return the original constrained residue cells"
        );
        assert_eq!(
            ctx.main().advice.len(),
            cells_after_first,
            "re-encoding one source index must assign no additional cells"
        );
        let _ = chip
            .assign_derived_encoding(&mut ctx, &doubled)
            .expect("doubled transcript encoding");
        assert!(
            chip.state
                .borrow()
                .sources
                .iter()
                .all(|source| source.transcript_encoding.is_some())
        );
        *builder.pool(0) = ctx;
        builder.calculate_params(Some(9));
        MockProver::run(builder.config_params.k as u32, &builder, vec![])
            .expect("lazy source-indexed transcript encoding mock prover")
            .assert_satisfied();
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
            let witness = chip.witness();
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
