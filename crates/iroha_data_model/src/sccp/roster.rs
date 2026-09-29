//! SCCP v1 bridge roster generations (`specs/sccp.md` §3.7, §4.3).
//!
//! A generation is the ordered list of `(peer, bridge-key address)` slots that signs a range of
//! Taira heights. Its contract-visible digest (§3.7) is computed by `iroha_sccp::v1` from the
//! addresses; this module holds the stored record and its state-independent shape rules.

use crate::{DeriveJsonDeserialize, DeriveJsonSerialize};
use iroha_model_base::peer::PeerId;
use iroha_schema::IntoSchema;
use norito::codec::{Decode, Encode};

/// Smallest roster size SCCP admits (`n ≥ 4`).
pub const SCCP_ROSTER_MIN_MEMBERS_V1: usize = 4;
/// Largest roster size SCCP admits (`n ≤ 31`, so a signer bitmap fits a `u32`).
pub const SCCP_ROSTER_MAX_MEMBERS_V1: usize = 31;

/// Return the §3.7 signing threshold `⌊2n/3⌋ + 1` for a roster of `n` members.
#[must_use]
pub const fn sccp_roster_threshold_v1(n: u8) -> u8 {
    // `⌊2n/3⌋ = n − ⌈n/3⌉`, which never overflows `u8` (at most 170).
    let ceil_third = n / 3 + if n.is_multiple_of(3) { 0 } else { 1 };
    n - ceil_third + 1
}

/// One slot of a roster generation, in §3.7 order.
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::roster::SccpRosterMemberV1")]
pub struct SccpRosterMemberV1 {
    /// Bridge-key address, or zero for a peer without an active key (never signs).
    pub address: [u8; 20],
    /// Validator peer holding the slot.
    #[norito(required)]
    pub peer: Option<PeerId>,
}

impl SccpRosterMemberV1 {
    /// Return whether the slot can sign (nonzero address).
    #[must_use]
    pub fn is_nonzero(&self) -> bool {
        self.address != [0; 20]
    }
}

/// Stored roster generation (`sccp_rosters[generation]`, §4.3.1).
#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Decode,
    Encode,
    IntoSchema,
    DeriveJsonSerialize,
    DeriveJsonDeserialize,
)]
#[norito(no_fast_from_json)]
#[norito(decode_from_slice)]
#[norito(deny_unknown_fields)]
#[derive(norito::NoritoSchema)]
#[norito_schema(name = "iroha_data_model::sccp::roster::SccpBridgeRosterV1")]
pub struct SccpBridgeRosterV1 {
    /// Generation number, starting at 1.
    pub generation: u64,
    /// Block time of the rotation height that created the generation.
    pub valid_from_ms: u64,
    /// `valid_from_ms + roster_validity_ms`; destinations stop trusting it afterwards.
    pub valid_until_ms: u64,
    /// First height signed by this generation.
    pub activation_height: u64,
    /// Rotation height that hands off to `generation + 1`; the last height it signs.
    #[norito(required)]
    pub handoff_height: Option<u64>,
    /// Slots in §3.7 order: zero addresses first, then nonzero addresses strictly ascending.
    pub members: Vec<SccpRosterMemberV1>,
    /// `⌊2n/3⌋ + 1`.
    pub threshold: u8,
    /// §3.7 roster digest.
    pub digest: [u8; 32],
}

/// A roster generation breaks a state-independent §3.7 or §4.3 rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SccpRosterShapeError {
    /// Generation 0 does not exist.
    #[error("roster generation must be at least 1")]
    ZeroGeneration,
    /// `valid_until_ms ≤ valid_from_ms`.
    #[error("roster valid_until_ms must exceed valid_from_ms")]
    EmptyValidity,
    /// The roster size is outside `4..=31`.
    #[error("roster has {size} members; SCCP admits 4..=31")]
    SizeOutOfRange {
        /// Number of members.
        size: usize,
    },
    /// `threshold ≠ ⌊2n/3⌋ + 1`.
    #[error("roster threshold {actual} differs from the required {expected}")]
    ThresholdMismatch {
        /// Stored threshold.
        actual: u8,
        /// `⌊2n/3⌋ + 1`.
        expected: u8,
    },
    /// Members are not zero-first then strictly ascending.
    #[error("roster member {index} breaks the zero-first, strictly ascending order")]
    MemberOrder {
        /// Index of the first out-of-order member.
        index: usize,
    },
    /// `handoff_height` precedes `activation_height`.
    #[error("roster handoff height precedes its activation height")]
    HandoffBeforeActivation,
}

impl SccpBridgeRosterV1 {
    /// Return the number of slots `n`.
    #[must_use]
    pub fn member_count(&self) -> usize {
        self.members.len()
    }

    /// Return the number of slots with a nonzero address.
    #[must_use]
    pub fn nonzero_member_count(&self) -> usize {
        self.members
            .iter()
            .filter(|member| member.is_nonzero())
            .count()
    }

    /// Return whether the generation is inert: fewer than `threshold` nonzero slots (§4.3.2).
    #[must_use]
    pub fn is_inert(&self) -> bool {
        self.nonzero_member_count() < usize::from(self.threshold)
    }

    /// Return the member addresses in slot order (the §3.7 digest input).
    pub fn addresses(&self) -> impl Iterator<Item = &[u8; 20]> {
        self.members.iter().map(|member| &member.address)
    }

    /// Check the state-independent §3.7 and §4.3 shape rules.
    ///
    /// The generation is at least 1, validity is non-empty, `n` is in `4..=31`, the threshold is
    /// `⌊2n/3⌋ + 1`, members are zero-first then strictly ascending, and a handoff does not
    /// precede activation. The digest itself is recomputed by `iroha_sccp::v1`.
    ///
    /// # Errors
    ///
    /// Returns the first violated rule.
    pub fn validate_shape(&self) -> Result<(), SccpRosterShapeError> {
        if self.generation == 0 {
            return Err(SccpRosterShapeError::ZeroGeneration);
        }
        if self.valid_until_ms <= self.valid_from_ms {
            return Err(SccpRosterShapeError::EmptyValidity);
        }
        let size = self.members.len();
        if !(SCCP_ROSTER_MIN_MEMBERS_V1..=SCCP_ROSTER_MAX_MEMBERS_V1).contains(&size) {
            return Err(SccpRosterShapeError::SizeOutOfRange { size });
        }
        let expected =
            sccp_roster_threshold_v1(u8::try_from(size).expect("size is at most 31 here"));
        if self.threshold != expected {
            return Err(SccpRosterShapeError::ThresholdMismatch {
                actual: self.threshold,
                expected,
            });
        }
        for (index, pair) in self.members.windows(2).enumerate() {
            let (previous, next) = (&pair[0], &pair[1]);
            let ordered = match (previous.is_nonzero(), next.is_nonzero()) {
                (false, _) => true,
                (true, false) => false,
                (true, true) => previous.address < next.address,
            };
            if !ordered {
                return Err(SccpRosterShapeError::MemberOrder { index: index + 1 });
            }
        }
        if self
            .handoff_height
            .is_some_and(|handoff| handoff < self.activation_height)
        {
            return Err(SccpRosterShapeError::HandoffBeforeActivation);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sccp::test_support::{assert_rejects_unknown_field, roundtrip};
    use iroha_crypto::{Algorithm, KeyPair};

    fn peer(seed: u8) -> PeerId {
        let key_pair = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("deterministic Ed25519 seed");
        PeerId::new(key_pair.public_key().clone())
    }

    fn member(address: u8, peer_seed: u8) -> SccpRosterMemberV1 {
        SccpRosterMemberV1 {
            address: [address; 20],
            peer: Some(peer(peer_seed)),
        }
    }

    fn roster(addresses: &[u8]) -> SccpBridgeRosterV1 {
        let n = u8::try_from(addresses.len()).expect("small roster");
        SccpBridgeRosterV1 {
            generation: 2,
            valid_from_ms: 1_000,
            valid_until_ms: 2_000,
            activation_height: 10,
            handoff_height: None,
            members: addresses
                .iter()
                .enumerate()
                .map(|(seed, address)| member(*address, u8::try_from(seed + 1).expect("seed")))
                .collect(),
            threshold: sccp_roster_threshold_v1(n),
            digest: [0xdd; 32],
        }
    }

    #[test]
    fn threshold_formula_matches_the_spec() {
        let expected = [(4, 3), (5, 4), (6, 5), (7, 5), (10, 7), (30, 21), (31, 21)];
        for (n, t) in expected {
            assert_eq!(sccp_roster_threshold_v1(n), t, "n = {n}");
        }
        assert_eq!(sccp_roster_threshold_v1(u8::MAX), 171);
        for n in 0..=u8::MAX {
            let reference = u8::try_from(2 * u16::from(n) / 3 + 1).expect("fits u8");
            assert_eq!(sccp_roster_threshold_v1(n), reference, "n = {n}");
        }
        assert_eq!(SCCP_ROSTER_MIN_MEMBERS_V1, 4);
        assert_eq!(SCCP_ROSTER_MAX_MEMBERS_V1, 31);
    }

    #[test]
    fn binary_and_json_roundtrip() {
        roundtrip(&member(1, 1));
        roundtrip(&SccpRosterMemberV1 {
            address: [0; 20],
            peer: None,
        });
        let mut value = roster(&[0, 1, 2, 3]);
        roundtrip(&value);
        value.handoff_height = Some(u64::MAX);
        roundtrip(&value);
        assert_rejects_unknown_field(&value, &[]);
        assert_rejects_unknown_field(&member(1, 1), &[]);
    }

    #[test]
    fn inertness_counts_nonzero_slots_against_the_threshold() {
        let active = roster(&[1, 2, 3, 4]);
        assert_eq!(active.nonzero_member_count(), 4);
        assert!(!active.is_inert());
        let at_threshold = roster(&[0, 1, 2, 3]);
        assert_eq!(at_threshold.threshold, 3);
        assert!(!at_threshold.is_inert());
        let inert = roster(&[0, 0, 1, 2]);
        assert!(inert.is_inert());
        assert!(roster(&[0, 0, 0, 0]).is_inert());
        assert!(member(1, 1).is_nonzero());
        assert!(!member(0, 1).is_nonzero());
    }

    #[test]
    fn addresses_are_in_slot_order() {
        let value = roster(&[1, 2, 3, 4]);
        assert_eq!(
            value.addresses().copied().collect::<Vec<_>>(),
            vec![[1; 20], [2; 20], [3; 20], [4; 20]]
        );
    }

    #[test]
    fn shape_rules_hold_at_their_bounds() {
        assert_eq!(roster(&[0, 1, 2, 3]).validate_shape(), Ok(()));
        let max: Vec<u8> = (1..=31).collect();
        assert_eq!(roster(&max).validate_shape(), Ok(()));
        assert_eq!(
            roster(&[1, 2, 3]).validate_shape(),
            Err(SccpRosterShapeError::SizeOutOfRange { size: 3 })
        );
        let over: Vec<u8> = (1..=32).collect();
        assert_eq!(
            roster(&over).validate_shape(),
            Err(SccpRosterShapeError::SizeOutOfRange { size: 32 })
        );

        let mut zero = roster(&[1, 2, 3, 4]);
        zero.generation = 0;
        assert_eq!(
            zero.validate_shape(),
            Err(SccpRosterShapeError::ZeroGeneration)
        );
        let mut empty = roster(&[1, 2, 3, 4]);
        empty.valid_until_ms = empty.valid_from_ms;
        assert_eq!(
            empty.validate_shape(),
            Err(SccpRosterShapeError::EmptyValidity)
        );
        let mut threshold = roster(&[1, 2, 3, 4]);
        threshold.threshold = 4;
        assert_eq!(
            threshold.validate_shape(),
            Err(SccpRosterShapeError::ThresholdMismatch {
                actual: 4,
                expected: 3
            })
        );
        let mut handoff = roster(&[1, 2, 3, 4]);
        handoff.handoff_height = Some(9);
        assert_eq!(
            handoff.validate_shape(),
            Err(SccpRosterShapeError::HandoffBeforeActivation)
        );
        handoff.handoff_height = Some(10);
        assert_eq!(handoff.validate_shape(), Ok(()));
    }

    #[test]
    fn member_order_is_zero_first_then_strictly_ascending() {
        assert_eq!(roster(&[0, 0, 5, 9]).validate_shape(), Ok(()));
        assert_eq!(
            roster(&[1, 0, 2, 3]).validate_shape(),
            Err(SccpRosterShapeError::MemberOrder { index: 1 })
        );
        assert_eq!(
            roster(&[1, 2, 2, 3]).validate_shape(),
            Err(SccpRosterShapeError::MemberOrder { index: 2 })
        );
        assert_eq!(
            roster(&[1, 2, 4, 3]).validate_shape(),
            Err(SccpRosterShapeError::MemberOrder { index: 3 })
        );
    }
}
