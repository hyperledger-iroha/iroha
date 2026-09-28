//! Bridge roster generations (`specs/sccp.md` §4.3). Owner: ws30; ws20 implemented the pure
//! lookups.
//!
//! A generation is created at genesis, at every boundary whose `(peer, address)` members
//! change, on forced rotation and on the heartbeat. The generation that signs height `h` is the
//! one with the greatest `activation_height ≤ h`. A generation with fewer than `t` nonzero
//! members is inert.

use super::store;
use crate::state::WorldReadOnly;
use iroha_data_model::sccp::roster::SccpBridgeRosterV1;

/// Return the generation that signs `height`: the greatest `activation_height ≤ height`.
#[must_use]
pub fn generation_for_height(world: &(impl WorldReadOnly + ?Sized), height: u64) -> Option<u64> {
    // Generations are numbered in activation order, so the newest qualifying one is found by
    // walking back from the newest generation.
    store::rosters::iter(world)
        .rev()
        .find(|(_, roster)| roster.activation_height <= height)
        .map(|(generation, _)| *generation)
}

/// Return whether `roster` is inert: fewer than `threshold` nonzero members (§4.3.2).
#[must_use]
pub fn is_inert(roster: &SccpBridgeRosterV1) -> bool {
    roster.is_inert()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::smartcontracts::isi::sccp::test_support::{blank_state, header, sample_roster};

    #[test]
    fn the_signing_generation_is_the_latest_activated_one() {
        let state = blank_state();
        let mut block = state.block(header(2));
        let mut stx = block.transaction();
        assert_eq!(generation_for_height(&*stx.world, 1), None);
        for (generation, activation_height) in [(1, 1), (2, 101), (3, 250)] {
            store::rosters::insert(
                &mut stx,
                generation,
                sample_roster(generation, activation_height),
            )
            .expect("a roster under its own generation");
        }
        assert_eq!(generation_for_height(&*stx.world, 1), Some(1));
        assert_eq!(generation_for_height(&*stx.world, 100), Some(1));
        assert_eq!(generation_for_height(&*stx.world, 101), Some(2));
        assert_eq!(generation_for_height(&*stx.world, 249), Some(2));
        assert_eq!(generation_for_height(&*stx.world, 10_000), Some(3));
    }

    #[test]
    fn inertness_follows_the_nonzero_member_count() {
        let mut roster = sample_roster(1, 1);
        assert!(!is_inert(&roster));
        for member in &mut roster.members {
            member.address = [0; 20];
        }
        assert!(is_inert(&roster));
    }
}
