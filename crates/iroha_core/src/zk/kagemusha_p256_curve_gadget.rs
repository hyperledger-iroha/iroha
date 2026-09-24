//! P-256-specific nonnative curve primitives for a future hardware-selection relation.
//!
//! The pinned `halo2-ecc` generic point checker and scalar multipliers assume
//! `y² = x³ + b`. P-256 instead has `y² = x³ - 3x + b`; using those generic
//! routines for App Attest or Android P-256 signatures is incorrect. These
//! primitives and the App Attest hash/counter slice are composed with staged
//! subject, credential and original-CBOR/DER relations. The composed stage is
//! not yet invoked by recursive monetary authorization.
// TODO: Qualify the full 256-bit signature relation, supported Apple assertion
// profiles, and governed credential fold before hardware proofs authorize money.

use halo2_base::{
    AssignedValue, Context,
    QuantumCell::Constant,
    gates::GateInstructions as _,
    gates::RangeInstructions as _,
    utils::{BigPrimeField, CurveAffineExt as _, modulus, power_of_two},
};
use halo2_ecc::{
    bigint::{FixedOverflowInteger, ProperCrtUint, big_less_than},
    ecc::EcPoint,
    fields::{FieldChip as _, Selectable as _, fp::FpChip},
};
use halo2_proofs::halo2curves::{
    CurveAffine as _,
    ff::Field as _,
    ff::PrimeField,
    secp256r1::{Fp as P256Base, Fq as P256Scalar, Secp256r1Affine},
};

use super::pasta_sha256::{PastaSha256BitV1, PastaSha256ByteV1, PastaSha256JobsV1};

#[path = "app_attest_der_gadget.rs"]
pub(crate) mod app_attest_der_gadget;

/// Constrain one reduced affine point to P-256, excluding the point at infinity.
///
/// SEC1 byte range and equality to an enrolled public key are separate required
/// constraints in the eventual signature relation. This function alone does
/// not bind a point to a credential.
pub(crate) fn assert_p256_affine_point<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    point: &EcPoint<F, ProperCrtUint<F>>,
) {
    let _ = chip.enforce_less_than(ctx, point.x.clone());
    let _ = chip.enforce_less_than(ctx, point.y.clone());
    let y2 = chip.mul_no_carry(ctx, &point.y, &point.y);
    let x2 = chip.mul(ctx, &point.x, &point.x);
    let x3 = chip.mul_no_carry(ctx, x2, &point.x);
    let minus_three_x = chip.scalar_mul_no_carry(ctx, &point.x, -3);
    let x3_minus_three_x = chip.add_no_carry(ctx, x3, minus_three_x);
    let rhs = chip.add_constant_no_carry(ctx, x3_minus_three_x, Secp256r1Affine::b());
    let difference = chip.sub_no_carry(ctx, y2, rhs);
    chip.check_carry_mod_to_zero(ctx, difference);
    let y_is_zero = chip.is_zero(ctx, &point.y);
    chip.gate().assert_is_const(ctx, &y_is_zero, &F::ZERO);
}

/// Double a validated nonidentity P-256 affine point with the `a = -3` slope.
///
/// Input validation is repeated here so a future caller cannot accidentally
/// use an unchecked or identity point with `divide_unsafe`.
#[cfg_attr(not(test), expect(dead_code, reason = "bounded point test helper"))]
pub(crate) fn double_p256_affine_point<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    point: &EcPoint<F, ProperCrtUint<F>>,
) -> EcPoint<F, ProperCrtUint<F>> {
    assert_p256_affine_point(chip, ctx, point);
    let x2 = chip.mul(ctx, &point.x, &point.x);
    let three_x2 = chip.scalar_mul_no_carry(ctx, x2, 3);
    let numerator = chip.add_constant_no_carry(ctx, three_x2, -P256Base::from(3_u64));
    let denominator = chip.scalar_mul_no_carry(ctx, &point.y, 2);
    let slope = chip.divide_unsafe(ctx, numerator, denominator);

    let slope2 = chip.mul_no_carry(ctx, &slope, &slope);
    let twice_x = chip.scalar_mul_no_carry(ctx, &point.x, 2);
    let x_difference_nc = chip.sub_no_carry(ctx, slope2, twice_x);
    let x_out = chip.carry_mod(ctx, x_difference_nc);
    let x_difference = chip.sub_no_carry(ctx, &point.x, &x_out);
    let slope_term = chip.mul_no_carry(ctx, &slope, x_difference);
    let y_difference_nc = chip.sub_no_carry(ctx, slope_term, &point.y);
    let y_out = chip.carry_mod(ctx, y_difference_nc);
    let output = EcPoint::new(x_out, y_out);
    assert_p256_affine_point(chip, ctx, &output);
    output
}

/// Constrain an affine P-256 point, allowing only `(0, 0)` as infinity.
///
/// SEC1 decoders must reject `(0, 0)` as a public key; it is used only as the
/// internal group identity for complete addition and scalar multiplication.
pub(crate) fn assert_p256_affine_or_identity<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    point: &EcPoint<F, ProperCrtUint<F>>,
) -> AssignedValue<F> {
    let _ = chip.enforce_less_than(ctx, point.x.clone());
    let _ = chip.enforce_less_than(ctx, point.y.clone());
    let x_zero = chip.is_zero(ctx, &point.x);
    let y_zero = chip.is_zero(ctx, &point.y);
    let is_identity = chip.gate().and(ctx, x_zero, y_zero);

    let generator = Secp256r1Affine::generator();
    let (gx, gy) = generator.into_coordinates();
    let gx = chip.load_constant(ctx, gx);
    let gy = chip.load_constant(ctx, gy);
    let checked_x = chip.select(ctx, gx, point.x.clone(), is_identity);
    let checked_y = chip.select(ctx, gy, point.y.clone(), is_identity);
    let checked = EcPoint::new(checked_x, checked_y);
    assert_p256_affine_point(chip, ctx, &checked);
    is_identity
}

/// Add two P-256 points, including identity, inverse and equal-point cases.
///
/// This uses the actual P-256 `a = -3` doubling numerator. Denominators are
/// masked to one in inactive cases before division, so no exceptional case
/// reaches an undefined nonnative-field division.
#[cfg_attr(not(test), expect(dead_code, reason = "bounded group test helper"))]
pub(crate) fn add_p256_affine_complete<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    p: &EcPoint<F, ProperCrtUint<F>>,
    q: &EcPoint<F, ProperCrtUint<F>>,
) -> EcPoint<F, ProperCrtUint<F>> {
    let _ = assert_p256_affine_or_identity(chip, ctx, p);
    let _ = assert_p256_affine_or_identity(chip, ctx, q);
    add_p256_affine_complete_validated(chip, ctx, p, q)
}

/// Complete addition for points that the caller has already proven valid.
///
/// The checked entry validates both inputs before calling here. The joint
/// ladder validates its initial points once, its table selection is Boolean,
/// and every result is validated below before the next round. This avoids
/// re-proving two entire curve equations at every 256-bit ladder step.
fn add_p256_affine_complete_validated<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    p: &EcPoint<F, ProperCrtUint<F>>,
    q: &EcPoint<F, ProperCrtUint<F>>,
) -> EcPoint<F, ProperCrtUint<F>> {
    let p_x_zero = chip.is_soft_zero(ctx, p.x.clone());
    let p_y_zero = chip.is_soft_zero(ctx, p.y.clone());
    let p_identity = chip.gate().and(ctx, p_x_zero, p_y_zero);
    let q_x_zero = chip.is_soft_zero(ctx, q.x.clone());
    let q_y_zero = chip.is_soft_zero(ctx, q.y.clone());
    let q_identity = chip.gate().and(ctx, q_x_zero, q_y_zero);
    let not_p_identity = chip.gate().not(ctx, p_identity);
    let not_q_identity = chip.gate().not(ctx, q_identity);
    let both = chip.gate().and(ctx, not_p_identity, not_q_identity);
    let x_equal = chip.is_equal(ctx, p.x.clone(), q.x.clone());
    let y_equal = chip.is_equal(ctx, p.y.clone(), q.y.clone());
    let not_x_equal = chip.gate().not(ctx, x_equal);
    let generic = chip.gate().and(ctx, both, not_x_equal);
    let both_same_x = chip.gate().and(ctx, both, x_equal);
    let double = chip.gate().and(ctx, both_same_x, y_equal);
    let not_y_equal = chip.gate().not(ctx, y_equal);
    let inverse = chip.gate().and(ctx, both_same_x, not_y_equal);
    let active = chip.gate().or(ctx, generic, double);

    // On the curve, equal x with unequal y must mean opposite y. Constrain it
    // explicitly so the inverse selector cannot silently discard a bad point.
    let y_sum_nc = chip.add_no_carry(ctx, &p.y, &q.y);
    let y_sum = chip.carry_mod(ctx, y_sum_nc);
    let zero = chip.load_constant(ctx, P256Base::ZERO);
    let one = chip.load_constant(ctx, P256Base::ONE);
    let inverse_sum = chip.select(ctx, y_sum, zero.clone(), inverse);
    chip.assert_equal(ctx, inverse_sum, zero.clone());

    let generic_num_nc = chip.sub_no_carry(ctx, &q.y, &p.y);
    let generic_den_nc = chip.sub_no_carry(ctx, &q.x, &p.x);
    let generic_num = chip.carry_mod(ctx, generic_num_nc);
    let generic_den = chip.carry_mod(ctx, generic_den_nc);
    let x2 = chip.mul(ctx, &p.x, &p.x);
    let three_x2 = chip.scalar_mul_no_carry(ctx, x2, 3);
    let double_num_nc = chip.add_constant_no_carry(ctx, three_x2, -P256Base::from(3_u64));
    let double_den_nc = chip.scalar_mul_no_carry(ctx, &p.y, 2);
    let double_num = chip.carry_mod(ctx, double_num_nc);
    let double_den = chip.carry_mod(ctx, double_den_nc);
    let selected_num = chip.select(ctx, generic_num, double_num, generic);
    let selected_den = chip.select(ctx, generic_den, double_den, generic);
    let numerator = chip.select(ctx, selected_num, zero.clone(), active);
    let denominator = chip.select(ctx, selected_den, one, active);
    let slope = chip.divide(ctx, numerator, denominator);

    let slope2 = chip.mul_no_carry(ctx, &slope, &slope);
    let x3_minus_p = chip.sub_no_carry(ctx, slope2, &p.x);
    let x3_nc = chip.sub_no_carry(ctx, x3_minus_p, &q.x);
    let x3 = chip.carry_mod(ctx, x3_nc);
    let x_difference = chip.sub_no_carry(ctx, &p.x, &x3);
    let slope_term = chip.mul_no_carry(ctx, &slope, x_difference);
    let y3_nc = chip.sub_no_carry(ctx, slope_term, &p.y);
    let y3 = chip.carry_mod(ctx, y3_nc);
    let arithmetic = EcPoint::new(x3, y3);
    let identity = EcPoint::new(zero.clone(), zero);
    let selected = select_p256_point(chip, ctx, arithmetic, identity, active);
    let selected = select_p256_point(chip, ctx, p.clone(), selected, q_identity);
    let output = select_p256_point(chip, ctx, q.clone(), selected, p_identity);
    let _ = assert_p256_affine_or_identity(chip, ctx, &output);
    output
}

/// Select a point coordinate-wise with a constrained Boolean selector.
fn select_p256_point<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    a: EcPoint<F, ProperCrtUint<F>>,
    b: EcPoint<F, ProperCrtUint<F>>,
    selector: AssignedValue<F>,
) -> EcPoint<F, ProperCrtUint<F>> {
    EcPoint::new(
        chip.select(ctx, a.x, b.x, selector),
        chip.select(ctx, a.y, b.y, selector),
    )
}

/// Constrain `left_scalar * left + right_scalar * right` with one joint ladder.
///
/// Both bit arrays are MSB-first and Boolean-constrained. Each round doubles
/// the accumulator and adds exactly one of `0`, `left`, `right`, or
/// `left + right`. Precomputing the fourth choice also handles inverse points:
/// the table entry is then the constrained identity. Compared with two
/// independent ladders this removes one doubling and one addition per bit,
/// without changing the public or private scalar range.
/// The caller must bind a 256-bit instance to canonical scalar residues; this
/// bounded group primitive alone does not prove that representation.
pub(crate) fn joint_multiply_p256_affine_bits<F: BigPrimeField, const N: usize>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    left: &EcPoint<F, ProperCrtUint<F>>,
    right: &EcPoint<F, ProperCrtUint<F>>,
    left_bits_msb_first: &[AssignedValue<F>; N],
    right_bits_msb_first: &[AssignedValue<F>; N],
) -> EcPoint<F, ProperCrtUint<F>> {
    assert!(N > 0 && N <= 256, "P-256 joint scalar bit bound");
    let _ = assert_p256_affine_or_identity(chip, ctx, left);
    let _ = assert_p256_affine_or_identity(chip, ctx, right);
    let sum = add_p256_affine_complete_validated(chip, ctx, left, right);
    let zero = chip.load_constant(ctx, P256Base::ZERO);
    let identity = EcPoint::new(zero.clone(), zero);
    let mut accumulator = identity.clone();
    for (&left_bit, &right_bit) in left_bits_msb_first.iter().zip(right_bits_msb_first.iter()) {
        chip.gate().assert_bit(ctx, left_bit);
        chip.gate().assert_bit(ctx, right_bit);
        accumulator = add_p256_affine_complete_validated(chip, ctx, &accumulator, &accumulator);
        let if_left = select_p256_point(chip, ctx, sum.clone(), left.clone(), right_bit);
        let if_not_left = select_p256_point(chip, ctx, right.clone(), identity.clone(), right_bit);
        let summand = select_p256_point(chip, ctx, if_left, if_not_left, left_bit);
        accumulator = add_p256_affine_complete_validated(chip, ctx, &accumulator, &summand);
    }
    accumulator
}

/// Constrain a proper three-limb integer to its unique 256-bit representation.
///
/// The last two bits of the 86-by-three limb layout must be zero. This works
/// for canonical P-256 base coordinates and scalar residues alike.
fn p256_uint_bits_le<F: BigPrimeField>(
    base_chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    value: &ProperCrtUint<F>,
) -> [AssignedValue<F>; 256] {
    assert_eq!(value.limbs().len(), 3, "P-256 uses three 86-bit limbs");
    let mut bits = Vec::with_capacity(258);
    for limb in value.limbs() {
        bits.extend(base_chip.gate().num_to_bits(ctx, *limb, 86));
    }
    for bit in &bits[256..] {
        base_chip.gate().assert_is_const(ctx, bit, &F::ZERO);
    }
    bits.truncate(256);
    bits.try_into().expect("exactly 256 constrained bits")
}

/// Interpret 32 constrained, big-endian digest bytes as a 256-bit integer.
fn p256_digest_bits_le<F: BigPrimeField>(
    base_chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    digest_bytes_be: &[AssignedValue<F>; 32],
) -> [AssignedValue<F>; 256] {
    let mut bits = Vec::with_capacity(256);
    for byte in digest_bytes_be.iter().rev() {
        bits.extend(base_chip.gate().num_to_bits(ctx, *byte, 8));
    }
    bits.try_into().expect("32 bytes contain exactly 256 bits")
}

/// Bind a reduced P-256 point to an exact uncompressed SEC1 credential key.
///
/// Every coordinate byte is decomposed to eight constrained bits, then
/// compared bit-for-bit with the canonical 256-bit nonnative coordinate.
/// Neither compressed keys nor alternative encodings of the same point are
/// accepted. The parent must copy-bind `credential_sec1` to its governed
/// hardware credential before this point may authorize a transition.
pub(crate) fn assert_p256_uncompressed_sec1_key<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    point: &EcPoint<F, ProperCrtUint<F>>,
    credential_sec1: &[AssignedValue<F>; 65],
) {
    assert_p256_affine_point(chip, ctx, point);
    chip.gate()
        .assert_is_const(ctx, &credential_sec1[0], &F::from(4_u64));
    let x_bytes: [AssignedValue<F>; 32] = std::array::from_fn(|i| credential_sec1[i + 1]);
    let y_bytes: [AssignedValue<F>; 32] = std::array::from_fn(|i| credential_sec1[i + 33]);
    let x_bits = p256_digest_bits_le(chip, ctx, &x_bytes);
    let y_bits = p256_digest_bits_le(chip, ctx, &y_bytes);
    for (actual, expected) in x_bits.iter().zip(p256_uint_bits_le(chip, ctx, &point.x)) {
        ctx.constrain_equal(actual, &expected);
    }
    for (actual, expected) in y_bits.iter().zip(p256_uint_bits_le(chip, ctx, &point.y)) {
        ctx.constrain_equal(actual, &expected);
    }
}

/// Constrain `actual = reduced + quotient * n` as 256-bit integers.
///
/// `n` is the P-256 scalar order. Both operands are below 2^256 and `n` is
/// above 2^255, so `quotient` must be one bit. Independent 128-bit halves keep
/// all arithmetic below the Pasta modulus and prevent native-field wraparound.
fn assert_p256_mod_n_relation<F: BigPrimeField>(
    base_chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    actual_bits_le: &[AssignedValue<F>; 256],
    reduced_bits_le: &[AssignedValue<F>; 256],
    quotient: AssignedValue<F>,
) {
    const N_LOW: u128 = 0xBCE6FAADA7179E84F3B9CAC2FC632551;
    const N_HIGH: u128 = 0xFFFFFFFF00000000FFFFFFFFFFFFFFFF;
    let gate = base_chip.gate();
    gate.assert_bit(ctx, quotient);
    let compose = |ctx: &mut Context<F>, bits: &[AssignedValue<F>]| {
        gate.inner_product(
            ctx,
            bits.iter().copied(),
            (0..128).map(|bit| Constant(power_of_two::<F>(bit))),
        )
    };
    let low = compose(ctx, &reduced_bits_le[..128]);
    let high = compose(ctx, &reduced_bits_le[128..]);
    let n_low = F::from(N_LOW as u64) + F::from((N_LOW >> 64) as u64) * power_of_two::<F>(64);
    let n_high = F::from(N_HIGH as u64) + F::from((N_HIGH >> 64) as u64) * power_of_two::<F>(64);
    let low_sum = gate.mul_add(ctx, quotient, Constant(n_low), low);
    let low_sum_bits = gate.num_to_bits(ctx, low_sum, 129);
    for (actual, expected) in actual_bits_le[..128].iter().zip(&low_sum_bits[..128]) {
        ctx.constrain_equal(actual, expected);
    }
    let high_plus_order = gate.mul_add(ctx, quotient, Constant(n_high), high);
    let high_sum = gate.add(ctx, high_plus_order, low_sum_bits[128]);
    let high_sum_bits = gate.num_to_bits(ctx, high_sum, 129);
    for (actual, expected) in actual_bits_le[128..].iter().zip(&high_sum_bits[..128]) {
        ctx.constrain_equal(actual, expected);
    }
    gate.assert_is_const(ctx, &high_sum_bits[128], &F::ZERO);
}

/// Constrain the direct-signature profile's canonical low-S rule.
fn assert_p256_low_s<F: BigPrimeField>(
    scalar_chip: &FpChip<'_, F, P256Scalar>,
    ctx: &mut Context<F>,
    s: &ProperCrtUint<F>,
) {
    let cutoff = (modulus::<P256Scalar>() >> 1usize) + 1u32;
    let cutoff = FixedOverflowInteger::from_native(&cutoff, 3, 86).assign(ctx);
    let low_s = big_less_than::assign(
        scalar_chip.range(),
        ctx,
        s.clone(),
        cutoff,
        86,
        scalar_chip.limb_bases[1],
    );
    scalar_chip.gate().assert_is_const(ctx, &low_s, &F::ONE);
}

/// Constrain P-256 ECDSA verification of a canonical 32-byte prehash.
///
/// `enrolled_public_key` must come from a credential-bound circuit input;
/// its exact uncompressed SEC1 bytes are equality-bound here. `digest_bytes_be` must be bound by
/// the caller to SHA-256 of the signed assertion preimage. `N=256` admits the
/// full scalar range; smaller `N` strictly constrains upper scalar bits to
/// zero and exists only for bounded proof tests. Direct-signature profiles
/// instantiate `REQUIRE_LOW_S=true`; App Attest profile policy may differ.
///
/// TODO: Bind signed authenticator data, App Attest counter, digest SHA-256,
/// and credential policy in the parent recursive hardware-selection relation.
pub(crate) fn assert_p256_ecdsa_digest<
    F: BigPrimeField,
    const N: usize,
    const REQUIRE_LOW_S: bool,
>(
    base_chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    signature_public_key: &EcPoint<F, ProperCrtUint<F>>,
    enrolled_public_key: &EcPoint<F, ProperCrtUint<F>>,
    enrolled_public_key_sec1: &[AssignedValue<F>; 65],
    r: &ProperCrtUint<F>,
    s: &ProperCrtUint<F>,
    z: &ProperCrtUint<F>,
    digest_bytes_be: &[AssignedValue<F>; 32],
    digest_reduction_quotient: AssignedValue<F>,
) {
    assert!(N > 0 && N <= 256, "P-256 scalar window must be 1..=256");
    assert_p256_affine_point(base_chip, ctx, signature_public_key);
    assert_p256_uncompressed_sec1_key(
        base_chip,
        ctx,
        enrolled_public_key,
        enrolled_public_key_sec1,
    );
    base_chip.assert_equal(
        ctx,
        signature_public_key.x.clone(),
        enrolled_public_key.x.clone(),
    );
    base_chip.assert_equal(
        ctx,
        signature_public_key.y.clone(),
        enrolled_public_key.y.clone(),
    );

    let scalar_chip = FpChip::<F, P256Scalar>::new(base_chip.range(), 86, 3);
    let _ = scalar_chip.enforce_less_than(ctx, r.clone());
    let _ = scalar_chip.enforce_less_than(ctx, s.clone());
    let r_valid = scalar_chip.is_soft_nonzero(ctx, r.clone());
    let s_valid = scalar_chip.is_soft_nonzero(ctx, s.clone());
    base_chip.gate().assert_is_const(ctx, &r_valid, &F::ONE);
    base_chip.gate().assert_is_const(ctx, &s_valid, &F::ONE);
    let _ = scalar_chip.enforce_less_than(ctx, z.clone());
    if REQUIRE_LOW_S {
        assert_p256_low_s(&scalar_chip, ctx, s);
    }

    let digest_bits = p256_digest_bits_le(base_chip, ctx, digest_bytes_be);
    let z_bits = p256_uint_bits_le(base_chip, ctx, z);
    assert_p256_mod_n_relation(
        base_chip,
        ctx,
        &digest_bits,
        &z_bits,
        digest_reduction_quotient,
    );

    let u1 = scalar_chip.divide(ctx, z.clone(), s.clone());
    let u2 = scalar_chip.divide(ctx, r.clone(), s.clone());
    let _ = scalar_chip.enforce_less_than(ctx, u1.clone());
    let _ = scalar_chip.enforce_less_than(ctx, u2.clone());
    let u1_bits = p256_uint_bits_le(base_chip, ctx, &u1);
    let u2_bits = p256_uint_bits_le(base_chip, ctx, &u2);
    for bit in u1_bits[N..].iter().chain(&u2_bits[N..]) {
        base_chip.gate().assert_is_const(ctx, bit, &F::ZERO);
    }
    let u1_bits_msb: [AssignedValue<F>; N] = std::array::from_fn(|index| u1_bits[N - 1 - index]);
    let u2_bits_msb: [AssignedValue<F>; N] = std::array::from_fn(|index| u2_bits[N - 1 - index]);

    let generator = Secp256r1Affine::generator();
    let (gx, gy) = generator.into_coordinates();
    let g = EcPoint::new(
        base_chip.load_constant(ctx, gx),
        base_chip.load_constant(ctx, gy),
    );
    let result = joint_multiply_p256_affine_bits(
        base_chip,
        ctx,
        &g,
        signature_public_key,
        &u1_bits_msb,
        &u2_bits_msb,
    );
    assert_p256_affine_point(base_chip, ctx, &result);
    let result_x_bits = p256_uint_bits_le(base_chip, ctx, &result.x);
    let r_bits = p256_uint_bits_le(base_chip, ctx, r);
    let scalar_order = modulus::<P256Scalar>();
    let x_quotient = ctx.load_witness(if result.x.value() >= scalar_order {
        F::ONE
    } else {
        F::ZERO
    });
    assert_p256_mod_n_relation(base_chip, ctx, &result_x_bits, &r_bits, x_quotient);
}

/// Convert eight copy-bound SHA-256 words to their canonical big-endian bytes.
fn sha256_words_to_be_bytes<F: BigPrimeField>(
    base_chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    words: &[AssignedValue<F>; 8],
) -> [PastaSha256ByteV1<F>; 32] {
    let gate = base_chip.range().gate();
    let mut bytes = Vec::with_capacity(32);
    for word in words {
        let bits = PastaSha256BitV1::decompose(ctx, gate, *word, 32);
        for byte in (0..4).rev() {
            bytes.push(PastaSha256ByteV1::from_bits_le(
                ctx,
                gate,
                &bits[byte * 8..(byte + 1) * 8],
            ));
        }
    }
    bytes.try_into().expect("eight SHA-256 words are 32 bytes")
}

/// Queue Apple's three assertion hashes over the complete authenticator data.
///
/// `S` is Core's canonical signing message. This function proves its domain
/// and length framing; the parent must additionally constrain the flat V1 body
/// to the independently reconstructed transition subject. The expected RP ID
/// hash and previous/next indices must come from the governed credential and
/// inherited monetary checkpoint, not from the assertion itself.
///
/// The full, release-bounded authenticator data, including extensions, enters
/// the signed SHA-256 preimage. `expected_flags` must be bound by the parent to
/// the governed Apple profile. This slice accepts the physically observed
/// 37-byte header with `0x40`; longer
/// authenticated data requires the ED bit and extension bytes. The model may
/// parse a fully signed suffix with ED unset, but this proof profile keeps that
/// form closed until qualified. Apple's
/// current assertion guide also
/// requires validation of `validationCategory` and
/// `bundleVersion`. This hash/counter slice does not
/// parse or authorize those extensions, so no production monetary profile may
/// rely on it until a qualified extension relation is recursively bound.
/// See <https://developer.apple.com/documentation/devicecheck/validating-apps-that-connect-to-your-server>.
pub(crate) fn queue_apple_assertion_digest<F, const S_LEN: usize, const AUTH_LEN: usize>(
    base_chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    canonical_s: &[AssignedValue<F>; S_LEN],
    authenticator_data: &[AssignedValue<F>; AUTH_LEN],
    governed_rp_id_hash: &[AssignedValue<F>; 32],
    expected_flags: AssignedValue<F>,
    previous_secure_index: AssignedValue<F>,
    next_secure_index: AssignedValue<F>,
) -> Result<[AssignedValue<F>; 32], String>
where
    F: BigPrimeField + PrimeField + From<u64>,
{
    const SIGNING_DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-transition-selection\0";
    assert!(
        S_LEN > SIGNING_DOMAIN.len() + 8 && S_LEN <= 1_024,
        "Core canonical selection message has a release-bounded length"
    );
    assert!(
        (37..=1_024).contains(&AUTH_LEN),
        "Apple authenticator data must contain the complete signed header"
    );
    let range = base_chip.range();
    let gate = range.gate();
    let body_len = (S_LEN - SIGNING_DOMAIN.len() - 8) as u64;
    for (assigned, expected) in canonical_s
        .iter()
        .zip(SIGNING_DOMAIN.iter().copied().chain(body_len.to_le_bytes()))
    {
        gate.assert_is_const(ctx, assigned, &F::from(u64::from(expected)));
    }

    let s_bytes = canonical_s
        .iter()
        .copied()
        .map(|byte| PastaSha256ByteV1::range_checked(ctx, range, byte))
        .collect::<Vec<_>>();
    let client_data_words = jobs.digest_constrained(ctx, &s_bytes)?;
    let client_data_hash = sha256_words_to_be_bytes(base_chip, ctx, &client_data_words);

    let auth_bytes = authenticator_data
        .iter()
        .copied()
        .map(|byte| PastaSha256ByteV1::range_checked(ctx, range, byte))
        .collect::<Vec<_>>();
    let mut rp_sum = Vec::with_capacity(32);
    for (actual, expected) in authenticator_data[..32].iter().zip(governed_rp_id_hash) {
        range.range_check(ctx, *expected, 8);
        ctx.constrain_equal(actual, expected);
        rp_sum.push(*expected);
    }
    let rp_sum = gate.sum(ctx, rp_sum);
    let rp_is_zero = gate.is_zero(ctx, rp_sum);
    gate.assert_is_const(ctx, &rp_is_zero, &F::ZERO);
    range.range_check(ctx, expected_flags, 8);
    ctx.constrain_equal(&authenticator_data[32], &expected_flags);
    let flag_bits = gate.num_to_bits(ctx, expected_flags, 8);
    for bit in &flag_bits[..6] {
        gate.assert_is_const(ctx, bit, &F::ZERO);
    }
    gate.assert_is_const(ctx, &flag_bits[6], &F::ONE);
    if AUTH_LEN == 37 {
        gate.assert_is_const(ctx, &flag_bits[7], &F::ZERO);
    } else {
        gate.assert_is_const(ctx, &flag_bits[7], &F::ONE);
    }

    range.range_check(ctx, previous_secure_index, 32);
    range.range_check(ctx, next_secure_index, 32);
    let exact_next = gate.add(ctx, previous_secure_index, Constant(F::ONE));
    ctx.constrain_equal(&exact_next, &next_secure_index);
    let signed_counter = gate.inner_product(
        ctx,
        authenticator_data[33..37].iter().copied(),
        [24, 16, 8, 0].map(|bit| Constant(power_of_two::<F>(bit))),
    );
    ctx.constrain_equal(&signed_counter, &next_secure_index);

    let mut message = auth_bytes;
    message.extend(client_data_hash);
    let nonce_words = jobs.digest_constrained(ctx, &message)?;
    let nonce = sha256_words_to_be_bytes(base_chip, ctx, &nonce_words);
    // Apple's ECDSA-P256-SHA256 assertion API signs the nonce as a message;
    // the signature equation therefore uses SHA256(nonce), not nonce itself.
    let prehash_words = jobs.digest_constrained(ctx, &nonce)?;
    let prehash = sha256_words_to_be_bytes(base_chip, ctx, &prehash_words);
    Ok(prehash.map(|byte| {
        byte.assigned()
            .expect("digest bytes are Boolean-composed assigned cells")
    }))
}

/// Feed the fully constrained Apple assertion prehash into P-256 ECDSA.
///
/// The parent must still bind `canonical_s` to the Core subject body, the RP
/// hash/key to the enrolled Apple credential, and the two indices to the
/// recursive transition. The SHA jobs must be realized with
/// [`PastaSha256JobsV1::synthesize`] after Base synthesis.
pub(crate) fn assert_apple_assertion_ecdsa<
    F,
    const S_LEN: usize,
    const N: usize,
    const AUTH_LEN: usize,
    const REQUIRE_LOW_S: bool,
>(
    base_chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    jobs: &mut PastaSha256JobsV1<F>,
    canonical_s: &[AssignedValue<F>; S_LEN],
    authenticator_data: &[AssignedValue<F>; AUTH_LEN],
    governed_rp_id_hash: &[AssignedValue<F>; 32],
    expected_flags: AssignedValue<F>,
    previous_secure_index: AssignedValue<F>,
    next_secure_index: AssignedValue<F>,
    signature_public_key: &EcPoint<F, ProperCrtUint<F>>,
    enrolled_public_key: &EcPoint<F, ProperCrtUint<F>>,
    enrolled_public_key_sec1: &[AssignedValue<F>; 65],
    r: &ProperCrtUint<F>,
    s: &ProperCrtUint<F>,
    z: &ProperCrtUint<F>,
    digest_reduction_quotient: AssignedValue<F>,
) -> Result<(), String>
where
    F: BigPrimeField + PrimeField + From<u64>,
{
    let digest = queue_apple_assertion_digest::<F, S_LEN, AUTH_LEN>(
        base_chip,
        ctx,
        jobs,
        canonical_s,
        authenticator_data,
        governed_rp_id_hash,
        expected_flags,
        previous_secure_index,
        next_secure_index,
    )?;
    assert_p256_ecdsa_digest::<F, N, REQUIRE_LOW_S>(
        base_chip,
        ctx,
        signature_public_key,
        enrolled_public_key,
        enrolled_public_key_sec1,
        r,
        s,
        z,
        &digest,
        digest_reduction_quotient,
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::gates::circuit::builder::BaseCircuitBuilder;
    use halo2_base::utils::CurveAffineExt as _;
    use halo2_ecc::bigint::FixedCRTInteger;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::{
            group::{Curve as _, prime::PrimeCurveAffine as _},
            pasta::{Fp, Fq},
        },
    };

    const TEST_K: u32 = 16;

    fn check_scalar_order_bound<F: BigPrimeField>(above_order: bool) -> bool {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(10)
            .use_lookup_bits(9)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let chip = FpChip::<F, P256Scalar>::new(&range, 86, 3);
        let order = modulus::<P256Scalar>();
        let value = if above_order {
            &order + 1_u32
        } else {
            &order - 1_u32
        };
        let ctx = builder.main(0);
        let assigned = FixedCRTInteger::from_native(value, 3, 86).assign(ctx, 86, &modulus::<F>());
        let _ = chip.enforce_less_than(ctx, assigned);
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(10, &builder, vec![Vec::new()])
            .expect("scalar range circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn p256_scalar_order_bound_rejects_noncanonical_residues_in_both_pasta_fields() {
        assert!(check_scalar_order_bound::<Fp>(false));
        assert!(check_scalar_order_bound::<Fq>(false));
        assert!(!check_scalar_order_bound::<Fp>(true));
        assert!(!check_scalar_order_bound::<Fq>(true));
    }

    fn check_point_and_double<F: BigPrimeField>(alter_y: bool, check_double: bool) -> bool {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits((TEST_K - 1) as usize)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let chip = FpChip::<F, P256Base>::new(&range, 86, 3);
        let generator = Secp256r1Affine::generator();
        let (x, y) = generator.into_coordinates();
        let y = if alter_y {
            y + P256Base::from(1_u64)
        } else {
            y
        };
        let ctx = builder.main(0);
        let point = EcPoint::new(chip.load_private(ctx, x), chip.load_private(ctx, y));
        if check_double {
            let actual = double_p256_affine_point(&chip, ctx, &point);
            let expected = (generator.to_curve() + generator.to_curve()).to_affine();
            let (expected_x, expected_y) = expected.into_coordinates();
            let expected_x = chip.load_constant(ctx, expected_x);
            let expected_y = chip.load_constant(ctx, expected_y);
            chip.assert_equal(ctx, actual.x, expected_x);
            chip.assert_equal(ctx, actual.y, expected_y);
        } else {
            assert_p256_affine_point(&chip, ctx, &point);
        }
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(TEST_K, &builder, vec![Vec::new()])
            .expect("P-256 Base circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn generator_and_double_match_p256_in_both_pasta_fields() {
        assert!(check_point_and_double::<Fp>(false, true));
        assert!(check_point_and_double::<Fq>(false, true));
    }

    #[test]
    fn off_curve_point_fails_in_both_pasta_fields() {
        assert!(!check_point_and_double::<Fp>(true, false));
        assert!(!check_point_and_double::<Fq>(true, false));
    }

    fn load_affine<F: BigPrimeField>(
        chip: &FpChip<'_, F, P256Base>,
        ctx: &mut Context<F>,
        point: Secp256r1Affine,
    ) -> EcPoint<F, ProperCrtUint<F>> {
        let (x, y) = if bool::from(point.is_identity()) {
            (P256Base::ZERO, P256Base::ZERO)
        } else {
            point.into_coordinates()
        };
        EcPoint::new(chip.load_private(ctx, x), chip.load_private(ctx, y))
    }

    fn uncompressed_sec1(point: Secp256r1Affine) -> [u8; 65] {
        let (x, y) = point.into_coordinates();
        let mut x_be = x.to_repr();
        let mut y_be = y.to_repr();
        x_be.reverse();
        y_be.reverse();
        let mut sec1 = [0_u8; 65];
        sec1[0] = 4;
        sec1[1..33].copy_from_slice(&x_be);
        sec1[33..65].copy_from_slice(&y_be);
        sec1
    }

    fn check_uncompressed_sec1_binding<F: BigPrimeField>(mutation: usize) -> bool {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits((TEST_K - 1) as usize)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let chip = FpChip::<F, P256Base>::new(&range, 86, 3);
        let generator = Secp256r1Affine::generator();
        let mut sec1 = uncompressed_sec1(generator);
        if mutation != 0 {
            sec1[mutation] ^= 1;
        }
        let ctx = builder.main(0);
        let point = load_affine(&chip, ctx, generator);
        let bytes: [AssignedValue<F>; 65] =
            std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(sec1[i]))));
        assert_p256_uncompressed_sec1_key(&chip, ctx, &point, &bytes);
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(TEST_K, &builder, vec![Vec::new()])
            .expect("P-256 SEC1 binding circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn uncompressed_sec1_key_binds_all_coordinates_in_both_pasta_fields() {
        for mutation in [0, 1, 33, 64] {
            assert_eq!(
                check_uncompressed_sec1_binding::<Fp>(mutation),
                mutation == 0
            );
            assert_eq!(
                check_uncompressed_sec1_binding::<Fq>(mutation),
                mutation == 0
            );
        }
    }

    fn check_complete_add<F: BigPrimeField>(
        left: Secp256r1Affine,
        right: Secp256r1Affine,
        expected: Secp256r1Affine,
        corrupt_right: bool,
    ) -> bool {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits((TEST_K - 1) as usize)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let chip = FpChip::<F, P256Base>::new(&range, 86, 3);
        let ctx = builder.main(0);
        let left = load_affine(&chip, ctx, left);
        let right = if corrupt_right {
            let (x, y) = right.into_coordinates();
            EcPoint::new(
                chip.load_private(ctx, x),
                chip.load_private(ctx, y + P256Base::ONE),
            )
        } else {
            load_affine(&chip, ctx, right)
        };
        let actual = add_p256_affine_complete(&chip, ctx, &left, &right);
        let expected = load_affine(&chip, ctx, expected);
        chip.assert_equal(ctx, actual.x, expected.x);
        chip.assert_equal(ctx, actual.y, expected.y);
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(TEST_K, &builder, vec![Vec::new()])
            .expect("P-256 complete-add circuit synthesizes")
            .verify()
            .is_ok()
    }

    fn check_all_add_cases<F: BigPrimeField>() {
        let g = Secp256r1Affine::generator();
        let identity = Secp256r1Affine::identity();
        let two = (g.to_curve() + g.to_curve()).to_affine();
        let three = (two.to_curve() + g.to_curve()).to_affine();
        let inverse = (-g.to_curve()).to_affine();
        assert!(check_complete_add::<F>(identity, identity, identity, false));
        assert!(check_complete_add::<F>(identity, g, g, false));
        assert!(check_complete_add::<F>(g, identity, g, false));
        assert!(check_complete_add::<F>(g, g, two, false));
        assert!(check_complete_add::<F>(g, inverse, identity, false));
        assert!(check_complete_add::<F>(g, two, three, false));
        assert!(!check_complete_add::<F>(g, g, two, true));
    }

    #[test]
    fn complete_add_covers_exceptional_cases_and_rejects_bad_point_in_both_pasta_fields() {
        check_all_add_cases::<Fp>();
        check_all_add_cases::<Fq>();
    }

    fn check_joint_two_bit_scalars<F: BigPrimeField>(
        left_scalar: u8,
        right_scalar: u8,
        inverse_right: bool,
        corrupt_result: bool,
    ) -> bool {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(18)
            .use_lookup_bits(17)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let chip = FpChip::<F, P256Base>::new(&range, 86, 3);
        let generator = Secp256r1Affine::generator();
        let right_host = if inverse_right {
            (-generator.to_curve()).to_affine()
        } else {
            (generator.to_curve() + generator.to_curve()).to_affine()
        };
        let mut expected_host = Secp256r1Affine::identity().to_curve();
        for _ in 0..left_scalar {
            expected_host += generator.to_curve();
        }
        for _ in 0..right_scalar {
            expected_host += right_host.to_curve();
        }
        if corrupt_result {
            expected_host += generator.to_curve();
        }

        let ctx = builder.main(0);
        let left = load_affine(&chip, ctx, generator);
        let right = load_affine(&chip, ctx, right_host);
        let left_bits: [AssignedValue<F>; 2] = std::array::from_fn(|bit| {
            ctx.load_witness(F::from(u64::from((left_scalar >> (1 - bit)) & 1)))
        });
        let right_bits: [AssignedValue<F>; 2] = std::array::from_fn(|bit| {
            ctx.load_witness(F::from(u64::from((right_scalar >> (1 - bit)) & 1)))
        });
        let actual =
            joint_multiply_p256_affine_bits(&chip, ctx, &left, &right, &left_bits, &right_bits);
        let expected = load_affine(&chip, ctx, expected_host.to_affine());
        chip.assert_equal(ctx, actual.x, expected.x);
        chip.assert_equal(ctx, actual.y, expected.y);
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(18, &builder, vec![Vec::new()])
            .expect("P-256 joint scalar circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn joint_scalar_table_and_inverse_points_match_p256_in_both_pasta_fields() {
        for (left, right, inverse_right) in [
            (0, 0, false),
            (1, 0, false),
            (0, 1, false),
            (1, 1, false),
            (2, 3, false),
            (1, 1, true),
        ] {
            assert!(check_joint_two_bit_scalars::<Fp>(
                left,
                right,
                inverse_right,
                false
            ));
            assert!(check_joint_two_bit_scalars::<Fq>(
                left,
                right,
                inverse_right,
                false
            ));
        }
        assert!(!check_joint_two_bit_scalars::<Fp>(2, 3, false, true));
        assert!(!check_joint_two_bit_scalars::<Fq>(2, 3, false, true));
    }

    fn check_ecdsa_small_scalar_slice<F: BigPrimeField>(
        change_digest: bool,
        change_enrolled_key: bool,
        zero_r: bool,
    ) -> bool {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(18)
            .use_lookup_bits(17)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let base_chip = FpChip::<F, P256Base>::new(&range, 86, 3);
        let scalar_chip = FpChip::<F, P256Scalar>::new(&range, 86, 3);
        let ctx = builder.main(0);
        let generator = Secp256r1Affine::generator();
        let two = (generator.to_curve() + generator.to_curve()).to_affine();
        let (two_x, _) = two.into_coordinates();
        let x_as_scalar = P256Scalar::from_repr(two_x.to_repr())
            .expect("2G x-coordinate is below the P-256 scalar order");
        // r = s = z = x(2G) gives u1 = u2 = 1 with Q = G.
        let r_value = if zero_r {
            P256Scalar::ZERO
        } else {
            x_as_scalar
        };
        let r = scalar_chip.load_private(ctx, r_value);
        let s = scalar_chip.load_private(ctx, x_as_scalar);
        let z = scalar_chip.load_private(ctx, x_as_scalar);
        let mut digest_be = x_as_scalar.to_repr();
        digest_be.reverse();
        if change_digest {
            digest_be[31] ^= 1;
        }
        let digest = std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(digest_be[i]))));
        let digest_quotient = ctx.load_witness(F::ZERO);
        let signature_key = load_affine(&base_chip, ctx, generator);
        let enrolled_host = if change_enrolled_key { two } else { generator };
        let enrolled_key = load_affine(&base_chip, ctx, enrolled_host);
        let enrolled_sec1 = uncompressed_sec1(enrolled_host);
        let enrolled_sec1: [AssignedValue<F>; 65] =
            std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(enrolled_sec1[i]))));
        assert_p256_ecdsa_digest::<F, 2, true>(
            &base_chip,
            ctx,
            &signature_key,
            &enrolled_key,
            &enrolled_sec1,
            &r,
            &s,
            &z,
            &digest,
            digest_quotient,
        );
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(18, &builder, vec![Vec::new()])
            .expect("P-256 ECDSA slice synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    #[ignore = "full-width P-256 capacity inventory is expensive and awaits the release build lane"]
    fn full_width_p256_equation_inventory_in_both_pasta_fields() {
        fn inventory<F: BigPrimeField>() -> (usize, usize) {
            let mut builder = BaseCircuitBuilder::<F>::new(false)
                .use_k(TEST_K as usize)
                .use_lookup_bits((TEST_K - 1) as usize)
                .use_instance_columns(1);
            let range = builder.range_chip();
            let base_chip = FpChip::<F, P256Base>::new(&range, 86, 3);
            let scalar_chip = FpChip::<F, P256Scalar>::new(&range, 86, 3);
            let ctx = builder.main(0);
            let generator = Secp256r1Affine::generator();
            let two = (generator.to_curve() + generator.to_curve()).to_affine();
            let (two_x, _) = two.into_coordinates();
            let scalar = P256Scalar::from_repr(two_x.to_repr())
                .expect("2G x-coordinate is below the P-256 scalar order");
            let r = scalar_chip.load_private(ctx, scalar);
            let s = scalar_chip.load_private(ctx, scalar);
            let z = scalar_chip.load_private(ctx, scalar);
            let mut digest_be = scalar.to_repr();
            digest_be.reverse();
            let digest =
                std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(digest_be[i]))));
            let key = load_affine(&base_chip, ctx, generator);
            let sec1 = uncompressed_sec1(generator);
            let enrolled_sec1: [AssignedValue<F>; 65] =
                std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(sec1[i]))));
            let digest_quotient = ctx.load_witness(F::ZERO);
            assert_p256_ecdsa_digest::<F, 256, true>(
                &base_chip,
                ctx,
                &key,
                &key,
                &enrolled_sec1,
                &r,
                &s,
                &z,
                &digest,
                digest_quotient,
            );
            let cells = builder.statistics().gate.total_advice_per_phase[0];
            let params = builder.calculate_params(Some(9));
            let columns = params.num_advice_per_phase[0];
            (cells, columns)
        }

        let fp = inventory::<Fp>();
        let fq = inventory::<Fq>();
        eprintln!("full-width P-256 k=16 Eq/Fp cells+columns: {fp:?}; Ep/Fq: {fq:?}");
        assert!(fp.0 > 0 && fq.0 > 0 && fp.1 > 0 && fq.1 > 0);
    }

    #[test]
    fn ecdsa_small_scalar_equation_binds_digest_key_and_r_in_both_pasta_fields() {
        for check in [
            check_ecdsa_small_scalar_slice::<Fp>(false, false, false),
            check_ecdsa_small_scalar_slice::<Fq>(false, false, false),
        ] {
            assert!(check);
        }
        for check in [
            check_ecdsa_small_scalar_slice::<Fp>(true, false, false),
            check_ecdsa_small_scalar_slice::<Fq>(true, false, false),
            check_ecdsa_small_scalar_slice::<Fp>(false, true, false),
            check_ecdsa_small_scalar_slice::<Fq>(false, true, false),
            check_ecdsa_small_scalar_slice::<Fp>(false, false, true),
            check_ecdsa_small_scalar_slice::<Fq>(false, false, true),
        ] {
            assert!(!check);
        }
    }

    fn check_low_s_policy<F: BigPrimeField>(s_value: P256Scalar) -> bool {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits((TEST_K - 1) as usize)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let scalar_chip = FpChip::<F, P256Scalar>::new(&range, 86, 3);
        let ctx = builder.main(0);
        let s = scalar_chip.load_private(ctx, s_value);
        assert_p256_low_s(&scalar_chip, ctx, &s);
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(TEST_K, &builder, vec![Vec::new()])
            .expect("P-256 low-S circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn direct_profile_rejects_high_s_in_both_pasta_fields() {
        let generator = Secp256r1Affine::generator();
        let two = (generator.to_curve() + generator.to_curve()).to_affine();
        let (two_x, _) = two.into_coordinates();
        let low_s = P256Scalar::from_repr(two_x.to_repr()).expect("2G x is a scalar");
        assert!(check_low_s_policy::<Fp>(low_s));
        assert!(check_low_s_policy::<Fq>(low_s));
        assert!(!check_low_s_policy::<Fp>(-low_s));
        assert!(!check_low_s_policy::<Fq>(-low_s));
    }

    fn check_digest_mod_n_branch<F: BigPrimeField>(quotient: u64) -> bool {
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits((TEST_K - 1) as usize)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let base_chip = FpChip::<F, P256Base>::new(&range, 86, 3);
        let scalar_chip = FpChip::<F, P256Scalar>::new(&range, 86, 3);
        let ctx = builder.main(0);
        // (2^256 - 1) - n, encoded as a canonical little-endian scalar.
        let remainder_be = [
            0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x43, 0x19, 0x05, 0x52, 0x58, 0xe8, 0x61, 0x7b, 0x0c, 0x46, 0x35, 0x3d,
            0x03, 0x9c, 0xda, 0xae,
        ];
        let mut remainder_le = remainder_be;
        remainder_le.reverse();
        let remainder = P256Scalar::from_repr(remainder_le).expect("canonical remainder");
        let reduced = scalar_chip.load_private(ctx, remainder);
        let reduced_bits = p256_uint_bits_le(&base_chip, ctx, &reduced);
        let bytes = std::array::from_fn(|_| ctx.load_witness(F::from(255_u64)));
        let actual_bits = p256_digest_bits_le(&base_chip, ctx, &bytes);
        let quotient = ctx.load_witness(F::from(quotient));
        assert_p256_mod_n_relation(&base_chip, ctx, &actual_bits, &reduced_bits, quotient);
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(TEST_K, &builder, vec![Vec::new()])
            .expect("P-256 digest reduction circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn full_width_digest_reduces_once_mod_n_in_both_pasta_fields() {
        assert!(check_digest_mod_n_branch::<Fp>(1));
        assert!(check_digest_mod_n_branch::<Fq>(1));
        assert!(!check_digest_mod_n_branch::<Fp>(0));
        assert!(!check_digest_mod_n_branch::<Fq>(0));
    }
}

#[cfg(test)]
mod apple_assertion_tests {
    use super::*;
    use crate::zk::pasta_sha256::PastaSha256ConfigV1;
    use halo2_base::{
        gates::circuit::{BaseCircuitParams, BaseConfig, builder::BaseCircuitBuilder},
        utils::ScalarField,
    };
    use halo2_proofs::{
        circuit::{Layouter, V1},
        dev::MockProver,
        halo2curves::{
            ff::{Field as _, PrimeField as _},
            group::{Curve as _, prime::PrimeCurveAffine as _},
            pasta::{Fp, Fq},
        },
        plonk::{Circuit, ConstraintSystem, Error},
    };
    use sha2::{Digest as _, Sha256};

    const TEST_K: u32 = 18;
    const UNUSABLE_ROWS: usize = 9;
    const DOMAIN: &[u8] = b"iroha:kagemusha:v1:hardware-transition-selection\0";
    const BODY_LEN: usize = 16;
    const S_LEN: usize = DOMAIN.len() + 8 + BODY_LEN;

    #[derive(Clone, Debug)]
    struct AppleConfig<F: ScalarField> {
        base: BaseConfig<F>,
        sha: PastaSha256ConfigV1,
    }

    #[derive(Clone)]
    struct AppleCircuit<F: BigPrimeField + PrimeField + From<u64>> {
        builder: BaseCircuitBuilder<F>,
        jobs: PastaSha256JobsV1<F>,
    }

    impl<F> Circuit<F> for AppleCircuit<F>
    where
        F: BigPrimeField + PrimeField + From<u64>,
    {
        type Config = AppleConfig<F>;
        type FloorPlanner = V1;
        type Params = BaseCircuitParams;

        fn params(&self) -> Self::Params {
            self.builder.config_params.clone()
        }

        fn without_witnesses(&self) -> Self {
            Self {
                builder: self.builder.deep_clone().unknown(true),
                jobs: self.jobs.unknown(),
            }
        }

        fn configure_with_params(
            meta: &mut ConstraintSystem<F>,
            params: Self::Params,
        ) -> Self::Config {
            let usable_rows = (1_usize << params.k) - UNUSABLE_ROWS;
            let mut base = BaseConfig::configure(meta, params);
            base.set_usable_rows(usable_rows);
            AppleConfig {
                base,
                sha: PastaSha256ConfigV1::configure(meta),
            }
        }

        fn configure(_: &mut ConstraintSystem<F>) -> Self::Config {
            unreachable!("Apple assertion test uses parameterized Base config")
        }

        fn synthesize(
            &self,
            config: Self::Config,
            mut layouter: impl Layouter<F>,
        ) -> Result<(), Error> {
            <BaseCircuitBuilder<F> as Circuit<F>>::synthesize(
                &self.builder,
                config.base,
                layouter.namespace(|| "Apple assertion Base"),
            )?;
            self.jobs.synthesize(
                &config.sha,
                &mut layouter,
                &self.builder.core().copy_manager,
                (1_usize << TEST_K) - UNUSABLE_ROWS,
            )
        }
    }

    #[derive(Clone, Copy)]
    enum Mutation {
        None,
        Domain,
        Body,
        RpId,
        Flags,
        Counter,
        Rollover,
        SignedExtension,
        ReservedFlag,
    }

    fn check_apple_hash_and_counter<F, const AUTH_LEN: usize, const FLAGS: u8>(
        mutation: Mutation,
    ) -> bool
    where
        F: BigPrimeField + PrimeField + From<u64>,
    {
        let mut s = Vec::with_capacity(S_LEN);
        s.extend_from_slice(DOMAIN);
        s.extend_from_slice(&(BODY_LEN as u64).to_le_bytes());
        s.extend_from_slice(&[0x42; BODY_LEN]);
        let rp: [u8; 32] = Sha256::digest(b"TEAM.bundle").into();
        let mut auth = [0_u8; AUTH_LEN];
        auth[..32].copy_from_slice(&rp);
        auth[32] = FLAGS;
        auth[36] = 1;
        if AUTH_LEN > 37 {
            // A signed CBOR extension fixture; semantic Apple extension checks
            // belong to the later qualified relation, not this hash test.
            auth[37..].copy_from_slice(&[0xa1, 0x61, b'x', 0x01]);
        }
        let h: [u8; 32] = Sha256::digest(&s).into();
        let mut preimage = auth.to_vec();
        preimage.extend_from_slice(&h);
        let nonce: [u8; 32] = Sha256::digest(&preimage).into();
        let mut expected: [u8; 32] = Sha256::digest(nonce).into();
        let mut governed_flags = auth[32];

        match mutation {
            Mutation::None | Mutation::Rollover => {}
            Mutation::Domain => s[0] ^= 1,
            Mutation::Body => s[S_LEN - 1] ^= 1,
            Mutation::RpId => auth[0] ^= 1,
            Mutation::Flags => auth[32] ^= 2,
            Mutation::Counter => auth[36] = 2,
            Mutation::SignedExtension => auth[AUTH_LEN - 1] ^= 1,
            Mutation::ReservedFlag => {
                auth[32] |= 0x01;
                governed_flags = auth[32];
            }
        }
        if matches!(mutation, Mutation::ReservedFlag) {
            let mut signed = auth.to_vec();
            signed.extend_from_slice(&h);
            let nonce: [u8; 32] = Sha256::digest(&signed).into();
            expected = Sha256::digest(nonce).into();
        }

        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits((TEST_K - 1) as usize);
        let range = builder.range_chip();
        let base_chip = FpChip::<F, P256Base>::new(&range, 86, 3);
        let mut jobs = PastaSha256JobsV1::default();
        let ctx = builder.main(0);
        let canonical_s: [AssignedValue<F>; S_LEN] =
            std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(s[index]))));
        let authenticator_data: [AssignedValue<F>; AUTH_LEN] =
            std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(auth[index]))));
        let governed_rp: [AssignedValue<F>; 32] =
            std::array::from_fn(|index| ctx.load_witness(F::from(u64::from(rp[index]))));
        let expected_flags = ctx.load_witness(F::from(u64::from(governed_flags)));
        let previous = ctx.load_witness(F::from(if matches!(mutation, Mutation::Rollover) {
            u64::from(u32::MAX)
        } else {
            0
        }));
        let next = ctx.load_witness(F::from(1_u64));
        let digest = queue_apple_assertion_digest::<F, S_LEN, AUTH_LEN>(
            &base_chip,
            ctx,
            &mut jobs,
            &canonical_s,
            &authenticator_data,
            &governed_rp,
            expected_flags,
            previous,
            next,
        )
        .expect("canonical Apple assertion fixture queues three hashes");
        for (actual, expected_byte) in digest.iter().zip(expected) {
            let expected_cell = ctx.load_constant(F::from(u64::from(expected_byte)));
            ctx.constrain_equal(actual, &expected_cell);
        }
        builder.calculate_params(Some(UNUSABLE_ROWS));
        let circuit = AppleCircuit { builder, jobs };
        MockProver::run(TEST_K, &circuit, vec![])
            .expect("Apple assertion SHA queue synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn apple_assertion_hash_rp_flags_and_exact_next_are_constrained_in_both_pasta_fields() {
        assert!(check_apple_hash_and_counter::<Fp, 41, 0xc0>(Mutation::None));
        assert!(check_apple_hash_and_counter::<Fq, 41, 0xc0>(Mutation::None));
        for mutation in [
            Mutation::Domain,
            Mutation::Body,
            Mutation::RpId,
            Mutation::Flags,
            Mutation::Counter,
            Mutation::Rollover,
            Mutation::SignedExtension,
            Mutation::ReservedFlag,
        ] {
            assert!(!check_apple_hash_and_counter::<Fp, 41, 0xc0>(mutation));
            assert!(!check_apple_hash_and_counter::<Fq, 41, 0xc0>(mutation));
        }
    }

    #[test]
    fn longer_authenticator_data_requires_ed_flag_in_both_pasta_fields() {
        assert!(!check_apple_hash_and_counter::<Fp, 41, 0x40>(
            Mutation::None
        ));
        assert!(!check_apple_hash_and_counter::<Fq, 41, 0x40>(
            Mutation::None
        ));
    }

    #[derive(Clone, Copy)]
    enum EcdsaMutation {
        None,
        Subject,
        Sec1Key,
        Counter,
    }

    fn check_apple_hash_and_p256_equation<F>(mutation: EcdsaMutation) -> bool
    where
        F: BigPrimeField + PrimeField + From<u64>,
    {
        let rp: [u8; 32] = Sha256::digest(b"TEAM.bundle").into();
        let mut auth = [0_u8; 37];
        auth[..32].copy_from_slice(&rp);
        auth[32] = 0x40;
        auth[36] = 1;
        let mut canonical_s = Vec::with_capacity(S_LEN);
        canonical_s.extend_from_slice(DOMAIN);
        canonical_s.extend_from_slice(&(BODY_LEN as u64).to_le_bytes());
        canonical_s.extend_from_slice(&[0x42; BODY_LEN]);

        // Construct a valid, small-scalar ECDSA verification equation for an
        // actual SHA-256 assertion digest. For r = s = z we have u1 = u2 = 1.
        // If z is an on-curve x-coordinate, Q = R - G makes G + Q = R and
        // x(R) = r. The loop only selects public test data, not circuit shape.
        let (z, q) = (0_u8..=u8::MAX)
            .find_map(|tweak| {
                canonical_s[S_LEN - 1] = tweak;
                let client_hash: [u8; 32] = Sha256::digest(&canonical_s).into();
                let mut preimage = auth.to_vec();
                preimage.extend_from_slice(&client_hash);
                let nonce: [u8; 32] = Sha256::digest(&preimage).into();
                let digest: [u8; 32] = Sha256::digest(nonce).into();
                let mut repr = digest;
                repr.reverse();
                let z = Option::<P256Scalar>::from(P256Scalar::from_repr(repr))?;
                if z == P256Scalar::ZERO {
                    return None;
                }
                let x = Option::<P256Base>::from(P256Base::from_repr(repr))?;
                let rhs = x * x * x - P256Base::from(3_u64) * x + Secp256r1Affine::b();
                let y = Option::<P256Base>::from(rhs.sqrt())?;
                let r_point = Option::<Secp256r1Affine>::from(Secp256r1Affine::from_xy(x, y))?;
                let q = (r_point.to_curve() - Secp256r1Affine::generator().to_curve()).to_affine();
                (!bool::from(q.is_identity())).then_some((z, q))
            })
            .expect("some bounded test nonce yields an on-curve digest x-coordinate");

        let (qx, qy) = q.into_coordinates();
        let mut qx_be = qx.to_repr();
        let mut qy_be = qy.to_repr();
        qx_be.reverse();
        qy_be.reverse();
        let mut sec1 = [0_u8; 65];
        sec1[0] = 4;
        sec1[1..33].copy_from_slice(&qx_be);
        sec1[33..].copy_from_slice(&qy_be);
        match mutation {
            EcdsaMutation::None => {}
            EcdsaMutation::Subject => canonical_s[S_LEN - 1] ^= 1,
            EcdsaMutation::Sec1Key => sec1[64] ^= 1,
            EcdsaMutation::Counter => auth[36] = 2,
        }

        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits((TEST_K - 1) as usize);
        let range = builder.range_chip();
        let base_chip = FpChip::<F, P256Base>::new(&range, 86, 3);
        let scalar_chip = FpChip::<F, P256Scalar>::new(&range, 86, 3);
        let mut jobs = PastaSha256JobsV1::default();
        let ctx = builder.main(0);
        let signed_s: [AssignedValue<F>; S_LEN] =
            std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(canonical_s[i]))));
        let signed_auth: [AssignedValue<F>; 37] =
            std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(auth[i]))));
        let governed_rp: [AssignedValue<F>; 32] =
            std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(rp[i]))));
        let sec1_cells: [AssignedValue<F>; 65] =
            std::array::from_fn(|i| ctx.load_witness(F::from(u64::from(sec1[i]))));
        let key = EcPoint::new(
            base_chip.load_private(ctx, qx),
            base_chip.load_private(ctx, qy),
        );
        let signature_r = scalar_chip.load_private(ctx, z);
        let signature_s = scalar_chip.load_private(ctx, z);
        let prehash = scalar_chip.load_private(ctx, z);
        let previous = ctx.load_witness(F::ZERO);
        let next = ctx.load_witness(F::ONE);
        let flags = ctx.load_witness(F::from(0x40_u64));
        let digest_quotient = ctx.load_witness(F::ZERO);
        assert_apple_assertion_ecdsa::<F, S_LEN, 2, 37, false>(
            &base_chip,
            ctx,
            &mut jobs,
            &signed_s,
            &signed_auth,
            &governed_rp,
            flags,
            previous,
            next,
            &key,
            &key,
            &sec1_cells,
            &signature_r,
            &signature_s,
            &prehash,
            digest_quotient,
        )
        .expect("bounded Apple assertion queues constrained SHA and P-256 jobs");
        builder.calculate_params(Some(UNUSABLE_ROWS));
        let circuit = AppleCircuit { builder, jobs };
        MockProver::run(TEST_K, &circuit, vec![])
            .expect("bounded Apple assertion and P-256 circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn apple_assertion_sha_and_p256_equation_share_bound_cells_in_both_pasta_fields() {
        for mutation in [
            EcdsaMutation::None,
            EcdsaMutation::Subject,
            EcdsaMutation::Sec1Key,
            EcdsaMutation::Counter,
        ] {
            let valid = matches!(mutation, EcdsaMutation::None);
            assert_eq!(check_apple_hash_and_p256_equation::<Fp>(mutation), valid);
            assert_eq!(check_apple_hash_and_p256_equation::<Fq>(mutation), valid);
        }
    }

    #[test]
    fn physical_header_without_extensions_is_accepted_in_both_pasta_fields() {
        assert!(check_apple_hash_and_counter::<Fp, 37, 0x40>(Mutation::None));
        assert!(check_apple_hash_and_counter::<Fq, 37, 0x40>(Mutation::None));
        assert!(!check_apple_hash_and_counter::<Fp, 37, 0xc0>(
            Mutation::None
        ));
        assert!(!check_apple_hash_and_counter::<Fq, 37, 0xc0>(
            Mutation::None
        ));
    }

    #[test]
    #[should_panic(expected = "Core canonical selection message has a release-bounded length")]
    fn empty_core_selection_body_is_rejected() {
        const EMPTY_S_LEN: usize = DOMAIN.len() + 8;
        let mut builder = BaseCircuitBuilder::<Fp>::new(false)
            .use_k(TEST_K as usize)
            .use_lookup_bits((TEST_K - 1) as usize);
        let range = builder.range_chip();
        let base_chip = FpChip::<Fp, P256Base>::new(&range, 86, 3);
        let ctx = builder.main(0);
        let mut s = DOMAIN.to_vec();
        s.extend_from_slice(&0_u64.to_le_bytes());
        let canonical_s: [AssignedValue<Fp>; EMPTY_S_LEN] =
            std::array::from_fn(|index| ctx.load_witness(Fp::from(u64::from(s[index]))));
        let authenticator_data = std::array::from_fn(|_| ctx.load_witness(Fp::ZERO));
        let governed_rp = std::array::from_fn(|_| ctx.load_witness(Fp::ZERO));
        let previous = ctx.load_witness(Fp::ZERO);
        let next = ctx.load_witness(Fp::ONE);
        let mut jobs = PastaSha256JobsV1::default();
        let expected_flags = ctx.load_witness(Fp::ONE);
        let _ = queue_apple_assertion_digest::<Fp, EMPTY_S_LEN, 37>(
            &base_chip,
            ctx,
            &mut jobs,
            &canonical_s,
            &authenticator_data,
            &governed_rp,
            expected_flags,
            previous,
            next,
        );
    }
}
