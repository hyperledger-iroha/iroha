//! Shared physical half selection, independent of register event geometry.

use super::F;

/// Select one limb from the two halves of an original physical memory cell.
/// The caller constrains the half bit and the original memory/register owners.
pub(in super::super) fn payload_limb(low: F, high: F, high_half: F) -> F {
    low.add(high_half.mul(high.sub(low)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_halves_are_selected_without_a_register_event_or_value_oracle() {
        for low in [F::ZERO, F::ONE, F(65535)] {
            for high in [F::ZERO, F::ONE, F(65535)] {
                assert_eq!(payload_limb(low, high, F::ZERO), low);
                assert_eq!(payload_limb(low, high, F::ONE), high);
            }
        }
    }
}
