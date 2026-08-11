//! Complete affine P-256 and ES256 verification gadgets over T256's scalar field.
//!
//! T256's scalar field is exactly the P-256 coordinate field, so curve
//! coordinates are native R1CS values. ECDSA scalars remain explicitly
//! bit-constrained below the P-256 group order.

use halo2curves::{
    ff::{Field as _, PrimeField as _},
    secp256r1::Fq as P256Scalar,
};

use super::{
    VEGA_T256_SCALAR_MODULUS_BE_V1, VegaT256ScalarV1 as Scalar,
    circuit::{Bit, CircuitBuilder, CircuitError, LinearCombination},
    sha256::{ByteVar, WordVar, allocate_bytes},
};

/// Big-endian order of the prime P-256 scalar field.
pub(super) const P256_ORDER_BE: [u8; 32] = [
    0xff, 0xff, 0xff, 0xff, 0x00, 0x00, 0x00, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xbc, 0xe6, 0xfa, 0xad, 0xa7, 0x17, 0x9e, 0x84, 0xf3, 0xb9, 0xca, 0xc2, 0xfc, 0x63, 0x25, 0x51,
];

// floor(n / 2) + 1. Requiring `s <` this constant is exactly the closed
// low-s condition `1 <= s <= floor(n / 2)`.
const P256_HALF_ORDER_PLUS_ONE_BE: [u8; 32] = [
    0x7f, 0xff, 0xff, 0xff, 0x80, 0x00, 0x00, 0x00, 0x7f, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
    0xde, 0x73, 0x7d, 0x56, 0xd3, 0x8b, 0xcf, 0x42, 0x79, 0xdc, 0xe5, 0x61, 0x7e, 0x31, 0x92, 0xa9,
];

const P256_B_BE: [u8; 32] = [
    0x5a, 0xc6, 0x35, 0xd8, 0xaa, 0x3a, 0x93, 0xe7, 0xb3, 0xeb, 0xbd, 0x55, 0x76, 0x98, 0x86, 0xbc,
    0x65, 0x1d, 0x06, 0xb0, 0xcc, 0x53, 0xb0, 0xf6, 0x3b, 0xce, 0x3c, 0x3e, 0x27, 0xd2, 0x60, 0x4b,
];

const P256_GENERATOR_X_BE: [u8; 32] = [
    0x6b, 0x17, 0xd1, 0xf2, 0xe1, 0x2c, 0x42, 0x47, 0xf8, 0xbc, 0xe6, 0xe5, 0x63, 0xa4, 0x40, 0xf2,
    0x77, 0x03, 0x7d, 0x81, 0x2d, 0xeb, 0x33, 0xa0, 0xf4, 0xa1, 0x39, 0x45, 0xd8, 0x98, 0xc2, 0x96,
];

const P256_GENERATOR_Y_BE: [u8; 32] = [
    0x4f, 0xe3, 0x42, 0xe2, 0xfe, 0x1a, 0x7f, 0x9b, 0x8e, 0xe7, 0xeb, 0x4a, 0x7c, 0x0f, 0x9e, 0x16,
    0x2b, 0xce, 0x33, 0x57, 0x6b, 0x31, 0x5e, 0xce, 0xcb, 0xb6, 0x40, 0x68, 0x37, 0xbf, 0x51, 0xf5,
];

#[derive(Clone)]
pub(super) struct P256PointVar {
    pub(super) x: LinearCombination,
    pub(super) y: LinearCombination,
    pub(super) infinity: Bit,
}

pub(super) struct ScalarBits {
    pub(super) bits_le: [Bit; 256],
}

impl ScalarBits {
    pub(super) fn lc(&self) -> LinearCombination {
        bits_to_lc(&self.bits_le)
    }
}

pub(super) fn allocate_scalar_be(
    builder: &mut CircuitBuilder,
    bytes: [u8; 32],
) -> Result<ScalarBits, CircuitError> {
    let bytes = allocate_bytes(builder, &bytes)?;
    scalar_bits_from_be_bytes(&bytes)
}

pub(super) fn scalar_bits_from_be_bytes(bytes: &[ByteVar]) -> Result<ScalarBits, CircuitError> {
    if bytes.len() != 32 {
        return Err(CircuitError::InvalidDimension);
    }
    Ok(ScalarBits {
        bits_le: core::array::from_fn(|index| {
            let byte_from_right = index / 8;
            let bit = index % 8;
            bytes[31 - byte_from_right].bits_le[bit]
        }),
    })
}

pub(super) fn digest_scalar_bits(words: [WordVar; 8]) -> ScalarBits {
    let bytes = words
        .into_iter()
        .flat_map(WordVar::to_be_bytes)
        .collect::<Vec<_>>();
    scalar_bits_from_be_bytes(&bytes).expect("eight SHA-256 words are 32 bytes")
}

pub(super) fn public_point(
    builder: &mut CircuitBuilder,
    x_index: usize,
    y_index: usize,
) -> Result<P256PointVar, CircuitError> {
    let point = P256PointVar {
        x: builder.public(x_index)?.into(),
        y: builder.public(y_index)?.into(),
        infinity: constant_bit(builder, false)?,
    };
    enforce_nonidentity_on_curve(builder, &point)?;
    Ok(point)
}

/// Allocate a public finite P-256 point and constrain the exact compressed
/// SEC1 prefix (`0x02` for even y, `0x03` for odd y).
pub(super) fn public_compressed_point(
    builder: &mut CircuitBuilder,
    x_index: usize,
    y_index: usize,
    prefix_index: usize,
) -> Result<P256PointVar, CircuitError> {
    let point = public_point(builder, x_index, y_index)?;
    let y_bits = decompose_field(builder, point.y.clone())?;
    builder.enforce_equal(
        builder.public(prefix_index)?.into(),
        LinearCombination::constant(Scalar::from_u64(2)).plus(&y_bits[0].lc()),
    )?;
    Ok(point)
}

pub(super) fn private_point_from_be_bytes(
    builder: &mut CircuitBuilder,
    x: &[ByteVar],
    y: &[ByteVar],
) -> Result<P256PointVar, CircuitError> {
    let x = scalar_bits_from_be_bytes(x)?;
    let y = scalar_bits_from_be_bytes(y)?;
    for coordinate in [&x, &y] {
        let (_, canonical) = subtract_constant(
            builder,
            &coordinate.bits_le,
            &VEGA_T256_SCALAR_MODULUS_BE_V1,
        )?;
        builder.enforce_equal(canonical.lc(), LinearCombination::one())?;
    }
    let point = P256PointVar {
        x: x.lc(),
        y: y.lc(),
        infinity: constant_bit(builder, false)?,
    };
    enforce_nonidentity_on_curve(builder, &point)?;
    Ok(point)
}

/// Verify one canonical low-s ES256 signature from its Figure 9
/// `(r, s^-1 mod n)` witness.
///
/// The unique P1363 `s` scalar is reconstructed over the P-256 group-order
/// field, allocated into the circuit, and constrained to the low-s half-order.
/// The circuit-derived recovery point `R = s^-1(H(m)G + rQ)` is then checked
/// with the independent group equation `sR = H(m)G + rQ`. A caller therefore
/// cannot substitute the high-s inverse even when bypassing native preflight.
pub(super) fn verify_es256_low_s_from_inverse(
    builder: &mut CircuitBuilder,
    message_digest: [WordVar; 8],
    public_key: &P256PointVar,
    r_be: [u8; 32],
    s_inverse_be: [u8; 32],
) -> Result<(), CircuitError> {
    let digest = digest_scalar_bits(message_digest);
    let r = allocate_scalar_be(builder, r_be)?;
    let s_inverse = allocate_scalar_be(builder, s_inverse_be)?;
    let s = allocate_low_s_scalar(builder, invert_p256_scalar_be_exact(s_inverse_be)?)?;
    enforce_nonzero_below_order(builder, &r)?;
    enforce_nonzero_below_order(builder, &s_inverse)?;

    let generator = constant_generator(builder)?;
    let digest_times_generator = scalar_mul(builder, &generator, &digest.bits_le)?;
    let r_times_key = scalar_mul(builder, public_key, &r.bits_le)?;
    let right = add_complete(builder, &digest_times_generator, &r_times_key)?;
    let recovery = scalar_mul(builder, &right, &s_inverse.bits_le)?;
    builder.enforce_zero(recovery.infinity.lc())?;
    enforce_x_mod_order_equals_r(builder, recovery.x.clone(), &r)?;
    let recomposed = scalar_mul(builder, &recovery, &s.bits_le)?;
    enforce_points_equal(builder, &recomposed, &right)
}

/// Verify one canonical low-s ES256 signature without trusting a host-side
/// modular inverse.
///
/// The private recovery point `R` is constrained on curve, constrained by
/// `x(R) mod n = r`, and used in the exact group equation
/// `sR = H(m)G + rQ`. This proves the ECDSA relation while T256's scalar field
/// remains the P-256 coordinate field rather than the P-256 group-order field.
pub(super) fn verify_es256_low_s(
    builder: &mut CircuitBuilder,
    message_digest: [WordVar; 8],
    public_key: &P256PointVar,
    r_be: [u8; 32],
    s_be: [u8; 32],
    recovery_x_be: [u8; 32],
    recovery_y_be: [u8; 32],
) -> Result<(), CircuitError> {
    let digest = digest_scalar_bits(message_digest);
    let r = allocate_scalar_be(builder, r_be)?;
    let s = allocate_low_s_scalar(builder, s_be)?;
    enforce_nonzero_below_order(builder, &r)?;

    let recovery_x = allocate_bytes(builder, &recovery_x_be)?;
    let recovery_y = allocate_bytes(builder, &recovery_y_be)?;
    let recovery = private_point_from_be_bytes(builder, &recovery_x, &recovery_y)?;
    enforce_x_mod_order_equals_r(builder, recovery.x.clone(), &r)?;

    let generator = constant_generator(builder)?;
    let digest_times_generator = scalar_mul(builder, &generator, &digest.bits_le)?;
    let r_times_key = scalar_mul(builder, public_key, &r.bits_le)?;
    let right = add_complete(builder, &digest_times_generator, &r_times_key)?;
    let left = scalar_mul(builder, &recovery, &s.bits_le)?;
    enforce_points_equal(builder, &left, &right)
}

fn allocate_low_s_scalar(
    builder: &mut CircuitBuilder,
    s_be: [u8; 32],
) -> Result<ScalarBits, CircuitError> {
    let s = allocate_scalar_be(builder, s_be)?;
    enforce_nonzero_below_order(builder, &s)?;
    let (_, is_low_s) = subtract_constant(builder, &s.bits_le, &P256_HALF_ORDER_PLUS_ONE_BE)?;
    builder.enforce_equal(is_low_s.lc(), LinearCombination::one())?;
    Ok(s)
}

fn invert_p256_scalar_be_exact(bytes: [u8; 32]) -> Result<[u8; 32], CircuitError> {
    let mut representation = bytes;
    representation.reverse();
    let scalar = Option::<P256Scalar>::from(P256Scalar::from_repr(representation.into()))
        .ok_or(CircuitError::InvalidAssignment)?;
    let inverse =
        Option::<P256Scalar>::from(scalar.invert()).ok_or(CircuitError::InvalidAssignment)?;
    let mut inverse_bytes: [u8; 32] = inverse.to_repr().into();
    inverse_bytes.reverse();
    Ok(inverse_bytes)
}

fn enforce_points_equal(
    builder: &mut CircuitBuilder,
    left: &P256PointVar,
    right: &P256PointVar,
) -> Result<(), CircuitError> {
    builder.enforce_equal(left.infinity.lc(), right.infinity.lc())?;
    builder.enforce_equal(left.x.clone(), right.x.clone())?;
    builder.enforce_equal(left.y.clone(), right.y.clone())
}

fn enforce_nonidentity_on_curve(
    builder: &mut CircuitBuilder,
    point: &P256PointVar,
) -> Result<(), CircuitError> {
    builder.enforce_zero(point.infinity.lc())?;
    let x_squared = builder.multiply(point.x.clone(), point.x.clone())?;
    let y_squared = builder.multiply(point.y.clone(), point.y.clone())?;
    builder.enforce(
        x_squared.into(),
        point.x.clone(),
        LinearCombination::from(y_squared)
            .plus(&point.x.clone().scaled(Scalar::from_u64(3)))
            .minus(&LinearCombination::constant(p256_b())),
    )
}

fn constant_generator(builder: &mut CircuitBuilder) -> Result<P256PointVar, CircuitError> {
    Ok(P256PointVar {
        x: LinearCombination::constant(
            Scalar::from_be_bytes_exact(P256_GENERATOR_X_BE)
                .expect("P-256 generator x is canonical"),
        ),
        y: LinearCombination::constant(
            Scalar::from_be_bytes_exact(P256_GENERATOR_Y_BE)
                .expect("P-256 generator y is canonical"),
        ),
        infinity: constant_bit(builder, false)?,
    })
}

fn identity(builder: &mut CircuitBuilder) -> Result<P256PointVar, CircuitError> {
    Ok(P256PointVar {
        x: LinearCombination::zero(),
        y: LinearCombination::zero(),
        infinity: constant_bit(builder, true)?,
    })
}

fn double_complete(
    builder: &mut CircuitBuilder,
    point: &P256PointVar,
) -> Result<P256PointVar, CircuitError> {
    let x_squared = builder.multiply(point.x.clone(), point.x.clone())?;
    let numerator = LinearCombination::from(x_squared)
        .scaled(Scalar::from_u64(3))
        .minus(&LinearCombination::constant(Scalar::from_u64(3)));
    let denominator = point.y.clone().scaled(Scalar::from_u64(2));
    let (denominator_zero, denominator_inverse) = builder.inverse_or_zero(denominator)?;
    builder.enforce_equal(denominator_zero.lc(), point.infinity.lc())?;
    let slope = builder.multiply(numerator, denominator_inverse.into())?;
    let slope_squared = builder.multiply(slope.into(), slope.into())?;
    let candidate_x =
        LinearCombination::from(slope_squared).minus(&point.x.clone().scaled(Scalar::from_u64(2)));
    let x_delta = point.x.clone().minus(&candidate_x);
    let candidate_y_product = builder.multiply(slope.into(), x_delta)?;
    let candidate_y = LinearCombination::from(candidate_y_product).minus(&point.y);
    let output_x = builder.select(point.infinity, LinearCombination::zero(), candidate_x)?;
    let output_y = builder.select(point.infinity, LinearCombination::zero(), candidate_y)?;
    Ok(P256PointVar {
        x: output_x.into(),
        y: output_y.into(),
        infinity: point.infinity,
    })
}

fn add_complete(
    builder: &mut CircuitBuilder,
    left: &P256PointVar,
    right: &P256PointVar,
) -> Result<P256PointVar, CircuitError> {
    let x_delta = right.x.clone().minus(&left.x);
    let y_delta = right.y.clone().minus(&left.y);
    let y_sum = right.y.clone().plus(&left.y);
    let (x_equal, x_delta_inverse) = builder.inverse_or_zero(x_delta.clone())?;
    let y_equal = builder.is_zero(y_delta.clone())?;
    let y_opposite = builder.is_zero(y_sum)?;

    let slope = builder.multiply(y_delta, x_delta_inverse.into())?;
    let slope_squared = builder.multiply(slope.into(), slope.into())?;
    let general_x = LinearCombination::from(slope_squared)
        .minus(&left.x)
        .minus(&right.x);
    let general_y_product = builder.multiply(slope.into(), left.x.clone().minus(&general_x))?;
    let general_y = LinearCombination::from(general_y_product).minus(&left.y);
    let general = P256PointVar {
        x: general_x,
        y: general_y,
        infinity: constant_bit(builder, false)?,
    };
    let doubled = double_complete(builder, left)?;

    let left_finite = builder.not(left.infinity)?;
    let right_finite = builder.not(right.infinity)?;
    let both_finite = builder.and(left_finite, right_finite)?;
    let x_different = builder.not(x_equal)?;
    let general_case = builder.and(both_finite, x_different)?;
    let same_coordinates = builder.and(x_equal, y_equal)?;
    let double_case = builder.and(both_finite, same_coordinates)?;
    let opposite_coordinates = builder.and(x_equal, y_opposite)?;
    let opposite_case = builder.and(both_finite, opposite_coordinates)?;
    let left_only = builder.and(left_finite, right.infinity)?;
    let right_only = builder.and(left.infinity, right_finite)?;
    let both_infinite = builder.and(left.infinity, right.infinity)?;
    let identity_case = builder.or(both_infinite, opposite_case)?;

    let cases = left_only
        .lc()
        .plus(&right_only.lc())
        .plus(&general_case.lc())
        .plus(&double_case.lc())
        .plus(&identity_case.lc());
    builder.enforce_equal(cases, LinearCombination::one())?;

    let mut output = identity(builder)?;
    output = select_point(builder, left_only, left, &output)?;
    output = select_point(builder, right_only, right, &output)?;
    output = select_point(builder, general_case, &general, &output)?;
    select_point(builder, double_case, &doubled, &output)
}

fn select_point(
    builder: &mut CircuitBuilder,
    condition: Bit,
    when_true: &P256PointVar,
    when_false: &P256PointVar,
) -> Result<P256PointVar, CircuitError> {
    let x = builder.select(condition, when_true.x.clone(), when_false.x.clone())?;
    let y = builder.select(condition, when_true.y.clone(), when_false.y.clone())?;
    let infinity = builder.select(condition, when_true.infinity.lc(), when_false.infinity.lc())?;
    Ok(P256PointVar {
        x: x.into(),
        y: y.into(),
        infinity: Bit { variable: infinity },
    })
}

fn scalar_mul(
    builder: &mut CircuitBuilder,
    base: &P256PointVar,
    scalar_bits_le: &[Bit],
) -> Result<P256PointVar, CircuitError> {
    if scalar_bits_le.is_empty() || scalar_bits_le.len() > 256 {
        return Err(CircuitError::InvalidDimension);
    }
    let mut accumulator = identity(builder)?;
    for bit in scalar_bits_le.iter().rev().copied() {
        accumulator = double_complete(builder, &accumulator)?;
        let with_base = add_complete(builder, &accumulator, base)?;
        accumulator = select_point(builder, bit, &with_base, &accumulator)?;
    }
    Ok(accumulator)
}

fn enforce_nonzero_below_order(
    builder: &mut CircuitBuilder,
    value: &ScalarBits,
) -> Result<(), CircuitError> {
    let (_, less_than) = subtract_constant(builder, &value.bits_le, &P256_ORDER_BE)?;
    builder.enforce_equal(less_than.lc(), LinearCombination::one())?;
    let zero = builder.is_zero(value.lc())?;
    builder.enforce_zero(zero.lc())
}

fn enforce_x_mod_order_equals_r(
    builder: &mut CircuitBuilder,
    x: LinearCombination,
    r: &ScalarBits,
) -> Result<(), CircuitError> {
    let x_bits = decompose_field(builder, x)?;
    let (difference, x_below_order) = subtract_constant(builder, &x_bits, &P256_ORDER_BE)?;
    let subtract_order = builder.not(x_below_order)?;
    for index in 0..256 {
        let remainder =
            builder.select(subtract_order, difference[index].lc(), x_bits[index].lc())?;
        builder.enforce_equal(remainder.into(), r.bits_le[index].lc())?;
    }
    Ok(())
}

fn decompose_field(
    builder: &mut CircuitBuilder,
    value: LinearCombination,
) -> Result<[Bit; 256], CircuitError> {
    let bytes = builder.evaluate(&value).to_be_bytes();
    let allocated = allocate_scalar_be(builder, bytes)?;
    builder.enforce_equal(allocated.lc(), value)?;
    Ok(allocated.bits_le)
}

/// Subtract a fixed 256-bit integer in binary. The returned final borrow is one
/// exactly when the input is strictly smaller than the constant.
fn subtract_constant(
    builder: &mut CircuitBuilder,
    value: &[Bit; 256],
    constant_be: &[u8; 32],
) -> Result<([Bit; 256], Bit), CircuitError> {
    let constant_bits = bytes_to_bits_le(constant_be);
    let mut borrow = constant_bit(builder, false)?;
    let mut difference = Vec::with_capacity(256);
    for index in 0..256 {
        let value_bit = builder.evaluate(&value[index].lc()) == Scalar::one();
        let borrow_bit = builder.evaluate(&borrow.lc()) == Scalar::one();
        let raw = i8::from(value_bit) - i8::from(constant_bits[index]) - i8::from(borrow_bit);
        let (difference_bit, next_borrow) = if raw < 0 {
            ((raw + 2) == 1, true)
        } else {
            (raw == 1, false)
        };
        let difference_variable = builder.alloc_bit(difference_bit)?;
        let next_borrow_variable = builder.alloc_bit(next_borrow)?;
        let mut equation = value[index]
            .lc()
            .minus(&borrow.lc())
            .minus(&difference_variable.lc())
            .plus(&next_borrow_variable.lc().scaled(Scalar::from_u64(2)));
        if constant_bits[index] {
            equation = equation.minus(&LinearCombination::one());
        }
        builder.enforce_zero(equation)?;
        difference.push(difference_variable);
        borrow = next_borrow_variable;
    }
    Ok((
        difference
            .try_into()
            .map_err(|_| CircuitError::InvalidDimension)?,
        borrow,
    ))
}

fn bits_to_lc(bits: &[Bit]) -> LinearCombination {
    let mut coefficient = Scalar::one();
    let mut result = LinearCombination::zero();
    for bit in bits {
        result = result.add_term(bit.variable(), coefficient);
        coefficient += coefficient;
    }
    result
}

fn bytes_to_bits_le(bytes: &[u8; 32]) -> [bool; 256] {
    core::array::from_fn(|index| {
        let byte_from_right = index / 8;
        bytes[31 - byte_from_right] & (1 << (index % 8)) != 0
    })
}

fn constant_bit(builder: &mut CircuitBuilder, value: bool) -> Result<Bit, CircuitError> {
    let bit = builder.alloc_bit(value)?;
    builder.enforce_equal(
        bit.lc(),
        LinearCombination::constant(Scalar::from_u64(u64::from(value))),
    )?;
    Ok(bit)
}

fn p256_b() -> Scalar {
    Scalar::from_be_bytes_exact(P256_B_BE).expect("P-256 b is canonical")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::vega::sha256::sha256;

    fn scalar_bits_small(builder: &mut CircuitBuilder, value: u8, width: usize) -> Vec<Bit> {
        (0..width)
            .map(|bit| builder.alloc_bit(value & (1 << bit) != 0).expect("bit"))
            .collect()
    }

    fn hex32(value: &str) -> [u8; 32] {
        hex::decode(value)
            .expect("hex")
            .try_into()
            .expect("32 bytes")
    }

    #[test]
    fn complete_scalar_multiplication_matches_independent_p256_vector() {
        // 7 * G, independently generated with OpenSSL/P-256.
        let expected_x = Scalar::from_be_bytes_exact(
            hex::decode("8e533b6fa0bf7b4625bb30667c01fb607ef9f8b8a80fef5b300628703187b2a3")
                .expect("hex")
                .try_into()
                .expect("coordinate"),
        )
        .expect("canonical");
        let expected_y = Scalar::from_be_bytes_exact(
            hex::decode("73eb1dbde03318366d069f83a6f5900053c73633cb041b21c55e1a86c1f400b4")
                .expect("hex")
                .try_into()
                .expect("coordinate"),
        )
        .expect("canonical");
        let mut builder = CircuitBuilder::new(vec![Scalar::one()]).expect("public");
        let generator = constant_generator(&mut builder).expect("generator");
        enforce_nonidentity_on_curve(&mut builder, &generator).expect("on curve");
        let bits = scalar_bits_small(&mut builder, 7, 3);
        let result = scalar_mul(&mut builder, &generator, &bits).expect("scalar mul");
        builder.enforce_zero(result.infinity.lc()).expect("finite");
        builder
            .enforce_equal(result.x, LinearCombination::constant(expected_x))
            .expect("x");
        builder
            .enforce_equal(result.y, LinearCombination::constant(expected_y))
            .expect("y");
        let assignment = builder.finalize().expect("shape");
        assignment
            .shape
            .validate_strict_assignment(&assignment.witness, &assignment.public_inputs)
            .expect("satisfying trace");
    }

    #[test]
    fn order_comparator_rejects_zero_order_and_above_order() {
        for (bytes, accepted) in [
            ([0_u8; 32], false),
            (
                {
                    let mut one = [0_u8; 32];
                    one[31] = 1;
                    one
                },
                true,
            ),
            (P256_ORDER_BE, false),
            ([0xff_u8; 32], false),
        ] {
            let mut builder = CircuitBuilder::new(vec![Scalar::one()]).expect("public");
            let bits = allocate_scalar_be(&mut builder, bytes).expect("bits");
            let result = enforce_nonzero_below_order(&mut builder, &bits);
            let assignment = builder.finalize().expect("shape");
            let satisfied = assignment
                .shape
                .validate_strict_assignment(&assignment.witness, &assignment.public_inputs)
                .is_ok();
            assert_eq!(result.is_ok() && satisfied, accepted);
        }
    }

    #[test]
    fn es256_verifier_accepts_low_s_and_rejects_r_and_high_s_mutations() {
        let qx = hex32("34c30b0b65edb2cfa5f65b122d53b7e095799a0a3b61c1dda5bcce3bd49aa1a7");
        let qy = hex32("a500bc7ee963713fbc76056b2c7090a3a5b76592af2d6dfcddb7dd2cb35a982e");
        let r = hex32("fd890a23bd79ca4428776a1785a6423203c2620148c096624c2008c191f7c053");
        let s_inverse = hex32("5e5782fe1833e0abeb20dc336de6123cde1ca1a4f51de133a6cb224c1bc071d4");
        let mut high_s_inverse_representation = s_inverse;
        high_s_inverse_representation.reverse();
        let high_s_inverse =
            Option::<P256Scalar>::from(P256Scalar::from_repr(high_s_inverse_representation.into()))
                .map(|value| -value)
                .map(|value| {
                    let mut bytes: [u8; 32] = value.to_repr().into();
                    bytes.reverse();
                    bytes
                })
                .expect("nonzero canonical P-256 scalar");
        let mut changed_r = r;
        changed_r[31] ^= 1;

        for (candidate_r, candidate_s_inverse, accepted) in [
            (r, s_inverse, true),
            (changed_r, s_inverse, false),
            (r, high_s_inverse, false),
        ] {
            let public = vec![
                Scalar::from_be_bytes_exact(qx).expect("qx"),
                Scalar::from_be_bytes_exact(qy).expect("qy"),
            ];
            let mut builder = CircuitBuilder::new(public).expect("public");
            let message =
                allocate_bytes(&mut builder, b"Vega ES256 circuit KAT").expect("message bits");
            let digest = sha256(&mut builder, &message).expect("SHA-256");
            let key = public_point(&mut builder, 0, 1).expect("public key");
            verify_es256_low_s_from_inverse(
                &mut builder,
                digest,
                &key,
                candidate_r,
                candidate_s_inverse,
            )
            .expect("fixed-shape synthesis");
            let assignment = builder.finalize().expect("shape");
            let satisfied = assignment
                .shape
                .validate_strict_assignment(&assignment.witness, &assignment.public_inputs)
                .is_ok();
            assert_eq!(satisfied, accepted);
        }
    }
}
