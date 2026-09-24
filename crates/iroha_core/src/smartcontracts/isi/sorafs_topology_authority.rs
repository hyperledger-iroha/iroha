//! Explicitly closed role-16 topology authority until native storage and finality are connected.

use super::{Execute, INITIAL_NATIVE_INSTRUCTION_CLOSED_REASON};
use crate::state::StateTransaction;
use iroha_data_model::{
    account::AccountId,
    isi::{
        error::{InstructionExecutionError, InvalidParameterError},
        sorafs::MutateSorafsTopologyAuthority,
    },
};

impl Execute for MutateSorafsTopologyAuthority {
    fn execute(
        self,
        _authority: &AccountId,
        _tx: &mut StateTransaction<'_, '_>,
    ) -> Result<(), InstructionExecutionError> {
        Err(InstructionExecutionError::InvalidParameter(
            InvalidParameterError::SmartContract(INITIAL_NATIVE_INSTRUCTION_CLOSED_REASON.into()),
        ))
    }
}

#[cfg(test)]
mod tests;
