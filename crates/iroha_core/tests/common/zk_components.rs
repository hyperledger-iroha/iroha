//! Direct ZK handler fixtures, separate from signed transaction/root admission.
use iroha_core::{smartcontracts::Execute, state::StateTransaction};
use iroha_data_model::{
    ValidationFail,
    prelude::{AccountId, InstructionBox},
};

/// Execute one component handler with its real permissions and validation.
/// This helper provides no transaction, genesis or block-publication authority.
pub(crate) fn execute_isi_component(
    state: &mut StateTransaction<'_, '_>,
    authority: &AccountId,
    instruction: InstructionBox,
) -> Result<(), ValidationFail> {
    instruction
        .execute(authority, state)
        .map_err(ValidationFail::InstructionFailed)
}
