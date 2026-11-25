//! Contract-bound FI transfer. The native executor derives the approved destination.

use super::*;
use iroha_data_model::isi::asset_fi::TransferToLinkedFiV1;
use ivm_abi::error::VmTrap;

impl<QS: Default + QueryStateAccess> CoreHostImpl<'_, '_, QS> {
    pub(super) fn transfer_to_linked_fi(
        &mut self,
        vm: &dyn ivm::host::SyscallContext,
    ) -> Result<u64, ivm::VMError> {
        let context = self
            .current_contract_runtime_context
            .as_ref()
            .ok_or(ivm::VMError::Trap(VmTrap::PermissionDenied))?;
        if context.contract_subject != context.contract_address.subject_id() {
            return Err(ivm::VMError::Trap(VmTrap::PermissionDenied));
        }
        for register in 10..=14 {
            vm.quote().ensure_public_register(register)?;
        }
        let asset_definition_id =
            Self::decode_tlv_typed(vm, vm.public_register(10)?, PointerType::AssetDefinitionId)?;
        let account_id =
            Self::decode_tlv_typed(vm, vm.public_register(11)?, PointerType::AccountId)?;
        let amount = Self::decode_quantity(vm, vm.public_register(12)?)?;
        // Kotodama lowers these two int arguments through checked unsigned-u64
        // conversion. The host retains the entire revision without truncation.
        let expected_policy_revision = vm.public_register(13)?;
        let expected_link_revision = vm.public_register(14)?;
        let instruction = TransferToLinkedFiV1 {
            asset_definition_id,
            account_id,
            amount,
            expected_link_revision,
            expected_policy_revision,
        };
        // The queued effect retains the actual contract frame. An outer human
        // signer is not an owner-approved controller. The native kernel checks
        // current link consent, policy revisions, current excess, both limits
        // and gross funding at application; a preceding quote grants no bypass.
        self.queue_instruction_after_preflight(vm, instruction.into())
    }
}
