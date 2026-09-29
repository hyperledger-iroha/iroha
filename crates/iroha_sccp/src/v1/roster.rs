//! SCCP v1 roster digest and destination roster light client (spec §3.7, §5.1.2, §5.1.5).
//!
//! ```text
//! roster_preimage = "SCCP/ROSTER/V1" ‖ taira_network_id ‖ u64 generation ‖ u64 valid_from_ms
//!                 ‖ u64 valid_until_ms ‖ u8 n ‖ u8 t ‖ member_0 ‖ … ‖ member_{n-1}
//! roster_digest   = keccak256(roster_preimage)
//! ```
//!
//! `n` is in 4..=31, `t = ⌊2n/3⌋ + 1`, `generation ≥ 1`, and the members are all zero members
//! first, then nonzero members strictly ascending as 160-bit integers. Every verifier checks the
//! `n` range, the `t` formula and the ordering while hashing; [`roster_digest_checked`] does the
//! same over the packed form a destination receives.
//!
//! [`RosterStateV1`] mirrors the destination roster light client: acceptance of an attestation's
//! roster (§5.1.2), the initial-roster bounds, and the rotation checks and state transition of
//! §5.1.5, all as pure functions of `now_ms`.

use super::{
    constants::{
        MAX_CLOCK_SKEW_MS, MAX_ROSTER_MEMBERS, MAX_ROSTER_VALIDITY_MS, MIN_ROSTER_MEMBERS,
        PREVIOUS_ROSTER_GRACE_MS, ROSTER_TAG,
    },
    eip712::AttestationFieldsV1,
    hashes::keccak256,
};

unit_error! {
    /// Violations of the §3.7 roster rules.
    pub enum RosterError {
        /// `n` is outside 4..=31, or the packed member bytes are not a multiple of 20.
        BadSize => "roster must have 4..=31 members of 20 bytes",
        /// `t` differs from `⌊2n/3⌋ + 1`.
        BadThreshold => "roster threshold must equal floor(2n/3) + 1",
        /// The generation is zero.
        ZeroGeneration => "roster generation must be at least 1",
        /// Members are not zero-first then strictly ascending.
        BadOrder => "roster members must be zero members first, then strictly ascending",
    }
}

unit_error! {
    /// Violations of the §5.1.5 validity bounds (`BadRosterValidity()` on EVM).
    pub enum RosterValidityError {
        /// `valid_until_ms <= valid_from_ms`.
        EmptyWindow => "roster valid_until_ms must exceed valid_from_ms",
        /// `valid_from_ms > now_ms + MAX_CLOCK_SKEW_MS`.
        FromTooFarAhead => "roster valid_from_ms exceeds now plus the clock-skew bound",
        /// `valid_until_ms − valid_from_ms > MAX_ROSTER_VALIDITY_MS`.
        WindowTooLong => "roster validity exceeds MAX_ROSTER_VALIDITY_MS",
        /// `valid_until_ms <= now_ms`.
        Expired => "roster valid_until_ms is not in the future",
        /// `valid_until_ms > now_ms + MAX_ROSTER_VALIDITY_MS`.
        UntilTooFarAhead => "roster valid_until_ms exceeds now plus MAX_ROSTER_VALIDITY_MS",
    }
}

unit_error! {
    /// Failures of a §5.1.5 rotation.
    pub enum RotationError {
        /// Step 1: the attestation is not signed by the current, unexpired roster
        /// (`RosterNotAccepted()`).
        RosterNotAccepted => "rotation attestation is not signed by the current unexpired roster",
        /// The supplied current roster does not hash to the current digest.
        CurrentRosterMismatch => "supplied current roster does not hash to the current digest",
        /// Step 2: `nextRosterDigest` is zero or differs from the digest of `next`
        /// (`BadRotation()`).
        NextDigestMismatch => "nextRosterDigest is zero or differs from the next roster digest",
        /// Step 3: `next.generation ≠ generation + 1` (`BadRotation()`).
        GenerationNotSequential => "next roster generation must be current generation + 1",
        /// Step 3: `next.validFromMs ≠ A.timestampMs` (`BadRotation()`).
        ValidFromMismatch => "next roster valid_from_ms must equal the attestation timestamp",
        /// The next roster breaks a §3.7 rule (`BadRoster()`).
        BadNextRoster => "next roster violates the roster rules",
        /// The next roster breaks a validity bound (`BadRosterValidity()`).
        BadValidity => "next roster violates the validity bounds",
    }
}

/// `t = ⌊2n/3⌋ + 1`.
#[must_use]
pub const fn threshold(n: usize) -> usize {
    (2 * n) / 3 + 1
}

/// Check the member ordering: zero members first, then nonzero strictly ascending.
///
/// # Errors
///
/// Returns [`RosterError::BadOrder`].
pub fn check_member_order(members: &[[u8; 20]]) -> Result<(), RosterError> {
    let mut previous = [0_u8; 20];
    for member in members {
        if *member == [0; 20] {
            if previous != [0; 20] {
                return Err(RosterError::BadOrder);
            }
        } else {
            if *member <= previous {
                return Err(RosterError::BadOrder);
            }
            previous = *member;
        }
    }
    Ok(())
}

/// One roster generation as the destinations see it (`RosterV1` of §5.2.2).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct RosterV1 {
    /// Generation number, at least 1.
    pub generation: u64,
    /// Start of validity (Taira time of the generation's first block).
    pub valid_from_ms: u64,
    /// End of validity.
    pub valid_until_ms: u64,
    /// Members in canonical order; zero entries are keyless slots.
    pub members: Vec<[u8; 20]>,
}

impl RosterV1 {
    /// Number of members `n`.
    #[must_use]
    pub fn n(&self) -> usize {
        self.members.len()
    }

    /// Threshold `t = ⌊2n/3⌋ + 1`.
    #[must_use]
    pub fn threshold(&self) -> usize {
        threshold(self.n())
    }

    /// Members packed as `n × 20` bytes.
    #[must_use]
    pub fn packed_members(&self) -> Vec<u8> {
        self.members.iter().flatten().copied().collect()
    }

    /// Parse the packed destination form, checking `threshold` against the formula.
    ///
    /// # Errors
    ///
    /// Returns [`RosterError`] for a bad size, threshold, generation or order.
    pub fn from_packed(
        generation: u64,
        valid_from_ms: u64,
        valid_until_ms: u64,
        threshold: u8,
        packed_members: &[u8],
    ) -> Result<Self, RosterError> {
        if !packed_members.len().is_multiple_of(20) {
            return Err(RosterError::BadSize);
        }
        let members = packed_members
            .chunks_exact(20)
            .map(|chunk| {
                let mut member = [0_u8; 20];
                member.copy_from_slice(chunk);
                member
            })
            .collect::<Vec<_>>();
        let roster = Self {
            generation,
            valid_from_ms,
            valid_until_ms,
            members,
        };
        roster.validate()?;
        if usize::from(threshold) != roster.threshold() {
            return Err(RosterError::BadThreshold);
        }
        Ok(roster)
    }

    /// Check the §3.7 rules a verifier checks while hashing: `n`, generation and ordering.
    ///
    /// # Errors
    ///
    /// Returns the first violated [`RosterError`].
    pub fn validate(&self) -> Result<(), RosterError> {
        if !(MIN_ROSTER_MEMBERS..=MAX_ROSTER_MEMBERS).contains(&self.n()) {
            return Err(RosterError::BadSize);
        }
        if self.generation == 0 {
            return Err(RosterError::ZeroGeneration);
        }
        check_member_order(&self.members)
    }

    /// The §3.7 preimage.
    ///
    /// # Errors
    ///
    /// See [`Self::validate`].
    pub fn preimage(&self, taira_network_id: &[u8; 32]) -> Result<Vec<u8>, RosterError> {
        self.validate()?;
        let n = u8::try_from(self.n()).map_err(|_| RosterError::BadSize)?;
        let t = u8::try_from(self.threshold()).map_err(|_| RosterError::BadThreshold)?;
        let mut out = Vec::with_capacity(72 + 20 * self.n());
        out.extend_from_slice(ROSTER_TAG);
        out.extend_from_slice(taira_network_id);
        out.extend_from_slice(&self.generation.to_be_bytes());
        out.extend_from_slice(&self.valid_from_ms.to_be_bytes());
        out.extend_from_slice(&self.valid_until_ms.to_be_bytes());
        out.push(n);
        out.push(t);
        for member in &self.members {
            out.extend_from_slice(member);
        }
        Ok(out)
    }

    /// `roster_digest` (§3.7).
    ///
    /// # Errors
    ///
    /// See [`Self::validate`].
    pub fn digest(&self, taira_network_id: &[u8; 32]) -> Result<[u8; 32], RosterError> {
        Ok(keccak256(&[&self.preimage(taira_network_id)?]))
    }

    /// Check the §5.1.5 validity bounds of this roster at `now_ms`.
    ///
    /// # Errors
    ///
    /// See [`check_validity_bounds`].
    pub fn check_validity(&self, now_ms: u64) -> Result<(), RosterValidityError> {
        check_validity_bounds(self.valid_from_ms, self.valid_until_ms, now_ms)
    }
}

/// The digest of a roster in its packed destination form, with every verifier check.
///
/// # Errors
///
/// Returns [`RosterError`] exactly where a destination reverts with `BadRoster()`.
pub fn roster_digest_checked(
    taira_network_id: &[u8; 32],
    generation: u64,
    valid_from_ms: u64,
    valid_until_ms: u64,
    threshold: u8,
    packed_members: &[u8],
) -> Result<[u8; 32], RosterError> {
    RosterV1::from_packed(
        generation,
        valid_from_ms,
        valid_until_ms,
        threshold,
        packed_members,
    )?
    .digest(taira_network_id)
}

/// The §5.1.5 validity bounds, applied to the initial roster and to every rotation:
/// `valid_until > valid_from`, `valid_from ≤ now + MAX_CLOCK_SKEW_MS`,
/// `valid_until − valid_from ≤ MAX_ROSTER_VALIDITY_MS` and
/// `now < valid_until ≤ now + MAX_ROSTER_VALIDITY_MS`.
///
/// # Errors
///
/// Returns the first violated [`RosterValidityError`].
pub fn check_validity_bounds(
    valid_from_ms: u64,
    valid_until_ms: u64,
    now_ms: u64,
) -> Result<(), RosterValidityError> {
    if valid_until_ms <= valid_from_ms {
        return Err(RosterValidityError::EmptyWindow);
    }
    // Saturation matches the contracts' uint256 arithmetic for these `≤` comparisons.
    if valid_from_ms > now_ms.saturating_add(MAX_CLOCK_SKEW_MS) {
        return Err(RosterValidityError::FromTooFarAhead);
    }
    if valid_until_ms - valid_from_ms > MAX_ROSTER_VALIDITY_MS {
        return Err(RosterValidityError::WindowTooLong);
    }
    if valid_until_ms <= now_ms {
        return Err(RosterValidityError::Expired);
    }
    if valid_until_ms > now_ms.saturating_add(MAX_ROSTER_VALIDITY_MS) {
        return Err(RosterValidityError::UntilTooFarAhead);
    }
    Ok(())
}

/// A destination's roster light-client state (§5.1.1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RosterStateV1 {
    /// Current roster digest.
    pub digest: [u8; 32],
    /// Current generation.
    pub generation: u64,
    /// Current roster expiry.
    pub valid_until_ms: u64,
    /// Previous roster digest, zero before the first rotation.
    pub prev_digest: [u8; 32],
    /// Grace-capped previous roster expiry, zero before the first rotation.
    pub prev_valid_until_ms: u64,
}

impl RosterStateV1 {
    /// The state a constructor (or `sccp_init`) installs for `roster` at `now_ms`.
    ///
    /// # Errors
    ///
    /// Returns [`RotationError::BadNextRoster`] or [`RotationError::BadValidity`].
    pub fn initial(
        roster: &RosterV1,
        taira_network_id: &[u8; 32],
        now_ms: u64,
    ) -> Result<Self, RotationError> {
        let digest = roster
            .digest(taira_network_id)
            .map_err(|_| RotationError::BadNextRoster)?;
        roster
            .check_validity(now_ms)
            .map_err(|_| RotationError::BadValidity)?;
        Ok(Self {
            digest,
            generation: roster.generation,
            valid_until_ms: roster.valid_until_ms,
            prev_digest: [0; 32],
            prev_valid_until_ms: 0,
        })
    }

    /// §5.1.2: whether an attestation with `roster_digest` is signed by an accepted roster.
    #[must_use]
    pub fn accepts(&self, roster_digest: &[u8; 32], now_ms: u64) -> bool {
        if *roster_digest == [0; 32] {
            return false;
        }
        (*roster_digest == self.digest && now_ms <= self.valid_until_ms)
            || (*roster_digest == self.prev_digest
                && self.prev_digest != [0; 32]
                && now_ms <= self.prev_valid_until_ms)
    }

    /// Whether the destination is frozen for `voidFrozen` (both rosters expired, §5.1.8).
    #[must_use]
    pub fn is_frozen(&self, now_ms: u64) -> bool {
        now_ms > self.valid_until_ms && now_ms > self.prev_valid_until_ms
    }

    /// §5.1.5 steps 1–3 without the signature check: the attestation names the current,
    /// unexpired roster; `current` hashes to it; `next` hashes to `nextRosterDigest`, is
    /// sequential, starts at the attestation timestamp and satisfies the validity bounds.
    /// Returns the next digest. The caller verifies `t` signatures of `current` over
    /// `attestation.digest(..)` (see [`super::signature::verify_signature_set`]).
    ///
    /// # Errors
    ///
    /// Returns the first violated [`RotationError`].
    pub fn check_rotation(
        &self,
        attestation: &AttestationFieldsV1,
        current: &RosterV1,
        next: &RosterV1,
        taira_network_id: &[u8; 32],
        now_ms: u64,
    ) -> Result<[u8; 32], RotationError> {
        if attestation.roster_digest != self.digest || now_ms > self.valid_until_ms {
            return Err(RotationError::RosterNotAccepted);
        }
        if current.digest(taira_network_id).ok() != Some(self.digest) {
            return Err(RotationError::CurrentRosterMismatch);
        }
        let next_digest = next
            .digest(taira_network_id)
            .map_err(|_| RotationError::BadNextRoster)?;
        if attestation.next_roster_digest == [0; 32]
            || attestation.next_roster_digest != next_digest
        {
            return Err(RotationError::NextDigestMismatch);
        }
        if Some(next.generation) != self.generation.checked_add(1) {
            return Err(RotationError::GenerationNotSequential);
        }
        if next.valid_from_ms != attestation.timestamp_ms {
            return Err(RotationError::ValidFromMismatch);
        }
        next.check_validity(now_ms)
            .map_err(|_| RotationError::BadValidity)?;
        Ok(next_digest)
    }

    /// §5.1.5 step 4: keep the old roster as previous with a grace-capped expiry and install
    /// the next one. Call only after [`Self::check_rotation`] succeeded.
    pub fn apply_rotation(&mut self, next_digest: [u8; 32], next: &RosterV1, now_ms: u64) {
        self.prev_digest = self.digest;
        self.prev_valid_until_ms = previous_valid_until(self.valid_until_ms, now_ms);
        self.digest = next_digest;
        self.generation = next.generation;
        self.valid_until_ms = next.valid_until_ms;
    }

    /// Check and apply one rotation.
    ///
    /// # Errors
    ///
    /// See [`Self::check_rotation`]; the state is unchanged on error.
    pub fn rotate(
        &mut self,
        attestation: &AttestationFieldsV1,
        current: &RosterV1,
        next: &RosterV1,
        taira_network_id: &[u8; 32],
        now_ms: u64,
    ) -> Result<(), RotationError> {
        let next_digest =
            self.check_rotation(attestation, current, next, taira_network_id, now_ms)?;
        self.apply_rotation(next_digest, next, now_ms);
        Ok(())
    }
}

/// `prevValidUntilMs = min(validUntilMs, now_ms + PREVIOUS_ROSTER_GRACE_MS)`.
#[must_use]
pub fn previous_valid_until(valid_until_ms: u64, now_ms: u64) -> u64 {
    valid_until_ms.min(now_ms.saturating_add(PREVIOUS_ROSTER_GRACE_MS))
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAIRA: [u8; 32] = [0x11; 32];
    const NOW: u64 = 1_800_000_000_000;
    const DAY: u64 = 86_400_000;

    fn member(byte: u8) -> [u8; 20] {
        [byte; 20]
    }

    fn roster(generation: u64, members: Vec<[u8; 20]>) -> RosterV1 {
        RosterV1 {
            generation,
            valid_from_ms: NOW,
            valid_until_ms: NOW + 14 * DAY,
            members,
        }
    }

    fn four() -> RosterV1 {
        roster(7, (1..=4).map(member).collect())
    }

    #[test]
    fn threshold_formula() {
        let expected = [(4, 3), (5, 4), (6, 5), (7, 5), (10, 7), (31, 21)];
        for (n, t) in expected {
            assert_eq!(threshold(n), t, "n = {n}");
        }
    }

    #[test]
    fn preimage_layout_and_digest() {
        let roster = four();
        let preimage = roster.preimage(&TAIRA).unwrap();
        assert_eq!(preimage.len(), 72 + 80);
        assert_eq!(&preimage[..14], b"SCCP/ROSTER/V1");
        assert_eq!(preimage[14..46], TAIRA);
        assert_eq!(preimage[46..54], 7_u64.to_be_bytes());
        assert_eq!(preimage[54..62], NOW.to_be_bytes());
        assert_eq!(preimage[62..70], (NOW + 14 * DAY).to_be_bytes());
        assert_eq!(preimage[70], 4);
        assert_eq!(preimage[71], 3);
        assert_eq!(preimage[72..92], member(1));
        assert_eq!(roster.digest(&TAIRA).unwrap(), keccak256(&[&preimage]));
        assert_eq!(
            roster_digest_checked(&TAIRA, 7, NOW, NOW + 14 * DAY, 3, &roster.packed_members()),
            roster.digest(&TAIRA)
        );
    }

    #[test]
    fn ton_stateinit_n4_digest_matches_the_contract_fixture() {
        // fixtures/sccp/ton_stateinit_v1.json, label "n4".
        let roster = RosterV1 {
            generation: 7,
            valid_from_ms: 1_800_000_000_000,
            valid_until_ms: 1_801_209_600_000,
            members: (1..=4).map(member).collect(),
        };
        let digest = roster.digest(&TAIRA).unwrap();
        let hex = crate::v1::hashes::to_hex(&digest);
        assert_eq!(
            hex,
            "c9eed4f02ae435a8a7913451e107c5fc9085258bb4dd15e80c3f51c0f3cf5321"
        );
    }

    #[test]
    fn ordering_rules() {
        assert_eq!(
            check_member_order(&[[0; 20], [0; 20], member(1), member(2)]),
            Ok(())
        );
        assert_eq!(
            check_member_order(&[member(1), [0; 20], member(2), member(3)]),
            Err(RosterError::BadOrder)
        );
        assert_eq!(
            check_member_order(&[member(1), member(1), member(2), member(3)]),
            Err(RosterError::BadOrder)
        );
        assert_eq!(
            check_member_order(&[member(2), member(1), member(3), member(4)]),
            Err(RosterError::BadOrder)
        );
        // Byte order is 160-bit integer order.
        let mut low = [0_u8; 20];
        low[19] = 0xff;
        let mut high = [0_u8; 20];
        high[0] = 0x01;
        assert_eq!(
            check_member_order(&[low, high, member(2), member(3)]),
            Ok(())
        );
    }

    #[test]
    fn size_threshold_and_generation_rules() {
        assert_eq!(
            roster(1, (1..=3).map(member).collect()).validate(),
            Err(RosterError::BadSize)
        );
        assert_eq!(
            roster(1, (1..=32).map(member).collect()).validate(),
            Err(RosterError::BadSize)
        );
        assert_eq!(roster(1, (1..=31).map(member).collect()).validate(), Ok(()));
        assert_eq!(
            roster(0, (1..=4).map(member).collect()).validate(),
            Err(RosterError::ZeroGeneration)
        );
        let packed = four().packed_members();
        assert_eq!(
            RosterV1::from_packed(7, NOW, NOW + DAY, 4, &packed),
            Err(RosterError::BadThreshold)
        );
        assert_eq!(
            RosterV1::from_packed(7, NOW, NOW + DAY, 3, &packed[..79]),
            Err(RosterError::BadSize)
        );
        assert!(RosterV1::from_packed(7, NOW, NOW + DAY, 3, &packed).is_ok());
    }

    #[test]
    fn validity_bounds_boundaries() {
        let max = MAX_ROSTER_VALIDITY_MS;
        let skew = MAX_CLOCK_SKEW_MS;
        assert_eq!(check_validity_bounds(NOW, NOW + DAY, NOW), Ok(()));
        assert_eq!(
            check_validity_bounds(NOW, NOW, NOW),
            Err(RosterValidityError::EmptyWindow)
        );
        assert_eq!(
            check_validity_bounds(NOW + skew, NOW + skew + DAY, NOW),
            Ok(())
        );
        assert_eq!(
            check_validity_bounds(NOW + skew + 1, NOW + skew + DAY, NOW),
            Err(RosterValidityError::FromTooFarAhead)
        );
        assert_eq!(check_validity_bounds(NOW - 1, NOW - 1 + max, NOW), Ok(()));
        assert_eq!(
            check_validity_bounds(NOW - 1, NOW + max, NOW),
            Err(RosterValidityError::WindowTooLong)
        );
        assert_eq!(
            check_validity_bounds(NOW - DAY, NOW, NOW),
            Err(RosterValidityError::Expired)
        );
        assert_eq!(check_validity_bounds(NOW - DAY, NOW + 1, NOW), Ok(()));
        assert_eq!(check_validity_bounds(NOW + 1, NOW + max, NOW), Ok(()));
        assert_eq!(
            check_validity_bounds(NOW + 2, NOW + max + 1, NOW),
            Err(RosterValidityError::UntilTooFarAhead)
        );
        // Saturating arithmetic near u64::MAX behaves like unbounded arithmetic.
        assert_eq!(
            check_validity_bounds(u64::MAX - DAY, u64::MAX, u64::MAX - DAY),
            Ok(())
        );
    }

    #[test]
    fn acceptance_and_freeze() {
        let state = RosterStateV1 {
            digest: [1; 32],
            generation: 3,
            valid_until_ms: NOW,
            prev_digest: [2; 32],
            prev_valid_until_ms: NOW - DAY,
        };
        assert!(state.accepts(&[1; 32], NOW));
        assert!(!state.accepts(&[1; 32], NOW + 1));
        assert!(state.accepts(&[2; 32], NOW - DAY));
        assert!(!state.accepts(&[2; 32], NOW - DAY + 1));
        assert!(!state.accepts(&[3; 32], NOW - 2 * DAY));
        assert!(!state.accepts(&[0; 32], 0));
        let initial = RosterStateV1 {
            prev_digest: [0; 32],
            prev_valid_until_ms: 0,
            ..state
        };
        assert!(!initial.accepts(&[0; 32], 0));
        assert!(!state.is_frozen(NOW));
        assert!(state.is_frozen(NOW + 1));
    }

    fn rotation_attestation(
        state: &RosterStateV1,
        next_digest: [u8; 32],
        ts: u64,
    ) -> AttestationFieldsV1 {
        AttestationFieldsV1 {
            height: 3_600,
            epoch: 1,
            timestamp_ms: ts,
            block_hash: [9; 32],
            roster_digest: state.digest,
            next_roster_digest: next_digest,
            ..AttestationFieldsV1::default()
        }
    }

    #[test]
    fn rotation_happy_path_and_grace() {
        let current = four();
        let mut state = RosterStateV1::initial(&current, &TAIRA, NOW).unwrap();
        assert_eq!(state.generation, 7);
        assert_eq!(state.prev_digest, [0; 32]);
        let now = NOW + DAY;
        let next = RosterV1 {
            generation: 8,
            valid_from_ms: now - 1_000,
            valid_until_ms: now + 14 * DAY,
            members: vec![[0; 20], member(2), member(3), member(5)],
        };
        let next_digest = next.digest(&TAIRA).unwrap();
        let attestation = rotation_attestation(&state, next_digest, now - 1_000);
        let before = state;
        state
            .rotate(&attestation, &current, &next, &TAIRA, now)
            .unwrap();
        assert_eq!(state.digest, next_digest);
        assert_eq!(state.generation, 8);
        assert_eq!(state.valid_until_ms, now + 14 * DAY);
        assert_eq!(state.prev_digest, before.digest);
        // The grace cap applies: 13 days of remaining validity become 24 hours.
        assert_eq!(state.prev_valid_until_ms, now + DAY);
        assert_eq!(previous_valid_until(now + 10, now), now + 10);
    }

    #[test]
    fn rotation_failures() {
        let current = four();
        let state = RosterStateV1::initial(&current, &TAIRA, NOW).unwrap();
        let now = NOW + DAY;
        let next = RosterV1 {
            generation: 8,
            valid_from_ms: now,
            valid_until_ms: now + 14 * DAY,
            members: (2..=5).map(member).collect(),
        };
        let digest = next.digest(&TAIRA).unwrap();
        let good = rotation_attestation(&state, digest, now);
        let check = |attestation: &AttestationFieldsV1, next: &RosterV1, now_ms: u64| {
            state.check_rotation(attestation, &current, next, &TAIRA, now_ms)
        };
        assert_eq!(check(&good, &next, now), Ok(digest));
        assert_eq!(
            check(&good, &next, state.valid_until_ms + 1),
            Err(RotationError::RosterNotAccepted)
        );
        let mut wrong_roster = good;
        wrong_roster.roster_digest = [5; 32];
        assert_eq!(
            check(&wrong_roster, &next, now),
            Err(RotationError::RosterNotAccepted)
        );
        let mut no_next = good;
        no_next.next_roster_digest = [0; 32];
        assert_eq!(
            check(&no_next, &next, now),
            Err(RotationError::NextDigestMismatch)
        );
        let skipped = RosterV1 {
            generation: 9,
            ..next.clone()
        };
        let skipped_att = rotation_attestation(&state, skipped.digest(&TAIRA).unwrap(), now);
        assert_eq!(
            check(&skipped_att, &skipped, now),
            Err(RotationError::GenerationNotSequential)
        );
        let shifted = RosterV1 {
            valid_from_ms: now - 1,
            ..next.clone()
        };
        let shifted_att = rotation_attestation(&state, shifted.digest(&TAIRA).unwrap(), now);
        assert_eq!(
            check(&shifted_att, &shifted, now),
            Err(RotationError::ValidFromMismatch)
        );
        let too_long = RosterV1 {
            valid_until_ms: now + MAX_ROSTER_VALIDITY_MS + 1,
            ..next.clone()
        };
        let too_long_att = rotation_attestation(&state, too_long.digest(&TAIRA).unwrap(), now);
        assert_eq!(
            check(&too_long_att, &too_long, now),
            Err(RotationError::BadValidity)
        );
        let unordered = RosterV1 {
            members: vec![member(3), member(2), member(4), member(5)],
            ..next.clone()
        };
        assert_eq!(
            check(&good, &unordered, now),
            Err(RotationError::BadNextRoster)
        );
        let other_current = roster(7, (2..=5).map(member).collect());
        assert_eq!(
            state.check_rotation(&good, &other_current, &next, &TAIRA, now),
            Err(RotationError::CurrentRosterMismatch)
        );
    }

    #[test]
    fn initial_state_rejects_bad_rosters() {
        let mut expired = four();
        expired.valid_until_ms = NOW;
        assert_eq!(
            RosterStateV1::initial(&expired, &TAIRA, NOW),
            Err(RotationError::BadValidity)
        );
        let unordered = roster(7, vec![member(2), member(1), member(3), member(4)]);
        assert_eq!(
            RosterStateV1::initial(&unordered, &TAIRA, NOW),
            Err(RotationError::BadNextRoster)
        );
    }
}
