//! Validate completed native output before granting a successful-dispatch receipt.
//!
//! These borrowed checks are shared with driverless tests. The callback records
//! completion only; it performs no launch and grants no device qualification.

use crate::bn254_vec::FieldElem;

/// Record one BN254 batch only when every field and the full shape are valid.
pub(super) fn bn254(output: &[[u64; 4]], expected_count: usize, completed: impl FnOnce()) -> bool {
    if expected_count == 0
        || output.len() != expected_count
        || !output.iter().all(|words| FieldElem(*words).is_canonical())
    {
        return false;
    }
    completed();
    true
}

/// Record one Poseidon batch only for its successful, complete canonical state.
/// Nonzero status remains inspectable by the caller without success credit.
pub(super) fn poseidon(
    status: [u32; 2],
    state: Option<&[u64]>,
    width: usize,
    count: usize,
    completed: impl FnOnce(),
) -> bool {
    if status != [0, 0] || !matches!(width, 3 | 6) || count == 0 {
        return false;
    }
    let Some(expected_words) = count.checked_mul(width).and_then(|n| n.checked_mul(4)) else {
        return false;
    };
    let Some(state) = state else {
        return false;
    };
    if state.len() != expected_words
        || !state.chunks_exact(4).all(|limbs| {
            limbs
                .try_into()
                .is_ok_and(|words| FieldElem(words).is_canonical())
        })
    {
        return false;
    }
    completed();
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bn254_vec::MODULUS;
    use std::cell::Cell;

    #[test]
    fn bn254_invalid_shape_or_field_cannot_record_success() {
        let calls = Cell::new(0);
        for (output, count) in [
            (&[][..], 0),
            (&[[0; 4]][..], 2),
            (&[[0; 4], [1, 0, 0, 0]][..], 1),
            (&[[0; 4], MODULUS][..], 2),
            (&[[0; 4], [u64::MAX; 4]][..], 2),
        ] {
            assert!(!bn254(output, count, || calls.set(calls.get() + 1)));
            assert_eq!(calls.get(), 0);
        }
    }

    #[test]
    fn bn254_valid_full_width_output_records_exactly_once() {
        let calls = Cell::new(0);
        let mut maximum = MODULUS;
        maximum[0] -= 1;
        let output = [[0; 4], [1, 0, 0, 0], maximum];
        assert!(bn254(&output, 3, || calls.set(calls.get() + 1)));
        assert_eq!(calls.get(), 1);
        assert_eq!(output, [[0; 4], [1, 0, 0, 0], maximum]);
    }

    #[test]
    fn poseidon_error_status_never_records_success_even_with_complete_state() {
        let calls = Cell::new(0);
        let state = [0; 12];
        for status in [[2, 1], [3, 0], [0, 1], [u32::MAX, u32::MAX]] {
            for output in [None, Some(&state[..])] {
                assert!(!poseidon(status, output, 3, 1, || calls.set(calls.get() + 1)));
                assert_eq!(calls.get(), 0);
            }
        }
    }

    #[test]
    fn poseidon_incomplete_noncanonical_and_overflowed_states_cannot_record_success() {
        let calls = Cell::new(0);
        let mut noncanonical = [0; 24];
        noncanonical[20..].copy_from_slice(&MODULUS);
        for (state, width, count) in [
            (None, 3, 1),
            (Some(&[][..]), 3, 0),
            (Some(&[0; 11][..]), 3, 1),
            (Some(&[0; 13][..]), 3, 1),
            (Some(&[0; 12][..]), 4, 1),
            (Some(&[0; 12][..]), 3, usize::MAX),
            (Some(&noncanonical[..]), 6, 1),
        ] {
            assert!(!poseidon([0, 0], state, width, count, || calls.set(calls.get() + 1)));
            assert_eq!(calls.get(), 0);
        }
    }

    #[test]
    fn poseidon_both_widths_record_only_after_validating_every_field() {
        let calls = Cell::new(0);
        let mut maximum = MODULUS;
        maximum[0] -= 1;
        let mut state = [0; 48];
        for field in state.chunks_exact_mut(4) {
            field.copy_from_slice(&maximum);
        }
        for width in [3, 6] {
            assert!(poseidon(
                [0, 0],
                Some(&state[..width * 8]),
                width,
                2,
                || {
                    calls.set(calls.get() + 1);
                }
            ));
        }
        assert_eq!(calls.get(), 2);
        assert!(state.chunks_exact(4).all(|field| field == maximum));
    }
}
