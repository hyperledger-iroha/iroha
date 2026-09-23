//! Fixed-topology, canonical positive DER encoding of the exact P-256 ECDSA scalar cells.
//!
//! This only binds DER to assigned `r,s`; the caller must pass the same cells to the
//! ECDSA verifier and bind these DER bytes to the original assertion CBOR.

use halo2_base::{
    AssignedValue, Context,
    QuantumCell::Constant,
    gates::{GateInstructions as _, RangeInstructions as _},
    utils::BigPrimeField,
};
use halo2_ecc::{
    bigint::ProperCrtUint,
    fields::{FieldChip as _, fp::FpChip},
};
use halo2_proofs::halo2curves::secp256r1::Fp as P256Base;

use super::{PastaSha256ByteV1, p256_uint_bits_le};

/// The only positive, minimally encoded DER sequence for one assigned `(r,s)` pair.
#[derive(Clone)]
pub(crate) struct P256CanonicalDerV1<F: BigPrimeField> {
    /// Complete fixed-capacity DER buffer; every byte after `len` is zero by construction.
    pub(crate) bytes: [PastaSha256ByteV1<F>; 72],
    /// Exact active byte count, in `8..=72` for nonzero scalars.
    pub(crate) len: AssignedValue<F>,
}

struct CanonicalIntegerV1<F: BigPrimeField> {
    bytes: [PastaSha256ByteV1<F>; 33],
    len: AssignedValue<F>,
    length_one_hot: [AssignedValue<F>; 34],
}

fn canonical_positive_integer_v1<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    value: &ProperCrtUint<F>,
) -> CanonicalIntegerV1<F> {
    let gate = chip.gate();
    let range = chip.range();
    let bits = p256_uint_bits_le(chip, ctx, value);
    let bytes_be: [PastaSha256ByteV1<F>; 32] = std::array::from_fn(|index| {
        let first_bit = (31 - index) * 8;
        let byte = gate.inner_product(
            ctx,
            bits[first_bit..first_bit + 8].iter().copied(),
            (0..8).map(|bit| Constant(F::from(1_u64 << bit))),
        );
        PastaSha256ByteV1::range_checked(ctx, range, byte)
    });
    let mut all_zero_so_far = ctx.load_constant(F::ONE);
    let mut no_sign = Vec::with_capacity(32);
    let mut sign = Vec::with_capacity(32);
    let mut length_one_hot = [ctx.load_constant(F::ZERO); 34];
    for (index, byte) in bytes_be.iter().enumerate() {
        let is_zero = gate.is_zero(ctx, byte.assigned().expect("integer byte assigned"));
        let nonzero = gate.not(ctx, is_zero);
        let first = gate.and(ctx, all_zero_so_far, nonzero);
        all_zero_so_far = gate.and(ctx, all_zero_so_far, is_zero);
        let sign_bit = bits[(31 - index) * 8 + 7];
        let with_sign = gate.and(ctx, first, sign_bit);
        let no_sign_bit = gate.not(ctx, sign_bit);
        let without_sign = gate.and(ctx, first, no_sign_bit);
        no_sign.push(without_sign);
        sign.push(with_sign);
        let ordinary_len = 32 - index;
        length_one_hot[ordinary_len] = gate.add(ctx, length_one_hot[ordinary_len], without_sign);
        length_one_hot[ordinary_len + 1] =
            gate.add(ctx, length_one_hot[ordinary_len + 1], with_sign);
    }
    // The ECDSA verifier also enforces nonzero. Repeating it here prevents a DER-only
    // caller from accepting zero with a fabricated zero-length integer.
    gate.assert_is_const(ctx, &all_zero_so_far, &F::ZERO);
    let exactly_one = gate.sum(ctx, length_one_hot[1..].iter().copied());
    gate.assert_is_const(ctx, &exactly_one, &F::ONE);
    let len = gate.inner_product(
        ctx,
        length_one_hot[1..].iter().copied(),
        (1..=33).map(|width| Constant(F::from(width as u64))),
    );
    range.range_check(ctx, len, 6);
    let bytes = std::array::from_fn(|output| {
        let mut terms = Vec::with_capacity(64);
        for first in 0..32 {
            if first + output < 32 {
                terms.push(gate.mul(ctx, no_sign[first], bytes_be[first + output].quantum_cell()));
            }
            if output != 0 && first + output - 1 < 32 {
                terms.push(gate.mul(
                    ctx,
                    sign[first],
                    bytes_be[first + output - 1].quantum_cell(),
                ));
            }
        }
        let value = gate.sum(ctx, terms);
        PastaSha256ByteV1::range_checked(ctx, range, value)
    });
    CanonicalIntegerV1 {
        bytes,
        len,
        length_one_hot,
    }
}

/// Derive the unique canonical DER sequence from the same assigned `r,s` used by ECDSA.
///
/// The `ProperCrtUint` limbs are decomposed into 256 constrained bits. One-hot first-nonzero
/// selectors and the selected high bit determine both integer lengths and mandatory sign
/// padding. Every dynamic shift is expressed by fixed-topology selector gates.
pub(crate) fn constrain_p256_canonical_der_v1<F: BigPrimeField>(
    chip: &FpChip<'_, F, P256Base>,
    ctx: &mut Context<F>,
    r: &ProperCrtUint<F>,
    s: &ProperCrtUint<F>,
) -> P256CanonicalDerV1<F> {
    let gate = chip.gate();
    let range = chip.range();
    let r = canonical_positive_integer_v1(chip, ctx, r);
    let s = canonical_positive_integer_v1(chip, ctx, s);
    let both_integer_lengths = gate.add(ctx, r.len, s.len);
    let content_len = gate.add(ctx, both_integer_lengths, Constant(F::from(4_u64)));
    let len = gate.add(ctx, content_len, Constant(F::from(2_u64)));
    range.range_check(ctx, content_len, 7);
    range.range_check(ctx, len, 7);
    let bytes = std::array::from_fn(|offset| match offset {
        0 => PastaSha256ByteV1::constant(0x30),
        1 => PastaSha256ByteV1::range_checked(ctx, range, content_len),
        2 => PastaSha256ByteV1::constant(0x02),
        3 => PastaSha256ByteV1::range_checked(ctx, range, r.len),
        _ => {
            let mut terms = Vec::with_capacity(35);
            if (4..37).contains(&offset) {
                terms.push(r.bytes[offset - 4].assigned().expect("R byte assigned"));
            }
            for r_len in 1..=33 {
                let selected = r.length_one_hot[r_len];
                if offset == 4 + r_len {
                    terms.push(gate.mul(ctx, selected, Constant(F::from(2_u64))));
                }
                if offset == 5 + r_len {
                    terms.push(gate.mul(ctx, selected, s.len));
                }
                if (6 + r_len..39 + r_len).contains(&offset) {
                    terms.push(gate.mul(ctx, selected, s.bytes[offset - 6 - r_len].quantum_cell()));
                }
            }
            let value = gate.sum(ctx, terms);
            PastaSha256ByteV1::range_checked(ctx, range, value)
        }
    });
    P256CanonicalDerV1 { bytes, len }
}

#[cfg(test)]
mod tests {
    use super::*;
    use halo2_base::{gates::circuit::builder::BaseCircuitBuilder, utils::modulus};
    use halo2_ecc::bigint::FixedCRTInteger;
    use halo2_proofs::{
        dev::MockProver,
        halo2curves::pasta::{Fp, Fq},
    };

    fn expected_integer(mode: u8) -> Vec<u8> {
        let order = modulus::<super::super::P256Scalar>();
        let value = match mode {
            0 => &order >> 255,
            1 => &order >> 9,
            2 => &order >> 1,
            3 => &order - 1_u32,
            4 => &order >> 256,
            _ => unreachable!("test scalar mode"),
        };
        let mut bytes = value.to_bytes_be();
        if bytes.is_empty() {
            bytes.push(0);
        }
        if bytes[0] & 0x80 != 0 {
            bytes.insert(0, 0);
        }
        bytes
    }

    fn expected_der(r_mode: u8, s_mode: u8) -> Vec<u8> {
        let r = expected_integer(r_mode);
        let s = expected_integer(s_mode);
        let mut der = vec![0x30, (4 + r.len() + s.len()) as u8, 0x02, r.len() as u8];
        der.extend(r);
        der.extend([0x02, s.len() as u8]);
        der.extend(s);
        der
    }

    fn check<F: BigPrimeField>(r_mode: u8, s_mode: u8, mutation: Option<(usize, u8)>) -> bool {
        let mut expected = expected_der(r_mode, s_mode);
        if let Some((at, xor)) = mutation {
            expected[at] ^= xor;
        }
        let order = modulus::<super::super::P256Scalar>();
        let value = |mode| match mode {
            0 => &order >> 255,
            1 => &order >> 9,
            2 => &order >> 1,
            3 => &order - 1_u32,
            4 => &order >> 256,
            _ => unreachable!("test scalar mode"),
        };
        let mut builder = BaseCircuitBuilder::<F>::new(false)
            .use_k(16)
            .use_lookup_bits(15)
            .use_instance_columns(1);
        let range = builder.range_chip();
        let chip = FpChip::<F, P256Base>::new(&range, 86, 3);
        let ctx = builder.main(0);
        let r = FixedCRTInteger::from_native(value(r_mode), 3, 86).assign(ctx, 86, &modulus::<F>());
        let s = FixedCRTInteger::from_native(value(s_mode), 3, 86).assign(ctx, 86, &modulus::<F>());
        let der = constrain_p256_canonical_der_v1(&chip, ctx, &r, &s);
        chip.gate()
            .assert_is_const(ctx, &der.len, &F::from(expected.len() as u64));
        for (offset, actual) in der.bytes.iter().enumerate() {
            let byte = expected.get(offset).copied().unwrap_or(0);
            let actual = chip
                .gate()
                .add(ctx, actual.quantum_cell(), Constant(F::ZERO));
            chip.gate()
                .assert_is_const(ctx, &actual, &F::from(u64::from(byte)));
        }
        builder.assigned_instances = vec![Vec::new()];
        builder.calculate_params(Some(9));
        MockProver::run(16, &builder, vec![Vec::new()])
            .expect("canonical DER circuit synthesizes")
            .verify()
            .is_ok()
    }

    #[test]
    fn canonical_der_lengths_and_sign_padding_in_both_pasta_fields() {
        for (r, s, width) in [(0, 0, 8), (1, 2, 69), (2, 2, 70), (3, 2, 71), (3, 3, 72)] {
            assert_eq!(expected_der(r, s).len(), width);
            assert!(check::<Fp>(r, s, None));
            assert!(check::<Fq>(r, s, None));
        }
    }

    #[test]
    fn mutated_or_zero_der_integer_is_rejected_in_both_pasta_fields() {
        for field in [0, 1, 2, 3, 4, 5, 6, 7] {
            assert!(!check::<Fp>(0, 0, Some((field, 1))));
            assert!(!check::<Fq>(0, 0, Some((field, 1))));
        }
        assert!(!check::<Fp>(4, 0, None));
        assert!(!check::<Fq>(4, 0, None));
        assert!(!check::<Fp>(3, 2, Some((4, 1))));
        assert!(!check::<Fq>(3, 2, Some((4, 1))));
    }
}
