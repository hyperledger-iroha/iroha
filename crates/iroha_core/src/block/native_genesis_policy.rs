// The sole native genesis execution must reproduce every signed deterministic policy.
impl ValidBlock {
    fn validate_native_genesis_policy(
        block: &SignedBlock,
        state: &StateBlock<'_>,
    ) -> Result<(), BlockValidationError> {
        if !block.header().is_genesis() {
            return Ok(());
        }
        let metadata =
            iroha_data_model::sumeragi_finality::signed_genesis_consensus_metadata(block)
                .map_err(Self::execution_context_error)?;
        let actual_execution = crate::sumeragi::staged_genesis_execution_policy_hash(state)
            .map_err(|error| Self::execution_context_error(error.to_string()))?;
        let actual_nexus = crate::sumeragi::staged_genesis_nexus_amx_context_hash(state);
        Self::require_native_genesis_policy(
            Hash::prehashed(metadata.sumeragi_v2.execution_policy_hash),
            actual_execution,
            Hash::prehashed(metadata.sumeragi_v2.nexus_amx_context_hash),
            actual_nexus,
        )
    }

    fn require_native_genesis_policy(
        expected_execution: Hash,
        actual_execution: Hash,
        expected_nexus: Hash,
        actual_nexus: Hash,
    ) -> Result<(), BlockValidationError> {
        if expected_execution != actual_execution || expected_nexus != actual_nexus {
            return Err(BlockValidationError::GenesisPolicyMismatch {
                expected_execution,
                actual_execution,
                expected_nexus,
                actual_nexus,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod native_genesis_policy_tests {
    use super::*;

    #[test]
    fn signed_genesis_requires_both_exact_policy_commitments() {
        let execution = Hash::new(b"original execution policy");
        let nexus = Hash::new(b"original Nexus policy");
        assert!(
            ValidBlock::require_native_genesis_policy(execution, execution, nexus, nexus).is_ok()
        );
        let foreign = Hash::new(b"substituted policy");
        for (actual_execution, actual_nexus) in
            [(foreign, nexus), (execution, foreign), (foreign, foreign)]
        {
            let error = ValidBlock::require_native_genesis_policy(
                execution,
                actual_execution,
                nexus,
                actual_nexus,
            )
            .unwrap_err();
            assert!(
                matches!(error, BlockValidationError::GenesisPolicyMismatch {
                expected_execution, actual_execution: retained_execution,
                expected_nexus, actual_nexus: retained_nexus,
            } if expected_execution == execution && expected_nexus == nexus
                && retained_execution == actual_execution && retained_nexus == actual_nexus)
            );
        }
    }
}
