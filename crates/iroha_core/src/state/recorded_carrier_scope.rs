//! Original recorder ownership acquired after State writers and before pristine work.
//!
//! These narrow forwards preserve the existing normal/replacement start hooks.
//! The guard moves with the applying caller; no effect phase may reset its capture.

use super::{State, StateBlock, StateBlockStartError};
use crate::exec_witness::{self as witness, ExecWitnessGuard};
use iroha_data_model::block::SignedBlock;

impl State {
    /// Keep one recorder over pristine carrier controls and all normal start hooks.
    pub(crate) fn block_with_recorded_pristine_carrier_stage<'state, E: std::fmt::Debug>(
        &'state self,
        carrier: &SignedBlock,
        stage: impl FnOnce(&mut StateBlock<'state>) -> Result<(), E>,
        recording_error: impl Fn(String) -> E,
    ) -> Result<(Box<StateBlock<'state>>, ExecWitnessGuard), StateBlockStartError<E>> {
        witness::ensure_exec_witness_capture_available()
            .map_err(|error| StateBlockStartError::Stage(recording_error(error)))?;
        self.block_with_owned_start_stages_with_carrier(
            carrier.header(),
            Some(carrier),
            |block| {
                let guard = witness::begin_exec_witness_capture().map_err(&recording_error)?;
                block
                    .bind_original_execution_recorder()
                    .map_err(&recording_error)?;
                stage(block)?;
                Ok(guard)
            },
            |_, guard| Ok(guard),
        )
    }

    /// Record the existing narrow replacement initialization without adding the
    /// normal lifecycle hooks or acquiring the recorder before State writers.
    pub(crate) fn block_and_revert_with_recorded_pristine_carrier_stage<
        'state,
        E: std::fmt::Debug,
    >(
        &'state self,
        carrier: &SignedBlock,
        stage: impl FnOnce(&mut StateBlock<'_>) -> Result<(), E>,
        recording_error: impl Fn(String) -> E,
    ) -> Result<(Box<StateBlock<'state>>, ExecWitnessGuard), StateBlockStartError<E>> {
        witness::ensure_exec_witness_capture_available()
            .map_err(|error| StateBlockStartError::Stage(recording_error(error)))?;
        let mut recording = None;
        let block = self.block_and_revert_with_pristine_carrier_stage(carrier, |block| {
            let guard = witness::begin_exec_witness_capture().map_err(&recording_error)?;
            block
                .bind_original_execution_recorder()
                .map_err(&recording_error)?;
            stage(block)?;
            recording = Some(guard);
            Ok(())
        })?;
        Ok((
            Box::new(block),
            recording.expect("successful pristine stage retains its original recorder"),
        ))
    }
}
