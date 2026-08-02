//! Bit-constrained SHA-256 gadget for the fixed Figure 9 byte strings.

use super::{
    VegaT256ScalarV1 as Scalar,
    circuit::{Bit, CircuitBuilder, CircuitError, LinearCombination},
};

const IV: [u32; 8] = [
    0x6a09_e667,
    0xbb67_ae85,
    0x3c6e_f372,
    0xa54f_f53a,
    0x510e_527f,
    0x9b05_688c,
    0x1f83_d9ab,
    0x5be0_cd19,
];

const ROUND_CONSTANTS: [u32; 64] = [
    0x428a_2f98,
    0x7137_4491,
    0xb5c0_fbcf,
    0xe9b5_dba5,
    0x3956_c25b,
    0x59f1_11f1,
    0x923f_82a4,
    0xab1c_5ed5,
    0xd807_aa98,
    0x1283_5b01,
    0x2431_85be,
    0x550c_7dc3,
    0x72be_5d74,
    0x80de_b1fe,
    0x9bdc_06a7,
    0xc19b_f174,
    0xe49b_69c1,
    0xefbe_4786,
    0x0fc1_9dc6,
    0x240c_a1cc,
    0x2de9_2c6f,
    0x4a74_84aa,
    0x5cb0_a9dc,
    0x76f9_88da,
    0x983e_5152,
    0xa831_c66d,
    0xb003_27c8,
    0xbf59_7fc7,
    0xc6e0_0bf3,
    0xd5a7_9147,
    0x06ca_6351,
    0x1429_2967,
    0x27b7_0a85,
    0x2e1b_2138,
    0x4d2c_6dfc,
    0x5338_0d13,
    0x650a_7354,
    0x766a_0abb,
    0x81c2_c92e,
    0x9272_2c85,
    0xa2bf_e8a1,
    0xa81a_664b,
    0xc24b_8b70,
    0xc76c_51a3,
    0xd192_e819,
    0xd699_0624,
    0xf40e_3585,
    0x106a_a070,
    0x19a4_c116,
    0x1e37_6c08,
    0x2748_774c,
    0x34b0_bcb5,
    0x391c_0cb3,
    0x4ed8_aa4a,
    0x5b9c_ca4f,
    0x682e_6ff3,
    0x748f_82ee,
    0x78a5_636f,
    0x84c8_7814,
    0x8cc7_0208,
    0x90be_fffa,
    0xa450_6ceb,
    0xbef9_a3f7,
    0xc671_78f2,
];

#[derive(Clone, Copy, Debug)]
pub(super) struct ByteVar {
    pub(super) bits_le: [Bit; 8],
}

impl ByteVar {
    pub(super) fn lc(self) -> LinearCombination {
        bits_to_lc(&self.bits_le)
    }

    pub(super) fn bits_le(self) -> [Bit; 8] {
        self.bits_le
    }
}

#[derive(Clone, Copy)]
pub(super) struct WordVar {
    bits_le: [Bit; 32],
}

impl WordVar {
    pub(super) fn lc(self) -> LinearCombination {
        bits_to_lc(&self.bits_le)
    }

    pub(super) fn bits_le(self) -> [Bit; 32] {
        self.bits_le
    }

    pub(super) fn to_be_bytes(self) -> [ByteVar; 4] {
        core::array::from_fn(|byte| {
            let source = 3 - byte;
            ByteVar {
                bits_le: core::array::from_fn(|bit| self.bits_le[source * 8 + bit]),
            }
        })
    }
}

#[derive(Clone)]
pub(super) struct Sha256Trace {
    pub(super) states_after_blocks: Vec<[WordVar; 8]>,
}

pub(super) fn allocate_bytes(
    builder: &mut CircuitBuilder,
    bytes: &[u8],
) -> Result<Vec<ByteVar>, CircuitError> {
    bytes
        .iter()
        .copied()
        .map(|byte| allocate_byte(builder, byte))
        .collect()
}

pub(super) fn allocate_byte(
    builder: &mut CircuitBuilder,
    byte: u8,
) -> Result<ByteVar, CircuitError> {
    let bits = (0..8)
        .map(|bit| builder.alloc_bit(byte & (1 << bit) != 0))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ByteVar {
        bits_le: bits
            .try_into()
            .map_err(|_| CircuitError::InvalidDimension)?,
    })
}

pub(super) fn enforce_byte_constant(
    builder: &mut CircuitBuilder,
    byte: ByteVar,
    expected: u8,
) -> Result<(), CircuitError> {
    builder.enforce_equal(
        byte.lc(),
        LinearCombination::constant(Scalar::from_u64(u64::from(expected))),
    )
}

/// Bit-decompose one public input as an exact unsigned 32-bit word.
pub(super) fn public_word(
    builder: &mut CircuitBuilder,
    public_input_index: usize,
) -> Result<WordVar, CircuitError> {
    let public = builder.public(public_input_index)?;
    let value = u32::try_from(scalar_to_u64(builder.evaluate(&public.into()))?)
        .map_err(|_| CircuitError::InvalidAssignment)?;
    let word = allocate_word(builder, value)?;
    builder.enforce_equal(word.lc(), public.into())?;
    Ok(word)
}

pub(super) fn sha256(
    builder: &mut CircuitBuilder,
    message: &[ByteVar],
) -> Result<[WordVar; 8], CircuitError> {
    sha256_with_trace(builder, message).map(|(digest, _)| digest)
}

pub(super) fn sha256_with_trace(
    builder: &mut CircuitBuilder,
    message: &[ByteVar],
) -> Result<([WordVar; 8], Sha256Trace), CircuitError> {
    let bit_length = u64::try_from(message.len())
        .map_err(|_| CircuitError::InvalidDimension)?
        .checked_mul(8)
        .ok_or(CircuitError::InvalidDimension)?;
    let padded_len = message
        .len()
        .checked_add(9)
        .and_then(|value| value.checked_add(63))
        .map(|value| value / 64 * 64)
        .ok_or(CircuitError::InvalidDimension)?;
    let mut padded = Vec::with_capacity(padded_len);
    padded.extend_from_slice(message);
    padded.push(allocate_constant_byte(builder, 0x80)?);
    while padded.len() + 8 < padded_len {
        padded.push(allocate_constant_byte(builder, 0)?);
    }
    for byte in bit_length.to_be_bytes() {
        padded.push(allocate_constant_byte(builder, byte)?);
    }
    if padded.len() != padded_len {
        return Err(CircuitError::InvalidDimension);
    }

    let mut state = IV
        .into_iter()
        .map(|word| allocate_constant_word(builder, word))
        .collect::<Result<Vec<_>, _>>()?;
    let mut states_after_blocks = Vec::with_capacity(padded_len / 64);
    for block in padded.chunks_exact(64) {
        let mut schedule = Vec::with_capacity(64);
        for word_bytes in block.chunks_exact(4) {
            schedule.push(word_from_be_bytes(word_bytes)?);
        }
        for index in 16..64 {
            let sigma_zero = small_sigma_zero(builder, schedule[index - 15])?;
            let sigma_one = small_sigma_one(builder, schedule[index - 2])?;
            schedule.push(add_mod_32(
                builder,
                &[
                    schedule[index - 16].lc(),
                    sigma_zero.lc(),
                    schedule[index - 7].lc(),
                    sigma_one.lc(),
                ],
                2,
            )?);
        }

        let original = state.clone();
        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];
        let mut e = state[4];
        let mut f = state[5];
        let mut g = state[6];
        let mut h = state[7];
        for round in 0..64 {
            let sigma_one = big_sigma_one(builder, e)?;
            let choice = choice(builder, e, f, g)?;
            let temp_one = add_mod_32(
                builder,
                &[
                    h.lc(),
                    sigma_one.lc(),
                    choice.lc(),
                    LinearCombination::constant(Scalar::from_u64(u64::from(
                        ROUND_CONSTANTS[round],
                    ))),
                    schedule[round].lc(),
                ],
                3,
            )?;
            let sigma_zero = big_sigma_zero(builder, a)?;
            let majority = majority(builder, a, b, c)?;
            let temp_two = add_mod_32(builder, &[sigma_zero.lc(), majority.lc()], 1)?;
            let new_e = add_mod_32(builder, &[d.lc(), temp_one.lc()], 1)?;
            let new_a = add_mod_32(builder, &[temp_one.lc(), temp_two.lc()], 1)?;

            h = g;
            g = f;
            f = e;
            e = new_e;
            d = c;
            c = b;
            b = a;
            a = new_a;
        }
        state = vec![a, b, c, d, e, f, g, h]
            .into_iter()
            .zip(original)
            .map(|(working, initial)| add_mod_32(builder, &[working.lc(), initial.lc()], 1))
            .collect::<Result<Vec<_>, _>>()?;
        states_after_blocks.push(
            state
                .clone()
                .try_into()
                .map_err(|_| CircuitError::InvalidDimension)?,
        );
    }
    let digest = state
        .try_into()
        .map_err(|_| CircuitError::InvalidDimension)?;
    Ok((
        digest,
        Sha256Trace {
            states_after_blocks,
        },
    ))
}

fn allocate_word(builder: &mut CircuitBuilder, word: u32) -> Result<WordVar, CircuitError> {
    let bits = (0..32)
        .map(|bit| builder.alloc_bit(word & (1 << bit) != 0))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(WordVar {
        bits_le: bits
            .try_into()
            .map_err(|_| CircuitError::InvalidDimension)?,
    })
}

fn allocate_constant_byte(builder: &mut CircuitBuilder, byte: u8) -> Result<ByteVar, CircuitError> {
    let allocated = allocate_byte(builder, byte)?;
    enforce_byte_constant(builder, allocated, byte)?;
    Ok(allocated)
}

fn allocate_constant_word(
    builder: &mut CircuitBuilder,
    word: u32,
) -> Result<WordVar, CircuitError> {
    let allocated = allocate_word(builder, word)?;
    builder.enforce_equal(
        allocated.lc(),
        LinearCombination::constant(Scalar::from_u64(u64::from(word))),
    )?;
    Ok(allocated)
}

fn word_from_be_bytes(bytes: &[ByteVar]) -> Result<WordVar, CircuitError> {
    if bytes.len() != 4 {
        return Err(CircuitError::InvalidDimension);
    }
    Ok(WordVar {
        bits_le: core::array::from_fn(|index| {
            let byte_from_right = index / 8;
            let bit = index % 8;
            bytes[3 - byte_from_right].bits_le[bit]
        }),
    })
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

fn scalar_to_u64(value: Scalar) -> Result<u64, CircuitError> {
    let bytes = value.to_be_bytes();
    if bytes[..24].iter().any(|byte| *byte != 0) {
        return Err(CircuitError::InvalidAssignment);
    }
    Ok(u64::from_be_bytes(
        bytes[24..]
            .try_into()
            .map_err(|_| CircuitError::InvalidAssignment)?,
    ))
}

fn add_mod_32(
    builder: &mut CircuitBuilder,
    inputs: &[LinearCombination],
    carry_bits: usize,
) -> Result<WordVar, CircuitError> {
    if inputs.is_empty() || carry_bits > 3 {
        return Err(CircuitError::InvalidDimension);
    }
    let sum = inputs.iter().try_fold(0_u64, |sum, input| {
        sum.checked_add(scalar_to_u64(builder.evaluate(input))?)
            .ok_or(CircuitError::InvalidAssignment)
    })?;
    let output = allocate_word(builder, sum as u32)?;
    let carry = sum >> 32;
    if carry >= (1_u64 << carry_bits) {
        return Err(CircuitError::InvalidAssignment);
    }
    let carry_variables = (0..carry_bits)
        .map(|bit| builder.alloc_bit(carry & (1 << bit) != 0))
        .collect::<Result<Vec<_>, _>>()?;
    let mut left = LinearCombination::zero();
    for input in inputs {
        left = left.plus(input);
    }
    let mut carry_lc = LinearCombination::zero();
    let mut coefficient = Scalar::from_u64(1_u64 << 32);
    for bit in carry_variables {
        carry_lc = carry_lc.add_term(bit.variable(), coefficient);
        coefficient += coefficient;
    }
    builder.enforce_equal(left, output.lc().plus(&carry_lc))?;
    Ok(output)
}

fn rotate_right(word: WordVar, distance: usize) -> WordVar {
    WordVar {
        bits_le: core::array::from_fn(|index| word.bits_le[(index + distance) % 32]),
    }
}

fn shift_right(
    builder: &mut CircuitBuilder,
    word: WordVar,
    distance: usize,
) -> Result<WordVar, CircuitError> {
    let zero = builder.alloc_bit(false)?;
    builder.enforce_zero(zero.lc())?;
    Ok(WordVar {
        bits_le: core::array::from_fn(|index| {
            word.bits_le.get(index + distance).copied().unwrap_or(zero)
        }),
    })
}

fn xor_words(
    builder: &mut CircuitBuilder,
    left: WordVar,
    right: WordVar,
) -> Result<WordVar, CircuitError> {
    let bits = left
        .bits_le
        .into_iter()
        .zip(right.bits_le)
        .map(|(left, right)| builder.xor(left, right))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(WordVar {
        bits_le: bits
            .try_into()
            .map_err(|_| CircuitError::InvalidDimension)?,
    })
}

fn xor_three_words(
    builder: &mut CircuitBuilder,
    first: WordVar,
    second: WordVar,
    third: WordVar,
) -> Result<WordVar, CircuitError> {
    let first_two = xor_words(builder, first, second)?;
    xor_words(builder, first_two, third)
}

fn small_sigma_zero(builder: &mut CircuitBuilder, word: WordVar) -> Result<WordVar, CircuitError> {
    let shifted = shift_right(builder, word, 3)?;
    xor_three_words(
        builder,
        rotate_right(word, 7),
        rotate_right(word, 18),
        shifted,
    )
}

fn small_sigma_one(builder: &mut CircuitBuilder, word: WordVar) -> Result<WordVar, CircuitError> {
    let shifted = shift_right(builder, word, 10)?;
    xor_three_words(
        builder,
        rotate_right(word, 17),
        rotate_right(word, 19),
        shifted,
    )
}

fn big_sigma_zero(builder: &mut CircuitBuilder, word: WordVar) -> Result<WordVar, CircuitError> {
    xor_three_words(
        builder,
        rotate_right(word, 2),
        rotate_right(word, 13),
        rotate_right(word, 22),
    )
}

fn big_sigma_one(builder: &mut CircuitBuilder, word: WordVar) -> Result<WordVar, CircuitError> {
    xor_three_words(
        builder,
        rotate_right(word, 6),
        rotate_right(word, 11),
        rotate_right(word, 25),
    )
}

fn choice(
    builder: &mut CircuitBuilder,
    x: WordVar,
    y: WordVar,
    z: WordVar,
) -> Result<WordVar, CircuitError> {
    let bits = x
        .bits_le
        .into_iter()
        .zip(y.bits_le)
        .zip(z.bits_le)
        .map(|((x, y), z)| {
            builder
                .select(x, y.lc(), z.lc())
                .map(|variable| Bit { variable })
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(WordVar {
        bits_le: bits
            .try_into()
            .map_err(|_| CircuitError::InvalidDimension)?,
    })
}

fn majority(
    builder: &mut CircuitBuilder,
    x: WordVar,
    y: WordVar,
    z: WordVar,
) -> Result<WordVar, CircuitError> {
    let mut bits = Vec::with_capacity(32);
    for ((x, y), z) in x.bits_le.into_iter().zip(y.bits_le).zip(z.bits_le) {
        let x_xor_y = builder.xor(x, y)?;
        let z_branch = builder.and(z, x_xor_y)?;
        let x_and_y = builder.and(x, y)?;
        let value = builder.evaluate(&z_branch.lc()) + builder.evaluate(&x_and_y.lc());
        let output = builder.alloc_bit(value == Scalar::one())?;
        builder.enforce_equal(output.lc(), z_branch.lc().plus(&x_and_y.lc()))?;
        bits.push(output);
    }
    Ok(WordVar {
        bits_le: bits
            .try_into()
            .map_err(|_| CircuitError::InvalidDimension)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expected_words(bytes: [u8; 32]) -> Vec<Scalar> {
        bytes
            .chunks_exact(4)
            .map(|word| {
                Scalar::from_u64(u64::from(u32::from_be_bytes(
                    word.try_into().expect("word"),
                )))
            })
            .collect()
    }

    fn synthesize(message: &[u8], expected: [u8; 32]) -> Result<(), CircuitError> {
        let public = expected_words(expected);
        let mut builder = CircuitBuilder::new(public.clone())?;
        let bytes = allocate_bytes(&mut builder, message)?;
        let digest = sha256(&mut builder, &bytes)?;
        for (index, word) in digest.into_iter().enumerate() {
            builder.enforce_equal(word.lc(), builder.public(index)?.into())?;
        }
        let assignment = builder.finalize()?;
        assignment
            .shape
            .validate_relaxed_assignment(
                &assignment.witness,
                Scalar::one(),
                &assignment.public_inputs,
                &vec![Scalar::zero(); assignment.shape.constraint_count()],
            )
            .map_err(CircuitError::from)
    }

    #[test]
    fn sha256_abc_matches_the_independent_standard_vector() {
        let expected: [u8; 32] =
            hex::decode("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
                .expect("hex")
                .try_into()
                .expect("digest");
        synthesize(b"abc", expected).expect("valid SHA-256 circuit");
        assert!(synthesize(b"abd", expected).is_err());
    }
}
