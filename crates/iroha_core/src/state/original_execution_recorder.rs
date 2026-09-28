//! A State execution keeps its original recorder from pristine construction to capture.

use super::*;

impl StateBlock<'_> {
    /// Bind once, before any pristine controls or block-start effects execute.
    pub(super) fn bind_original_execution_recorder(&mut self) -> Result<(), String> {
        if self.original_execution_recorder.is_some() || self.start_of_block_effects_applied {
            return Err(
                "State execution recorder can only be bound once before block effects".into(),
            );
        }
        let identity = crate::exec_witness::current_exec_witness_capture_identity()
            .ok_or("State execution requires an owned active recorder")?;
        identity.require_current()?;
        self.original_execution_recorder = Some(identity);
        Ok(())
    }

    /// Refuse a missing, foreign, suppressed, retired or reset original recorder.
    pub(crate) fn require_original_execution_recorder(&self) -> Result<(), String> {
        self.original_execution_recorder
            .as_ref()
            .ok_or("State execution has no original recorder")?
            .require_current()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{exec_witness, query::store::LiveQueryStore};
    use iroha_data_model::block::builder::BlockBuilder;

    fn state() -> State {
        State::new(
            World::new(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        )
    }

    fn carrier() -> SignedBlock {
        BlockBuilder::new(BlockHeader::new(NonZeroU64::MIN, None, None, 1000, 0))
            .build_with_signature(
                0,
                iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key(),
            )
    }

    #[test]
    fn recorded_constructor_binds_before_pristine_effects_and_refuses_replacement() {
        let state = state();
        let carrier = carrier();
        let outcome = state.block_with_recorded_pristine_carrier_stage(
            &carrier,
            |block| {
                block.require_original_execution_recorder().unwrap();
                assert!(!block.start_of_block_effects_applied);
                assert!(block.bind_original_execution_recorder().is_err());
                let suppressed = exec_witness::suppress_recording_for_current_thread();
                assert!(block.require_original_execution_recorder().is_err());
                drop(suppressed);
                block.require_original_execution_recorder().unwrap();
                exec_witness::start_block();
                assert!(block.require_original_execution_recorder().is_err());
                assert!(block.bind_original_execution_recorder().is_err());
                Err::<(), String>("stop before block effects".into())
            },
            |error| error,
        );
        assert!(
            matches!(outcome, Err(StateBlockStartError::Stage(error)) if error == "stop before block effects")
        );
        assert!(exec_witness::current_exec_witness_capture_identity().is_none());
        assert_eq!(state.committed_height(), 0);
        assert_eq!(state.kura.blocks_count(), 0);
    }

    #[test]
    fn a_new_recorder_cannot_retroactively_own_an_existing_block() {
        let state = state();
        let carrier = carrier();
        let outcome = state.block_with_pristine_stage(carrier.header(), |block| {
            assert!(block.require_original_execution_recorder().is_err());
            let _guard = exec_witness::begin_exec_witness_capture().unwrap();
            assert!(block.require_original_execution_recorder().is_err());
            Err::<(), String>("stop before block effects".into())
        });
        assert!(
            matches!(outcome, Err(StateBlockStartError::Stage(error)) if error == "stop before block effects")
        );
        assert!(exec_witness::current_exec_witness_capture_identity().is_none());
    }
}
