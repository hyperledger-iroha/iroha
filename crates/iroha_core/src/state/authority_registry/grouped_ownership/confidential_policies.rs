//! Original pending-policy relation shared by the seven-owner definition check.
//!
//! Both original indexes remain retained through canonical encoding. The shared
//! body borrows exact native maps, never zk_assets or a reconstructed projection.
//! This is not finalized State authority or physical scratch funding.

use super::asset_definitions::{
    AssetDefinitionWork, AssetWorkKey, equal, lookup, prepay_asset_definition_id, visit_original,
};
use super::*;
use crate::state::authority_registry::original_images::RawStorageImages;
use iroha_data_model::asset::{
    AssetDefinition, AssetDefinitionId, definition::AssetConfidentialPolicy,
};

const TRANSITIONS: &str = "world.confidential_policy_transition_index";
const COUNTS: &str = "world.confidential_policy_transition_counts";

/// Retained indexes whose validation borrows the parent's original definitions.
pub(super) struct RetainedConfidentialPolicies<'world> {
    world: &'world World,
    transitions: CommittedStorageView<'world, (u64, AssetDefinitionId), ()>,
    counts: CommittedStorageView<'world, u64, u32>,
}
impl<'world> RetainedConfidentialPolicies<'world> {
    /// Retain both original lookup readers before checking any source row.
    pub(super) fn retain(world: &'world World) -> Result<Self, GroupedOwnershipError> {
        Ok(Self {
            world,
            transitions: world
                .confidential_policy_transition_index
                .try_committed_view_nonblocking()?,
            counts: world
                .confidential_policy_transition_counts
                .try_committed_view_nonblocking()?,
        })
    }
    pub(super) fn originals(
        &self,
    ) -> (
        &CommittedStorageView<'world, (u64, AssetDefinitionId), ()>,
        &CommittedStorageView<'world, u64, u32>,
    ) {
        (&self.transitions, &self.counts)
    }

    #[cfg(test)]
    fn validate(
        &self,
        definitions: &CommittedStorageView<'world, AssetDefinitionId, AssetDefinition>,
        work: &mut AssetDefinitionWork,
    ) -> Result<(), GroupedOwnershipError> {
        validate_original_confidential_policies(definitions, &self.transitions, &self.counts, work)
    }
    pub(super) fn current_results(
        &self,
    ) -> [Result<bool, PublicationPreparationError<Infallible>>; 2] {
        [
            self.transitions
                .try_matches_current(&self.world.confidential_policy_transition_index),
            self.counts
                .try_matches_current(&self.world.confidential_policy_transition_counts),
        ]
    }
    #[cfg(test)]
    fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        let [transitions, counts] = self.current_results();
        let transitions = transitions?;
        let counts = counts?;
        Ok(transitions && counts)
    }
}

pub(super) fn prepay_policy(
    policy: &AssetConfidentialPolicy,
    work: &mut AssetDefinitionWork,
) -> Result<(), GroupedOwnershipError> {
    work.prepay(2)?; // mode and pending Option tag, before copying a transition
    if let Some(transition) = policy.pending_transition() {
        work.prepay(1 + 8 + 1 + 32 + 1)?;
        if transition.conversion_window().is_some() {
            work.prepay(8)?;
        }
    }
    Ok(())
}

// Borrow the original tuple components instead of copying even the fixed ID.
fn lookup_transition<'a>(
    rows: &'a impl RawStorageImages<(u64, AssetDefinitionId), ()>,
    image: GroupImage,
    height: u64,
    id: &AssetDefinitionId,
    work: &mut AssetDefinitionWork,
) -> Result<Option<&'a ()>, GroupedOwnershipError> {
    let mut found = None;
    visit_original(rows, image, work, |key, value, work| {
        work.prepay(8)?;
        prepay_asset_definition_id(id, |amount| work.prepay(amount))?;
        key.prepay(work)?;
        if height == key.0 && id == &key.1 {
            found = Some(value);
        }
        Ok(())
    })?;
    Ok(found)
}

pub(super) fn validate_original_confidential_policies(
    definitions: &impl RawStorageImages<AssetDefinitionId, AssetDefinition>,
    transitions: &impl RawStorageImages<(u64, AssetDefinitionId), ()>,
    counts: &impl RawStorageImages<u64, u32>,
    work: &mut AssetDefinitionWork,
) -> Result<(), GroupedOwnershipError> {
    for image in [GroupImage::Current, GroupImage::Predecessor] {
        let corrupt = |index, mismatch| GroupedOwnershipError::Corrupt {
            index,
            image,
            mismatch,
        };
        visit_original(definitions, image, work, |id, definition, work| {
            let policy = definition.confidential_policy();
            prepay_policy(policy, work)?;
            if !policy.pending_transition_is_valid() {
                return Err(GroupedOwnershipError::Source {
                    table: "world.asset_definitions",
                    image,
                    reason: "invalid pending confidential-policy transition",
                });
            }
            if let Some(transition) = policy.pending_transition() {
                let height = transition.effective_height();
                if lookup_transition(transitions, image, height, id, work)?.is_none() {
                    return Err(corrupt(TRANSITIONS, GroupMismatch::MissingMember));
                }
                match lookup(counts, image, &height, work)? {
                    None => return Err(corrupt(COUNTS, GroupMismatch::MissingMember)),
                    Some(count) => {
                        work.prepay(4)?;
                        if *count == 0 {
                            return Err(corrupt(COUNTS, GroupMismatch::EmptyGroup));
                        }
                    }
                }
            }
            Ok(())
        })?;
        visit_original(transitions, image, work, |(height, id), (), work| {
            let Some(definition) = lookup(definitions, image, id, work)? else {
                return Err(corrupt(TRANSITIONS, GroupMismatch::ForeignMember));
            };
            let policy = definition.confidential_policy();
            prepay_policy(policy, work)?;
            let Some(transition) = policy.pending_transition() else {
                return Err(corrupt(TRANSITIONS, GroupMismatch::ForeignMember));
            };
            if !equal(&transition.effective_height(), height, work)? {
                return Err(corrupt(TRANSITIONS, GroupMismatch::ForeignMember));
            }
            Ok(())
        })?;
        visit_original(counts, image, work, |height, count, work| {
            work.prepay(4)?;
            if *count == 0 {
                return Err(corrupt(COUNTS, GroupMismatch::EmptyGroup));
            }
            work.prepay(4)?;
            let mut expected = 0_u32;
            visit_original(definitions, image, work, |_, definition, work| {
                let policy = definition.confidential_policy();
                prepay_policy(policy, work)?;
                if let Some(transition) = policy.pending_transition() {
                    if equal(&transition.effective_height(), height, work)? {
                        work.prepay(4)?;
                        expected =
                            expected
                                .checked_add(1)
                                .ok_or(GroupedOwnershipError::Source {
                                    table: "world.asset_definitions",
                                    image,
                                    reason: "confidential-policy transition count overflow",
                                })?;
                    }
                }
                Ok(())
            })?;
            work.prepay(8)?;
            if expected != *count {
                return Err(corrupt(
                    COUNTS,
                    if expected > *count {
                        GroupMismatch::MissingMember
                    } else {
                        GroupMismatch::ForeignMember
                    },
                ));
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests;
