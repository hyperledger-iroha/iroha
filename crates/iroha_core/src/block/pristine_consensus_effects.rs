/// Actual pristine consensus work, shared by ordinary and Native execution.
/// Preparation reads the committed predecessor before acquiring State writers;
/// consumption applies these exact effects once on the retained overlay.
struct PreparedPristineConsensusEffects {
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
impl PreparedPristineConsensusEffects {
    fn apply(self, state_block: &mut StateBlock<'_>) -> Result<(), BlockValidationError> {
        if state_block._curr_block != self.header {
            return Err(ValidBlock::npos_effects_error(
                "pristine effects have another carrier",
            ));
        }
        state_block
            .apply_pristine_npos_consensus_effects(
                &self.effects,
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
}
