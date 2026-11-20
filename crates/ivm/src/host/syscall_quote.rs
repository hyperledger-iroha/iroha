//! Restricted, side-effect-free view for quoting a host syscall.

use super::{IVM, PointerType, VMError};

/// The authenticated image and public request metadata available before the gas debit.
///
/// This view cannot replace the image, mutate guest memory, or read raw proof state.
/// ```compile_fail
/// fn clear_log(view: ivm::host::SyscallQuote<'_>) { view.memory.clear_tracking(); }
/// ```
/// ```compile_fail
/// fn take_vm(view: ivm::host::SyscallQuote<'_>) -> &ivm::IVM { &*view }
/// ```
#[derive(Clone, Copy)]
pub struct SyscallQuote<'a> {
    vm: &'a IVM,
}

impl IVM {
    /// Borrow this VM through the same restricted interface used during syscall preparation.
    pub fn syscall_quote(&self) -> SyscallQuote<'_> {
        SyscallQuote { vm: self }
    }
}

impl<'a> SyscallQuote<'a> {
    /// Syscall policy selected by the authenticated image.
    pub fn syscall_policy(self) -> crate::SyscallPolicy {
        self.vm.syscall_policy()
    }

    /// ABI version authenticated by this program.
    pub fn abi_version(self) -> u8 {
        self.vm.abi_version()
    }

    /// Spendable gas remaining at preparation.
    pub fn remaining_gas(self) -> u64 {
        self.vm.remaining_gas()
    }

    /// Code hash authenticated by the loaded image.
    pub fn code_hash(self) -> [u8; 32] {
        self.vm.code_hash()
    }

    /// Read the immutable contract interface, including event and error descriptors.
    pub fn contract_interface(self) -> Option<&'a crate::metadata::EmbeddedContractInterfaceV1> {
        self.vm.contract_interface()
    }

    /// Check a register before its value crosses the host boundary.
    pub fn ensure_public_register(self, index: usize) -> Result<(), VMError> {
        if index
            >= match self.vm.engine() {
                crate::IvmEngine::C10 => crate::parallel::REGISTER_COUNT,
                crate::IvmEngine::Baseline => crate::registers::BASELINE_REGISTER_COUNT,
            }
        {
            return Err(VMError::RegisterOutOfBounds);
        }
        self.vm.ensure_public_register(index)
    }

    /// Read a public register in the register geometry of this program.
    pub fn public_register(self, index: usize) -> Result<u64, VMError> {
        self.ensure_public_register(index)?;
        Ok(self.vm.register(index))
    }

    /// Read the checked TLV header and extent without hashing or copying the payload.
    pub fn quote_any_tlv_at(self, address: u64) -> Result<(PointerType, usize), VMError> {
        super::quote_any_tlv_at(self.vm, address)
    }

    /// Quote a TLV having this type and valid original bounds.
    pub fn quote_tlv_payload_len_at(
        self,
        address: u64,
        expected: PointerType,
    ) -> Result<usize, VMError> {
        super::quote_tlv_payload_len_at(self.vm, address, expected)
    }

    /// Quote a typed, bounded durable state path.
    pub fn quote_state_path_payload_len_at(self, address: u64) -> Result<usize, VMError> {
        super::quote_state_path_payload_len_at(self.vm, address)
    }

    /// Quote a bounded single proof.
    pub fn quote_zk_single_at(
        self,
        address: u64,
        schedule: crate::gas::ZkGasScheduleV1,
    ) -> Result<(usize, u64), VMError> {
        super::quote_zk_single_at(self.vm, address, schedule)
    }

    /// Quote a bounded proof batch after validating its public envelope.
    pub fn quote_zk_batch_at(
        self,
        address: u64,
        schedule: crate::gas::ZkGasScheduleV1,
    ) -> Result<super::ZkBatchGasQuote, VMError> {
        let payload = self.quote_tlv_payload_len_at(address, PointerType::NoritoBytes)?;
        if u64::try_from(payload).unwrap_or(u64::MAX) > schedule.max_payload_bytes {
            return Err(VMError::NoritoInvalid);
        }
        let extent = super::TLV_ENVELOPE_OVERHEAD
            .checked_add(payload)
            .and_then(|len| u64::try_from(len).ok())
            .ok_or(VMError::NoritoInvalid)?;
        self.vm.ensure_owned_public_tlv_range(address, extent)?;
        super::quote_zk_batch_at(self.vm, address, schedule)
    }

    /// Validate the caller-owned destination for an ABI return table.
    pub fn validate_return_destination(self, address: u64, words: usize) -> Result<(), VMError> {
        crate::value_record::validate_return_destination(self.vm, address, words)
    }

    /// Quote a bounded, public schema-directed guest value.
    pub fn quote_value_record(
        self,
        schema: &iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1,
        address: u64,
        words: usize,
        gas_limit: u64,
    ) -> Result<crate::value_record::ValueRecordCaptureQuote, VMError> {
        crate::value_record::quote_value_record(self.vm, schema, address, words, gas_limit)
    }

    /// Reserve the current budget or reject an unaffordable minimum.
    pub fn reserve_available_syscall_gas_at_least(self, minimum: u64) -> Result<u64, VMError> {
        super::reserve_available_syscall_gas_at_least(self.vm, minimum)
    }

    /// Reserve the current nonempty budget.
    pub fn reserve_available_syscall_gas(self) -> Result<u64, VMError> {
        self.reserve_available_syscall_gas_at_least(1)
    }

    /// Quote a state scan after checking its public argument window.
    pub fn state_scan_minimum(self) -> Result<u64, VMError> {
        for register in 10..=15 {
            self.ensure_public_register(register)?;
        }
        crate::state_scan::prepare_minimum(self.vm)
    }

    pub(crate) fn bounded_payload_len(
        self,
        address: u64,
        expected: PointerType,
        maximum: usize,
    ) -> Result<usize, VMError> {
        super::quote_bounded_tlv_payload_len_at(self.vm, address, expected, maximum)
    }

    pub(crate) fn resolve_code_tlv_addr(self, address: u64) -> u64 {
        super::DefaultHost::resolve_code_tlv_addr(self.vm, address)
    }

    pub(crate) fn stack_top(self) -> u64 {
        self.vm.memory.stack_top()
    }
    pub(crate) fn merkle_depth(self) -> usize {
        super::DefaultHost::memory_merkle_path_depth(self.vm)
    }
    pub(crate) fn output_used_len(self) -> u64 {
        self.vm.output_used_len()
    }
    pub(crate) fn execution_summary_gas_quote(self) -> Result<u64, VMError> {
        super::DefaultHost::execution_summary_gas_quote(self.vm)
    }

    /// Apply the VM's argument privacy rule for `number` before quoting it.
    ///
    /// Every argument window is public except the commitment syscall's two private operands.
    fn check_public_inputs(self, number: u32) -> Result<(), VMError> {
        self.vm.validate_syscall_privacy(number)
    }

    /// Compute a conservative charge from the declared public inputs of a syscall.
    pub fn conservative_syscall_gas_quote(self, number: u32) -> Result<u64, VMError> {
        self.check_public_inputs(number)?;
        Ok(super::conservative_syscall_gas_quote(number, self.vm))
    }

    pub(crate) fn common_syscall_gas_quote(self, number: u32) -> Result<Option<u64>, VMError> {
        self.check_public_inputs(number)?;
        super::common_syscall_gas_quote(number, self.vm)
    }

    pub(crate) fn quote_codec_tlv_payload_len(
        self,
        register: usize,
        expected: PointerType,
        nullable: bool,
    ) -> Result<usize, VMError> {
        self.ensure_public_register(register)?;
        crate::core_host::CoreHost::quote_codec_tlv_payload_len(
            self.vm, register, expected, nullable,
        )
    }

    pub(crate) fn codec_gas_quote(self, number: u32) -> Result<Option<u64>, VMError> {
        self.check_public_inputs(number)?;
        crate::core_host::CoreHost::codec_gas_quote(number, self.vm)
    }

    pub(crate) fn schema_gas_quote(self, number: u32) -> Result<Option<u64>, VMError> {
        self.check_public_inputs(number)?;
        crate::core_host::CoreHost::schema_gas_quote(number, self.vm)
    }

    pub(crate) fn quote_state_map_key_at(self) -> Result<u64, VMError> {
        self.ensure_public_register(10)?;
        self.ensure_public_register(11)?;
        crate::core_host::CoreHost::quote_state_map_key_at(self.vm)
    }

    pub(crate) fn quote_canonical_state_map_path(self) -> Result<u64, VMError> {
        self.ensure_public_register(10)?;
        self.ensure_public_register(11)?;
        crate::core_host::CoreHost::quote_canonical_state_map_path(self.vm)
    }

    pub(crate) fn signature_verify_gas_quote(self) -> Result<u64, VMError> {
        for register in 10..=12 {
            self.ensure_public_register(register)?;
        }
        super::DefaultHost::signature_verify_gas_quote(self.vm)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_checks_register_geometry_and_privacy_before_reading_guest_values() {
        let mut vm = IVM::new(100);
        vm.set_zk_mode(true).unwrap();
        vm.set_register(12, 37);
        vm.registers.set_tag(12, true);
        let quote = vm.syscall_quote();
        assert!(matches!(
            quote.public_register(12),
            Err(VMError::PrivacyViolation)
        ));
        assert!(matches!(
            quote.public_register(usize::MAX),
            Err(VMError::RegisterOutOfBounds)
        ));
        assert_eq!(quote.public_register(13).unwrap(), 0);
        assert_eq!(quote.remaining_gas(), 100);
        assert_eq!(quote.reserve_available_syscall_gas().unwrap(), 100);
        assert!(matches!(
            quote.reserve_available_syscall_gas_at_least(101),
            Err(VMError::OutOfGas)
        ));
        assert_eq!(vm.register(12), 37);
        assert!(vm.registers.tag(12));
    }

    #[test]
    fn commitment_quote_admits_its_private_operands_and_no_other_private_argument() {
        use crate::syscalls::{SYSCALL_DEBUG_PRINT, SYSCALL_PRIVATE_NUMERIC_VALCOM};
        let mut vm = IVM::new(100);
        vm.set_zk_mode(true).unwrap();
        vm.registers.set_tag(10, true);
        vm.registers.set_tag(11, true);
        let quote = vm.syscall_quote();
        assert!(
            quote
                .common_syscall_gas_quote(SYSCALL_PRIVATE_NUMERIC_VALCOM)
                .is_ok()
        );
        assert!(matches!(
            quote.common_syscall_gas_quote(SYSCALL_DEBUG_PRINT),
            Err(VMError::PrivacyViolation)
        ));
        vm.registers.set_tag(11, false);
        assert!(matches!(
            vm.syscall_quote()
                .common_syscall_gas_quote(SYSCALL_PRIVATE_NUMERIC_VALCOM),
            Err(VMError::PrivacyViolation)
        ));
    }
}
