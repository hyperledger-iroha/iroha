//! Compact, deterministic single-block BLAKE2b-256 AIR with complete Iroha output.
//!
//! One invocation occupies exactly 408 rows of 310 base-field cells. Full u32
//! register/message limbs are copied between rows; bit decompositions prove
//! imports and every changed value. Each G uses four fused add/XOR-rotation rows.
//! RFC 7693 fixes the schedule: <https://www.rfc-editor.org/rfc/rfc7693.html>.
//!
//! Row indices and invocation selectors must come from an authenticated fixed
//! schedule. Apply local numerators on their declared row domains and transition
//! numerators only on the 407 actual edges. Do not gate quadratic equations with
//! witness opcodes or cyclically connect the final row. All 310 columns require
//! commitments and degree proofs. Input/output ports need exact semantic bindings.
//!
//! TODO: Integrate fixed-domain zerofiers, the committed trace and full hash/SMT
//! statement bus. Multi-block keys require separate framing/chaining and are
//! rejected here. This primitive is not connected to proof admission and does not
//! remove replay, change production limits or establish protocol qualification.

use super::{
    blake2b_compression_air::INITIALIZATION_VECTOR, transfer_integer_air::IntegerAirField,
};

/// Exact rows per bounded final-block invocation.
pub const ROW_COUNT: usize = 408;
/// Exact base-field witness cells per row, excluding an outer selector.
pub const COLUMN_COUNT: usize = 310;
/// Maximum degree of all local/transition numerators in witness variables.
pub const MAX_CONSTRAINT_DEGREE: usize = 2;
/// Maximum exact input bytes; longer keys require another proven construction.
pub const MAX_MESSAGE_BYTES: usize = 128;

const PARAMETER_WORD: u64 = 0x0101_0020;
const REGISTERS: [[usize; 4]; 8] = [
    [0, 4, 8, 12],
    [1, 5, 9, 13],
    [2, 6, 10, 14],
    [3, 7, 11, 15],
    [0, 5, 10, 15],
    [1, 6, 11, 12],
    [2, 7, 8, 13],
    [3, 4, 9, 14],
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

/// Checked position in the fixed 408-row schedule; never decoded as a witness opcode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RowIndex(usize);

impl RowIndex {
    /// Accept exactly positions zero through 407.
    #[must_use]
    pub fn new(index: usize) -> Option<Self> {
        (index < ROW_COUNT).then_some(Self(index))
    }
    /// Position in the authenticated invocation schedule.
    #[must_use]
    pub const fn get(self) -> usize {
        self.0
    }
}

/// One narrow row; state limbs are exact u32 values by initialization and induction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactRow<F = u64> {
    /// Low/high u32 limbs of all 16 working registers.
    pub working: [F; 32],
    /// Low/high u32 limbs of all 16 immutable message words.
    pub message: [F; 32],
    /// Eight chaining words, updated only during feed-forward.
    pub chaining: [F; 16],
    /// Three little-endian bit slots: sum/left, old/right, XOR output.
    pub bits: [[F; 64]; 3],
    /// Two bits per low/high carry; each pair represents 0, 1 or 2.
    pub carries: [F; 4],
    /// Present-byte prefix for three imported words; zero outside import rows.
    pub present: [F; 24],
    /// Exact input byte length copied throughout the invocation.
    pub byte_len: F,
    /// Inclusive byte-prefix count, then constant after import.
    pub prefix_count: F,
    /// Full eight-u32-limb marked output port, zero except on the export row.
    pub digest: [F; 8],
}

impl<F: IntegerAirField> CompactRow<F> {
    /// Canonical zero row.
    #[must_use]
    pub fn zero() -> Self {
        Self {
            working: [F::ZERO; 32],
            message: [F::ZERO; 32],
            chaining: [F::ZERO; 16],
            bits: [[F::ZERO; 64]; 3],
            carries: [F::ZERO; 4],
            present: [F::ZERO; 24],
            byte_len: F::ZERO,
            prefix_count: F::ZERO,
            digest: [F::ZERO; 8],
        }
    }
}

/// Heap-backed fixed-shape invocation, avoiding the direct wide compression witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompactHashWitness<F = u64> {
    rows: Box<[CompactRow<F>]>,
}

impl<F> CompactHashWitness<F> {
    /// Check the exact public shape; this does not check arithmetic or authenticate inputs.
    #[must_use]
    pub fn from_rows(rows: Vec<CompactRow<F>>) -> Option<Self> {
        (rows.len() == ROW_COUNT).then(|| Self {
            rows: rows.into_boxed_slice(),
        })
    }
    /// All 408 rows in their fixed schedule order.
    #[must_use]
    pub fn rows(&self) -> &[CompactRow<F>] {
        &self.rows
    }
    /// Mutate openings without changing the fixed schedule length.
    #[must_use]
    pub fn rows_mut(&mut self) -> &mut [CompactRow<F>] {
        &mut self.rows
    }
}

impl<F: IntegerAirField> CompactHashWitness<F> {
    /// All-zero inactive invocation.
    #[must_use]
    pub fn inactive() -> Self {
        Self::from_rows((0..ROW_COUNT).map(|_| CompactRow::zero()).collect())
            .expect("fixed row count")
    }
}

fn word_bits(word: u64) -> [u64; 64] {
    core::array::from_fn(|bit| (word >> bit) & 1)
}
fn limbs<const N: usize, const M: usize>(words: &[u64; N]) -> [u64; M] {
    core::array::from_fn(|limb| (words[limb / 2] >> (32 * (limb % 2))) & u64::from(u32::MAX))
}

fn fused_schedule(index: usize) -> (usize, usize, usize, Option<usize>, usize) {
    let step = index - 7;
    let g = step / 4;
    let phase = step % 4;
    let [a, b, c, d] = REGISTERS[g % 8];
    let sigma = SIGMA[(g / 8) % 10];
    match phase {
        0 => (a, b, d, Some(sigma[2 * (g % 8)]), 32),
        1 => (c, d, b, None, 24),
        2 => (a, b, d, Some(sigma[2 * (g % 8) + 1]), 16),
        _ => (c, d, b, None, 63),
    }
}

impl CompactHashWitness<u64> {
    /// Generate a complete exact 0..128-byte final-block witness; reject longer inputs.
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() > MAX_MESSAGE_BYTES {
            return None;
        }
        let mut padded = [0_u8; 128];
        padded[..bytes.len()].copy_from_slice(bytes);
        let message: [u64; 16] = core::array::from_fn(|i| {
            u64::from_le_bytes(padded[8 * i..8 * i + 8].try_into().expect("word"))
        });
        let mut h = INITIALIZATION_VECTOR;
        h[0] ^= PARAMETER_WORD;
        let mut v = [0_u64; 16];
        let mut count = 0;
        let mut rows = Vec::with_capacity(ROW_COUNT);
        for index in 0..ROW_COUNT {
            let mut row = CompactRow {
                working: limbs(&v),
                message: limbs(&message),
                chaining: limbs(&h),
                byte_len: bytes.len() as u64,
                prefix_count: count,
                ..CompactRow::zero()
            };
            match index {
                0..6 => {
                    for slot in 0..3 {
                        if 3 * index + slot < 16 {
                            row.bits[slot] = word_bits(message[3 * index + slot]);
                        }
                    }
                    row.present =
                        core::array::from_fn(|byte| u64::from(24 * index + byte < bytes.len()));
                    count += row.present.iter().sum::<u64>();
                    row.prefix_count = count;
                }
                6 => {
                    row.bits[0] = word_bits(bytes.len() as u64);
                    v[..8].copy_from_slice(&h);
                    v[8..].copy_from_slice(&INITIALIZATION_VECTOR);
                    v[12] ^= bytes.len() as u64;
                    v[14] = !v[14];
                }
                7..391 => {
                    let (sum_reg, add_reg, xor_reg, msg, rotation) = fused_schedule(index);
                    let extra = msg.map(|i| message[i]).unwrap_or(0);
                    let wide = u128::from(v[sum_reg]) + u128::from(v[add_reg]) + u128::from(extra);
                    let low = (v[sum_reg] & u64::from(u32::MAX))
                        + (v[add_reg] & u64::from(u32::MAX))
                        + (extra & u64::from(u32::MAX));
                    let carries = [low >> 32, (wide >> 64) as u64];
                    row.carries = core::array::from_fn(|bit| (carries[bit / 2] >> (bit % 2)) & 1);
                    let sum = wide as u64;
                    let old = v[xor_reg];
                    let output = (old ^ sum).rotate_right(rotation as u32);
                    row.bits = [word_bits(sum), word_bits(old), word_bits(output)];
                    v[sum_reg] = sum;
                    v[xor_reg] = output;
                }
                391..407 => {
                    let feed = index - 391;
                    let word = feed / 2;
                    let right = v[word + if feed % 2 == 0 { 0 } else { 8 }];
                    row.bits = [
                        word_bits(h[word]),
                        word_bits(right),
                        word_bits(h[word] ^ right),
                    ];
                    h[word] ^= right;
                }
                _ => {
                    row.bits[0] = word_bits(h[3]);
                    row.digest = limbs::<8, 8>(&h);
                    row.digest[7] |= 1 << 24;
                }
            }
            rows.push(row);
        }
        Self::from_rows(rows)
    }
}

fn pack<F: IntegerAirField>(bits: &[F]) -> F {
    bits.iter()
        .rev()
        .fold(F::ZERO, |value, &bit| value.add(value).add(bit))
}
fn packed_half<F: IntegerAirField>(bits: &[F; 64], half: usize) -> F {
    pack(&bits[32 * half..32 * half + 32])
}
fn fixed_limb<F: IntegerAirField>(word: u64, half: usize, active: F) -> F {
    F::from_u32((word >> (32 * half)) as u32).mul(active)
}
fn xor_bits<F: IntegerAirField>(out: &mut Vec<F>, row: &CompactRow<F>, rotation: usize) {
    for bit in 0..64 {
        let source = (bit + rotation) % 64;
        let a = row.bits[0][source];
        let b = row.bits[1][source];
        let product = a.mul(b);
        out.push(row.bits[2][bit].sub(a.add(b).sub(product.add(product))));
    }
}

/// Local numerators for one authenticated schedule position; maximum degree two.
#[must_use]
pub fn local_residues<F: IntegerAirField>(
    active: F,
    index: RowIndex,
    row: &CompactRow<F>,
) -> Vec<F> {
    let i = index.0;
    let mut out = Vec::new();
    out.push(active.mul(active.sub(F::ONE)));
    for bit in row
        .bits
        .iter()
        .flatten()
        .chain(row.carries.iter())
        .chain(row.present.iter())
    {
        out.push(bit.mul(bit.sub(active)));
    }
    out.push(row.carries[0].mul(row.carries[1]));
    out.push(row.carries[2].mul(row.carries[3]));
    if i >= 6 {
        out.extend(row.present);
    }
    if !(7..391).contains(&i) {
        out.extend(row.carries);
    }
    if i != 407 {
        out.extend(row.digest);
    }
    if i == 0 {
        out.extend(row.working);
        for limb in 0..16 {
            let word =
                INITIALIZATION_VECTOR[limb / 2] ^ if limb / 2 == 0 { PARAMETER_WORD } else { 0 };
            out.push(row.chaining[limb].sub(fixed_limb(word, limb % 2, active)));
        }
        out.push(
            row.prefix_count.sub(
                row.present
                    .iter()
                    .copied()
                    .fold(F::ZERO, IntegerAirField::add),
            ),
        );
    }
    match i {
        0..6 => {
            for byte in 0..24 {
                if byte > 0 {
                    out.push(row.present[byte].mul(active.sub(row.present[byte - 1])));
                }
                for bit in 0..8 {
                    let value = row.bits[byte / 8][8 * (byte % 8) + bit];
                    out.push(value.mul(value.sub(row.present[byte])));
                }
            }
            for slot in 0..3 {
                if 3 * i + slot < 16 {
                    for half in 0..2 {
                        out.push(
                            packed_half(&row.bits[slot], half)
                                .sub(row.message[2 * (3 * i + slot) + half]),
                        );
                    }
                } else {
                    out.extend(row.bits[slot]);
                }
            }
            if i == 5 {
                out.extend_from_slice(&row.present[8..]);
                out.push(row.byte_len.sub(row.prefix_count));
            }
        }
        6 => {
            out.push(row.byte_len.sub(pack(&row.bits[0][..8])));
            out.extend_from_slice(&row.bits[0][8..]);
            out.extend(row.bits[1]);
            out.extend(row.bits[2]);
        }
        7..391 => {
            let (sum_reg, add_reg, xor_reg, msg, rotation) = fused_schedule(i);
            let radix = F::from_u32(u32::MAX).add(F::ONE);
            let c0 = row.carries[0].add(row.carries[1].add(row.carries[1]));
            let c1 = row.carries[2].add(row.carries[3].add(row.carries[3]));
            if msg.is_none() {
                out.push(row.carries[1]);
                out.push(row.carries[3]);
            }
            for half in 0..2 {
                let extra = msg
                    .map(|word| row.message[2 * word + half])
                    .unwrap_or(F::ZERO);
                let mut value = row.working[2 * sum_reg + half]
                    .add(row.working[2 * add_reg + half])
                    .add(extra)
                    .sub(packed_half(&row.bits[0], half));
                if half == 0 {
                    value = value.sub(radix.mul(c0));
                } else {
                    value = value.add(c0).sub(radix.mul(c1));
                }
                out.push(value);
                out.push(packed_half(&row.bits[1], half).sub(row.working[2 * xor_reg + half]));
            }
            xor_bits(&mut out, row, rotation);
        }
        391..407 => {
            let feed = i - 391;
            let word = feed / 2;
            let right = word + if feed % 2 == 0 { 0 } else { 8 };
            for half in 0..2 {
                out.push(packed_half(&row.bits[0], half).sub(row.chaining[2 * word + half]));
                out.push(packed_half(&row.bits[1], half).sub(row.working[2 * right + half]));
            }
            xor_bits(&mut out, row, 0);
        }
        _ => {
            for half in 0..2 {
                out.push(packed_half(&row.bits[0], half).sub(row.chaining[6 + half]));
            }
            out.extend(row.bits[1]);
            out.extend(row.bits[2]);
            for limb in 0..8 {
                let expected = if limb == 7 {
                    row.chaining[7].add(F::from_u32(1 << 24).mul(active.sub(row.bits[0][56])))
                } else {
                    row.chaining[limb]
                };
                out.push(row.digest[limb].sub(expected));
            }
        }
    }
    out
}

/// Exact adjacent-row copies/updates; returns `None` on the final export row.
///
/// The caller must apply these only on within-invocation transition domains.
#[must_use]
pub fn transition_residues<F: IntegerAirField>(
    active: F,
    index: RowIndex,
    row: &CompactRow<F>,
    next: &CompactRow<F>,
) -> Option<Vec<F>> {
    let i = index.0;
    if i == 407 {
        return None;
    }
    let mut out = Vec::new();
    for limb in 0..32 {
        out.push(next.message[limb].sub(row.message[limb]));
    }
    out.push(next.byte_len.sub(row.byte_len));
    let count = if i < 5 {
        row.prefix_count.add(
            next.present
                .iter()
                .copied()
                .fold(F::ZERO, IntegerAirField::add),
        )
    } else {
        row.prefix_count
    };
    out.push(next.prefix_count.sub(count));
    if i < 5 {
        out.push(next.present[0].mul(active.sub(row.present[23])));
    }
    for limb in 0..32 {
        let mut expected = row.working[limb];
        if i == 6 {
            expected = if limb < 16 {
                row.chaining[limb]
            } else {
                let word = limb / 2 - 8;
                let mut value = fixed_limb(INITIALIZATION_VECTOR[word], limb % 2, active);
                if limb == 24 {
                    for bit in 0..8 {
                        let delta = F::from_u32(1 << bit).mul(row.bits[0][bit]);
                        value = if (INITIALIZATION_VECTOR[4] >> bit) & 1 == 1 {
                            value.sub(delta)
                        } else {
                            value.add(delta)
                        };
                    }
                }
                if word == 6 {
                    value = fixed_limb(!INITIALIZATION_VECTOR[6], limb % 2, active);
                }
                value
            };
        } else if (7..391).contains(&i) {
            let (sum, _, xor, _, _) = fused_schedule(i);
            if limb / 2 == sum {
                expected = packed_half(&row.bits[0], limb % 2);
            } else if limb / 2 == xor {
                expected = packed_half(&row.bits[2], limb % 2);
            }
        }
        out.push(next.working[limb].sub(expected));
    }
    for limb in 0..16 {
        let expected = if (391..407).contains(&i) && limb / 2 == (i - 391) / 2 {
            packed_half(&row.bits[2], limb % 2)
        } else {
            row.chaining[limb]
        };
        out.push(next.chaining[limb].sub(expected));
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{GOLDILOCKS_MODULUS_V1, GoldilocksFp4V1};

    fn valid(active: u64, witness: &CompactHashWitness) -> bool {
        witness.rows().iter().enumerate().all(|(index, row)| {
            let index = RowIndex::new(index).unwrap();
            local_residues(active, index, row)
                .iter()
                .all(|&value| value == 0)
                && (index.get() == ROW_COUNT - 1
                    || transition_residues(active, index, row, &witness.rows()[index.get() + 1])
                        .unwrap()
                        .iter()
                        .all(|&value| value == 0))
        })
    }

    fn digest(witness: &CompactHashWitness) -> [u8; 32] {
        let output = &witness.rows()[407].digest;
        core::array::from_fn(|byte| (output[byte / 4] >> (8 * (byte % 4))) as u8)
    }

    fn map_row<F: Copy>(row: &CompactRow, mut map: impl FnMut(u64) -> F) -> CompactRow<F> {
        CompactRow {
            working: row.working.map(&mut map),
            message: row.message.map(&mut map),
            chaining: row.chaining.map(&mut map),
            bits: row.bits.map(|bits| bits.map(&mut map)),
            carries: row.carries.map(&mut map),
            present: row.present.map(&mut map),
            byte_len: map(row.byte_len),
            prefix_count: map(row.prefix_count),
            digest: row.digest.map(map),
        }
    }

    #[test]
    fn complete_compact_hash_matches_native_boundary_and_seeded_messages() {
        for length in [0, 1, 7, 8, 23, 24, 31, 32, 63, 64, 83, 119, 120, 127, 128] {
            for seed in [0, 255] {
                let bytes: Vec<_> = (0..length).map(|i| ((i * 73 + seed) % 256) as u8).collect();
                let witness = CompactHashWitness::from_bytes(&bytes).unwrap();
                assert!(valid(1, &witness), "length {length}, seed {seed}");
                assert_eq!(digest(&witness), *iroha_crypto::Hash::new(&bytes).as_ref());
                assert_eq!(witness.rows().len(), ROW_COUNT);
            }
        }
        assert!(CompactHashWitness::from_bytes(&[0; 129]).is_none());
        assert!(CompactHashWitness::<u64>::from_rows(vec![CompactRow::zero(); 407]).is_none());
        assert!(RowIndex::new(408).is_none());
        let inactive = CompactHashWitness::<u64>::inactive();
        assert!(
            transition_residues(
                0,
                RowIndex::new(407).unwrap(),
                &inactive.rows()[407],
                &inactive.rows()[0]
            )
            .is_none()
        );
    }

    #[test]
    fn every_state_copy_is_bound_at_all_phase_boundaries() {
        let witness = CompactHashWitness::from_bytes(&[255; 128]).unwrap();
        for index in [0, 4, 5, 6, 7, 10, 38, 134, 390, 391, 406] {
            let original = witness.rows()[index + 1];
            for column in 0..80 {
                let mut next = original;
                let value = match column {
                    0..32 => &mut next.working[column],
                    32..64 => &mut next.message[column - 32],
                    _ => &mut next.chaining[column - 64],
                };
                *value += 1;
                assert!(
                    transition_residues(
                        1,
                        RowIndex::new(index).unwrap(),
                        &witness.rows()[index],
                        &next
                    )
                    .unwrap()
                    .iter()
                    .any(|&value| value != 0),
                    "edge {index}, state column {column}"
                );
            }
        }
        let mut changed = witness.clone();
        changed.rows_mut()[135].working[0] = 1 << 32;
        assert!(
            !valid(1, &changed),
            "a canonical field value outside u32 cannot enter the register chain"
        );
        changed.rows_mut()[135].working[0] = GOLDILOCKS_MODULUS_V1 - 1;
        assert!(
            !valid(1, &changed),
            "field alias cannot replace a range-authenticated register"
        );
    }

    #[test]
    fn fused_carries_and_cross_half_rotations_reject_changed_bits() {
        let witness = CompactHashWitness::from_bytes(&[255; 128]).unwrap();
        for index in 7..391 {
            let row = witness.rows()[index];
            assert!(
                local_residues(1, RowIndex::new(index).unwrap(), &row)
                    .iter()
                    .all(|&value| value == 0)
            );
            for carry in 0..4 {
                let mut changed = row;
                changed.carries[carry] ^= 1;
                assert!(
                    local_residues(1, RowIndex::new(index).unwrap(), &changed)
                        .iter()
                        .any(|&v| v != 0),
                    "carry {carry}, row {index}"
                );
            }
            for bit in [0, 15, 16, 23, 24, 31, 32, 55, 56, 62, 63] {
                let mut changed = row;
                changed.bits[2][bit] ^= 1;
                assert!(
                    local_residues(1, RowIndex::new(index).unwrap(), &changed)
                        .iter()
                        .any(|&v| v != 0),
                    "rotation bit {bit}, row {index}"
                );
            }
        }
        let mut changed = witness.rows()[7];
        changed.carries[0] = 1;
        changed.carries[1] = 1;
        assert!(
            local_residues(1, RowIndex::new(7).unwrap(), &changed)
                .iter()
                .any(|&v| v != 0),
            "carry three is excluded"
        );
    }

    #[test]
    fn message_prefix_count_padding_and_cross_row_edges_are_exact() {
        let mut witness = CompactHashWitness::from_bytes(&[0; 25]).unwrap();
        witness.rows_mut()[0].present[5] = 0;
        witness.rows_mut()[1].present[1] = 1;
        assert!(
            !valid(1, &witness),
            "equal-cardinality hole fails local/cross-row prefix constraints"
        );
        let witness = CompactHashWitness::from_bytes(&[0; 128]).unwrap();
        for row_index in 0..6 {
            for slot in 0..3 {
                for bit in [0, 31, 32, 63] {
                    let mut row = witness.rows()[row_index];
                    row.bits[slot][bit] ^= 1;
                    assert!(
                        local_residues(1, RowIndex::new(row_index).unwrap(), &row)
                            .iter()
                            .any(|&v| v != 0)
                    );
                }
            }
        }
        let mut long = witness.clone();
        for row in long.rows_mut() {
            row.byte_len = 129;
        }
        long.rows_mut()[5].prefix_count = 129;
        assert!(
            !valid(1, &long),
            "fixed 128-byte import cannot represent a second block"
        );
        let mut padding = CompactHashWitness::from_bytes(&[0; 1]).unwrap();
        padding.rows_mut()[0].bits[0][15] = 1;
        padding.rows_mut()[0].message[0] = 1 << 15;
        assert!(
            local_residues(1, RowIndex::new(0).unwrap(), &padding.rows()[0])
                .iter()
                .any(|&v| v != 0)
        );
        let mut byte_length = witness.rows()[5];
        byte_length.byte_len = 127;
        assert!(
            local_residues(1, RowIndex::new(5).unwrap(), &byte_length)
                .iter()
                .any(|&v| v != 0)
        );
    }

    #[test]
    fn digest_parameter_counter_and_final_complement_are_fixed() {
        let witness = CompactHashWitness::from_bytes(b"abc").unwrap();
        let mut first = witness.rows()[0];
        first.chaining[0] ^= 0x20 ^ 0x40;
        assert!(
            local_residues(1, RowIndex::new(0).unwrap(), &first)
                .iter()
                .any(|&v| v != 0)
        );
        for limb in 0..32 {
            let mut next = witness.rows()[7];
            next.working[limb] ^= 1;
            assert!(
                transition_residues(1, RowIndex::new(6).unwrap(), &witness.rows()[6], &next)
                    .unwrap()
                    .iter()
                    .any(|&v| v != 0),
                "initializer limb {limb}"
            );
        }
        let mut initialization = witness.rows()[6];
        initialization.bits[0][8] = 1;
        assert!(
            local_residues(1, RowIndex::new(6).unwrap(), &initialization)
                .iter()
                .any(|&v| v != 0),
            "counter has no hidden high bits"
        );
        initialization = witness.rows()[6];
        initialization.bits[0][0] ^= 1;
        assert!(
            local_residues(1, RowIndex::new(6).unwrap(), &initialization)
                .iter()
                .any(|&v| v != 0)
        );
    }

    #[test]
    fn full_marked_digest_feedforward_and_inactive_rows_are_bound() {
        let witness = CompactHashWitness::from_bytes(b"abc").unwrap();
        for bit in 0..256 {
            let mut row = witness.rows()[407];
            row.digest[bit / 32] ^= 1 << (bit % 32);
            assert!(
                local_residues(1, RowIndex::new(407).unwrap(), &row)
                    .iter()
                    .any(|&v| v != 0),
                "output bit {bit}"
            );
        }
        for index in 391..407 {
            let mut row = witness.rows()[index];
            row.bits[2][63] ^= 1;
            assert!(
                local_residues(1, RowIndex::new(index).unwrap(), &row)
                    .iter()
                    .any(|&v| v != 0)
            );
        }
        let mut inactive = CompactHashWitness::<u64>::inactive();
        assert!(valid(0, &inactive));
        assert!(!valid(1, &inactive));
        inactive.rows_mut()[407].digest[7] = 1 << 24;
        assert!(!valid(0, &inactive));
        assert!(
            !local_residues(2, RowIndex::new(7).unwrap(), &CompactRow::<u64>::zero())
                .iter()
                .all(|&v| v == 0)
        );
    }

    #[test]
    fn compact_evaluators_agree_over_base_and_fp4_on_every_phase() {
        let witness = CompactHashWitness::from_bytes(b"abc").unwrap();
        let embed = |value| GoldilocksFp4V1::from_base(value).unwrap();
        for index in [0, 5, 6, 7, 8, 9, 10, 390, 391, 406, 407] {
            let mut row = witness.rows()[index];
            row.bits[0][17] = 2;
            let extension = map_row(&row, embed);
            let expected = local_residues(1, RowIndex::new(index).unwrap(), &row);
            assert_eq!(
                expected.into_iter().map(embed).collect::<Vec<_>>(),
                local_residues(
                    GoldilocksFp4V1::ONE,
                    RowIndex::new(index).unwrap(),
                    &extension
                )
            );
            if index < 407 {
                let next = &witness.rows()[index + 1];
                let expected =
                    transition_residues(1, RowIndex::new(index).unwrap(), &row, next).unwrap();
                assert_eq!(
                    expected.into_iter().map(embed).collect::<Vec<_>>(),
                    transition_residues(
                        GoldilocksFp4V1::ONE,
                        RowIndex::new(index).unwrap(),
                        &extension,
                        &map_row(next, embed)
                    )
                    .unwrap()
                );
            }
        }
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
    fn every_fixed_phase_has_quadratic_degree_and_the_exact_narrow_width() {
        let mut cells = 0;
        let row = map_row(&CompactRow::zero(), |_| {
            cells += 1;
            Degree(1)
        });
        assert_eq!(cells, COLUMN_COUNT);
        assert_eq!(COLUMN_COUNT, 310);
        assert_eq!(ROW_COUNT, 408);
        let mut maximum = 0;
        for index in 0..ROW_COUNT {
            let position = RowIndex::new(index).unwrap();
            for degree in local_residues(Degree(1), position, &row).into_iter().chain(
                transition_residues(Degree(1), position, &row, &row)
                    .into_iter()
                    .flatten(),
            ) {
                maximum = maximum.max(degree.0);
            }
        }
        assert_eq!(maximum, MAX_CONSTRAINT_DEGREE);
        assert!(core::mem::size_of::<CompactRow<GoldilocksFp4V1>>() < 16 * 1024);
    }
}
