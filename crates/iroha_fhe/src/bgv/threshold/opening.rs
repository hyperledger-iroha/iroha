//! Local opening by one trusted owner of an exact consumed additive subset.
//!
//! This module does not export partial decryptions, reconstruct the full secret
//! key, or authenticate a ciphertext's registered key, purpose, epoch or request.
//! TODO(C.4): distributed malicious-party-secure opening needs reviewed masked
//! contributions, ciphertext-bound proofs, authorization and replay handling.

use super::{SubsetSecretKeyShare, ThresholdError, ThresholdSelection};
use crate::bgv::{
    BgvError, BgvProfile, Ciphertext, Plaintext,
    cipher::decode_inner_product,
    fault::{self, FaultPoint},
    poly::RnsPoly,
};

/// An opening refusal that retains every original, unmodified share owner.
///
/// Inspect [`Self::reason`] and recover the shares with [`Self::into_parts`] to
/// retry. Dropping this error clears the shares, as does a successful opening.
#[derive(Debug)]
pub struct ThresholdOpeningError {
    reason: ThresholdError,
    shares: Vec<SubsetSecretKeyShare>,
}

impl ThresholdOpeningError {
    /// Public reason for the refusal.
    #[must_use]
    pub const fn reason(&self) -> ThresholdError {
        self.reason
    }

    /// Recover the reason and the original ordered vector of unchanged shares.
    #[must_use]
    pub fn into_parts(self) -> (ThresholdError, Vec<SubsetSecretKeyShare>) {
        (self.reason, self.shares)
    }
}

impl std::fmt::Display for ThresholdOpeningError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.reason, formatter)
    }
}

impl std::error::Error for ThresholdOpeningError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.reason)
    }
}

impl ThresholdSelection<'_> {
    /// Peak owned polynomial-word bytes during [`Self::open_local`].
    ///
    /// Counts every retained full main-plus-special input share and three
    /// current-level polynomials: the transformed public ciphertext component,
    /// the accumulator, and one secret-share product. NTTs are in-place. Before
    /// centered conversion, the public transformed component is dropped; the
    /// accumulator plus plaintext output fit below the three-polynomial peak.
    /// Conversion uses an existing cleared stack array, not heap scratch.
    ///
    /// Excludes the input ciphertext, profile, public metadata, share handles,
    /// allocator overhead and stack/register storage. This is a word-buffer
    /// ceiling, not a process-memory estimate. It depends only on public inputs.
    ///
    /// # Errors
    /// Another profile, an invalid level, or checked size overflow.
    pub fn required_opening_secret_bytes(
        &self,
        profile: &BgvProfile,
        level: usize,
    ) -> Result<usize, ThresholdError> {
        if profile.identity() != &self.context.profile {
            return Err(ThresholdError::ProfileMismatch);
        }
        profile.level(level)?;
        let full_limbs = profile.top_level() + 1 + profile.special_count();
        let retained_words = full_limbs
            .checked_mul(profile.degree())
            .and_then(|words| words.checked_mul(self.participants.len()))
            .ok_or(ThresholdError::SizeOverflow)?;
        let scratch_words = (level + 1)
            .checked_mul(profile.degree())
            .and_then(|words| words.checked_mul(3))
            .ok_or(ThresholdError::SizeOverflow)?;
        retained_words
            .checked_add(scratch_words)
            .and_then(|words| words.checked_mul(size_of::<u64>()))
            .ok_or(ThresholdError::SizeOverflow)
    }

    /// Consume the exact ordered selected subset and open a two-component ciphertext.
    ///
    /// Intended only for a trusted, co-located owner of dealer-issued shares.
    /// Forms `c0 + sum(c1 * lambda_i * s_i)` over the ciphertext's main-prime
    /// basis and reduces its centered representative modulo the plaintext
    /// modulus, using the same arithmetic as [`crate::bgv::SecretKey::decrypt`].
    /// No complete secret key or per-participant decryption is returned.
    /// Three-component tensors must first be relinearized by their owner.
    ///
    /// Share metadata must exactly match this selection. These local checks do
    /// not authenticate a registered encryption key, ciphertext provenance,
    /// application purpose, authorized epoch or decryption request. As with
    /// ordinary secret-key decryption, a plaintext-correctness claim requires
    /// well-formed inputs whose true noise is within capacity. Do not expose
    /// this operation as an untrusted decryption service.
    ///
    /// All preflight checks occur before new polynomial allocation. Success
    /// consumes and clears all shares. Any error returns the original vector
    /// and unmodified shares; temporary polynomials clear on error or unwind.
    ///
    /// # Errors
    /// Wrong profile, level, component/word shape, share count/order/context,
    /// sharing instance or selected subset; insufficient word-buffer ceiling;
    /// or failure of an existing BGV arithmetic kernel.
    pub fn open_local(
        &self,
        profile: &BgvProfile,
        ciphertext: &Ciphertext,
        shares: Vec<SubsetSecretKeyShare>,
        max_secret_bytes: usize,
    ) -> Result<Plaintext, ThresholdOpeningError> {
        let result = self
            .validate_opening(profile, ciphertext, &shares, max_secret_bytes)
            .and_then(|()| self.open_validated(profile, ciphertext, &shares));
        match result {
            Ok(plaintext) => Ok(plaintext),
            Err(reason) => Err(ThresholdOpeningError { reason, shares }),
        }
    }

    fn validate_opening(
        &self,
        profile: &BgvProfile,
        ciphertext: &Ciphertext,
        shares: &[SubsetSecretKeyShare],
        max_secret_bytes: usize,
    ) -> Result<(), ThresholdError> {
        let required = self.required_opening_secret_bytes(profile, ciphertext.level())?;
        if ciphertext.profile_identity() != profile.identity() {
            return Err(ThresholdError::ProfileMismatch);
        }
        if ciphertext.component_count() != 2 {
            return Err(BgvError::ComponentCount {
                found: ciphertext.component_count(),
            }
            .into());
        }
        let limbs = ciphertext.level() + 1;
        let full_limbs = profile.top_level() + 1 + profile.special_count();
        for component in ciphertext.components() {
            check_shape(component, limbs, profile.degree())?;
        }
        if shares.len() < self.participants.len() {
            return Err(ThresholdError::InsufficientShares);
        }
        if shares.len() > self.participants.len() {
            return Err(ThresholdError::TooManyShares);
        }
        for pair in shares.windows(2) {
            if pair[0].participant == pair[1].participant {
                return Err(ThresholdError::DuplicateParticipant);
            }
            if pair[0].participant > pair[1].participant {
                return Err(ThresholdError::InvalidRoster);
            }
        }
        for (share, &participant) in shares.iter().zip(self.participants) {
            if share.context.profile != *profile.identity() {
                return Err(ThresholdError::ProfileMismatch);
            }
            if share.context.identity != self.context.identity {
                return Err(ThresholdError::ContextMismatch);
            }
            if share.instance != self.instance {
                return Err(ThresholdError::InstanceMismatch);
            }
            if share.selection_identity != self.identity {
                return Err(ThresholdError::SelectionMismatch);
            }
            if share.participant != participant {
                return Err(ThresholdError::ParticipantNotSelected);
            }
            check_shape(&share.words, full_limbs, profile.degree())?;
        }
        if required > max_secret_bytes {
            return Err(ThresholdError::SecretBufferLimit {
                required,
                limit: max_secret_bytes,
            });
        }
        Ok(())
    }

    fn open_validated(
        &self,
        profile: &BgvProfile,
        ciphertext: &Ciphertext,
        shares: &[SubsetSecretKeyShare],
    ) -> Result<Plaintext, ThresholdError> {
        let limbs = ciphertext.level() + 1;
        let plan_of = |limb: usize| profile.main_plan(limb);
        let mut c1 = ciphertext.components()[1].clone();
        c1.forward(plan_of)?;
        let mut inner = RnsPoly::zero(limbs, profile.degree());
        for (index, share) in shares.iter().enumerate() {
            let mut product = share.words.prefix(limbs);
            product.forward(plan_of)?;
            product.pointwise_mul_assign(&c1, plan_of)?;
            inner.add_assign(&product, plan_of)?;
            if index == 0 {
                fault::reached(FaultPoint::ThresholdOpeningAfterFirstProduct);
            }
        }
        drop(c1);
        inner.inverse(plan_of)?;
        inner.add_assign(&ciphertext.components()[0], plan_of)?;
        fault::reached(FaultPoint::ThresholdOpeningAfterInnerProduct);
        Ok(decode_inner_product(
            profile,
            ciphertext.plaintext_precision(),
            ciphertext.level(),
            &inner,
        )?)
    }
}

/// Validate both limb count and degree before taking prefixes or transforming.
fn check_shape(words: &RnsPoly, limbs: usize, degree: usize) -> Result<(), ThresholdError> {
    let expected = limbs
        .checked_mul(degree)
        .ok_or(ThresholdError::SizeOverflow)?;
    if words.words().len() != expected {
        return Err(BgvError::LengthMismatch {
            expected,
            found: words.words().len(),
        }
        .into());
    }
    if words.limbs() != limbs {
        return Err(BgvError::LengthMismatch {
            expected: limbs,
            found: words.limbs(),
        }
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn internal_shape_check_rejects_wrong_word_count_and_degree() {
        let correct = RnsPoly::zero(2, 16);
        assert_eq!(check_shape(&correct, 2, 16), Ok(()));
        assert_eq!(
            check_shape(&correct, 3, 16),
            Err(ThresholdError::Bgv(BgvError::LengthMismatch {
                expected: 48,
                found: 32
            }))
        );
        let wrong_degree = RnsPoly::zero(4, 8);
        assert_eq!(
            check_shape(&wrong_degree, 2, 16),
            Err(ThresholdError::Bgv(BgvError::LengthMismatch {
                expected: 2,
                found: 4
            }))
        );
        assert_eq!(
            check_shape(&correct, usize::MAX, 16),
            Err(ThresholdError::SizeOverflow)
        );
    }

    #[test]
    fn malformed_internal_cipher_and_share_shapes_are_refused_before_arithmetic() {
        use super::super::{DealerKeyShares, ThresholdContext};
        use crate::bgv::{
            BgvParameters,
            fixtures::{self, FixtureEntropy},
        };
        let profile =
            BgvProfile::new(BgvParameters::derive(4, 257, 1, (40, 2), (41, 1), 1).expect("params"))
                .expect("profile");
        let context = ThresholdContext::new(&profile, [1; 32], 1, 2, vec![1, 2]).expect("context");
        let key = fixtures::secret_key(&profile, &mut FixtureEntropy::new([1; 64])).expect("key");
        let limit = DealerKeyShares::required_secret_bytes(&profile, &context).expect("budget");
        let mut dealer = fixtures::threshold_dealer(
            &profile,
            context.clone(),
            key,
            limit,
            &mut FixtureEntropy::new([2; 64]),
        )
        .expect("dealer");
        let plan = ThresholdSelection::new(&profile, &context, *dealer.instance(), &[1, 2])
            .expect("selection");
        let mut local: Vec<_> = [1, 2]
            .into_iter()
            .map(|participant| {
                dealer
                    .take_share(participant)
                    .expect("share")
                    .try_into_subset_additive(&profile, &plan)
                    .expect("conversion")
            })
            .collect();
        // Same total word count but a wrong ring degree must fail before NTT.
        let malformed = Ciphertext::from_shape(
            &profile,
            profile
                .plan_trivial(profile.master_plaintext(), 0)
                .expect("shape"),
            vec![RnsPoly::zero(2, 8), RnsPoly::zero(1, 16)],
        );
        let error = plan
            .open_local(&profile, &malformed, local, usize::MAX)
            .expect_err("wrong component degree");
        assert_eq!(
            error.reason(),
            ThresholdError::Bgv(BgvError::LengthMismatch {
                expected: 1,
                found: 2
            })
        );
        (_, local) = error.into_parts();
        // A coherent tall-profile ciphertext exercises the target level preflight.
        // The opening checks its requested level before the ciphertext profile.
        let taller =
            BgvProfile::new(BgvParameters::derive(4, 257, 1, (40, 3), (41, 1), 1).expect("params"))
                .expect("taller profile");
        let wrong_level = Ciphertext::from_shape(
            &taller,
            taller
                .plan_trivial(taller.master_plaintext(), 2)
                .expect("tall shape"),
            vec![RnsPoly::zero(3, 16), RnsPoly::zero(3, 16)],
        );
        assert_eq!(wrong_level.profile_identity(), taller.identity());
        let error = plan
            .open_local(&profile, &wrong_level, local, usize::MAX)
            .expect_err("wrong level");
        assert_eq!(
            error.reason(),
            ThresholdError::Bgv(BgvError::LevelOutOfRange { level: 2, top: 1 })
        );
        (_, local) = error.into_parts();
        let plain = Plaintext::constant(&profile, profile.master_plaintext(), 1).expect("plain");
        let valid = Ciphertext::trivial(&profile, &plain, 0).expect("valid cipher");
        local[0].words = RnsPoly::zero(6, 8);
        let error = plan
            .open_local(&profile, &valid, local, usize::MAX)
            .expect_err("wrong share degree");
        assert_eq!(
            error.reason(),
            ThresholdError::Bgv(BgvError::LengthMismatch {
                expected: 3,
                found: 6
            })
        );
    }
}
