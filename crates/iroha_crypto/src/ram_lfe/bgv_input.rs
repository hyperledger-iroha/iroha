//! Closed, genuinely encrypted packed inputs shared by RAM-LFE evaluators.
//!
//! The sole constructor starts with an admitted byte input, zero-extends its
//! 64 reference slots to 128 scalar slots, and performs the canonical encoding
//! and local encryption, followed by actual modulus switching to an explicitly
//! admitted source level. General local ciphertext provenance alone does not
//! establish this packing invariant. No raw or local ciphertext import exists.
//!
//! This owner is local construction evidence, not imported input authentication,
//! a parameter qualification, circuit privacy or a committed execution proof.
//! TODO(R.7, R.11): complete those protocol and security requirements before
//! application activation. The input/key/profile privacy qualifications remain
//! separate from the arithmetic and encoding tested here.

use super::{RamLfeReferenceInputV1, clearing::ClearingArray};
use iroha_fhe::bgv::{
    BgvError, BgvProfile, BootstrapError, Ciphertext, CiphertextShape, LocalBootstrapCiphertext,
    LocalBootstrapKey, PlaintextPrecision, SlotEncoding,
};
use std::fmt;
use thiserror::Error;

const SLOTS: usize = 128;

/// A public profile, encoding, allocation or local encryption refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum RamLfeBgvInputErrorV1 {
    /// RAM-LFE input packing requires exactly 128 effective F257 scalar slots.
    #[error("RAM-LFE BGV input requires 128 effective F257 slots")]
    UnsupportedProfile,
    /// The fixed clearing slot allocation could not be reserved.
    #[error("RAM-LFE BGV input allocation failed")]
    Allocation,
    /// A canonical profile, precision, encoding or fresh-noise check failed.
    #[error(transparent)]
    Bgv(#[from] BgvError),
    /// Canonical local encryption failed, including entropy refusal.
    #[error(transparent)]
    Bootstrap(#[from] BootstrapError),
}

/// An honestly constructed local ciphertext with canonical RAM-LFE packing.
///
/// Slots 0..64 contain one admitted reference input and slots 64..128 are zero.
/// The immutable local key is borrowed for this owner's lifetime. There is no
/// cloning, raw-ciphertext constructor, local-ciphertext cast or mutable access.
/// Borrowing the ordinary ciphertext below cannot manufacture another owner.
///
/// The new 128-word staging allocation clears on every exit and is dropped as
/// soon as encoding finishes. The canonical plaintext and ciphertext owners
/// retain their existing clearing behavior. Compiler-created scalar copies in
/// registers or on the stack are outside this heap-ownership guarantee.
pub struct RamLfeBgvPackedInputV1<'key> {
    source: LocalBootstrapCiphertext<'key>,
}

impl<'key> RamLfeBgvPackedInputV1<'key> {
    /// Encode and encrypt admitted bytes at explicit effective F257 precision and level.
    ///
    /// Public key/profile, precision, slot geometry and the complete fresh-to-source
    /// noise plan are checked before reading private reference slots. Actual fresh
    /// encryption starts at the master profile's top level, then canonical modulus
    /// switching consumes it down to `source_level`; no shape or bound is relabeled.
    /// Master-scaled error and the zero-extended packing survive every switch.
    /// This local constructor does not establish an imported input proof or admit
    /// a complete wire envelope; those protocol requirements remain separate.
    /// The input constructor's byte admission is preserved; this path does not
    /// accept arbitrary field vectors, encoded polynomials or local ciphertexts.
    ///
    /// # Errors
    /// Refuses mismatched master profiles, a different effective modulus or slot
    /// count, an out-of-range or inadmissible source level, canonical encoding/noise
    /// failures, allocation or entropy failures.
    pub fn encrypt(
        profile: &BgvProfile,
        precision: PlaintextPrecision,
        source_level: usize,
        key: &'key LocalBootstrapKey,
        input: &RamLfeReferenceInputV1,
    ) -> Result<Self, RamLfeBgvInputErrorV1> {
        if key.profile_identity() != profile.identity()
            || precision.profile_identity() != profile.identity()
        {
            return Err(BgvError::ProfileMismatch.into());
        }
        if precision.modulus() != 257 {
            return Err(RamLfeBgvInputErrorV1::UnsupportedProfile);
        }
        let encoding = SlotEncoding::new(profile, precision)?;
        if encoding.slots() != SLOTS {
            return Err(RamLfeBgvInputErrorV1::UnsupportedProfile);
        }
        let expected =
            profile.plan_switch_to_level(&profile.plan_fresh(precision)?, source_level)?;
        let mut slots = ClearingArray::<u64, SLOTS>::try_zeroed()
            .map_err(|_| RamLfeBgvInputErrorV1::Allocation)?;
        for (to, &from) in slots.iter_mut().zip(input.slots()) {
            *to = u64::from(from);
        }
        #[cfg(test)]
        tests::checkpoint()?;
        let plaintext = encoding.encode(profile, &*slots)?;
        drop(slots);
        #[cfg(test)]
        tests::checkpoint()?;
        let source = key.encrypt(profile, &plaintext)?;
        drop(plaintext);
        #[cfg(test)]
        tests::checkpoint()?;
        let source = key.switch_to(profile, source, source_level)?;
        #[cfg(test)]
        tests::checkpoint()?;
        debug_assert_eq!(source.shape(), expected);
        Ok(Self { source })
    }

    // Only the canonical RAM-LFE evaluator consumes this construction capability.
    // No reverse conversion exists; ordinary outputs never regain byte admission.
    pub(super) fn into_local(self) -> LocalBootstrapCiphertext<'key> {
        self.source
    }

    /// Borrow the ordinary ciphertext without granting a packed-input constructor.
    #[must_use]
    pub fn ciphertext(&self) -> &Ciphertext {
        self.source.ciphertext()
    }

    /// Exact constructed shape, including master identity, source level and precision.
    #[must_use]
    pub fn shape(&self) -> CiphertextShape {
        self.source.shape()
    }
}

impl fmt::Debug for RamLfeBgvPackedInputV1<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[REDACTED RAM-LFE packed input]")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ram_lfe::clearing::cleared_cells;
    use iroha_fhe::bgv::{BgvParameters, LocalBootstrapSetupBudget, SecretKey, fixtures};
    use num_bigint::{BigInt, Sign};
    use std::cell::Cell;
    use zeroize::Zeroizing;

    thread_local! {
        static CALLS: Cell<usize> = const { Cell::new(0) };
        static FAULT: Cell<(usize, bool)> = const { Cell::new((usize::MAX, false)) };
    }

    pub(super) fn checkpoint() -> Result<(), RamLfeBgvInputErrorV1> {
        let call = CALLS.with(|v| {
            let next = v.get() + 1;
            v.set(next);
            next
        });
        let (at, unwind) = FAULT.with(Cell::get);
        if call == at {
            assert!(!unwind, "packed input lifecycle unwind control");
            return Err(RamLfeBgvInputErrorV1::Allocation);
        }
        Ok(())
    }

    fn reset() {
        CALLS.with(|v| v.set(0));
        FAULT.with(|v| v.set((usize::MAX, false)));
    }

    fn profile(log_degree: u32, exponent: u32) -> BgvProfile {
        BgvProfile::new(
            BgvParameters::derive(log_degree, 257, exponent, (50, 4), (60, 2), 2).unwrap(),
        )
        .unwrap()
    }

    fn key(profile: &BgvProfile) -> (SecretKey, LocalBootstrapKey) {
        LocalBootstrapKey::generate(
            profile,
            &[],
            LocalBootstrapSetupBudget {
                retained_residue_bytes: 1 << 20,
                returned_secret_bytes: 1 << 20,
                plaintext_bytes: 1 << 16,
            },
        )
        .unwrap()
    }

    // Independent integer CRT of the actual decryption phase; these synthetic
    // test BigInts are not production clearing owners and are never printed.
    fn assert_phase(profile: &BgvProfile, secret: &SecretKey, input: &RamLfeBgvPackedInputV1<'_>) {
        let cipher = input.ciphertext();
        let primes = &profile.parameters().main_primes[..=cipher.level()];
        let q = primes.iter().fold(BigInt::from(1), |a, &p| a * p);
        let half = (&q - 1_u32) / 2_u32;
        let basis: Vec<_> = primes
            .iter()
            .map(|&p| {
                let partial = &q / p;
                let inverse = partial.modpow(&BigInt::from(p - 2), &BigInt::from(p));
                partial * inverse
            })
            .collect();
        let declared = cipher
            .noise_bound()
            .as_natural()
            .words_le()
            .iter()
            .rev()
            .fold(BigInt::from(0), |value, &word| (value << 64) + word);
        assert!(declared <= half);
        let words = Zeroizing::new(fixtures::inner_product_words(profile, secret, cipher).unwrap());
        let plaintext = secret.decrypt(profile, cipher).unwrap();
        let t = BigInt::from(257);
        for i in 0..profile.degree() {
            let value = basis
                .iter()
                .enumerate()
                .fold(BigInt::from(0), |value, (j, basis)| {
                    value + basis * words[j * profile.degree() + i]
                })
                % &q;
            let centered = if value > half { value - &q } else { value };
            let magnitude = if centered.sign() == Sign::Minus {
                -&centered
            } else {
                centered.clone()
            };
            assert!(magnitude <= declared);
            let reduced = ((&centered % &t) + &t) % &t;
            assert_eq!(reduced, BigInt::from(plaintext.coefficients()[i]));
        }
    }

    #[test]
    fn genuine_packed_inputs_preserve_admitted_bytes_padding_context_and_master_phase() {
        reset();
        for log_degree in [7, 8] {
            for exponent in 1..=3 {
                let profile = profile(log_degree, exponent);
                let precision = profile.plaintext_precision(1).unwrap();
                let (secret, key) = key(&profile);
                assert_eq!(key.profile_identity(), profile.identity());
                let encoding = SlotEncoding::new(&profile, precision).unwrap();
                for bytes in [vec![], vec![0, 255, 1, 128], vec![255; 63]] {
                    let input = RamLfeReferenceInputV1::from_bytes(&bytes).unwrap();
                    let copied = RamLfeReferenceInputV1::from_slots(input.slots()).unwrap();
                    let before = cleared_cells();
                    let packed = RamLfeBgvPackedInputV1::encrypt(
                        &profile,
                        precision,
                        profile.top_level(),
                        &key,
                        &copied,
                    )
                    .unwrap();
                    assert_eq!(cleared_cells() - before, 128);
                    assert_eq!(packed.shape(), profile.plan_fresh(precision).unwrap());
                    assert_eq!(packed.shape().components(), 2);
                    assert_eq!(packed.shape().level(), profile.top_level());
                    assert_eq!(packed.shape().plaintext_precision(), precision);
                    let plain = secret.decrypt(&profile, packed.ciphertext()).unwrap();
                    let decoded = encoding.decode(&plain).unwrap();
                    assert_eq!(&decoded[..64], &copied.slots().map(u64::from));
                    assert!(decoded[64..].iter().all(|&x| x == 0));
                    let stride = profile.degree() / 128;
                    for (i, &coefficient) in plain.coefficients().iter().enumerate() {
                        if i % stride != 0 {
                            assert_eq!(coefficient, 0);
                        }
                    }
                    // Independent direct Vandermonde evaluation, with the
                    // canonical F257 root3, checks every slot without decode.
                    for slot in 0..128 {
                        let beta = (0..(2 * slot + 1)).fold(1_u64, |v, _| v * 3 % 257);
                        let actual = plain
                            .coefficients()
                            .iter()
                            .step_by(stride)
                            .rev()
                            .fold(0_u64, |v, &c| (v * beta + c) % 257);
                        let expected = if slot < 64 {
                            u64::from(copied.slots()[slot])
                        } else {
                            0
                        };
                        assert_eq!(actual, expected);
                    }
                    assert_phase(&profile, &secret, &packed);
                    assert_eq!(format!("{packed:?}"), "[REDACTED RAM-LFE packed input]");
                    let original = Zeroizing::new(packed.ciphertext().canonical_words());
                    let original_shape = packed.shape();
                    let moved = std::convert::identity(packed);
                    assert_eq!(moved.shape(), original_shape);
                    assert_eq!(
                        *Zeroizing::new(moved.ciphertext().canonical_words()),
                        *original
                    );
                    let independent_fresh = BigInt::from(128)
                        + BigInt::from(profile.master_plaintext_modulus())
                            * 21_u32
                            * BigInt::from(2 * profile.degree() + 1);
                    let bound = moved
                        .shape()
                        .bound()
                        .words_le()
                        .iter()
                        .rev()
                        .fold(BigInt::from(0), |a, &word| (a << 64) + word);
                    assert_eq!(bound, independent_fresh);
                }
            }
        }
        reset();
    }

    #[test]
    fn wrong_master_effective_precision_and_slot_count_precede_private_staging() {
        reset();
        let profile = profile(7, 3);
        let (_, key) = key(&profile);
        let foreign = self::profile(8, 3);
        let (_, foreign_key) = self::key(&foreign);
        let small = self::profile(6, 3);
        let (_, small_key) = self::key(&small);
        let input = RamLfeReferenceInputV1::from_bytes(&[255; 63]).unwrap();
        let before = cleared_cells();
        for (profile, precision, key, expected) in [
            (
                &profile,
                profile.plaintext_precision(2).unwrap(),
                &key,
                RamLfeBgvInputErrorV1::UnsupportedProfile,
            ),
            (
                &profile,
                profile.plaintext_precision(3).unwrap(),
                &key,
                RamLfeBgvInputErrorV1::UnsupportedProfile,
            ),
            (
                &profile,
                foreign.plaintext_precision(1).unwrap(),
                &key,
                BgvError::ProfileMismatch.into(),
            ),
            (
                &profile,
                profile.plaintext_precision(1).unwrap(),
                &foreign_key,
                BgvError::ProfileMismatch.into(),
            ),
            (
                &small,
                small.plaintext_precision(1).unwrap(),
                &small_key,
                RamLfeBgvInputErrorV1::UnsupportedProfile,
            ),
        ] {
            assert_eq!(
                RamLfeBgvPackedInputV1::encrypt(
                    profile,
                    precision,
                    profile.top_level(),
                    key,
                    &input
                )
                .unwrap_err(),
                expected
            );
            assert_eq!(CALLS.with(Cell::get), 0);
            assert_eq!(cleared_cells(), before);
        }
        reset();
    }

    #[test]
    fn invalid_length_byte_and_tail_cannot_reach_the_packed_constructor() {
        reset();
        let profile = profile(7, 1);
        let (_, key) = key(&profile);
        let precision = profile.plaintext_precision(1).unwrap();
        assert!(
            RamLfeReferenceInputV1::from_bytes(&[0; 64])
                .map(|input| {
                    RamLfeBgvPackedInputV1::encrypt(
                        &profile,
                        precision,
                        profile.top_level(),
                        &key,
                        &input,
                    )
                })
                .is_err()
        );
        let mut length = [0_u16; 64];
        length[0] = 64;
        let mut byte = [0_u16; 64];
        byte[0] = 1;
        byte[1] = 256;
        let mut tail = [0_u16; 64];
        tail[63] = 1;
        for values in [&length[..], &byte[..], &tail[..], &[0_u16; 63][..]] {
            assert!(
                RamLfeReferenceInputV1::from_slots(values)
                    .map(|input| {
                        RamLfeBgvPackedInputV1::encrypt(
                            &profile,
                            precision,
                            profile.top_level(),
                            &key,
                            &input,
                        )
                    })
                    .is_err()
            );
        }
        assert_eq!(CALLS.with(Cell::get), 0);
        reset();
    }

    fn assert_packed_slots(
        profile: &BgvProfile,
        secret: &SecretKey,
        packed: &RamLfeBgvPackedInputV1<'_>,
        input: &RamLfeReferenceInputV1,
    ) {
        let plain = secret.decrypt(profile, packed.ciphertext()).unwrap();
        let stride = profile.degree() / SLOTS;
        for (index, &coefficient) in plain.coefficients().iter().enumerate() {
            if index % stride != 0 {
                assert_eq!(coefficient, 0, "scalar embedding padding");
            }
        }
        // Independent direct Vandermonde evaluation, without SlotEncoding::decode.
        for slot in 0..SLOTS {
            let beta = (0..2 * slot + 1).fold(1_u64, |v, _| v * 3 % 257);
            let actual = plain
                .coefficients()
                .iter()
                .step_by(stride)
                .rev()
                .fold(0_u64, |v, &coefficient| (v * beta + coefficient) % 257);
            let expected = input.slots().get(slot).copied().map_or(0, u64::from);
            assert_eq!(actual, expected, "independent canonical packed slot");
        }
        assert_phase(profile, secret, packed);
    }

    #[test]
    fn every_admitted_source_level_preserves_slots_embedding_and_actual_phase() {
        reset();
        let input = RamLfeReferenceInputV1::from_bytes(&[0, 255, 173, 1, 128]).unwrap();
        for log_degree in [7, 8] {
            for exponent in 1..=3 {
                let profile = profile(log_degree, exponent);
                let precision = profile.plaintext_precision(1).unwrap();
                let (secret, key) = key(&profile);
                let fresh = profile.plan_fresh(precision).unwrap();
                for level in 0..=profile.top_level() {
                    let packed =
                        RamLfeBgvPackedInputV1::encrypt(&profile, precision, level, &key, &input)
                            .unwrap();
                    assert_eq!(
                        packed.shape(),
                        profile.plan_switch_to_level(&fresh, level).unwrap()
                    );
                    assert_eq!(packed.ciphertext().level(), level);
                    assert_packed_slots(&profile, &secret, &packed, &input);
                }
            }
        }
        reset();
    }

    #[test]
    fn invalid_source_level_refuses_before_private_staging_or_encryption() {
        reset();
        let profile = profile(7, 3);
        let (_, key) = key(&profile);
        let precision = profile.plaintext_precision(1).unwrap();
        let input = RamLfeReferenceInputV1::from_bytes(&[255; 63]).unwrap();
        let before = cleared_cells();
        for level in [profile.top_level() + 1, usize::MAX] {
            assert_eq!(
                RamLfeBgvPackedInputV1::encrypt(&profile, precision, level, &key, &input)
                    .unwrap_err(),
                BgvError::LevelMismatch {
                    lhs: profile.top_level(),
                    rhs: level,
                }
                .into()
            );
            assert_eq!(CALLS.with(Cell::get), 0);
            assert_eq!(cleared_cells(), before);
        }
        reset();
    }

    #[test]
    #[ignore = "unselected N65536 encrypted-input diagnostic; no imported proof or production qualification"]
    fn full_size_q23_honest_level_zero_input_fits_raw_frame_and_preserves_packing() {
        reset();
        let profile =
            BgvProfile::new(BgvParameters::derive(16, 257, 3, (60, 23), (60, 2), 2).unwrap())
                .unwrap();
        let precision = profile.plaintext_precision(1).unwrap();
        // Finite exact setup payload ceilings, separate from any wire/proof cap.
        let (secret, key) = LocalBootstrapKey::generate(
            &profile,
            &[],
            LocalBootstrapSetupBudget {
                retained_residue_bytes: 362_807_296,
                returned_secret_bytes: 13_631_488,
                plaintext_bytes: 524_288,
            },
        )
        .unwrap();
        let input = RamLfeReferenceInputV1::from_bytes(&[255; 63]).unwrap();
        let fresh = profile.plan_fresh(precision).unwrap();
        let expected = profile.plan_switch_to_level(&fresh, 0).unwrap();
        assert_eq!(expected.bound().words_le(), &[556_231_950_720]);
        let packed = RamLfeBgvPackedInputV1::encrypt(&profile, precision, 0, &key, &input).unwrap();
        assert_eq!(packed.shape(), expected);
        assert_packed_slots(&profile, &secret, &packed, &input);
        let bytes = Zeroizing::new(packed.ciphertext().to_bytes(&profile).unwrap());
        assert_eq!(bytes.len(), 983_252);
        assert!(bytes.len() < super::super::MAX_INPUT_BYTES);
        let decoded = Ciphertext::from_bytes(&profile, &bytes).unwrap();
        assert_eq!(decoded.shape(), packed.shape());
        assert_eq!(
            *Zeroizing::new(decoded.canonical_words()),
            *Zeroizing::new(packed.ciphertext().canonical_words())
        );
        // Ordinary decoding grants no packed-input capability. The complete
        // authenticated envelope and imported-key/input proofs remain open.
        reset();
    }

    #[test]
    fn new_private_slot_owner_clears_at_every_constructor_error_and_unwind_boundary() {
        reset();
        let profile = profile(7, 3);
        let (_, key) = key(&profile);
        let precision = profile.plaintext_precision(1).unwrap();
        let input = RamLfeReferenceInputV1::from_bytes(&[173; 63]).unwrap();
        for level in [profile.top_level(), 0] {
            for at in 1..=4 {
                for unwind in [false, true] {
                    reset();
                    FAULT.with(|v| v.set((at, unwind)));
                    let before = cleared_cells();
                    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                        RamLfeBgvPackedInputV1::encrypt(&profile, precision, level, &key, &input)
                    }));
                    if unwind {
                        assert!(result.is_err());
                    } else {
                        assert_eq!(
                            result.unwrap().unwrap_err(),
                            RamLfeBgvInputErrorV1::Allocation
                        );
                    }
                    assert_eq!(CALLS.with(Cell::get), at);
                    assert_eq!(cleared_cells() - before, 128);
                }
            }
        }
        reset();
    }
}
