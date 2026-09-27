/// Exact funded index produced by deterministic parent-state action validation.
/// It cannot be substituted with a fresh budget or rebuilt after controls start.
struct ValidatedNposPenaltyIndex<'state> {
    state: &'state State,
    generation: u64,
    header: BlockHeader,
    index: Option<crate::smartcontracts::isi::staking::PublicLaneStakeIndex>,
}
impl std::fmt::Debug for ValidatedNposPenaltyIndex<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ValidatedNposPenaltyIndex")
            .field("generation", &self.generation)
            .field("has_index", &self.index.is_some())
            .finish_non_exhaustive()
    }
}
impl ValidatedNposPenaltyIndex<'_> {
    fn validate_source(
        state: &State,
        generation: u64,
        header: &BlockHeader,
        overlay: &StateBlock<'_>,
    ) -> Result<(), BlockValidationError> {
        let validation = overlay.validate_native_pristine_control_owner(state, generation, header);
        if !crate::state::is_stable_state_view_generation(generation, state.state_view_generation())
        {
            return Err(BlockValidationError::LocalStorageRecoveryRequired {
                reason: "pristine NPoS index observation changed before acquired effects".into(),
            });
        }
        validation.map_err(ValidBlock::execution_context_error)
    }
}

/// Actual pristine consensus work, shared by ordinary and Native execution.
/// Preparation reads the committed predecessor before acquiring State writers;
/// consumption applies these exact effects once on the retained overlay.
struct PreparedPristineConsensusEffects<'state> {
    penalty_index: ValidatedNposPenaltyIndex<'state>,
    header: BlockHeader,
    effects: iroha_data_model::consensus::NposConsensusEffects,
    prune_keys: mv::allocation::ChargedBuffer<Hash>,
    expected_anchor: Option<iroha_data_model::consensus::GlobalThresholdBeaconChainAnchorV1>,
    roster: Vec<PeerId>,
}
fn classify_pristine_npos_application_error(error: eyre::Report) -> BlockValidationError {
    if let Some(local) = error.downcast_ref::<crate::state::EvidencePreparationError>() {
        BlockValidationError::EvidencePreparation(local.clone())
    } else {
        ValidBlock::npos_effects_error(format!(
            "NPoS consensus effects are not applicable to pristine parent state: {error}"
        ))
    }
}
impl PreparedPristineConsensusEffects<'_> {
    fn apply(self, state_block: &mut StateBlock<'_>) -> Result<(), BlockValidationError> {
        if state_block._curr_block != self.header {
            return Err(ValidBlock::npos_effects_error(
                "pristine effects have another carrier",
            ));
        }
        state_block
            .apply_pristine_npos_consensus_effects(
                &self.effects,
                self.penalty_index.index.as_ref(),
                self.prune_keys.as_slice(),
                self.expected_anchor,
                &self.roster,
                self.header.height().get(),
                self.header.view_change_index(),
                self.header.creation_time_ms,
            )
            .map(|_| ())
            .map_err(classify_pristine_npos_application_error)
    }
}

#[cfg(test)]
mod pristine_consensus_effects_tests {
    use super::*;
    use mv::allocation::{AllocationBudget, ChargedBuffer};

    #[test]
    fn pristine_npos_application_keeps_stake_index_capacity_refusal_local() {
        let budget = AllocationBudget::new(1);
        let owner = budget.try_reserve_bytes(1).expect("hold original capacity");
        let refused = match ChargedBuffer::<u64>::new(1, &budget) {
            Ok(_) => panic!("exact backing cannot fit"),
            Err(error) => error,
        };
        let local = crate::state::EvidencePreparationError::from(refused);
        let classified = classify_pristine_npos_application_error(local.clone().into());
        assert!(matches!(
            classified,
            BlockValidationError::EvidencePreparation(refusal) if refusal == local
        ));
        drop(owner);
        assert!(matches!(
            classify_pristine_npos_application_error(eyre::eyre!("invalid effects")),
            BlockValidationError::NposEffectsInvalid(_)
        ));
    }

    #[test]
    fn pristine_penalty_index_capacity_refuses_before_writes_and_retries_exact_source() {
        let (state, block, _, share_key) =
            crate::sumeragi::penalties::pristine_penalty_component_fixture_for_tests();
        let original_share = state
            .world
            .public_lane_stake_shares
            .view()
            .get(&share_key)
            .cloned()
            .expect("original accepted stake");
        let original_body = block.encode_wire().expect("canonical component carrier");
        let generation = state.state_view_generation();
        let budget = state.stake_index_budget();
        let first = ValidBlock::validate_npos_effects_with_state(&block, &state, None, None)
            .expect("derive actual parent index");
        let backing = budget.reserved_bytes();
        assert!(backing > 0);
        assert!(first.index.is_some());
        drop(first);
        assert_eq!(budget.reserved_bytes(), 0);

        let exact_held = budget
            .try_reserve_bytes(budget.limit_bytes() - backing)
            .expect("leave the measured original backing available");
        let exact = ValidBlock::validate_npos_effects_with_state(&block, &state, None, None)
            .expect("exact remaining capacity admits the real deterministic producer");
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        drop(exact);
        drop(exact_held);
        let held_bytes = budget.limit_bytes() - backing + 1;
        let held = budget
            .try_reserve_bytes(held_bytes)
            .expect("leave one byte less than demand");
        let error = ValidBlock::validate_npos_effects_with_state(&block, &state, None, None)
            .expect_err("one byte below exact demand must refuse before effects");
        let BlockValidationError::EvidencePreparation(refusal) = error else {
            panic!("stake capacity refusal must remain local: {error:?}");
        };
        assert!(matches!(&refusal,
            crate::state::EvidencePreparationError::Admission(
                mv::allocation::AllocationRefusal::Capacity { requested_bytes, .. }
            ) if *requested_bytes == backing
        ));
        assert!(refusal.release_wait().is_some());
        assert_eq!(budget.reserved_bytes(), held_bytes);
        assert_eq!(state.state_view_generation(), generation);
        assert_eq!(
            state.world.public_lane_stake_shares.view().get(&share_key),
            Some(&original_share)
        );
        assert_eq!(
            block.encode_wire().expect("same canonical carrier"),
            original_body,
            "refusal preserves the same caller-owned carrier"
        );
        drop(held);
        let retry = ValidBlock::validate_npos_effects_with_state(&block, &state, None, None)
            .expect("retry on the original State and body after capacity release");
        assert_eq!(budget.reserved_bytes(), backing);
        assert_eq!(
            retry
                .index
                .as_ref()
                .unwrap()
                .share_keys(share_key.0, &share_key.1),
            &[share_key]
        );
        drop(retry);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn pristine_penalty_index_moves_into_apply_without_rebuilding_under_full_budget() {
        let (state, block, context, share_key) =
            crate::sumeragi::penalties::pristine_penalty_component_fixture_for_tests();
        let original_share = state
            .world
            .public_lane_stake_shares
            .view()
            .get(&share_key)
            .cloned()
            .expect("original accepted stake");
        // This tests action/control ownership. The fixture is not global block
        // admission, and its prior-height context supplies only the same roster.
        let plan = ValidBlock::validate_npos_effects_with_state(&block, &state, None, None)
            .expect("validate actual due actions and retain producer index");
        let identity = plan.index.as_ref().unwrap().allocation_identity_for_test();
        let backing = state.stake_index_budget().reserved_bytes();
        let prepared =
            ValidBlock::prepare_pristine_consensus_effects(&block, &state, plan, Some(&context))
                .expect("prepare original validated owner")
                .expect("due slash controls");
        assert_eq!(
            prepared
                .penalty_index
                .index
                .as_ref()
                .unwrap()
                .allocation_identity_for_test(),
            identity
        );
        assert_eq!(state.stake_index_budget().reserved_bytes(), backing);
        let held_bytes = state.stake_index_budget().limit_bytes() - backing;
        let held = state
            .stake_index_budget()
            .try_reserve_bytes(held_bytes)
            .expect("occupy every byte not owned by the prepared index");
        let overlay = state
            .block_with_pristine_stage(block.header(), |overlay| {
                ValidatedNposPenaltyIndex::validate_source(
                    prepared.penalty_index.state,
                    prepared.penalty_index.generation,
                    &prepared.header,
                    overlay,
                )?;
                assert_eq!(
                    prepared
                        .penalty_index
                        .index
                        .as_ref()
                        .unwrap()
                        .allocation_identity_for_test(),
                    identity
                );
                assert_eq!(
                    state.stake_index_budget().reserved_bytes(),
                    state.stake_index_budget().limit_bytes()
                );
                prepared.apply(overlay)?;
                assert_eq!(
                    state.stake_index_budget().reserved_bytes(),
                    held_bytes,
                    "consumption releases the original index after the sole kernel returns"
                );
                let applied = overlay
                    .world
                    .public_lane_stake_shares
                    .get(&share_key)
                    .expect("slashed share remains present");
                assert!(applied.bonded < original_share.bonded);
                Ok::<(), BlockValidationError>(())
            })
            .expect("application cannot need a second index allocation");
        drop(overlay);
        assert_eq!(
            state.world.public_lane_stake_shares.view().get(&share_key),
            Some(&original_share),
            "discarding this component overlay publishes no changes"
        );
        drop(held);
        assert_eq!(state.stake_index_budget().reserved_bytes(), 0);
    }

    #[test]
    fn pristine_penalty_index_rejects_tampered_actions_and_crossed_carrier_owner() {
        let (state, block, context, share_key) =
            crate::sumeragi::penalties::pristine_penalty_component_fixture_for_tests();
        let original_share = state
            .world
            .public_lane_stake_shares
            .view()
            .get(&share_key)
            .cloned()
            .expect("original accepted stake");
        let mut changed = block.clone();
        let mut effects = changed.npos_consensus_effects().unwrap().clone();
        let slash = effects
            .penalty_actions
            .iter_mut()
            .find_map(|action| match action {
                iroha_data_model::consensus::NposPenaltyAction::ConsensusSlash(slash) => {
                    Some(slash)
                }
                _ => None,
            })
            .expect("actual due slash fixture");
        slash.amount = iroha_primitives::numeric::Quantity::from(1_u64);
        assert_ne!(&effects, block.npos_consensus_effects().unwrap());
        changed.set_npos_consensus_effects(Some(effects));
        assert!(matches!(
            ValidBlock::validate_npos_effects_with_state(&changed, &state, None, None),
            Err(BlockValidationError::NposEffectsInvalid(_))
        ));
        assert_eq!(state.stake_index_budget().reserved_bytes(), 0);
        let plan =
            ValidBlock::validate_npos_effects_with_state(&block, &state, None, None).unwrap();
        assert!(
            matches!(
                ValidBlock::prepare_pristine_consensus_effects(
                    &changed,
                    &state,
                    plan,
                    Some(&context)
                ),
                Err(BlockValidationError::NposEffectsInvalid(_))
            ),
            "a validated index cannot authenticate a changed carrier/effects hash"
        );
        assert_eq!(state.stake_index_budget().reserved_bytes(), 0);
        let (other, _, _, _) =
            crate::sumeragi::penalties::pristine_penalty_component_fixture_for_tests();
        let plan =
            ValidBlock::validate_npos_effects_with_state(&block, &state, None, None).unwrap();
        let error = other.block_with_pristine_stage(block.header(), |overlay| {
            ValidatedNposPenaltyIndex::validate_source(
                &state,
                plan.generation,
                &plan.header,
                overlay,
            )
        });
        assert!(
            error.is_err(),
            "same-header foreign State cannot consume original controls"
        );
        drop(error);
        let stale = state.block_with_pristine_stage(block.header(), |overlay| {
            ValidatedNposPenaltyIndex::validate_source(
                &state,
                plan.generation + 2,
                &plan.header,
                overlay,
            )
        });
        assert!(
            stale.is_err(),
            "stale observation refuses before control entry"
        );
        drop(stale);
        assert!(state.stake_index_budget().reserved_bytes() > 0);
        drop(plan);
        assert_eq!(state.stake_index_budget().reserved_bytes(), 0);
        assert_eq!(
            state.world.public_lane_stake_shares.view().get(&share_key),
            Some(&original_share)
        );
    }

    #[test]
    fn pristine_penalty_index_unwind_releases_original_lease_and_uncommitted_overlay() {
        let (state, block, context, share_key) =
            crate::sumeragi::penalties::pristine_penalty_component_fixture_for_tests();
        let original_share = state
            .world
            .public_lane_stake_shares
            .view()
            .get(&share_key)
            .cloned()
            .expect("original accepted stake");
        for panic_before_apply in [true, false] {
            let plan = ValidBlock::validate_npos_effects_with_state(&block, &state, None, None)
                .expect("the same source remains retryable");
            let prepared = ValidBlock::prepare_pristine_consensus_effects(
                &block,
                &state,
                plan,
                Some(&context),
            )
            .unwrap()
            .unwrap();
            assert!(state.stake_index_budget().reserved_bytes() > 0);
            let reached_panic = std::cell::Cell::new(0);
            let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = state.block_with_pristine_stage(
                    block.header(),
                    |overlay| -> Result<(), BlockValidationError> {
                        ValidatedNposPenaltyIndex::validate_source(
                            prepared.penalty_index.state,
                            prepared.penalty_index.generation,
                            &prepared.header,
                            overlay,
                        )?;
                        if panic_before_apply {
                            reached_panic.set(1);
                            panic!("test original prepared-owner unwind before consumption");
                        }
                        prepared.apply(overlay)?;
                        assert_eq!(state.stake_index_budget().reserved_bytes(), 0);
                        reached_panic.set(2);
                        panic!("test original overlay unwind after consumption");
                    },
                );
            }));
            assert!(unwind.is_err());
            assert_eq!(
                reached_panic.get(),
                if panic_before_apply { 1 } else { 2 },
                "only the intended pre/post-consumption panic satisfies this test"
            );
            assert_eq!(state.stake_index_budget().reserved_bytes(), 0);
            assert_eq!(
                state.world.public_lane_stake_shares.view().get(&share_key),
                Some(&original_share)
            );
        }
    }

    #[test]
    fn pristine_penalty_absence_is_fenced_before_ordinary_control_entry() {
        let (state, mut block, _, share_key) =
            crate::sumeragi::penalties::pristine_penalty_component_fixture_for_tests();
        let (evidence_key, evidence) = state
            .world
            .consensus_evidence
            .view()
            .iter()
            .next()
            .map(|(key, record)| (*key, record.clone()))
            .expect("real authenticated due evidence");
        let original_share = state
            .world
            .public_lane_stake_shares
            .view()
            .get(&share_key)
            .cloned()
            .expect("original accepted stake");
        {
            let mut rows = state.world.consensus_evidence.block();
            rows.remove(evidence_key);
            rows.commit();
        }
        block.set_npos_consensus_effects(None);
        let original_body = block
            .encode_wire()
            .expect("canonical action-free component carrier");
        let plan = ValidBlock::validate_npos_effects_with_state(&block, &state, None, None)
            .expect("validate actual absence of due evidence");
        assert!(plan.index.is_none());
        assert!(block.npos_consensus_effects().is_none());
        assert_eq!(state.stake_index_budget().reserved_bytes(), 0);
        let observed_generation = plan.generation;
        let mut publication = state.block(block.header());
        publication
            .world
            .consensus_evidence
            .insert(evidence_key, evidence);
        publication
            .commit_world_overlay_for_testing()
            .expect("publish newly due evidence through actual State generation");
        assert_eq!(state.state_view_generation(), observed_generation + 2);
        let current_generation = state.state_view_generation();
        let refusal =
            ValidBlock::state_block_for_execution(&block, &state, plan, false, None, None, None)
                .err()
                .expect("stale absence must refuse in the acquired pristine callback");
        assert!(matches!(
            refusal,
            BlockValidationError::LocalStorageRecoveryRequired { .. }
        ));
        assert_eq!(state.state_view_generation(), current_generation);
        assert_eq!(
            block.encode_wire().expect("same source body"),
            original_body
        );
        assert_eq!(
            state.world.public_lane_stake_shares.view().get(&share_key),
            Some(&original_share)
        );
        assert!(
            matches!(
                ValidBlock::validate_npos_effects_with_state(&block, &state, None, None),
                Err(BlockValidationError::NposEffectsInvalid(_))
            ),
            "the newly observed State now requires the omitted due penalty"
        );
        assert_eq!(state.stake_index_budget().reserved_bytes(), 0);
    }
}
