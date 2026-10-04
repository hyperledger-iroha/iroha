//! Original confidential-policy transition lookups for a retained definition cut.
//!
//! The definition capture supplies its existing reader and work allowance. This
//! helper never reacquires definitions, allocates a replacement index, or treats
//! `zk_assets` as the authority for an asset definition's pending transition.
//! Both original indexes remain with `CheckedAssetDefinitions` through encoding;
//! this scoped check supplies no finalized State authority.

use super::*;
use iroha_data_model::asset::{AssetDefinition, AssetDefinitionId};

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

    /// Check both native images using the enclosing capture's one work counter.
    /// The caller checks every original source/index identity even on rejection.
    pub(super) fn validate(
        &self,
        definitions: &CommittedStorageView<'world, AssetDefinitionId, AssetDefinition>,
        work: &mut Work,
    ) -> Result<(), GroupedOwnershipError> {
        for image in [GroupImage::Current, GroupImage::Predecessor] {
            let corrupt = |index, mismatch| GroupedOwnershipError::Corrupt {
                index,
                image,
                mismatch,
            };
            visit_image(definitions, image, work, |id, definition, work| {
                let policy = definition.confidential_policy();
                if !policy.pending_transition_is_valid() {
                    return Err(GroupedOwnershipError::Source {
                        table: "world.asset_definitions",
                        image,
                        reason: "invalid pending confidential-policy transition",
                    });
                }
                if let Some(transition) = policy.pending_transition() {
                    let height = transition.effective_height();
                    // AssetDefinitionId owns only its sixteen UUID bytes. This
                    // stack key neither clones a heap payload nor retains state.
                    let key = (height, id.clone());
                    work.charge()?;
                    if get_at(&self.transitions, image, &key).is_none() {
                        return Err(corrupt(TRANSITIONS, GroupMismatch::MissingMember));
                    }
                    work.charge()?;
                    match get_at(&self.counts, image, &height) {
                        None => return Err(corrupt(COUNTS, GroupMismatch::MissingMember)),
                        Some(0) => return Err(corrupt(COUNTS, GroupMismatch::EmptyGroup)),
                        Some(_) => {}
                    }
                }
                Ok(())
            })?;
            visit_image(&self.transitions, image, work, |(height, id), (), work| {
                work.charge()?;
                if !get_at(definitions, image, id).is_some_and(|definition| {
                    definition
                        .confidential_policy()
                        .pending_transition()
                        .is_some_and(|transition| transition.effective_height() == *height)
                }) {
                    return Err(corrupt(TRANSITIONS, GroupMismatch::ForeignMember));
                }
                Ok(())
            })?;
            visit_image(&self.counts, image, work, |height, count, work| {
                if *count == 0 {
                    return Err(corrupt(COUNTS, GroupMismatch::EmptyGroup));
                }
                let mut expected = 0_u32;
                // Count original canonical sources, not a projection which
                // could omit rows. Every physical source/undo row is charged
                // before masking, including tombstones and redundant touches.
                visit_image(definitions, image, work, |_, definition, _| {
                    if definition
                        .confidential_policy()
                        .pending_transition()
                        .is_some_and(|transition| transition.effective_height() == *height)
                    {
                        expected =
                            expected
                                .checked_add(1)
                                .ok_or(GroupedOwnershipError::Source {
                                    table: "world.asset_definitions",
                                    image,
                                    reason: "confidential-policy transition count overflow",
                                })?;
                    }
                    Ok(())
                })?;
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

    /// Keep both original identities through source validation and row encoding.
    pub(super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        let transitions = self
            .transitions
            .try_matches_current(&self.world.confidential_policy_transition_index)?;
        let counts = self
            .counts
            .try_matches_current(&self.world.confidential_policy_transition_counts)?;
        Ok(transitions && counts)
    }
}

#[cfg(test)]
mod tests;
