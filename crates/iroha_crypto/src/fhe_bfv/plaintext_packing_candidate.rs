//! Unused scalar F257 packing candidate; no keys, ciphertexts or wire profile.
//!
//! See `specs/ram_lfe_plaintext_packing.md`. The private owners clear their own
//! allocated words. Borrowed input copies remain the caller's responsibility.
//! TODO: independently qualify parameters, input admission, noise, circuit privacy
//! and the complete execution relation before considering production adoption.

use core::fmt;
use zeroize::{Zeroize, Zeroizing};

const MODULUS: u16 = 257;
const SLOTS: usize = 128;
const RING_DEGREE: usize = 4096;
const STRIDE: usize = RING_DEGREE / SLOTS;
const ROOT: u16 = 3;
const INVERSE_SLOTS: u16 = 255;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PackingError {
    Length,
    NonCanonical { index: usize },
    Slot,
    Automorphism,
}

struct ClearingWords(Box<[u16; SLOTS]>);

impl ClearingWords {
    fn zero() -> Self {
        Self(Box::new([0; SLOTS]))
    }

    fn copy_canonical(input: &[u16]) -> Result<Self, PackingError> {
        if input.len() != SLOTS {
            return Err(PackingError::Length);
        }
        let mut owned = Self::zero();
        for (index, &word) in input.iter().enumerate() {
            if word >= MODULUS {
                return Err(PackingError::NonCanonical { index });
            }
            owned.0[index] = word;
        }
        Ok(owned)
    }
}

impl Drop for ClearingWords {
    fn drop(&mut self) {
        self.0.as_mut().zeroize();
        observe_cleared_cells(self.0.as_ref());
    }
}

impl fmt::Debug for ClearingWords {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ClearingWords([REDACTED; 128])")
    }
}

#[derive(Debug)]
struct ScalarSlots(ClearingWords);

impl ScalarSlots {
    fn copy_canonical(input: &[u16]) -> Result<Self, PackingError> {
        ClearingWords::copy_canonical(input).map(Self)
    }
}

/// Stores only the plaintext coefficients of `g(x^32)`, never ciphertext data.
#[derive(Debug)]
struct SparsePlaintext(ClearingWords);

impl SparsePlaintext {
    fn copy_canonical(input: &[u16]) -> Result<Self, PackingError> {
        ClearingWords::copy_canonical(input).map(Self)
    }

    fn encode(slots: &ScalarSlots) -> Self {
        let mut output = ClearingWords::zero();
        for k in 0..SLOTS {
            let mut sum = Zeroizing::new(0_u16);
            for j in 0..SLOTS {
                // The inverse transform factors depend only on public indices.
                let inverse_beta_power = power(ROOT, (256 - ((2 * j + 1) * k) % 256) % 256);
                *sum = add(*sum, mul(slots.0.0[j], inverse_beta_power));
            }
            output.0[k] = mul(*sum, INVERSE_SLOTS);
        }
        Self(output)
    }

    fn decode(&self) -> ScalarSlots {
        let mut output = ClearingWords::zero();
        for j in 0..SLOTS {
            let beta = power(ROOT, 2 * j + 1);
            let mut value = Zeroizing::new(0_u16);
            for &coefficient in self.0.0.iter().rev() {
                *value = add(mul(*value, beta), coefficient);
            }
            output.0[j] = *value;
        }
        ScalarSlots(output)
    }

    fn mask(slot: usize) -> Result<Self, PackingError> {
        if slot >= SLOTS {
            return Err(PackingError::Slot);
        }
        let mut output = ClearingWords::zero();
        for k in 0..SLOTS {
            let inverse_beta_power = power(ROOT, (256 - ((2 * slot + 1) * k) % 256) % 256);
            output.0[k] = mul(INVERSE_SLOTS, inverse_beta_power);
        }
        Ok(Self(output))
    }

    fn add(&self, other: &Self) -> Self {
        let mut output = ClearingWords::zero();
        for k in 0..SLOTS {
            output.0[k] = add(self.0.0[k], other.0.0[k]);
        }
        Self(output)
    }

    fn subtract(&self, other: &Self) -> Self {
        let mut output = ClearingWords::zero();
        for k in 0..SLOTS {
            output.0[k] = sub(self.0.0[k], other.0.0[k]);
        }
        Self(output)
    }

    fn multiply(&self, other: &Self) -> Self {
        let mut output = ClearingWords::zero();
        for i in 0..SLOTS {
            for j in 0..SLOTS {
                let product = Zeroizing::new(mul(self.0.0[i], other.0.0[j]));
                let k = i + j;
                output.0[k % SLOTS] = if k < SLOTS {
                    add(output.0[k], *product)
                } else {
                    sub(output.0[k % SLOTS], *product)
                };
            }
        }
        Self(output)
    }

    fn automorphism(&self, exponent: usize) -> Result<Self, PackingError> {
        if exponent == 0 || exponent >= 2 * RING_DEGREE || exponent.is_multiple_of(2) {
            return Err(PackingError::Automorphism);
        }
        let mut output = ClearingWords::zero();
        for k in 0..SLOTS {
            let mapped = k * exponent % (2 * SLOTS);
            output.0[mapped % SLOTS] = if mapped < SLOTS {
                self.0.0[k]
            } else {
                sub(0, self.0.0[k])
            };
        }
        Ok(Self(output))
    }

    fn coefficient(&self, exponent: usize) -> Option<u16> {
        if exponent >= RING_DEGREE {
            None
        } else if exponent.is_multiple_of(STRIDE) {
            Some(self.0.0[exponent / STRIDE])
        } else {
            Some(0)
        }
    }
}

fn add(a: u16, b: u16) -> u16 {
    ((u32::from(a) + u32::from(b)) % u32::from(MODULUS)) as u16
}

fn sub(a: u16, b: u16) -> u16 {
    ((u32::from(a) + u32::from(MODULUS) - u32::from(b)) % u32::from(MODULUS)) as u16
}

fn mul(a: u16, b: u16) -> u16 {
    ((u32::from(a) * u32::from(b)) % u32::from(MODULUS)) as u16
}

/// Only used with public transform bases and indices, never secret exponents.
fn power(mut base: u16, mut exponent: usize) -> u16 {
    let mut result = 1;
    while exponent > 0 {
        if exponent & 1 != 0 {
            result = mul(result, base);
        }
        base = mul(base, base);
        exponent >>= 1;
    }
    result
}

std::thread_local! {
    static WIPE_OBSERVATIONS: core::cell::RefCell<Option<Vec<bool>>> = const {
        core::cell::RefCell::new(None)
    };
}

fn observe_cleared_cells(words: &[u16; SLOTS]) {
    WIPE_OBSERVATIONS.with_borrow_mut(|observations| {
        if let Some(observations) = observations {
            observations.push(words.iter().all(|&word| word == 0));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    const BROADCAST_EXPONENTS: [usize; 7] = [5, 25, 625, 5601, 4033, 3969, 8191];

    struct WipeObserver;

    impl WipeObserver {
        fn start() -> Self {
            WIPE_OBSERVATIONS.with_borrow_mut(|log| {
                assert!(log.is_none());
                *log = Some(Vec::new());
            });
            Self
        }

        fn assert_cleared(&self, expected: usize) {
            WIPE_OBSERVATIONS.with_borrow(|log| {
                let log = log.as_ref().unwrap();
                assert_eq!(log.len(), expected);
                assert!(log.iter().all(|&cleared| cleared));
            });
        }
    }

    impl Drop for WipeObserver {
        fn drop(&mut self) {
            WIPE_OBSERVATIONS.with_borrow_mut(|log| *log = None);
        }
    }

    #[test]
    fn canonical_owners_reject_length_and_late_invalid_values() {
        let observer = WipeObserver::start();
        assert!(matches!(
            ScalarSlots::copy_canonical(&[0; SLOTS - 1]),
            Err(PackingError::Length)
        ));
        assert!(matches!(
            SparsePlaintext::copy_canonical(&[0; SLOTS + 1]),
            Err(PackingError::Length)
        ));
        observer.assert_cleared(0);
        let mut input = [256; SLOTS];
        input[SLOTS - 1] = 257;
        assert!(matches!(
            ScalarSlots::copy_canonical(&input),
            Err(PackingError::NonCanonical { index: 127 })
        ));
        input[0] = u16::MAX;
        assert!(matches!(
            SparsePlaintext::copy_canonical(&input),
            Err(PackingError::NonCanonical { index: 0 })
        ));
        observer.assert_cleared(2);
        assert_eq!(input[SLOTS - 1], 257, "borrowed caller input is not erased");
    }

    #[test]
    fn every_basis_roundtrips_with_fixed_sparse_positions_and_mask_norm() {
        assert_eq!(power(ROOT, 128), 256);
        assert_eq!(mul(INVERSE_SLOTS, 128), 1);
        for slot in 0..SLOTS {
            let input = core::array::from_fn::<_, SLOTS, _>(|j| u16::from(j == slot));
            let values = ScalarSlots::copy_canonical(&input).unwrap();
            let encoded = SparsePlaintext::encode(&values);
            assert_eq!(*encoded.decode().0.0, input);
            assert_eq!(*encoded.0.0, *SparsePlaintext::mask(slot).unwrap().0.0);
            assert_eq!(
                encoded
                    .0
                    .0
                    .iter()
                    .map(|&x| usize::from(x.min(MODULUS - x)))
                    .sum::<usize>(),
                8256
            );
            for exponent in 0..RING_DEGREE {
                assert_eq!(
                    encoded.coefficient(exponent),
                    Some(if exponent % STRIDE == 0 {
                        encoded.0.0[exponent / STRIDE]
                    } else {
                        0
                    })
                );
            }
            assert_eq!(encoded.coefficient(RING_DEGREE), None);
        }
        assert!(matches!(
            SparsePlaintext::mask(SLOTS),
            Err(PackingError::Slot)
        ));
        assert!(matches!(
            SparsePlaintext::mask(usize::MAX),
            Err(PackingError::Slot)
        ));
    }

    #[test]
    fn encode_matches_independent_public_algebra_vector() {
        // Generated by the reviewed integer-only interpolation reference, not by this module.
        let expected = [
            192, 96, 183, 162, 205, 149, 213, 24, 215, 76, 129, 32, 248, 196, 26, 178, 45, 131,
            175, 158, 121, 177, 81, 11, 60, 192, 155, 117, 237, 151, 202, 203, 34, 35, 48, 7, 234,
            211, 94, 111, 63, 145, 101, 123, 83, 10, 77, 104, 85, 132, 210, 87, 21, 88, 240, 139,
            164, 154, 207, 219, 165, 255, 86, 98, 249, 98, 86, 255, 165, 219, 207, 154, 164, 139,
            240, 88, 21, 87, 210, 132, 85, 104, 77, 10, 83, 123, 101, 145, 63, 111, 94, 211, 234,
            7, 48, 35, 34, 203, 202, 151, 237, 117, 155, 192, 60, 11, 81, 177, 121, 158, 175, 131,
            45, 178, 26, 196, 248, 32, 129, 76, 215, 24, 213, 149, 205, 162, 183, 96,
        ];
        let input = core::array::from_fn::<_, SLOTS, _>(|j| j as u16);
        let encoded = SparsePlaintext::encode(&ScalarSlots::copy_canonical(&input).unwrap());
        assert_eq!(*encoded.0.0, expected);
        assert_eq!(
            *SparsePlaintext::copy_canonical(&expected)
                .unwrap()
                .decode()
                .0
                .0,
            input
        );
    }

    #[test]
    fn products_and_complete_selection_preserve_all_scalar_values() {
        let one = SparsePlaintext::encode(&ScalarSlots::copy_canonical(&[1; SLOTS]).unwrap());
        for offset in [0, 128, 256] {
            let condition = core::array::from_fn::<_, SLOTS, _>(|j| ((j + offset) % 257) as u16);
            let zero = core::array::from_fn::<_, SLOTS, _>(|j| ((7 * j + 2) % 257) as u16);
            let nonzero = core::array::from_fn::<_, SLOTS, _>(|j| ((j * j + 9) % 257) as u16);
            let a = SparsePlaintext::encode(&ScalarSlots::copy_canonical(&zero).unwrap());
            let b = SparsePlaintext::encode(&ScalarSlots::copy_canonical(&nonzero).unwrap());
            assert_eq!(
                *a.multiply(&b).decode().0.0,
                core::array::from_fn(|j| mul(zero[j], nonzero[j]))
            );
            let mut powered =
                SparsePlaintext::encode(&ScalarSlots::copy_canonical(&condition).unwrap());
            for _ in 0..8 {
                powered = powered.multiply(&powered);
            }
            powered = one.multiply(&powered);
            let indicator = one.subtract(&powered);
            assert_eq!(
                *indicator.decode().0.0,
                core::array::from_fn(|j| u16::from(condition[j] == 0))
            );
            let selected = b.add(&indicator.multiply(&a.subtract(&b)));
            assert_eq!(
                *selected.decode().0.0,
                core::array::from_fn(|j| if condition[j] == 0 {
                    zero[j]
                } else {
                    nonzero[j]
                })
            );
        }
        // An extension-field coordinate is not an admitted scalar slot.
        assert_eq!(sub(1, power(ROOT, 8)), 122);
    }

    #[test]
    fn fixed_automorphisms_broadcast_every_masked_slot() {
        for slot in 0..SLOTS {
            let mut encoded = SparsePlaintext::mask(slot).unwrap();
            for exponent in BROADCAST_EXPONENTS {
                let permuted = encoded.automorphism(exponent).unwrap();
                let slots = encoded.decode();
                assert_eq!(
                    *permuted.decode().0.0,
                    core::array::from_fn(|j| slots.0.0[((exponent * (2 * j + 1) - 1) / 2) % SLOTS])
                );
                encoded = encoded.add(&permuted);
            }
            assert_eq!(*encoded.decode().0.0, [1; SLOTS]);
        }
        let input = SparsePlaintext::mask(0).unwrap();
        for invalid in [0, 2, 8192, usize::MAX] {
            assert!(matches!(
                input.automorphism(invalid),
                Err(PackingError::Automorphism)
            ));
        }
        assert_eq!(*input.automorphism(1).unwrap().0.0, *input.0.0);
    }

    #[test]
    fn output_mask_clears_every_undeclared_slot() {
        let input = core::array::from_fn::<_, SLOTS, _>(|j| ((13 * j) % 257) as u16);
        let mask = core::array::from_fn::<_, SLOTS, _>(|j| u16::from(j < 64));
        let output = SparsePlaintext::encode(&ScalarSlots::copy_canonical(&input).unwrap())
            .multiply(&SparsePlaintext::encode(
                &ScalarSlots::copy_canonical(&mask).unwrap(),
            ));
        assert_eq!(
            *output.decode().0.0,
            core::array::from_fn(|j| if j < 64 { input[j] } else { 0 })
        );
    }

    #[test]
    fn owners_clear_live_cells_before_deallocation_on_success_and_unwind() {
        let observer = WipeObserver::start();
        {
            let values = ScalarSlots::copy_canonical(&[256; SLOTS]).unwrap();
            let encoded = SparsePlaintext::encode(&values);
            assert!(!format!("{values:?} {encoded:?}").contains("256"));
            assert!(format!("{values:?}").contains("REDACTED"));
        }
        observer.assert_cleared(2);
        let result = std::panic::catch_unwind(|| {
            let _owned = SparsePlaintext::copy_canonical(&[123; SLOTS]).unwrap();
            panic!("public fixture unwind");
        });
        assert!(result.is_err());
        observer.assert_cleared(3);
    }
}
