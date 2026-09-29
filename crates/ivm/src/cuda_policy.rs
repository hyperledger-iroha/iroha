//! IVM kernel identity and public-geometry device selection.
//! Artifact qualification belongs to cuda_dispatch; physical health belongs to iroha_accel.

/// Independently admitted production CUDA kernels.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(usize)]
pub(crate) enum Kernel {
    Add32,
    Add64,
    And,
    Xor,
    Or,
    Sha256,
    ShaLeaves,
    ShaPairs,
    Keccak,
    Poseidon2,
    Poseidon6,
    AesEnc,
    AesDec,
    AesEncFused,
    AesDecFused,
    BnAdd,
    BnSub,
    BnMul,
    Ed25519,
    Bitonic,
}

impl Kernel {
    pub(crate) const ALL: [Self; 20] = [
        Self::Add32,
        Self::Add64,
        Self::And,
        Self::Xor,
        Self::Or,
        Self::Sha256,
        Self::ShaLeaves,
        Self::ShaPairs,
        Self::Keccak,
        Self::Poseidon2,
        Self::Poseidon6,
        Self::AesEnc,
        Self::AesDec,
        Self::AesEncFused,
        Self::AesDecFused,
        Self::BnAdd,
        Self::BnSub,
        Self::BnMul,
        Self::Ed25519,
        Self::Bitonic,
    ];
}

/// Try each usable candidate once, preserving a pinned device for compound work.
pub(crate) fn select_admitted_device(
    count: usize,
    preferred: usize,
    pinned: Option<usize>,
    mut admit: impl FnMut(usize) -> bool,
) -> Option<usize> {
    if let Some(index) = pinned {
        return (index < count && admit(index)).then_some(index);
    }
    if count == 0 {
        return None;
    }
    let start = preferred % count;
    (start..count).chain(0..start).find(|&index| admit(index))
}

/// Assign work using only operation identity and public input geometry.
pub(crate) fn public_workload_task_id(seed: u64, geometry: &[u64]) -> u64 {
    geometry.iter().copied().fold(seed, |mut state, size| {
        state ^= size.wrapping_add(0x9e37_79b9_7f4a_7c15);
        state = state.rotate_left(27);
        state = state.wrapping_mul(0x94d0_49bb_1331_11eb);
        state ^ (state >> 31)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selection_visits_each_public_slot_once_and_respects_pins() {
        assert_eq!(
            select_admitted_device(3, 0, None, |index| index == 2),
            Some(2)
        );
        assert_eq!(
            select_admitted_device(3, 0, Some(0), |index| index == 2),
            None
        );
        assert_eq!(
            select_admitted_device(0, 0, None, |_| panic!("empty selection")),
            None
        );
        assert_eq!(
            select_admitted_device(3, usize::MAX, Some(3), |_| panic!("invalid pin")),
            None
        );
        let mut visited = Vec::new();
        assert_eq!(
            select_admitted_device(3, 2, None, |index| {
                visited.push(index);
                false
            }),
            None
        );
        assert_eq!(visited, [2, 0, 1]);
    }
    #[test]
    fn task_assignment_uses_geometry_without_reading_operands() {
        let left = [1u64, 2, 3, 4];
        let secret = [u64::MAX, 0, 0, 0];
        let task = |input: &[u64]| public_workload_task_id(0x626e323534, &[input.len() as u64]);
        assert_eq!(task(&left), task(&secret));
        assert_ne!(task(&left), task(&left[..2]));
        let preferred = task(&left) as usize % 2;
        assert_eq!(
            select_admitted_device(2, preferred, None, |index| index != preferred),
            Some(1 - preferred)
        );
        assert_eq!(left, [1, 2, 3, 4]);
        assert_eq!(secret, [u64::MAX, 0, 0, 0]);
    }
}
