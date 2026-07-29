//! Shared fail-closed health wrappers for privacy-prover entropy.
//!
//! The wrapper prefetches exactly 64 bytes, rejects catastrophic constant or
//! short-period streams, and then replays those exact bytes before delegating
//! to the source. Honest deterministic known-answer streams therefore do not
//! change, while a stuck RNG is rejected before witness-dependent proving.
//! Separate adapters preserve the crate's rand-core 0.6 curve API and its
//! rand 0.9 fallible producer API without silently bridging error semantics.

use rand::{TryCryptoRng, TryRngCore};
use rand_core_06::{CryptoRng, RngCore};
use sha2::{Digest as _, Sha256};
use thiserror::Error;
use zeroize::{Zeroize, Zeroizing};

const HEALTH_PREFIX_BYTES_V1: usize = 64;
const HEALTH_HALF_BYTES_V1: usize = HEALTH_PREFIX_BYTES_V1 / 2;
const PROHIBITED_PERIODS_V1: [usize; 6] = [1, 2, 4, 8, 16, 32];

/// Canonical producer policy committed by every curve-engine manifest.
pub(crate) const CURVE_PROVER_RANDOMNESS_POLICY_V1: &[u8] = b"prover-rng:fallible-prefix64:reject-constant-half+periods-1,2,4,8,16,32:zeroize-and-replay:v1";
/// Canonical rand 0.9 `TryCryptoRng` producer and seed-bridge policy.
pub(crate) const TRY_CRYPTO_PROVER_RANDOMNESS_POLICY_V1: &[u8] = b"prover-rng-api=rand0.9-TryCryptoRng:fallible-prefix64:reject-constant-half+periods-1,2,4,8,16,32:zeroize-and-exact-replay:seed-bridge=sha256-domain+all64:v1";

const TRY_CRYPTO_DERIVED_SEED_DOMAIN_V1: &[u8] =
    b"iroha:privacy:try-crypto-prover:derived-entropy-seed:v1";

/// Failure while preflighting one cryptographic prover random source.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum ProverRandomnessErrorV1 {
    /// The source returned an error, including after partially filling the prefix.
    #[error("cryptographic prover randomness is unavailable")]
    Unavailable,
    /// The prefix was constant or repeated with a prohibited short period.
    #[error("cryptographic prover randomness failed its health check")]
    Unhealthy,
}

/// Failure while preflighting or deriving rand 0.9 prover entropy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub(crate) enum TryCryptoProverRandomnessErrorV1 {
    /// The source failed, including after partially filling a destination.
    #[error("fallible cryptographic prover randomness is unavailable")]
    Unavailable,
    /// The source emitted a constant or prohibited short-period pattern.
    #[error("fallible cryptographic prover randomness failed its health check")]
    Unhealthy,
}

fn is_constant(bytes: &[u8]) -> bool {
    bytes
        .first()
        .is_some_and(|first| bytes.iter().all(|byte| byte == first))
}

fn repeats_with_period(bytes: &[u8], period: usize) -> bool {
    period < bytes.len()
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| *byte == bytes[index % period])
}

fn prefix_is_unhealthy_v1(prefix: &[u8; HEALTH_PREFIX_BYTES_V1]) -> bool {
    let (left, right) = prefix.split_at(HEALTH_HALF_BYTES_V1);
    is_constant(left)
        || is_constant(right)
        || PROHIBITED_PERIODS_V1
            .into_iter()
            .any(|period| repeats_with_period(prefix, period))
}

/// A cryptographic RNG that health-checks and faithfully replays its prefix.
///
/// All privacy prover code uses `try_fill_bytes`, retaining typed failure
/// propagation after the prefix. The infallible `RngCore` methods are provided
/// only because upstream curve APIs require the complete trait; they preserve
/// the source's pre-existing infallible failure semantics.
pub(crate) struct HealthCheckedCryptoRngV1<'a, R> {
    source: &'a mut R,
    prefix: Zeroizing<[u8; HEALTH_PREFIX_BYTES_V1]>,
    cursor: usize,
}

impl<'a, R> HealthCheckedCryptoRngV1<'a, R>
where
    R: CryptoRng + RngCore,
{
    /// Prefetch, health-check, and retain the exact first 64 source bytes.
    pub(crate) fn new(source: &'a mut R) -> Result<Self, ProverRandomnessErrorV1> {
        let mut prefix = Zeroizing::new([0_u8; HEALTH_PREFIX_BYTES_V1]);
        source
            .try_fill_bytes(&mut *prefix)
            .map_err(|_| ProverRandomnessErrorV1::Unavailable)?;
        if prefix_is_unhealthy_v1(&prefix) {
            return Err(ProverRandomnessErrorV1::Unhealthy);
        }
        Ok(Self {
            source,
            prefix,
            cursor: 0,
        })
    }

    fn copy_prefix(&mut self, destination: &mut [u8]) -> usize {
        let remaining = HEALTH_PREFIX_BYTES_V1.saturating_sub(self.cursor);
        let copied = remaining.min(destination.len());
        destination[..copied].copy_from_slice(&self.prefix[self.cursor..self.cursor + copied]);
        self.cursor += copied;
        copied
    }
}

impl<R> RngCore for HealthCheckedCryptoRngV1<'_, R>
where
    R: CryptoRng + RngCore,
{
    fn next_u32(&mut self) -> u32 {
        let mut bytes = [0_u8; 4];
        self.fill_bytes(&mut bytes);
        u32::from_le_bytes(bytes)
    }

    fn next_u64(&mut self) -> u64 {
        let mut bytes = [0_u8; 8];
        self.fill_bytes(&mut bytes);
        u64::from_le_bytes(bytes)
    }

    fn fill_bytes(&mut self, destination: &mut [u8]) {
        let copied = self.copy_prefix(destination);
        if copied < destination.len() {
            self.source.fill_bytes(&mut destination[copied..]);
        }
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
        let copied = self.copy_prefix(destination);
        if copied < destination.len() {
            self.source.try_fill_bytes(&mut destination[copied..])?;
        }
        Ok(())
    }
}

impl<R> CryptoRng for HealthCheckedCryptoRngV1<'_, R> where R: CryptoRng + RngCore {}

/// A rand 0.9 cryptographic RNG that health-checks and replays its exact prefix.
pub(crate) struct HealthCheckedTryCryptoRngV1<'a, R: ?Sized> {
    source: &'a mut R,
    prefix: Zeroizing<[u8; HEALTH_PREFIX_BYTES_V1]>,
    cursor: usize,
}

impl<'a, R> HealthCheckedTryCryptoRngV1<'a, R>
where
    R: TryCryptoRng + ?Sized,
{
    /// Check the exact first source block without changing its byte stream.
    pub(crate) fn new(source: &'a mut R) -> Result<Self, TryCryptoProverRandomnessErrorV1> {
        let mut prefix = Zeroizing::new([0_u8; HEALTH_PREFIX_BYTES_V1]);
        source
            .try_fill_bytes(prefix.as_mut())
            .map_err(|_| TryCryptoProverRandomnessErrorV1::Unavailable)?;
        if prefix_is_unhealthy_v1(&prefix) {
            return Err(TryCryptoProverRandomnessErrorV1::Unhealthy);
        }
        Ok(Self {
            source,
            prefix,
            cursor: 0,
        })
    }

    fn copy_prefix(&mut self, destination: &mut [u8]) -> usize {
        let remaining = HEALTH_PREFIX_BYTES_V1.saturating_sub(self.cursor);
        let copied = remaining.min(destination.len());
        destination[..copied].copy_from_slice(&self.prefix[self.cursor..self.cursor + copied]);
        self.cursor += copied;
        copied
    }

    /// Draw a separately health-checked seed after the replay prefix is spent.
    ///
    /// The fresh block must not repeat the initial prover prefix. This catches
    /// a source that cycles between otherwise non-periodic 64-byte blocks.
    pub(crate) fn derive_independent_seed_v1(
        &mut self,
        purpose: &[u8],
    ) -> Result<Zeroizing<[u8; 32]>, TryCryptoProverRandomnessErrorV1> {
        if self.cursor != HEALTH_PREFIX_BYTES_V1 {
            return Err(TryCryptoProverRandomnessErrorV1::Unhealthy);
        }
        let mut entropy = Zeroizing::new([0_u8; HEALTH_PREFIX_BYTES_V1]);
        self.try_fill_bytes(entropy.as_mut())
            .map_err(|_| TryCryptoProverRandomnessErrorV1::Unavailable)?;
        if prefix_is_unhealthy_v1(&entropy) || entropy.as_slice() == self.prefix.as_slice() {
            return Err(TryCryptoProverRandomnessErrorV1::Unhealthy);
        }
        derive_try_crypto_seed_from_block_v1(&entropy, purpose)
    }
}

impl<R> TryRngCore for HealthCheckedTryCryptoRngV1<'_, R>
where
    R: TryCryptoRng + ?Sized,
{
    type Error = R::Error;

    fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
        let mut bytes = [0_u8; 4];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }

    fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
        let mut bytes = [0_u8; 8];
        self.try_fill_bytes(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }

    fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
        let copied = self.copy_prefix(destination);
        if copied < destination.len() {
            self.source.try_fill_bytes(&mut destination[copied..])?;
        }
        Ok(())
    }
}

impl<R> TryCryptoRng for HealthCheckedTryCryptoRngV1<'_, R> where R: TryCryptoRng + ?Sized {}

/// Draw and domain-derive one 32-byte seed from an independent healthy block.
///
/// Every sampled byte influences the result. Both the source block and
/// derived seed are zeroized on every exit path.
pub(crate) fn derive_healthy_try_crypto_seed_v1<R: TryCryptoRng + ?Sized>(
    rng: &mut R,
    purpose: &[u8],
) -> Result<Zeroizing<[u8; 32]>, TryCryptoProverRandomnessErrorV1> {
    let mut entropy = Zeroizing::new([0_u8; HEALTH_PREFIX_BYTES_V1]);
    rng.try_fill_bytes(entropy.as_mut())
        .map_err(|_| TryCryptoProverRandomnessErrorV1::Unavailable)?;
    if prefix_is_unhealthy_v1(&entropy) {
        return Err(TryCryptoProverRandomnessErrorV1::Unhealthy);
    }
    derive_try_crypto_seed_from_block_v1(&entropy, purpose)
}

fn derive_try_crypto_seed_from_block_v1(
    entropy: &[u8; HEALTH_PREFIX_BYTES_V1],
    purpose: &[u8],
) -> Result<Zeroizing<[u8; 32]>, TryCryptoProverRandomnessErrorV1> {
    let mut hash = Sha256::new();
    hash.update(TRY_CRYPTO_DERIVED_SEED_DOMAIN_V1);
    hash.update(
        u64::try_from(purpose.len())
            .map_err(|_| TryCryptoProverRandomnessErrorV1::Unhealthy)?
            .to_be_bytes(),
    );
    hash.update(purpose);
    hash.update(entropy);
    let mut seed = Zeroizing::new(<[u8; 32]>::from(hash.finalize()));
    if seed.iter().all(|byte| *byte == 0) {
        seed.zeroize();
        return Err(TryCryptoProverRandomnessErrorV1::Unhealthy);
    }
    Ok(seed)
}

#[cfg(test)]
mod tests {
    use rand_08::{SeedableRng as _, rngs::StdRng};

    use super::*;

    #[derive(Clone, Copy)]
    enum Mode {
        Constant,
        ConstantLeftHalf,
        ConstantRightHalf,
        Period(usize),
        PartialFailure,
    }

    struct AdversarialRng(Mode);

    impl RngCore for AdversarialRng {
        fn next_u32(&mut self) -> u32 {
            0
        }

        fn next_u64(&mut self) -> u64 {
            0
        }

        fn fill_bytes(&mut self, destination: &mut [u8]) {
            self.try_fill_bytes(destination)
                .expect("infallible adversarial mode")
        }

        fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), rand_core_06::Error> {
            match self.0 {
                Mode::Constant => destination.fill(0x5A),
                Mode::ConstantLeftHalf => {
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = if index < HEALTH_HALF_BYTES_V1 {
                            0x5A
                        } else {
                            (index as u8).wrapping_mul(41).wrapping_add(3)
                        };
                    }
                }
                Mode::ConstantRightHalf => {
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = if index < HEALTH_HALF_BYTES_V1 {
                            (index as u8).wrapping_mul(41).wrapping_add(3)
                        } else {
                            0xA5
                        };
                    }
                }
                Mode::Period(period) => {
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = ((index % period) as u8).wrapping_mul(41).wrapping_add(3);
                    }
                }
                Mode::PartialFailure => {
                    let partial = destination.len() / 2;
                    destination
                        .iter_mut()
                        .take(partial)
                        .enumerate()
                        .for_each(|(index, byte)| *byte = index as u8);
                    return Err(rand_core_06::Error::new(
                        "injected partial prover entropy failure",
                    ));
                }
            }
            Ok(())
        }
    }

    impl CryptoRng for AdversarialRng {}

    #[test]
    fn healthy_wrapper_preserves_the_exact_source_stream_across_chunking() {
        let mut direct = StdRng::from_seed([0x39; 32]);
        let mut wrapped_source = StdRng::from_seed([0x39; 32]);
        let mut expected = [0_u8; 257];
        direct.fill_bytes(&mut expected);

        let mut wrapped =
            HealthCheckedCryptoRngV1::new(&mut wrapped_source).expect("healthy source");
        let mut actual = [0_u8; 257];
        wrapped.try_fill_bytes(&mut actual[..13]).expect("prefix");
        wrapped
            .try_fill_bytes(&mut actual[13..191])
            .expect("prefix and source");
        wrapped
            .try_fill_bytes(&mut actual[191..])
            .expect("source suffix");
        assert_eq!(actual, expected);
    }

    #[test]
    fn constant_short_period_and_partial_failure_are_rejected() {
        for mode in [
            Mode::Constant,
            Mode::ConstantLeftHalf,
            Mode::ConstantRightHalf,
            Mode::Period(1),
            Mode::Period(2),
            Mode::Period(4),
            Mode::Period(8),
            Mode::Period(16),
            Mode::Period(32),
        ] {
            assert!(matches!(
                HealthCheckedCryptoRngV1::new(&mut AdversarialRng(mode)),
                Err(ProverRandomnessErrorV1::Unhealthy)
            ));
        }
        assert!(matches!(
            HealthCheckedCryptoRngV1::new(&mut AdversarialRng(Mode::PartialFailure)),
            Err(ProverRandomnessErrorV1::Unavailable)
        ));
    }

    mod try_crypto {
        use rand::{RngCore as _, SeedableRng as _, TryCryptoRng, TryRngCore, rngs::StdRng};

        use super::super::*;

        #[derive(Clone, Copy)]
        enum TryMode {
            Constant,
            ConstantLeftHalf,
            ConstantRightHalf,
            Period(usize),
            PartialFailure,
        }

        #[derive(Debug)]
        struct InjectedTryEntropyError;

        impl core::fmt::Display for InjectedTryEntropyError {
            fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                formatter.write_str("injected fallible prover entropy failure")
            }
        }

        struct TryAdversarialRng(TryMode);

        impl TryRngCore for TryAdversarialRng {
            type Error = InjectedTryEntropyError;

            fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
                Err(InjectedTryEntropyError)
            }

            fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
                Err(InjectedTryEntropyError)
            }

            fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
                match self.0 {
                    TryMode::Constant => destination.fill(0x5A),
                    TryMode::ConstantLeftHalf => {
                        for (index, byte) in destination.iter_mut().enumerate() {
                            *byte = if index < HEALTH_HALF_BYTES_V1 {
                                0x5A
                            } else {
                                (index as u8).wrapping_mul(41).wrapping_add(3)
                            };
                        }
                    }
                    TryMode::ConstantRightHalf => {
                        for (index, byte) in destination.iter_mut().enumerate() {
                            *byte = if index < HEALTH_HALF_BYTES_V1 {
                                (index as u8).wrapping_mul(41).wrapping_add(3)
                            } else {
                                0xA5
                            };
                        }
                    }
                    TryMode::Period(period) => {
                        for (index, byte) in destination.iter_mut().enumerate() {
                            *byte = ((index % period) as u8).wrapping_mul(41).wrapping_add(3);
                        }
                    }
                    TryMode::PartialFailure => {
                        for (index, byte) in destination.iter_mut().take(17).enumerate() {
                            *byte = index as u8;
                        }
                        return Err(InjectedTryEntropyError);
                    }
                }
                Ok(())
            }
        }

        impl TryCryptoRng for TryAdversarialRng {}

        struct RepeatedHealthyBlockRng {
            cursor: usize,
        }

        impl TryRngCore for RepeatedHealthyBlockRng {
            type Error = InjectedTryEntropyError;

            fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
                Err(InjectedTryEntropyError)
            }

            fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
                Err(InjectedTryEntropyError)
            }

            fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
                for byte in destination {
                    *byte = ((self.cursor % HEALTH_PREFIX_BYTES_V1) as u8)
                        .wrapping_mul(73)
                        .wrapping_add(11);
                    self.cursor += 1;
                }
                Ok(())
            }
        }

        impl TryCryptoRng for RepeatedHealthyBlockRng {}

        struct AdversarialSecondBlockRng {
            mode: TryMode,
            fills: usize,
        }

        impl TryRngCore for AdversarialSecondBlockRng {
            type Error = InjectedTryEntropyError;

            fn try_next_u32(&mut self) -> Result<u32, Self::Error> {
                Err(InjectedTryEntropyError)
            }

            fn try_next_u64(&mut self) -> Result<u64, Self::Error> {
                Err(InjectedTryEntropyError)
            }

            fn try_fill_bytes(&mut self, destination: &mut [u8]) -> Result<(), Self::Error> {
                let fill = self.fills;
                self.fills = self.fills.saturating_add(1);
                if fill == 0 {
                    for (index, byte) in destination.iter_mut().enumerate() {
                        *byte = (index as u8).wrapping_mul(73).wrapping_add(11);
                    }
                    return Ok(());
                }
                TryAdversarialRng(self.mode).try_fill_bytes(destination)
            }
        }

        impl TryCryptoRng for AdversarialSecondBlockRng {}

        fn consume_checked_prefix_v1<R: TryCryptoRng + ?Sized>(
            checked: &mut HealthCheckedTryCryptoRngV1<'_, R>,
        ) {
            let mut replay = Zeroizing::new([0_u8; HEALTH_PREFIX_BYTES_V1]);
            checked
                .try_fill_bytes(replay.as_mut())
                .expect("replay initial block");
        }

        #[test]
        fn healthy_wrapper_preserves_the_exact_try_crypto_stream() {
            let mut direct = StdRng::from_seed([0x39; 32]);
            let mut wrapped_source = StdRng::from_seed([0x39; 32]);
            let mut expected = [0_u8; 257];
            direct.fill_bytes(&mut expected);
            let mut wrapped =
                HealthCheckedTryCryptoRngV1::new(&mut wrapped_source).expect("healthy source");
            let mut actual = [0_u8; 257];
            wrapped.try_fill_bytes(&mut actual[..13]).expect("prefix");
            wrapped
                .try_fill_bytes(&mut actual[13..191])
                .expect("prefix and source");
            wrapped
                .try_fill_bytes(&mut actual[191..])
                .expect("source suffix");
            assert_eq!(actual, expected);
        }

        #[test]
        fn constant_period_one_to_thirty_two_and_partial_try_sources_fail_closed() {
            for mode in [
                TryMode::Constant,
                TryMode::ConstantLeftHalf,
                TryMode::ConstantRightHalf,
                TryMode::Period(1),
                TryMode::Period(2),
                TryMode::Period(4),
                TryMode::Period(8),
                TryMode::Period(16),
                TryMode::Period(32),
            ] {
                assert_eq!(
                    HealthCheckedTryCryptoRngV1::new(&mut TryAdversarialRng(mode)).map(|_| ()),
                    Err(TryCryptoProverRandomnessErrorV1::Unhealthy)
                );
                assert_eq!(
                    derive_healthy_try_crypto_seed_v1(
                        &mut TryAdversarialRng(mode),
                        b"adversarial-test",
                    )
                    .map(|_| ()),
                    Err(TryCryptoProverRandomnessErrorV1::Unhealthy)
                );
            }
            assert_eq!(
                HealthCheckedTryCryptoRngV1::new(&mut TryAdversarialRng(TryMode::PartialFailure,))
                    .map(|_| ()),
                Err(TryCryptoProverRandomnessErrorV1::Unavailable)
            );
            assert_eq!(
                derive_healthy_try_crypto_seed_v1(
                    &mut TryAdversarialRng(TryMode::PartialFailure),
                    b"partial-test",
                )
                .map(|_| ()),
                Err(TryCryptoProverRandomnessErrorV1::Unavailable)
            );
        }

        #[test]
        fn independent_seed_rejects_reused_and_adversarial_second_blocks() {
            let mut repeated_source = RepeatedHealthyBlockRng { cursor: 0 };
            let mut repeated = HealthCheckedTryCryptoRngV1::new(&mut repeated_source)
                .expect("initial block is healthy");
            consume_checked_prefix_v1(&mut repeated);
            assert_eq!(
                repeated
                    .derive_independent_seed_v1(b"independent-test")
                    .map(|_| ()),
                Err(TryCryptoProverRandomnessErrorV1::Unhealthy)
            );

            let mut early_source = AdversarialSecondBlockRng {
                mode: TryMode::Constant,
                fills: 0,
            };
            let mut early = HealthCheckedTryCryptoRngV1::new(&mut early_source)
                .expect("initial block is healthy");
            assert_eq!(
                early
                    .derive_independent_seed_v1(b"early-independent-seed")
                    .map(|_| ()),
                Err(TryCryptoProverRandomnessErrorV1::Unhealthy)
            );

            for (mode, expected) in [
                (
                    TryMode::PartialFailure,
                    TryCryptoProverRandomnessErrorV1::Unavailable,
                ),
                (
                    TryMode::Constant,
                    TryCryptoProverRandomnessErrorV1::Unhealthy,
                ),
                (
                    TryMode::Period(8),
                    TryCryptoProverRandomnessErrorV1::Unhealthy,
                ),
                (
                    TryMode::Period(32),
                    TryCryptoProverRandomnessErrorV1::Unhealthy,
                ),
            ] {
                let mut source = AdversarialSecondBlockRng { mode, fills: 0 };
                let mut checked = HealthCheckedTryCryptoRngV1::new(&mut source)
                    .expect("initial block is healthy");
                consume_checked_prefix_v1(&mut checked);
                assert_eq!(
                    checked
                        .derive_independent_seed_v1(b"adversarial-second-block")
                        .map(|_| ()),
                    Err(expected)
                );
            }
        }

        #[test]
        fn independent_seed_is_deterministic_and_purpose_separated() {
            let mut first_source = StdRng::from_seed([0x63; 32]);
            let mut second_source = StdRng::from_seed([0x63; 32]);
            let mut third_source = StdRng::from_seed([0x63; 32]);
            let mut first =
                HealthCheckedTryCryptoRngV1::new(&mut first_source).expect("healthy source");
            let mut second =
                HealthCheckedTryCryptoRngV1::new(&mut second_source).expect("healthy source");
            let mut third =
                HealthCheckedTryCryptoRngV1::new(&mut third_source).expect("healthy source");
            consume_checked_prefix_v1(&mut first);
            consume_checked_prefix_v1(&mut second);
            consume_checked_prefix_v1(&mut third);
            let first_seed = first
                .derive_independent_seed_v1(b"authorization")
                .expect("healthy independent seed");
            let second_seed = second
                .derive_independent_seed_v1(b"authorization")
                .expect("healthy independent seed");
            let third_seed = third
                .derive_independent_seed_v1(b"wallet-encryption")
                .expect("healthy independent seed");
            assert_eq!(*first_seed, *second_seed);
            assert_ne!(*first_seed, *third_seed);
        }
    }
}
