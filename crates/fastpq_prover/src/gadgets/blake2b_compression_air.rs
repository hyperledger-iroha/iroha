//! Exact BLAKE2b compression-function AIR composition, without hash admission.
//!
//! RFC 7693 sections 2.6, 2.7 and 3.2 fix the IV, message permutations and
//! twelve-round compression schedule:
//! <https://www.rfc-editor.org/rfc/rfc7693#section-3.2>.
//! This relation binds all 96 G invocations, the initial chaining words, both
//! 64-bit counter halves, the final-block flag, all message words and all eight
//! feed-forward output words. Untouched registers retain their preceding word
//! reference, so a step cannot replace a register between scheduled writes.
//!
//! The direct witness is an unintegrated algebraic composition: 253,185 field
//! cells plus one selector and 404,306 numerators of degree at most two. Its G
//! sequence and larger register blocks are heap-backed. It would require 976 ARX operation rows before a
//! separately constrained register/message layout; the current 512-column trace
//! limit is unchanged. No production proof commits or verifies these cells yet.
//!
//! TODO: Add the committed narrow layout and its degree/quotient proof, canonical
//! byte packing and padding, digest parameter initialization, counter progression,
//! block chaining, digest truncation, Iroha's hash marker and complete SMT/root
//! bindings. This compression relation alone is neither a hash nor an SMT proof
//! and does not replace full witness replay or authenticate caller statements.

use super::{
    arx64_air::{self, BitWord64, Blake2bRotation, WORD_BITS, XorRotate64Witness},
    blake2b_g_air::{self, Blake2bGWitness},
    transfer_integer_air::IntegerAirField,
};

/// Compression rounds fixed by BLAKE2b.
pub const ROUND_COUNT: usize = 12;
/// G invocations in each round, four columns followed by four diagonals.
pub const GS_PER_ROUND: usize = 8;
/// Exact number of G witnesses in one compression invocation.
pub const G_COUNT: usize = ROUND_COUNT * GS_PER_ROUND;
/// Two feed-forward XORs for each of eight output words.
pub const FEED_FORWARD_XOR_COUNT: usize = 16;
/// ARX operation rows before adding a committed register/message binding layout.
pub const OPERATION_ROW_COUNT: usize =
    G_COUNT * blake2b_g_air::OPERATION_ROW_COUNT + FEED_FORWARD_XOR_COUNT;
/// Direct witness cells, excluding the shared invocation selector.
pub const WITNESS_CELL_COUNT: usize = G_COUNT * blake2b_g_air::WITNESS_CELL_COUNT
    + 50 * WORD_BITS
    + 1
    + FEED_FORWARD_XOR_COUNT * 3 * WORD_BITS;
/// All fixed-shape polynomial numerators for one compression invocation.
pub const CONSTRAINT_COUNT: usize = 2
    + 2 * WORD_BITS
    + 16 * WORD_BITS
    + G_COUNT * (blake2b_g_air::CONSTRAINT_COUNT + 6 * WORD_BITS)
    + FEED_FORWARD_XOR_COUNT * arx64_air::XOR_ROTATE_CONSTRAINT_COUNT
    + 40 * WORD_BITS;
/// Maximum degree in invocation selectors and arbitrary witness openings.
pub const MAX_CONSTRAINT_DEGREE: usize = 2;

/// Fixed BLAKE2b initialization words, before any digest parameter block.
pub const INITIALIZATION_VECTOR: [u64; 8] = [
    0x6a09_e667_f3bc_c908,
    0xbb67_ae85_84ca_a73b,
    0x3c6e_f372_fe94_f82b,
    0xa54f_f53a_5f1d_36f1,
    0x510e_527f_ade6_82d1,
    0x9b05_688c_2b3e_6c1f,
    0x1f83_d9ab_fb41_bd6b,
    0x5be0_cd19_137e_2179,
];

const SIGMA: [[usize; 16]; 10] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
];

const REGISTERS: [[usize; 4]; GS_PER_ROUND] = [
    [0, 4, 8, 12],
    [1, 5, 9, 13],
    [2, 6, 10, 14],
    [3, 7, 11, 15],
    [0, 5, 10, 15],
    [1, 6, 11, 12],
    [2, 7, 8, 13],
    [3, 4, 9, 14],
];

/// Heap-backed, structurally fixed sequence; callers cannot add or omit rounds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompressionSteps<F> {
    steps: Box<[Blake2bGWitness<F>]>,
}

impl<F> CompressionSteps<F> {
    /// Accept exactly 96 witnesses; this checks shape, not arithmetic validity.
    #[must_use]
    pub fn new(steps: Vec<Blake2bGWitness<F>>) -> Option<Self> {
        (steps.len() == G_COUNT).then(|| Self {
            steps: steps.into_boxed_slice(),
        })
    }

    /// Borrow all fixed-position G witnesses in round/schedule order.
    #[must_use]
    pub fn as_slice(&self) -> &[Blake2bGWitness<F>] {
        &self.steps
    }

    /// Mutate fixed-position openings while preserving the complete schedule shape.
    #[must_use]
    pub fn as_mut_slice(&mut self) -> &mut [Blake2bGWitness<F>] {
        &mut self.steps
    }
}

/// Exact unrotated XOR witness used in compression feed-forward.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Xor64Witness<F = u64> {
    /// First complete Boolean word.
    pub left: BitWord64<F>,
    /// Second complete Boolean word.
    pub right: BitWord64<F>,
    /// Unrotated XOR output.
    pub output: BitWord64<F>,
}

impl Xor64Witness<u64> {
    /// Generate an exact integer XOR witness.
    #[must_use]
    pub fn from_operands(left: u64, right: u64) -> Self {
        Self {
            left: BitWord64::from_integer(left),
            right: BitWord64::from_integer(right),
            output: BitWord64::from_integer(left ^ right),
        }
    }
}

impl<F: IntegerAirField> Xor64Witness<F> {
    /// Canonical zero witness for an inactive operation.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            left: BitWord64::zero(),
            right: BitWord64::zero(),
            output: BitWord64::zero(),
        }
    }
}

/// Reuse activated ARX XOR constraints, undoing its fixed rotation by bit indexing.
///
/// Rotating the unrotated output right by 32 produces exactly the word expected
/// by the existing Ror32 evaluator. This permutation introduces no new variables.
#[must_use]
pub fn xor_residues<F: IntegerAirField>(
    active: F,
    witness: &Xor64Witness<F>,
) -> [F; arx64_air::XOR_ROTATE_CONSTRAINT_COUNT] {
    arx64_air::xor_rotate_residues(
        active,
        Blake2bRotation::Ror32,
        &XorRotate64Witness {
            left: witness.left,
            right: witness.right,
            output: BitWord64 {
                bits: core::array::from_fn(|bit| witness.output.bits[(bit + 32) % 64]),
            },
        },
    )
}

/// All explicit inputs, initial registers, round witnesses and complete outputs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Blake2bCompressionWitness<F = u64> {
    /// Eight chaining words before compression; digest initialization is separate.
    pub chaining: [BitWord64<F>; 8],
    /// Sixteen little-endian message words; byte packing/padding must be bound separately.
    pub message: [BitWord64<F>; 16],
    /// Complete low/high 64-bit halves of the byte offset through this block.
    pub counter: [BitWord64<F>; 2],
    /// Activated Boolean final-block flag.
    pub final_block: F,
    /// Complete working vector after IV/counter/final-flag initialization.
    pub initialized: Box<[BitWord64<F>; 16]>,
    /// All G invocations in the fixed twelve-round schedule.
    pub steps: CompressionSteps<F>,
    /// Two consecutive XOR witnesses per output: `h XOR v_low`, then `previous XOR v_high`.
    pub feed_forward: Box<[Xor64Witness<F>; FEED_FORWARD_XOR_COUNT]>,
    /// All eight chaining words after feed-forward, before digest truncation/marking.
    pub output: [BitWord64<F>; 8],
}

fn decode_word(word: &BitWord64) -> u64 {
    word.bits
        .iter()
        .enumerate()
        .fold(0, |value, (bit, &set)| value | (set << bit))
}

impl Blake2bCompressionWitness<u64> {
    /// Generate the complete fixed compression witness using exact native words.
    #[must_use]
    pub fn from_inputs(
        chaining: [u64; 8],
        message: [u64; 16],
        counter: [u64; 2],
        final_block: bool,
    ) -> Self {
        let mut registers: [u64; 16] = core::array::from_fn(|index| {
            if index < 8 {
                chaining[index]
            } else {
                INITIALIZATION_VECTOR[index - 8]
            }
        });
        registers[12] ^= counter[0];
        registers[13] ^= counter[1];
        if final_block {
            registers[14] = !registers[14];
        }
        let initialized = Box::new(registers.map(BitWord64::from_integer));
        let mut steps = Vec::with_capacity(G_COUNT);
        for round in 0..ROUND_COUNT {
            let sigma = SIGMA[round % SIGMA.len()];
            for (position, indices) in REGISTERS.iter().enumerate() {
                let g = Blake2bGWitness::from_inputs(
                    indices.map(|index| registers[index]),
                    [
                        message[sigma[2 * position]],
                        message[sigma[2 * position + 1]],
                    ],
                );
                for (index, word) in
                    indices
                        .iter()
                        .zip([g.outputs.a, g.outputs.b, g.outputs.c, g.outputs.d])
                {
                    registers[*index] = decode_word(&word);
                }
                steps.push(g);
            }
        }
        let feed_forward = Box::new(core::array::from_fn::<_, FEED_FORWARD_XOR_COUNT, _>(
            |index| {
                let word = index / 2;
                if index % 2 == 0 {
                    Xor64Witness::from_operands(chaining[word], registers[word])
                } else {
                    Xor64Witness::from_operands(
                        chaining[word] ^ registers[word],
                        registers[word + 8],
                    )
                }
            },
        ));
        Self {
            chaining: chaining.map(BitWord64::from_integer),
            message: message.map(BitWord64::from_integer),
            counter: counter.map(BitWord64::from_integer),
            final_block: u64::from(final_block),
            initialized,
            steps: CompressionSteps::new(steps)
                .expect("fixed compression schedule has 96 G invocations"),
            output: core::array::from_fn(|index| feed_forward[2 * index + 1].output),
            feed_forward,
        }
    }
}

impl<F: IntegerAirField> Blake2bCompressionWitness<F> {
    /// Canonical zero openings for an inactive invocation, including all G steps.
    #[must_use]
    pub fn inactive() -> Self {
        Self {
            chaining: [BitWord64::zero(); 8],
            message: [BitWord64::zero(); 16],
            counter: [BitWord64::zero(); 2],
            final_block: F::ZERO,
            initialized: Box::new([BitWord64::zero(); 16]),
            steps: CompressionSteps::new(
                (0..G_COUNT).map(|_| Blake2bGWitness::inactive()).collect(),
            )
            .expect("inactive witness preserves the fixed schedule"),
            feed_forward: Box::new([Xor64Witness::inactive(); FEED_FORWARD_XOR_COUNT]),
            output: [BitWord64::zero(); 8],
        }
    }
}

fn link_word<F: IntegerAirField>(residues: &mut Vec<F>, left: &BitWord64<F>, right: &BitWord64<F>) {
    residues.extend(
        left.bits
            .iter()
            .zip(right.bits)
            .map(|(&left, right)| left.sub(right)),
    );
}

fn append_g_input_links<F: IntegerAirField>(
    residues: &mut Vec<F>,
    registers: &[&BitWord64<F>; 16],
    message: &[BitWord64<F>; 16],
    g: &Blake2bGWitness<F>,
    step: usize,
) {
    let position = step % GS_PER_ROUND;
    let sigma = SIGMA[(step / GS_PER_ROUND) % SIGMA.len()];
    for (input, index) in [&g.inputs.a, &g.inputs.b, &g.inputs.c, &g.inputs.d]
        .into_iter()
        .zip(REGISTERS[position])
    {
        link_word(residues, input, registers[index]);
    }
    link_word(residues, &g.message[0], &message[sigma[2 * position]]);
    link_word(residues, &g.message[1], &message[sigma[2 * position + 1]]);
}

/// Evaluate complete compression numerators with no native witness acceptance tests.
///
/// Order: selector/final-flag and counter Booleanity, initialized registers,
/// each G's operation/link numerators then its six external input links, followed
/// by feed-forward XORs and links. Constant IV bits are multiplied by the shared
/// selector; final-flag and counter XORs with constants are affine polynomials.
/// All returned numerators need the surrounding committed degree/quotient proof.
#[must_use]
pub fn constraint_residues<F: IntegerAirField>(
    active: F,
    witness: &Blake2bCompressionWitness<F>,
) -> Vec<F> {
    let mut residues = Vec::with_capacity(CONSTRAINT_COUNT);
    residues.push(active.mul(active.sub(F::ONE)));
    residues.push(witness.final_block.mul(witness.final_block.sub(active)));
    for word in &witness.counter {
        residues.extend(word.bits.iter().map(|&bit| bit.mul(bit.sub(active))));
    }
    for index in 0..16 {
        for bit in 0..WORD_BITS {
            let expected = if index < 8 {
                witness.chaining[index].bits[bit]
            } else {
                let iv_bit = (INITIALIZATION_VECTOR[index - 8] >> bit) & 1;
                let toggled = match index {
                    12 => witness.counter[0].bits[bit],
                    13 => witness.counter[1].bits[bit],
                    14 => witness.final_block,
                    _ => F::ZERO,
                };
                if iv_bit == 0 {
                    toggled
                } else {
                    active.sub(toggled)
                }
            };
            residues.push(witness.initialized[index].bits[bit].sub(expected));
        }
    }
    let mut registers = core::array::from_fn::<_, 16, _>(|index| &witness.initialized[index]);
    for (step, g) in witness.steps.as_slice().iter().enumerate() {
        residues.extend(blake2b_g_air::constraint_residues(active, g));
        append_g_input_links(&mut residues, &registers, &witness.message, g, step);
        for (index, output) in REGISTERS[step % GS_PER_ROUND].into_iter().zip([
            &g.outputs.a,
            &g.outputs.b,
            &g.outputs.c,
            &g.outputs.d,
        ]) {
            registers[index] = output;
        }
    }
    for xor in witness.feed_forward.iter() {
        residues.extend(xor_residues(active, xor));
    }
    for index in 0..8 {
        let first = &witness.feed_forward[2 * index];
        let second = &witness.feed_forward[2 * index + 1];
        link_word(&mut residues, &first.left, &witness.chaining[index]);
        link_word(&mut residues, &first.right, registers[index]);
        link_word(&mut residues, &second.left, &first.output);
        link_word(&mut residues, &second.right, registers[index + 8]);
        link_word(&mut residues, &witness.output[index], &second.output);
    }
    residues
}

#[cfg(test)]
mod tests {
    use super::super::{arx64_air::Add64Witness, blake2b_g_air::GRegisters};
    use super::*;
    use crate::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1};

    fn valid(active: u64, witness: &Blake2bCompressionWitness) -> bool {
        let residues = constraint_residues(active, witness);
        assert_eq!(residues.len(), CONSTRAINT_COUNT);
        residues.iter().all(|&value| value == 0)
    }

    fn reference(
        chaining: [u64; 8],
        message: [u64; 16],
        counter: [u64; 2],
        final_block: bool,
    ) -> [u64; 8] {
        fn g(v: &mut [u64; 16], [a, b, c, d]: [usize; 4], words: [u64; 2]) {
            let mask = u128::from(u64::MAX);
            for (word, [r_d, r_b]) in words.into_iter().zip([[32, 24], [16, 63]]) {
                v[a] = ((u128::from(v[a]) + u128::from(v[b]) + u128::from(word)) & mask) as u64;
                let xor_d = v[d] ^ v[a];
                v[d] = (xor_d >> r_d) | (xor_d << (64 - r_d));
                v[c] = ((u128::from(v[c]) + u128::from(v[d])) & mask) as u64;
                let xor_b = v[b] ^ v[c];
                v[b] = (xor_b >> r_b) | (xor_b << (64 - r_b));
            }
        }
        let mut v = [0; 16];
        v[..8].copy_from_slice(&chaining);
        v[8..].copy_from_slice(&INITIALIZATION_VECTOR);
        v[12] ^= counter[0];
        v[13] ^= counter[1];
        if final_block {
            v[14] ^= u64::MAX;
        }
        for round in 0..12 {
            let s = SIGMA[round % 10];
            for column in 0..4 {
                g(
                    &mut v,
                    [column, column + 4, column + 8, column + 12],
                    [message[s[2 * column]], message[s[2 * column + 1]]],
                );
            }
            for diagonal in 0..4 {
                g(
                    &mut v,
                    [
                        diagonal,
                        4 + (diagonal + 1) % 4,
                        8 + (diagonal + 2) % 4,
                        12 + (diagonal + 3) % 4,
                    ],
                    [message[s[8 + 2 * diagonal]], message[s[9 + 2 * diagonal]]],
                );
            }
        }
        core::array::from_fn(|index| chaining[index] ^ v[index] ^ v[index + 8])
    }

    fn sample() -> Blake2bCompressionWitness {
        Blake2bCompressionWitness::from_inputs(
            core::array::from_fn(|index| INITIALIZATION_VECTOR[index] ^ (index as u64 + 1)),
            core::array::from_fn(|index| (index as u64 + 1).wrapping_mul(0x1234_5678_9abc_def0)),
            [GOLDILOCKS_MODULUS_V1, u64::MAX],
            true,
        )
    }

    fn message_words(bytes: &[u8]) -> [u64; 16] {
        assert!(bytes.len() <= 128);
        let mut block = [0; 128];
        block[..bytes.len()].copy_from_slice(bytes);
        core::array::from_fn(|index| {
            u64::from_le_bytes(block[8 * index..8 * index + 8].try_into().unwrap())
        })
    }

    fn hash_bytes(output: &[BitWord64; 8]) -> Vec<u8> {
        output
            .iter()
            .flat_map(|word| decode_word(word).to_le_bytes())
            .collect()
    }

    fn map_word<F: Copy>(word: &BitWord64, map: &mut impl FnMut(u64) -> F) -> BitWord64<F> {
        BitWord64 {
            bits: word.bits.map(map),
        }
    }

    fn map_registers<F: Copy>(r: &GRegisters, map: &mut impl FnMut(u64) -> F) -> GRegisters<F> {
        GRegisters {
            a: map_word(&r.a, map),
            b: map_word(&r.b, map),
            c: map_word(&r.c, map),
            d: map_word(&r.d, map),
        }
    }

    fn map_g<F: Copy>(g: &Blake2bGWitness, map: &mut impl FnMut(u64) -> F) -> Blake2bGWitness<F> {
        Blake2bGWitness {
            inputs: map_registers(&g.inputs, map),
            message: g.message.map(|word| map_word(&word, map)),
            outputs: map_registers(&g.outputs, map),
            additions: g.additions.map(|add| Add64Witness {
                left: map_word(&add.left, map),
                right: map_word(&add.right, map),
                output: map_word(&add.output, map),
                carry_32: map(add.carry_32),
                carry_64: map(add.carry_64),
            }),
            xor_rotations: g.xor_rotations.map(|xor| XorRotate64Witness {
                left: map_word(&xor.left, map),
                right: map_word(&xor.right, map),
                output: map_word(&xor.output, map),
            }),
        }
    }

    fn map_witness<F: Copy>(
        w: &Blake2bCompressionWitness,
        mut map: impl FnMut(u64) -> F,
    ) -> Blake2bCompressionWitness<F> {
        Blake2bCompressionWitness {
            chaining: w.chaining.map(|word| map_word(&word, &mut map)),
            message: w.message.map(|word| map_word(&word, &mut map)),
            counter: w.counter.map(|word| map_word(&word, &mut map)),
            final_block: map(w.final_block),
            initialized: Box::new(core::array::from_fn(|index| {
                map_word(&w.initialized[index], &mut map)
            })),
            steps: CompressionSteps::new(
                w.steps
                    .as_slice()
                    .iter()
                    .map(|g| map_g(g, &mut map))
                    .collect(),
            )
            .unwrap(),
            feed_forward: Box::new(core::array::from_fn(|index| {
                let xor = &w.feed_forward[index];
                Xor64Witness {
                    left: map_word(&xor.left, &mut map),
                    right: map_word(&xor.right, &mut map),
                    output: map_word(&xor.output, &mut map),
                }
            })),
            output: w.output.map(|word| map_word(&word, &mut map)),
        }
    }

    #[test]
    fn schedule_shape_cannot_omit_or_extend_rounds() {
        for count in [0, G_COUNT - 1, G_COUNT + 1] {
            assert!(
                CompressionSteps::<u64>::new(
                    (0..count).map(|_| Blake2bGWitness::inactive()).collect()
                )
                .is_none()
            );
        }
        let mut steps = CompressionSteps::<u64>::new(
            (0..G_COUNT).map(|_| Blake2bGWitness::inactive()).collect(),
        )
        .unwrap();
        assert_eq!(steps.as_slice().len(), G_COUNT);
        steps.as_mut_slice()[95].message[0].bits[63] = 1;
        assert_eq!(steps.as_slice()[95].message[0].bits[63], 1);
    }

    #[test]
    fn unrotated_xor_reuses_all_arx_bit_relations() {
        for left in [0, 1, 1 << 32, GOLDILOCKS_MODULUS_V1, u64::MAX] {
            let right = left.rotate_right(17) ^ 0x0123_4567_89ab_cdef;
            let witness = Xor64Witness::from_operands(left, right);
            assert_eq!(decode_word(&witness.output), left ^ right);
            assert!(xor_residues(1, &witness).iter().all(|&value| value == 0));
            for bit in 0..64 {
                let mut changed = witness;
                changed.output.bits[bit] ^= 1;
                assert!(xor_residues(1, &changed).iter().any(|&value| value != 0));
            }
        }
        assert!(
            xor_residues(0, &Xor64Witness::<u64>::inactive())
                .iter()
                .all(|&value| value == 0)
        );
    }

    #[test]
    fn complete_compression_matches_rfc_abc_and_independent_hash_backend() {
        let mut chaining = INITIALIZATION_VECTOR;
        chaining[0] ^= 0x0101_0040;
        let witness =
            Blake2bCompressionWitness::from_inputs(chaining, message_words(b"abc"), [3, 0], true);
        assert!(valid(1, &witness));
        let expected = "ba80a53f981c4d0d6a2797b69f12f6e94c212f14685ac4b74b12bb6fdbffa2d17d87c5392aab792dc252d5de4533cc9518d38aa8dbf1925ab92386edd4009923";
        let bytes: Vec<_> = expected
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(core::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        assert_eq!(hash_bytes(&witness.output), bytes);
        // This test supplies digest initialization/padding natively. Those are
        // not yet constraints in the compression gadget's production contract.
        for input in [Vec::new(), b"abc".to_vec(), (0..128).collect::<Vec<u8>>()] {
            let mut chaining = INITIALIZATION_VECTOR;
            chaining[0] ^= 0x0101_0020;
            let witness = Blake2bCompressionWitness::from_inputs(
                chaining,
                message_words(&input),
                [input.len() as u64, 0],
                true,
            );
            assert!(valid(1, &witness));
            let mut digest: [u8; 32] = hash_bytes(&witness.output)[..32].try_into().unwrap();
            digest[31] |= 1;
            assert_eq!(digest, *iroha_crypto::Hash::new(&input).as_ref());
        }
    }

    #[test]
    fn counter_halves_flags_and_nonfinal_states_match_integer_reference() {
        for (seed, counter) in [
            [0, 0],
            [u64::MAX, 0],
            [0, 1],
            [GOLDILOCKS_MODULUS_V1, u64::MAX],
        ]
        .into_iter()
        .enumerate()
        {
            for final_block in [false, true] {
                let chaining = core::array::from_fn(|index| {
                    INITIALIZATION_VECTOR[index] ^ (seed as u64).wrapping_mul(index as u64 + 1)
                });
                let message = core::array::from_fn(|index| {
                    (index as u64 + 1).wrapping_mul(0xfedc_ba98_7654_3210 ^ seed as u64)
                });
                let witness =
                    Blake2bCompressionWitness::from_inputs(chaining, message, counter, final_block);
                assert!(valid(1, &witness));
                assert_eq!(
                    witness.output.map(|word| decode_word(&word)),
                    reference(chaining, message, counter, final_block)
                );
            }
        }
    }

    #[test]
    fn every_g_is_bound_to_its_fixed_register_and_message_position() {
        let mut witness = sample();
        assert!(valid(1, &witness));
        let mut register_values = *witness.initialized;
        for step in 0..G_COUNT {
            let original = witness.steps.as_slice()[step];
            let registers = core::array::from_fn(|index| &register_values[index]);
            let mut baseline = Vec::new();
            append_g_input_links(&mut baseline, &registers, &witness.message, &original, step);
            assert_eq!(baseline, vec![0; 6 * WORD_BITS]);
            // Every edge is tested through the same link evaluator used above;
            // recomputing all 96 local G constraints per edge is unnecessary.
            for edge in 0..6 {
                let mut replacement = original;
                let word = match edge {
                    0 => &mut replacement.inputs.a,
                    1 => &mut replacement.inputs.b,
                    2 => &mut replacement.inputs.c,
                    3 => &mut replacement.inputs.d,
                    _ => &mut replacement.message[edge - 4],
                };
                word.bits[63] ^= 1;
                let mut residues = Vec::new();
                append_g_input_links(
                    &mut residues,
                    &registers,
                    &witness.message,
                    &replacement,
                    step,
                );
                assert!(
                    residues.iter().any(|&value| value != 0),
                    "step {step}, edge {edge}"
                );
            }
            if [0, 7, 8, 79, 95].contains(&step) {
                let mut inputs = [
                    original.inputs.a,
                    original.inputs.b,
                    original.inputs.c,
                    original.inputs.d,
                ]
                .map(|word| decode_word(&word));
                inputs[0] ^= 1;
                let message = original.message.map(|word| decode_word(&word));
                let replacement = Blake2bGWitness::from_inputs(inputs, message);
                assert!(
                    blake2b_g_air::constraint_residues(1, &replacement)
                        .iter()
                        .all(|&value| value == 0)
                );
                witness.steps.as_mut_slice()[step] = replacement;
                assert!(
                    !valid(1, &witness),
                    "locally valid detached invocation {step}"
                );
                witness.steps.as_mut_slice()[step] = original;
            }
            for (index, output) in REGISTERS[step % GS_PER_ROUND].into_iter().zip([
                original.outputs.a,
                original.outputs.b,
                original.outputs.c,
                original.outputs.d,
            ]) {
                register_values[index] = output;
            }
        }
    }

    #[test]
    fn all_public_words_initialization_and_feed_forward_are_bound() {
        let mut witness = sample();
        for index in 0..50 {
            let word = match index {
                0..8 => &mut witness.chaining[index],
                8..24 => &mut witness.message[index - 8],
                24..26 => &mut witness.counter[index - 24],
                26..42 => &mut witness.initialized[index - 26],
                _ => &mut witness.output[index - 42],
            };
            word.bits[63] ^= 1;
            assert!(
                !valid(1, &witness),
                "unbound external/initialized word {index}"
            );
            let word = match index {
                0..8 => &mut witness.chaining[index],
                8..24 => &mut witness.message[index - 8],
                24..26 => &mut witness.counter[index - 24],
                26..42 => &mut witness.initialized[index - 26],
                _ => &mut witness.output[index - 42],
            };
            word.bits[63] ^= 1;
        }
        witness.final_block = 0;
        assert!(!valid(1, &witness));
        witness.final_block = 2;
        assert!(!valid(1, &witness));
        witness.final_block = 1;
        for index in 0..FEED_FORWARD_XOR_COUNT {
            let original = witness.feed_forward[index];
            witness.feed_forward[index] = Xor64Witness::from_operands(
                decode_word(&original.left) ^ 1,
                decode_word(&original.right),
            );
            assert!(
                xor_residues(1, &witness.feed_forward[index])
                    .iter()
                    .all(|&value| value == 0)
            );
            assert!(!valid(1, &witness), "detached feed-forward XOR {index}");
            witness.feed_forward[index] = original;
        }
    }

    #[test]
    fn inactive_compression_has_zero_inputs_constants_and_intermediates() {
        let mut witness = Blake2bCompressionWitness::<u64>::inactive();
        assert!(valid(0, &witness));
        assert!(
            !valid(1, &witness),
            "active IV initialization is not the zero vector"
        );
        witness.final_block = 1;
        assert!(!valid(0, &witness));
        witness.final_block = 0;
        witness.counter[1].bits[63] = 1;
        assert!(!valid(0, &witness));
        witness.counter[1].bits[63] = 0;
        witness.steps.as_mut_slice()[95].outputs.d.bits[63] = 1;
        assert!(!valid(0, &witness));
        witness.steps.as_mut_slice()[95].outputs.d.bits[63] = 0;
        witness.output[7].bits[63] = 1;
        assert!(!valid(0, &witness));
    }

    #[test]
    fn complete_base_and_fp4_evaluators_agree_on_invalid_openings() {
        let mut base = sample();
        base.counter[0].bits[17] = 2;
        base.steps.as_mut_slice()[53].additions[2].output.bits[41] = 3;
        base.feed_forward[13].output.bits[37] = 4;
        let extension = map_witness(&base, |value| GoldilocksFp4V1::from_base(value).unwrap());
        let expected = constraint_residues(1, &base);
        assert!(expected.iter().any(|&value| value != 0));
        let actual = constraint_residues(GoldilocksFp4V1::ONE, &extension);
        assert_eq!(expected.len(), actual.len());
        assert!(
            expected
                .into_iter()
                .zip(actual)
                .all(|(left, right)| GoldilocksFp4V1::from_base(left).unwrap() == right)
        );
    }

    #[derive(Clone, Copy)]
    struct Degree(usize);
    impl IntegerAirField for Degree {
        const ZERO: Self = Self(0);
        const ONE: Self = Self(0);
        fn from_u32(_: u32) -> Self {
            Self(0)
        }
        fn add(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }
        fn sub(self, other: Self) -> Self {
            Self(self.0.max(other.0))
        }
        fn mul(self, other: Self) -> Self {
            Self(self.0 + other.0)
        }
    }

    #[test]
    fn every_compression_numerator_is_quadratic_with_bounded_structural_cost() {
        let mut visited = 0;
        let witness = map_witness(&sample(), |_| {
            visited += 1;
            Degree(1)
        });
        assert_eq!(visited, WITNESS_CELL_COUNT);
        let residues = constraint_residues(Degree(1), &witness);
        assert_eq!(residues.len(), CONSTRAINT_COUNT);
        assert_eq!(
            residues.iter().map(|value| value.0).max(),
            Some(MAX_CONSTRAINT_DEGREE)
        );
        assert_eq!(WITNESS_CELL_COUNT, 253_185);
        assert_eq!(CONSTRAINT_COUNT, 404_306);
        assert_eq!(OPERATION_ROW_COUNT, 976);
        assert!(core::mem::size_of::<Blake2bCompressionWitness<GoldilocksFp4V1>>() < 128 * 1024);
    }
}
