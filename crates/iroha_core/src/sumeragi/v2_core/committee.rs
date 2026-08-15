//! Deterministic bounded committee-role projection for Sumeragi v2.
//!
//! A [`HeightContext`] carries the adapter-normalized
//! `H(epoch_seed, height)` leader seed. This module projects that frozen input
//! onto the production `n = 3f + 1` geometry without changing validator
//! identities: [`ValidatorIndex`] values always refer to positions in the
//! context's canonical roster.
use super::types::{
    HeightContext, MAX_FAULT_TOLERANCE, MAX_VOTING_ROSTER_LEN, MIN_FAULT_TOLERANCE,
    MIN_VOTING_ROSTER_LEN, ValidatorId,
};
use std::{error::Error, fmt};
/// Stable position of a validator in the canonically ordered height roster.
pub type ValidatorIndex = u32;
/// Smallest production committee (`3 * 1 + 1`).
pub const MIN_COMMITTEE_SIZE: usize = MIN_VOTING_ROSTER_LEN;
/// Largest production committee (`3 * 10 + 1`).
pub const MAX_COMMITTEE_SIZE: usize = MAX_VOTING_ROSTER_LEN;
/// Role occupied by a validator in one projected height/view committee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitteeRole {
    /// First Set A member, responsible for proposing the block.
    Leader,
    /// A Set A validator between the leader and proxy tail.
    SetAValidator,
    /// Last Set A member, responsible for collecting votes.
    ProxyTail,
    /// A Set B validator available for the recovery path.
    SetBValidator,
}
/// Deterministic role assignment for one height and view.
///
/// `order` is a permutation of stable roster indices. Set A occupies the
/// first `q = 2f + 1` positions; the remaining `f` positions form Set B.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Committee {
    height: u64,
    view: u64,
    fault_tolerance: usize,
    quorum_size: usize,
    order: Vec<ValidatorIndex>,
}
impl Committee {
    /// Projects a frozen height context onto the roles for `view`.
    ///
    /// The base order starts at the context's height-seeded view-zero leader
    /// and follows the canonical roster cyclically. Advancing the view rotates
    /// that same permutation by one position, so validator indices remain
    /// stable across all views.
    ///
    /// # Errors
    ///
    /// Returns an error unless the roster has exact `n = 3f + 1` geometry for
    /// `1 <= f <= 10`.
    pub fn project(context: &HeightContext, view: u64) -> Result<Self, CommitteeError> {
        let roster_len = context.roster().len();
        let leader = context.leader(view);
        let leader_position = context
            .roster()
            .iter()
            .position(|validator| validator.id() == leader)
            .ok_or(CommitteeError::LeaderNotInRoster(leader))?;
        let leader_index = ValidatorIndex::try_from(leader_position)
            .map_err(|_| CommitteeError::ValidatorIndexOverflow(leader_position))?;
        Self::project_indices(context.height(), view, roster_len, leader_index)
    }
    /// Project roles from a stable roster length and already-derived leader
    /// index.
    ///
    /// This is the adapter seam for wire contexts, whose leader is already a
    /// stable roster index. It deliberately performs no hashing: the caller
    /// must derive `leader_index` from the authenticated height context.
    ///
    /// # Errors
    ///
    /// Returns an error for non-production geometry or an out-of-range leader.
    pub fn project_indices(
        height: u64,
        view: u64,
        roster_len: usize,
        leader_index: ValidatorIndex,
    ) -> Result<Self, CommitteeError> {
        let fault_tolerance = validate_geometry(roster_len)?;
        let quorum_size = 2 * fault_tolerance + 1;
        let leader_position = usize::try_from(leader_index).map_err(|_| {
            CommitteeError::ValidatorIndexOutOfRange {
                index: leader_index,
                roster_len,
            }
        })?;
        if leader_position >= roster_len {
            return Err(CommitteeError::ValidatorIndexOutOfRange {
                index: leader_index,
                roster_len,
            });
        }
        let mut order = Vec::with_capacity(roster_len);
        for offset in 0..roster_len {
            let roster_position = (leader_position + offset) % roster_len;
            let index = ValidatorIndex::try_from(roster_position)
                .map_err(|_| CommitteeError::ValidatorIndexOverflow(roster_position))?;
            order.push(index);
        }
        Ok(Self {
            height,
            view,
            fault_tolerance,
            quorum_size,
            order,
        })
    }
    /// Returns the height for which this projection was derived.
    #[must_use]
    pub const fn height(&self) -> u64 {
        self.height
    }
    /// Returns the projected view.
    #[must_use]
    pub const fn view(&self) -> u64 {
        self.view
    }
    /// Returns the tolerated Byzantine-validator count `f`.
    #[must_use]
    pub const fn fault_tolerance(&self) -> usize {
        self.fault_tolerance
    }
    /// Returns the exact quorum size `q = 2f + 1`.
    #[must_use]
    pub const fn quorum_size(&self) -> usize {
        self.quorum_size
    }
    /// Returns the complete role-ordered stable-index permutation.
    #[must_use]
    pub fn order(&self) -> &[ValidatorIndex] {
        &self.order
    }
    /// Returns Set A, including the leader and proxy tail.
    #[must_use]
    pub fn set_a(&self) -> &[ValidatorIndex] {
        &self.order[..self.quorum_size]
    }
    /// Returns Set B, which contains exactly `f` validators.
    #[must_use]
    pub fn set_b(&self) -> &[ValidatorIndex] {
        &self.order[self.quorum_size..]
    }
    /// Returns the first Set A member.
    #[must_use]
    pub fn leader(&self) -> ValidatorIndex {
        self.order[0]
    }
    /// Returns the last Set A member.
    #[must_use]
    pub fn proxy_tail(&self) -> ValidatorIndex {
        self.order[self.quorum_size - 1]
    }
    /// Looks up the projected role of a stable roster index.
    ///
    /// # Errors
    ///
    /// Returns [`CommitteeError::ValidatorIndexOutOfRange`] when `index` does
    /// not identify a member of this committee.
    pub fn role(&self, index: ValidatorIndex) -> Result<CommitteeRole, CommitteeError> {
        if usize::try_from(index)
            .ok()
            .is_none_or(|index| index >= self.order.len())
        {
            return Err(CommitteeError::ValidatorIndexOutOfRange {
                index,
                roster_len: self.order.len(),
            });
        }
        let position = self
            .order
            .iter()
            .position(|candidate| *candidate == index)
            .ok_or(CommitteeError::ValidatorIndexOutOfRange {
                index,
                roster_len: self.order.len(),
            })?;
        Ok(if position == 0 {
            CommitteeRole::Leader
        } else if position == self.quorum_size - 1 {
            CommitteeRole::ProxyTail
        } else if position < self.quorum_size {
            CommitteeRole::SetAValidator
        } else {
            CommitteeRole::SetBValidator
        })
    }
}
fn validate_geometry(roster_len: usize) -> Result<usize, CommitteeError> {
    if roster_len < MIN_COMMITTEE_SIZE {
        return Err(CommitteeError::CommitteeTooSmall {
            actual: roster_len,
            minimum: MIN_COMMITTEE_SIZE,
        });
    }
    if roster_len > MAX_COMMITTEE_SIZE {
        return Err(CommitteeError::CommitteeTooLarge {
            actual: roster_len,
            maximum: MAX_COMMITTEE_SIZE,
        });
    }
    if (roster_len - 1) % 3 != 0 {
        return Err(CommitteeError::InvalidCommitteeGeometry { actual: roster_len });
    }
    let fault_tolerance = (roster_len - 1) / 3;
    if !(MIN_FAULT_TOLERANCE..=MAX_FAULT_TOLERANCE).contains(&fault_tolerance) {
        return Err(CommitteeError::InvalidCommitteeGeometry { actual: roster_len });
    }
    Ok(fault_tolerance)
}
/// Failure to project or query a bounded Sumeragi committee.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommitteeError {
    /// The roster cannot tolerate the minimum supported Byzantine fault.
    CommitteeTooSmall {
        /// Actual roster length.
        actual: usize,
        /// Minimum accepted roster length.
        minimum: usize,
    },
    /// The roster exceeds the bounded production geometry.
    CommitteeTooLarge {
        /// Actual roster length.
        actual: usize,
        /// Maximum accepted roster length.
        maximum: usize,
    },
    /// The roster length is within bounds but is not exactly `3f + 1`.
    InvalidCommitteeGeometry {
        /// Actual roster length.
        actual: usize,
    },
    /// The context selected a leader absent from its own frozen roster.
    LeaderNotInRoster(ValidatorId),
    /// A canonical roster position could not fit the stable wire index.
    ValidatorIndexOverflow(usize),
    /// A role lookup used an index outside this committee.
    ValidatorIndexOutOfRange {
        /// Requested stable roster index.
        index: ValidatorIndex,
        /// Number of validators in the committee.
        roster_len: usize,
    },
}
impl fmt::Display for CommitteeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CommitteeTooSmall { actual, minimum } => write!(
                formatter,
                "committee has {actual} validators; at least {minimum} are required"
            ),
            Self::CommitteeTooLarge { actual, maximum } => write!(
                formatter,
                "committee has {actual} validators; at most {maximum} are supported"
            ),
            Self::InvalidCommitteeGeometry { actual } => write!(
                formatter,
                "committee has {actual} validators; production geometry requires n = 3f + 1"
            ),
            Self::LeaderNotInRoster(leader) => {
                write!(
                    formatter,
                    "height context leader {leader} is absent from its roster"
                )
            }
            Self::ValidatorIndexOverflow(index) => {
                write!(
                    formatter,
                    "roster position {index} does not fit ValidatorIndex"
                )
            }
            Self::ValidatorIndexOutOfRange { index, roster_len } => write!(
                formatter,
                "validator index {index} is outside the {roster_len}-member committee"
            ),
        }
    }
}
impl Error for CommitteeError {}
#[cfg(test)]
mod tests {
    use super::super::types::{
        ContextId, Digest, HeightContext, NetworkId, Validator, VotingMode, VotingPower,
    };
    use super::*;
    fn validator_id(index: usize) -> ValidatorId {
        let marker = u8::try_from(index + 1).expect("committee fixtures are bounded to 31 members");
        ValidatorId::repeat(marker)
    }
    fn context(roster_len: usize, seed_offset: u8) -> HeightContext {
        let roster = (0..roster_len)
            .map(|index| Validator::new(validator_id(index), VotingPower::new(1)))
            .collect();
        let mut height_seed = [0_u8; 32];
        height_seed[31] = seed_offset;
        HeightContext::new(
            ContextId::repeat(0x50),
            NetworkId::repeat(0x51),
            1,
            None,
            0,
            roster,
            VotingMode::Permissioned,
            Digest::repeat(0x52),
            Digest::repeat(0x53),
            Digest::repeat(0x54),
            Digest::new(height_seed),
        )
        .expect("valid committee fixture context")
    }
    #[test]
    fn accepts_every_supported_three_f_plus_one_geometry() {
        for fault_tolerance in MIN_FAULT_TOLERANCE..=MAX_FAULT_TOLERANCE {
            let roster_len = 3 * fault_tolerance + 1;
            let committee =
                Committee::project(&context(roster_len, 0), 0).expect("supported geometry");
            assert_eq!(committee.fault_tolerance(), fault_tolerance);
            assert_eq!(committee.quorum_size(), 2 * fault_tolerance + 1);
            assert_eq!(committee.order().len(), roster_len);
            assert_eq!(committee.set_a().len(), 2 * fault_tolerance + 1);
            assert_eq!(committee.set_b().len(), fault_tolerance);
        }
    }
    #[test]
    fn rejects_out_of_bound_and_non_three_f_plus_one_rosters() {
        assert_eq!(
            Committee::project_indices(1, 0, 3, 0),
            Err(CommitteeError::CommitteeTooSmall {
                actual: 3,
                minimum: 4,
            })
        );
        assert_eq!(
            Committee::project_indices(1, 0, 32, 0),
            Err(CommitteeError::CommitteeTooLarge {
                actual: 32,
                maximum: 31,
            })
        );
        assert_eq!(
            Committee::project_indices(1, 0, 5, 0),
            Err(CommitteeError::InvalidCommitteeGeometry { actual: 5 })
        );
    }
    #[test]
    fn projects_stable_indices_and_expected_roles() {
        let committee = Committee::project(&context(7, 2), 0).expect("valid committee");
        assert_eq!(committee.height(), 1);
        assert_eq!(committee.view(), 0);
        assert_eq!(committee.order(), &[2, 3, 4, 5, 6, 0, 1]);
        assert_eq!(committee.set_a(), &[2, 3, 4, 5, 6]);
        assert_eq!(committee.set_b(), &[0, 1]);
        assert_eq!(committee.leader(), 2);
        assert_eq!(committee.proxy_tail(), 6);
        assert_eq!(committee.role(2), Ok(CommitteeRole::Leader));
        assert_eq!(committee.role(3), Ok(CommitteeRole::SetAValidator));
        assert_eq!(committee.role(6), Ok(CommitteeRole::ProxyTail));
        assert_eq!(committee.role(0), Ok(CommitteeRole::SetBValidator));
        assert_eq!(
            committee.role(7),
            Err(CommitteeError::ValidatorIndexOutOfRange {
                index: 7,
                roster_len: 7,
            })
        );
    }
    #[test]
    fn view_change_cyclically_rotates_one_height_permutation() {
        let context = context(7, 2);
        let base = Committee::project(&context, 0).expect("base view");
        for view in 0_u64..21 {
            let projected = Committee::project(&context, view).expect("rotated view");
            let rotation = usize::try_from(view % 7).expect("bounded rotation");
            let expected = base
                .order()
                .iter()
                .cycle()
                .skip(rotation)
                .take(7)
                .copied()
                .collect::<Vec<_>>();
            assert_eq!(projected.order(), expected);
        }
    }
    #[test]
    fn projection_is_deterministic_for_a_frozen_height_context() {
        let context = context(10, 7);
        let first = Committee::project(&context, u64::MAX).expect("first projection");
        let second = Committee::project(&context, u64::MAX).expect("second projection");
        assert_eq!(first, second);
    }
}
