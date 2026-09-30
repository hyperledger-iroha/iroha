// Shared SHA-256 residue helpers for the IVM private-note and PQ-MASP STARK relations.
// Both engines include this file textually, so every name resolves in the including
// engine's scope.
fn f(value: impl Into<u64>) -> F {
    F(value.into())
}
fn set(columns: &mut [Vec<F>], column: usize, row: usize, value: F) {
    columns[column][row] = value;
}
fn boolean(value: F) -> F {
    value.mul(value.sub(F::ONE))
}
fn pack_bits(bits: &[F]) -> F {
    bits.iter()
        .copied()
        .enumerate()
        .fold(F::ZERO, |sum, (bit, value)| {
            sum.add(value.mul(F(1_u64 << bit)))
        })
}
fn xor_three(x: F, y: F, z: F) -> F {
    x.add(y)
        .add(z)
        .sub(F(2).mul(x.mul(y).add(x.mul(z)).add(y.mul(z))))
        .add(F(4).mul(x.mul(y).mul(z)))
}
fn choose(x: F, y: F, z: F) -> F {
    x.mul(y).add(F::ONE.sub(x).mul(z))
}
fn majority(x: F, y: F, z: F) -> F {
    x.mul(y)
        .add(x.mul(z))
        .add(y.mul(z))
        .sub(F(2).mul(x.mul(y).mul(z)))
}
fn bit_at(bits: &[F], index: usize) -> F {
    bits[index % 32]
}
fn rotr(bits: &[F], shift: usize, index: usize) -> F {
    bit_at(bits, index + shift)
}
fn shr(bits: &[F], shift: usize, index: usize) -> F {
    if index + shift < 32 {
        bits[index + shift]
    } else {
        F::ZERO
    }
}
fn sigma_small_0_bits(bits: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(
            xor_three(
                rotr(bits, 7, index),
                rotr(bits, 18, index),
                shr(bits, 3, index),
            )
            .mul(F(1_u64 << index)),
        )
    })
}
fn sigma_small_1_bits(bits: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(
            xor_three(
                rotr(bits, 17, index),
                rotr(bits, 19, index),
                shr(bits, 10, index),
            )
            .mul(F(1_u64 << index)),
        )
    })
}
fn sigma_big_0_bits(bits: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(
            xor_three(
                rotr(bits, 2, index),
                rotr(bits, 13, index),
                rotr(bits, 22, index),
            )
            .mul(F(1_u64 << index)),
        )
    })
}
fn sigma_big_1_bits(bits: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(
            xor_three(
                rotr(bits, 6, index),
                rotr(bits, 11, index),
                rotr(bits, 25, index),
            )
            .mul(F(1_u64 << index)),
        )
    })
}
fn choose_word(e: &[F], f_bits: &[F], g: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(choose(e[index], f_bits[index], g[index]).mul(F(1_u64 << index)))
    })
}
fn majority_word(a: &[F], b: &[F], c: &[F]) -> F {
    (0..32).fold(F::ZERO, |sum, index| {
        sum.add(majority(a[index], b[index], c[index]).mul(F(1_u64 << index)))
    })
}
fn selector_sum(fixed: &[F], range: core::ops::Range<usize>) -> F {
    fixed[range].iter().copied().fold(F::ZERO, F::add)
}
fn selected_schedule(current: &[F], fixed: &[F], index: impl Fn(usize) -> Option<usize>) -> F {
    (0..64).fold(F::ZERO, |sum, round| {
        let Some(schedule_index) = index(round) else {
            return sum;
        };
        sum.add(
            fixed[FIXED_ROUND_SELECTOR_OFFSET + round]
                .mul(current[SHA_SCHEDULE_OFFSET + schedule_index]),
        )
    })
}
fn selected_round_constant(fixed: &[F]) -> F {
    (0..64).fold(F::ZERO, |sum, round| {
        sum.add(
            fixed[FIXED_ROUND_SELECTOR_OFFSET + round]
                .mul(F(u64::from(SHA256_ROUND_CONSTANTS_V1[round]))),
        )
    })
}
fn push_weighted(residues: &mut Vec<F>, selector: F, value: F) {
    residues.push(selector.mul(value));
}
fn push_boolean(residues: &mut Vec<F>, selector: F, value: F) {
    push_weighted(residues, selector, boolean(value));
}
