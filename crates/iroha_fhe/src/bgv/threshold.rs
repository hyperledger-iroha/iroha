//! Offline dealer-assisted custody and local consumed-subset opening of a BGV key.
//!
//! A trusted dealer consumes the complete key and samples independent Shamir
//! polynomials over every main and special prime. The declared threshold and
//! roster have no relationship to consensus quorum rules. Context and a fresh
//! sharing-instance identifier prevent accidental mixing of epochs or dealer
//! invocations. Each participant receives at most one opaque share from this
//! in-memory dealer. All secret buffers use the existing clearing owners.
//!
//! There is no share codec, untrusted import, exported decryption contribution,
//! public reconstruction of a secret key, or network integration. Selection
//! checks cover metadata only; they are not algebraic proofs. Test fixtures alone expose share words
//! for independent interpolation. No BGV profile is qualified by this module.
//!
//! A canonical public selection can consume a local share into a subset-additive
//! share, in place. This private arithmetic neither releases a contribution nor
//! establishes a reusable, authenticated or retry-secure opening protocol.
//! [`ThresholdSelection::open_local`] consumes the exact selected shares under
//! one trusted, co-located owner and returns only the plaintext. It is not a
//! distributed threshold-decryption protocol or an authorization boundary.
// TODO(C.4): reviewed malicious-party-secure key generation, authenticated durable
// custody, masked ciphertext-bound contributions and zero-knowledge share verification.
mod opening;

pub use opening::ThresholdOpeningError;

use std::sync::Arc;

use super::{
    BgvError, BgvProfile, SecretKey,
    coins::{COIN_STATE_BYTES, CoinPurpose, Coins, EntropySource, OsEntropy, small_residue},
    fault::{self, FaultPoint},
    poly::RnsPoly,
};
use thiserror::Error;

/// Domain separator for the public threshold context.
pub const THRESHOLD_CONTEXT_DOMAIN: &str = "iroha_fhe bgv v1 offline threshold context";

/// Domain separator for a canonical public subset of one offline sharing instance.
pub const THRESHOLD_SELECTION_DOMAIN: &str = "iroha_fhe bgv v1 offline threshold selection";

/// A refusal by offline key sharing or local consumed-subset opening.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Error)]
pub enum ThresholdError {
    /// Threshold is zero or greater than the participant count.
    #[error("threshold must be in 1..=participant count")]
    InvalidThreshold,
    /// The roster is empty, too long, unsorted, or has zero/duplicate coordinates.
    #[error("participant coordinates must be nonzero, distinct and strictly ascending")]
    InvalidRoster,
    /// A participant coordinate is not below every RNS prime.
    #[error("participant coordinate is outside an RNS prime field")]
    CoordinateOutOfField,
    /// The supplied profile or key belongs to another profile.
    #[error("threshold context or key belongs to another profile")]
    ProfileMismatch,
    /// Secret-buffer size arithmetic overflowed.
    #[error("threshold secret-buffer size overflow")]
    SizeOverflow,
    /// The caller's secret-buffer ceiling is insufficient.
    #[error("threshold requires {required} secret-buffer bytes, limit is {limit}")]
    SecretBufferLimit {
        /// Peak secret-buffer bytes accounted for by this component.
        required: usize,
        /// Caller-supplied limit.
        limit: usize,
    },
    /// A coordinate is absent from the declared roster.
    #[error("participant is absent from the threshold roster")]
    UnknownParticipant,
    /// The local share's participant is absent from this canonical selected subset.
    #[error("participant is absent from the selected threshold subset")]
    ParticipantNotSelected,
    /// This in-memory dealer has already issued this participant's share.
    #[error("participant share has already been issued")]
    AlreadyIssued,
    /// Fewer shares than the threshold or exact selected subset requires.
    #[error("fewer shares than the threshold or exact selected subset requires")]
    InsufficientShares,
    /// More shares than the roster or exact selected subset permits.
    #[error("more shares than the roster or exact selected subset permits")]
    TooManyShares,
    /// The same participant occurs twice in a selection.
    #[error("selection repeats a participant")]
    DuplicateParticipant,
    /// A selected share has another application context, epoch, threshold or roster.
    #[error("share belongs to another threshold context")]
    ContextMismatch,
    /// A selected share was issued by another dealer invocation.
    #[error("share belongs to another sharing instance")]
    InstanceMismatch,
    /// A share was weighted for another selected subset.
    #[error("share belongs to another selected threshold subset")]
    SelectionMismatch,
    /// A BGV operation failed; no fallback arithmetic or entropy is used.
    #[error(transparent)]
    Bgv(#[from] BgvError),
}

/// Public, immutable declaration of one offline sharing context.
///
/// `key_context` identifies the application/key purpose; it is caller-provided
/// metadata, not proof of a registered public key. `epoch` is equally explicit:
/// this component neither chooses nor advances an application's epoch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ThresholdContext {
    profile: [u8; 32],
    key_context: [u8; 32],
    epoch: u64,
    threshold: u16,
    participants: Vec<u16>,
    identity: [u8; 32],
}

impl ThresholdContext {
    /// Validate a declared `threshold` of the listed participants.
    ///
    /// Coordinates are nonzero field elements in strictly ascending order.
    /// A threshold of one is permitted and provides no threshold secrecy.
    ///
    /// # Errors
    /// Invalid threshold, roster, or coordinates outside any profile prime.
    pub fn new(
        profile: &BgvProfile,
        key_context: [u8; 32],
        epoch: u64,
        threshold: u16,
        participants: Vec<u16>,
    ) -> Result<Self, ThresholdError> {
        if participants.is_empty()
            || participants.len() > usize::from(u16::MAX)
            || participants[0] == 0
            || participants.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(ThresholdError::InvalidRoster);
        }
        if threshold == 0 || usize::from(threshold) > participants.len() {
            return Err(ThresholdError::InvalidThreshold);
        }
        let largest = u64::from(participants[participants.len() - 1]);
        let parameters = profile.parameters();
        if parameters
            .main_primes
            .iter()
            .chain(&parameters.special_primes)
            .any(|&prime| largest >= prime)
        {
            return Err(ThresholdError::CoordinateOutOfField);
        }
        // Only public data enters this hasher; no additional secret-state owner.
        let mut hasher = blake3::Hasher::new_derive_key(THRESHOLD_CONTEXT_DOMAIN);
        hasher.update(&[1]);
        hasher.update(profile.identity());
        hasher.update(&key_context);
        hasher.update(&epoch.to_le_bytes());
        hasher.update(&threshold.to_le_bytes());
        hasher.update(
            &u16::try_from(participants.len())
                .map_err(|_| ThresholdError::InvalidRoster)?
                .to_le_bytes(),
        );
        for participant in &participants {
            hasher.update(&participant.to_le_bytes());
        }
        Ok(Self {
            profile: *profile.identity(),
            key_context,
            epoch,
            threshold,
            participants,
            identity: *hasher.finalize().as_bytes(),
        })
    }

    /// Digest of the exact profile, key context, epoch, threshold and roster.
    #[must_use]
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    /// The application-provided key context; not a public-key validation result.
    #[must_use]
    pub const fn key_context(&self) -> &[u8; 32] {
        &self.key_context
    }

    /// The explicitly declared epoch.
    #[must_use]
    pub const fn epoch(&self) -> u64 {
        self.epoch
    }

    /// The explicitly declared reconstruction threshold.
    #[must_use]
    pub const fn threshold(&self) -> u16 {
        self.threshold
    }

    /// The ascending, distinct field coordinates of all participants.
    #[must_use]
    pub fn participants(&self) -> &[u16] {
        &self.participants
    }

    /// Check selection metadata against this context and a retained instance ID.
    ///
    /// This does not inspect secret words or prove a decryption relation. It
    /// accepts any distinct subset of size at least the declared threshold,
    /// including a superset. Repeated calls are not durable replay protection.
    ///
    /// # Errors
    /// Insufficient/excessive shares, another context/instance, or a missing or
    /// repeated participant. No selected share is consumed on refusal.
    pub fn validate_selection_metadata(
        &self,
        instance: &[u8; 32],
        shares: &[&SecretKeyShare],
    ) -> Result<(), ThresholdError> {
        if shares.len() < usize::from(self.threshold) {
            return Err(ThresholdError::InsufficientShares);
        }
        if shares.len() > self.participants.len() {
            return Err(ThresholdError::TooManyShares);
        }
        let mut seen = std::collections::BTreeSet::new();
        for share in shares {
            if self.identity() != share.context.identity() {
                return Err(ThresholdError::ContextMismatch);
            }
            if instance != &share.instance {
                return Err(ThresholdError::InstanceMismatch);
            }
            if self.participants.binary_search(&share.participant).is_err() {
                return Err(ThresholdError::UnknownParticipant);
            }
            if !seen.insert(share.participant) {
                return Err(ThresholdError::DuplicateParticipant);
            }
        }
        Ok(())
    }
}

/// One participant's opaque, non-Clone secret share; its word buffer clears on drop.
///
/// Shares have no production export/import or reconstruction interface.
///
/// ```compile_fail
/// use iroha_fhe::bgv::threshold::SecretKeyShare;
/// fn duplicate(share: &SecretKeyShare) -> SecretKeyShare {
///     (*share).clone()
/// }
/// ```
pub struct SecretKeyShare {
    context: Arc<ThresholdContext>,
    instance: [u8; 32],
    participant: u16,
    words: RnsPoly,
}

/// A borrowed, immutable subset of one offline sharing context and instance.
///
/// The subset is explicitly supplied in strictly ascending order. Its identity
/// binds public metadata only, not a ciphertext, an authenticated key registration
/// or permission to disclose a contribution. Reusing this plan is not replay
/// protection. The borrowed coordinates cannot change while this plan exists.
#[derive(Debug)]
pub struct ThresholdSelection<'a> {
    context: &'a ThresholdContext,
    instance: [u8; 32],
    participants: &'a [u16],
    identity: [u8; 32],
}

impl<'a> ThresholdSelection<'a> {
    /// Validate a canonical subset without allocating or handling secret material.
    ///
    /// Every size from the declared threshold through the complete roster is
    /// supported; no participant is silently removed or reordered.
    ///
    /// # Errors
    /// Another profile, insufficient/excess participants, repeated or unsorted
    /// coordinates, or a coordinate absent from the context's roster.
    pub fn new(
        profile: &BgvProfile,
        context: &'a ThresholdContext,
        instance: [u8; 32],
        participants: &'a [u16],
    ) -> Result<Self, ThresholdError> {
        if profile.identity() != &context.profile {
            return Err(ThresholdError::ProfileMismatch);
        }
        if participants.len() < usize::from(context.threshold) {
            return Err(ThresholdError::InsufficientShares);
        }
        if participants.len() > context.participants.len() {
            return Err(ThresholdError::TooManyShares);
        }
        for pair in participants.windows(2) {
            if pair[0] == pair[1] {
                return Err(ThresholdError::DuplicateParticipant);
            }
            if pair[0] > pair[1] {
                return Err(ThresholdError::InvalidRoster);
            }
        }
        for participant in participants {
            if context.participants.binary_search(participant).is_err() {
                return Err(ThresholdError::UnknownParticipant);
            }
        }
        let mut hasher = blake3::Hasher::new_derive_key(THRESHOLD_SELECTION_DOMAIN);
        hasher.update(&1_u16.to_le_bytes());
        hasher.update(context.identity());
        hasher.update(&instance);
        // The context constructor bounds the full roster to u16::MAX.
        let count = u16::try_from(participants.len()).map_err(|_| ThresholdError::TooManyShares)?;
        hasher.update(&count.to_le_bytes());
        for participant in participants {
            hasher.update(&participant.to_le_bytes());
        }
        Ok(Self {
            context,
            instance,
            participants,
            identity: *hasher.finalize().as_bytes(),
        })
    }

    /// The original public threshold context.
    #[must_use]
    pub const fn context(&self) -> &ThresholdContext {
        self.context
    }

    /// The offline dealer instance selected by the caller.
    #[must_use]
    pub const fn instance(&self) -> &[u8; 32] {
        &self.instance
    }

    /// The exact canonical selected coordinates.
    #[must_use]
    pub const fn participants(&self) -> &[u16] {
        self.participants
    }

    /// Identity of the context, instance and exact canonical subset.
    #[must_use]
    pub const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    /// Public Lagrange coefficient at zero, for an already validated local party.
    fn coefficient(&self, profile: &BgvProfile, participant: u16, limb: usize) -> u64 {
        let fixed: crate::constant_time::FixedModulus = profile.full_plan(limb).fixed();
        let mut numerator = 1;
        let mut denominator = 1;
        for &other in self.participants {
            if other != participant {
                numerator = fixed.multiply(numerator, u64::from(other));
                denominator = fixed.multiply(
                    denominator,
                    fixed.sub_canonical(u64::from(other), u64::from(participant)),
                );
            }
        }
        // Distinct coordinates below every prime give a nonzero denominator.
        // Only public inputs enter this existing prime-field inversion schedule.
        let inverse = fixed.power(denominator, fixed.modulus - 2);
        fixed.multiply(numerator, inverse)
    }
}

/// Refused conversion retaining the original, unmodified secret share.
///
/// Dropping this error clears its owned secret. Recover it explicitly to retry
/// with corrected public metadata; formatting prints no secret words.
///
/// ```compile_fail
/// use iroha_fhe::bgv::threshold::ThresholdShareConversionError;
/// fn duplicate(error: &ThresholdShareConversionError) -> ThresholdShareConversionError {
///     (*error).clone()
/// }
/// ```
#[derive(Debug)]
pub struct ThresholdShareConversionError {
    reason: ThresholdError,
    share: SecretKeyShare,
}

impl ThresholdShareConversionError {
    /// The public reason no conversion occurred.
    #[must_use]
    pub const fn reason(&self) -> ThresholdError {
        self.reason
    }

    /// Recover the refusal and the exact original secret-share owner.
    #[must_use]
    pub fn into_parts(self) -> (ThresholdError, SecretKeyShare) {
        (self.reason, self.share)
    }
}

impl std::fmt::Display for ThresholdShareConversionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.reason, formatter)
    }
}

impl std::error::Error for ThresholdShareConversionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.reason)
    }
}

/// One local additive share for an exact public subset, with clearing ownership.
///
/// These full-field words are not a ternary [`SecretKey`]. No production export,
/// import, reconstruction or Clone interface is provided. The local
/// [`ThresholdSelection::open_local`] operation consumes all selected shares;
/// it never exports a per-participant decryption contribution.
///
/// ```compile_fail
/// use iroha_fhe::bgv::threshold::SubsetSecretKeyShare;
/// fn duplicate(share: &SubsetSecretKeyShare) -> SubsetSecretKeyShare {
///     (*share).clone()
/// }
/// ```
pub struct SubsetSecretKeyShare {
    context: Arc<ThresholdContext>,
    instance: [u8; 32],
    participant: u16,
    selection_identity: [u8; 32],
    words: RnsPoly,
}

impl std::fmt::Debug for SubsetSecretKeyShare {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SubsetSecretKeyShare")
            .field("context", self.context.identity())
            .field("instance", &self.instance)
            .field("participant", &self.participant)
            .field("selection_identity", &self.selection_identity)
            .finish_non_exhaustive()
    }
}

impl SubsetSecretKeyShare {
    /// Original public threshold context.
    #[must_use]
    pub fn context(&self) -> &ThresholdContext {
        &self.context
    }

    /// Original offline sharing instance, not a decryption session.
    #[must_use]
    pub const fn instance(&self) -> &[u8; 32] {
        &self.instance
    }

    /// Local participant's field coordinate.
    #[must_use]
    pub const fn participant(&self) -> u16 {
        self.participant
    }

    /// Exact public subset identity used for this private conversion.
    #[must_use]
    pub const fn selection_identity(&self) -> &[u8; 32] {
        &self.selection_identity
    }

    /// Bytes in the retained clearing buffer, excluding public metadata.
    #[must_use]
    pub fn secret_buffer_bytes(&self) -> usize {
        std::mem::size_of_val(self.words.words())
    }

    #[cfg(any(test, feature = "bgv-test-fixtures"))]
    pub(super) fn words_for_fixture(&self) -> &[u64] {
        self.words.words()
    }
}

impl std::fmt::Debug for SecretKeyShare {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SecretKeyShare")
            .field("context", self.context.identity())
            .field("instance", &self.instance)
            .field("participant", &self.participant)
            .finish_non_exhaustive()
    }
}

impl SecretKeyShare {
    /// Consume this local share into a subset-additive share in the same buffer.
    ///
    /// All validation precedes mutation. The canonical subset plan supplies
    /// public Lagrange coefficients independently for every main/special prime;
    /// each secret word uses the existing fixed-schedule modular owner. This
    /// operation allocates no new buffer and publishes no decryption material.
    ///
    /// # Errors
    /// Returns an owning error with the untouched original share for a different
    /// profile, context, instance or a subset that omits this participant.
    pub fn try_into_subset_additive(
        mut self,
        profile: &BgvProfile,
        selection: &ThresholdSelection<'_>,
    ) -> Result<SubsetSecretKeyShare, ThresholdShareConversionError> {
        let reason = if profile.identity() != &self.context.profile
            || profile.identity() != &selection.context.profile
        {
            Some(ThresholdError::ProfileMismatch)
        } else if self.context.identity() != selection.context.identity() {
            Some(ThresholdError::ContextMismatch)
        } else if self.instance != selection.instance {
            Some(ThresholdError::InstanceMismatch)
        } else if selection
            .participants
            .binary_search(&self.participant)
            .is_err()
        {
            Some(ThresholdError::ParticipantNotSelected)
        } else {
            None
        };
        if let Some(reason) = reason {
            return Err(ThresholdShareConversionError {
                reason,
                share: self,
            });
        }
        for limb in 0..self.words.limbs() {
            let coefficient = selection.coefficient(profile, self.participant, limb);
            let fixed: crate::constant_time::FixedModulus = profile.full_plan(limb).fixed();
            for word in self.words.limb_mut(limb) {
                *word = fixed.multiply(*word, coefficient);
            }
            if limb == 0 {
                fault::reached(FaultPoint::ThresholdAfterFirstSubsetLimb);
            }
        }
        Ok(SubsetSecretKeyShare {
            context: self.context,
            instance: self.instance,
            participant: self.participant,
            selection_identity: selection.identity,
            words: self.words,
        })
    }

    /// Public context under which this share was issued.
    #[must_use]
    pub fn context(&self) -> &ThresholdContext {
        &self.context
    }

    /// Fresh identifier of the dealer invocation, not a decryption session.
    #[must_use]
    pub const fn instance(&self) -> &[u8; 32] {
        &self.instance
    }

    /// This participant's field coordinate.
    #[must_use]
    pub const fn participant(&self) -> u16 {
        self.participant
    }

    /// Bytes in this share's clearing word buffer, excluding public metadata.
    #[must_use]
    pub fn secret_buffer_bytes(&self) -> usize {
        std::mem::size_of_val(self.words.words())
    }

    #[cfg(any(test, feature = "bgv-test-fixtures"))]
    pub(super) fn words_for_fixture(&self) -> &[u64] {
        self.words.words()
    }
}

/// Material retained only until the dealer is dropped or issues the final share.
struct DealerMaterial {
    key: SecretKey,
    /// Nonconstant coefficients, in increasing polynomial degree.
    polynomials: Vec<RnsPoly>,
}

/// An offline trusted dealer; never a distributed key-generation protocol.
///
/// The complete key is consumed and stays here until drop or final issuance.
/// Neither this owner nor the issued shares implement `Clone`. An incomplete
/// dealer must be dropped explicitly when its offline custody operation ends.
///
/// ```compile_fail
/// use iroha_fhe::bgv::threshold::DealerKeyShares;
/// fn duplicate<'a>(dealer: &DealerKeyShares<'a>) -> DealerKeyShares<'a> {
///     (*dealer).clone()
/// }
/// ```
pub struct DealerKeyShares<'profile> {
    profile: &'profile BgvProfile,
    context: Arc<ThresholdContext>,
    instance: [u8; 32],
    issued: Vec<bool>,
    remaining: usize,
    material: Option<DealerMaterial>,
}

impl std::fmt::Debug for DealerKeyShares<'_> {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DealerKeyShares")
            .field("context", self.context.identity())
            .field("instance", &self.instance)
            .field("remaining", &self.remaining())
            .finish_non_exhaustive()
    }
}

impl<'profile> DealerKeyShares<'profile> {
    /// Peak bytes of secret heap buffers retained by the dealer plus one issuance.
    ///
    /// Counts the consumed key's coefficients and evaluation form, `t-1`
    /// sampled polynomials, and the larger of the coin block or one issued
    /// share. Caller-retained shares, public metadata, allocator overhead,
    /// stack/register copies and profile storage are excluded. This is not a
    /// process-memory estimate. Size depends only on public inputs.
    ///
    /// # Errors
    /// Another profile or checked size overflow.
    pub fn required_secret_bytes(
        profile: &BgvProfile,
        context: &ThresholdContext,
    ) -> Result<usize, ThresholdError> {
        if profile.identity() != &context.profile {
            return Err(ThresholdError::ProfileMismatch);
        }
        let parameters = profile.parameters();
        let limbs = parameters.main_primes.len() + parameters.special_primes.len();
        let coefficients = profile
            .degree()
            .checked_mul(size_of::<i64>())
            .ok_or(ThresholdError::SizeOverflow)?;
        let poly = profile
            .degree()
            .checked_mul(limbs)
            .and_then(|words| words.checked_mul(size_of::<u64>()))
            .ok_or(ThresholdError::SizeOverflow)?;
        poly.checked_mul(usize::from(context.threshold))
            .and_then(|bytes| bytes.checked_add(coefficients))
            .and_then(|bytes| bytes.checked_add(poly.max(COIN_STATE_BYTES)))
            .ok_or(ThresholdError::SizeOverflow)
    }

    /// Consume a complete key and create an offline dealer using OS entropy.
    ///
    /// The secret-buffer ceiling is checked before new secret allocations or
    /// entropy use. On refusal the consumed key is dropped and cleared.
    ///
    /// # Errors
    /// Profile mismatch, insufficient secret-buffer limit, or failed/stuck entropy.
    pub fn split(
        profile: &'profile BgvProfile,
        context: ThresholdContext,
        key: SecretKey,
        max_secret_bytes: usize,
    ) -> Result<Self, ThresholdError> {
        Self::split_with(profile, context, key, max_secret_bytes, &mut OsEntropy)
    }

    pub(super) fn split_with(
        profile: &'profile BgvProfile,
        context: ThresholdContext,
        key: SecretKey,
        max_secret_bytes: usize,
        entropy: &mut dyn EntropySource,
    ) -> Result<Self, ThresholdError> {
        let required = Self::required_secret_bytes(profile, &context)?;
        if key.profile_identity() != profile.identity() {
            return Err(ThresholdError::ProfileMismatch);
        }
        if required > max_secret_bytes {
            return Err(ThresholdError::SecretBufferLimit {
                required,
                limit: max_secret_bytes,
            });
        }
        let mut coins = Coins::draw(entropy)?;
        let mut instance = [0_u8; 32];
        coins
            .stream(context.identity(), CoinPurpose::ThresholdInstance, 0, 0)
            .bytes(&mut instance);
        let parameters = profile.parameters();
        // This vector contains public moduli only.
        let primes: Vec<u64> = parameters
            .main_primes
            .iter()
            .chain(&parameters.special_primes)
            .copied()
            .collect();
        let mut polynomials = Vec::with_capacity(usize::from(context.threshold - 1));
        for power in 1..context.threshold {
            let mut poly = RnsPoly::zero(primes.len(), profile.degree());
            coins
                .stream(
                    context.identity(),
                    CoinPurpose::ThresholdPolynomial,
                    u32::from(power),
                    0,
                )
                .uniform(&primes, profile.degree(), poly.words_mut());
            polynomials.push(poly);
            if power == 1 {
                fault::reached(FaultPoint::ThresholdAfterFirstPolynomial);
            }
        }
        Ok(Self {
            profile,
            issued: vec![false; context.participants.len()],
            remaining: context.participants.len(),
            context: Arc::new(context),
            instance,
            material: Some(DealerMaterial { key, polynomials }),
        })
    }

    /// Public context retained even after dealer material has been cleared.
    #[must_use]
    pub fn context(&self) -> &ThresholdContext {
        &self.context
    }

    /// Fresh identifier to retain with an offline selection of shares.
    #[must_use]
    pub const fn instance(&self) -> &[u8; 32] {
        &self.instance
    }

    /// Number of declared participants whose shares have not been issued.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.remaining
    }

    /// Issue one share, rejecting an unknown participant or repeated issuance.
    ///
    /// Every coefficient uses the same public Horner schedule and the existing
    /// constant-time modular owner. The last successful issuance drops all
    /// dealer secret material before returning. Returned share owners still
    /// contain secrets and must be dropped by their respective custody owners.
    ///
    /// # Errors
    /// Unknown participant or a participant whose share was already issued.
    pub fn take_share(&mut self, participant: u16) -> Result<SecretKeyShare, ThresholdError> {
        let index = self
            .context
            .participants
            .binary_search(&participant)
            .map_err(|_| ThresholdError::UnknownParticipant)?;
        if self.issued[index] {
            return Err(ThresholdError::AlreadyIssued);
        }
        let material = self
            .material
            .as_ref()
            .ok_or(ThresholdError::AlreadyIssued)?;
        let limbs = self.profile.top_level() + 1 + self.profile.special_count();
        let mut words = RnsPoly::zero(limbs, self.profile.degree());
        for limb in 0..limbs {
            let plan = self.profile.full_plan(limb);
            let fixed: crate::constant_time::FixedModulus = plan.fixed();
            for (coefficient, value) in words.limb_mut(limb).iter_mut().enumerate() {
                for poly in material.polynomials.iter().rev() {
                    *value = fixed.add_canonical(
                        fixed.multiply(*value, u64::from(participant)),
                        poly.limb(limb)[coefficient],
                    );
                }
                *value = fixed.add_canonical(
                    fixed.multiply(*value, u64::from(participant)),
                    small_residue(material.key.coefficients()[coefficient], plan.prime()),
                );
            }
        }
        fault::reached(FaultPoint::ThresholdBeforeShareRelease);
        self.issued[index] = true;
        self.remaining -= 1;
        if self.remaining == 0 {
            drop(self.material.take());
        }
        Ok(SecretKeyShare {
            context: Arc::clone(&self.context),
            instance: self.instance,
            participant,
            words,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bgv::{
        BgvParameters,
        fixtures::{self, FixtureEntropy},
    };

    fn profile() -> BgvProfile {
        BgvProfile::new(BgvParameters::derive(4, 257, 1, (40, 2), (41, 1), 1).expect("parameters"))
            .expect("profile")
    }

    #[test]
    fn metadata_checks_refuse_forged_participants_but_do_not_claim_algebraic_proofs() {
        let profile = profile();
        let context =
            ThresholdContext::new(&profile, [1; 32], 1, 2, vec![1, 2, 3]).expect("context");
        let secret =
            fixtures::secret_key(&profile, &mut FixtureEntropy::new([7; 64])).expect("key");
        let limit = DealerKeyShares::required_secret_bytes(&profile, &context).expect("limit");
        let mut dealer = fixtures::threshold_dealer(
            &profile,
            context.clone(),
            secret,
            limit,
            &mut FixtureEntropy::new([8; 64]),
        )
        .expect("dealer");
        let a = dealer.take_share(1).expect("a");
        let mut b = dealer.take_share(2).expect("b");
        b.participant = 4;
        assert_eq!(
            context.validate_selection_metadata(dealer.instance(), &[&a, &b]),
            Err(ThresholdError::UnknownParticipant)
        );
        b.participant = 2;
        // A malicious canonical residue is not an honest polynomial evaluation.
        // Metadata-only checks cannot detect that; production has no import or
        // mutator capable of creating this deliberately forged internal object.
        let prime = profile.parameters().main_primes[0];
        let original = b.words.words()[0];
        b.words.words_mut()[0] = (original + 1) % prime;
        assert_ne!(b.words.words()[0], original);
        context
            .validate_selection_metadata(dealer.instance(), &[&a, &b])
            .expect("metadata only");
    }
}
