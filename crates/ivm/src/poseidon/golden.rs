//! Fixed public Poseidon known-answer vectors, independently checked by the circuit reference suite.

pub(crate) const TWO: [(u64, u64, u64); 5] = [
    (0, 0, 0x541b_c08e_21ea_84d9),
    (1, 0, 0xf0e5_b216_08c2_4308),
    (0, 1, 0x70c2_d9e7_d478_7f50),
    (1, 1, 0x6ce7_51f5_2456_cdf3),
    (u64::MAX, u64::MAX, 0x2a3b_041a_8625_f023),
];
pub(crate) const SIX: [([u64; 6], u64); 5] = [
    ([0, 0, 0, 0, 0, 0], 0x6300_6c10_f267_d188),
    ([1, 2, 3, 4, 5, 6], 0xe56f_9ee6_b038_389a),
    ([1, 0, 0, 0, 0, 0], 0xd8c9_b0fc_f749_9786),
    ([0, 1, 0, 0, 0, 0], 0x819b_7cdd_1631_9d0f),
    ([u64::MAX; 6], 0xe4f4_13ec_7ee9_62ad),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_controls_match_current_cpu_poseidon_relations() {
        for (a, b, expected) in TWO {
            assert_eq!(crate::poseidon::poseidon2_simd(a, b), expected);
        }
        for (input, expected) in SIX {
            assert_eq!(crate::poseidon::poseidon6_simd(input), expected);
        }
    }
}
