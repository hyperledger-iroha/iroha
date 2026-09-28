//! Interpreter transitions for the sole authenticated V1 table-call convention.

use super::IVM;
use crate::{IVMHost, PointerType, VMError};
use ivm_abi::call::{CallWordV1, EmbeddedCallableV1};

impl IVM {
    /// Select a public entrypoint from the loaded, authenticated contract interface.
    ///
    /// Missing selectors and standalone opcode images have no public call authority.
    pub fn select_entrypoint(&mut self, name: &str) -> Result<(), VMError> {
        let entrypoint = self
            .contract_interface
            .as_ref()
            .and_then(|interface| {
                interface
                    .entrypoints
                    .iter()
                    .find(|entry| entry.name == name)
            })
            .ok_or(VMError::PermissionDenied)?;
        let pc = self
            .program_prefix_len
            .checked_add(entrypoint.entry_pc)
            .ok_or(VMError::DecodeError)?;
        self.set_program_counter(pc)
    }

    /// Number of initialized words returned by the successfully completed root invocation.
    ///
    /// This reads interpreter-owned completion state, never guest descriptor registers.
    pub fn call_result_word_count(&self) -> Result<usize, VMError> {
        self.memory.call_frames.completed_word_count()
    }

    /// Read one public word from the successfully completed root result table.
    ///
    /// An unfinished invocation, out-of-range index, or private slot is rejected.
    pub fn public_call_result_word(&self, index: usize) -> Result<u64, VMError> {
        let address = self.memory.call_frames.completed_result_word(index)?;
        self.ensure_public_memory(address, 8)?;
        self.memory.load_u64(address)
    }

    fn callable_index(&self, pc: u64) -> Result<usize, VMError> {
        let relative = pc
            .checked_sub(self.program_prefix_len)
            .ok_or(VMError::DecodeError)?;
        self.contract_interface
            .as_ref()
            .ok_or(VMError::DecodeError)?
            .callables
            .binary_search_by_key(&relative, |entry| entry.entry_pc)
            .map_err(|_| VMError::DecodeError)
    }

    pub(super) fn begin_root_call(&mut self, host: &mut dyn IVMHost) -> Result<(), VMError> {
        if !self.strict_return_integrity {
            return Ok(());
        }
        let index = self.callable_index(self.pc)?;
        let interface = self
            .contract_interface
            .clone()
            .ok_or(VMError::DecodeError)?;
        let callable = &interface.callables[index];
        let entrypoint = interface
            .entrypoints
            .iter()
            .find(|entry| entry.entry_pc == callable.entry_pc);
        if entrypoint.is_none() && !self.allow_koto_test_syscalls {
            return Err(VMError::PermissionDenied);
        }
        if let Some(schema) = entrypoint.and_then(|entry| entry.argument_schema.as_ref()) {
            if let Some(prepared) = host.prepared_entrypoint_arguments() {
                if prepared.word_count() != callable.argument_words.len()
                    || !prepared.is_bound_to(schema, prepared.canonical_bytes())?
                {
                    return Err(VMError::DecodeError);
                }
                prepared.install_call_arguments(self, callable.result_words.len())?;
            } else {
                crate::argument_record::prepare_default_call_arguments(
                    host,
                    self,
                    schema,
                    callable.result_words.len(),
                )?;
            }
        } else {
            if !callable.argument_words.is_empty() {
                return Err(VMError::DecodeError);
            }
            crate::argument_record::install_empty_call_arguments(
                self,
                callable.result_words.len(),
            )?;
        }
        let prepared_frame = self.memory.call_frames.prepare_root(
            self.memory.stack_top(),
            callable,
            crate::call_frame::CallTables {
                argument_base: self.registers.get(10),
                argument_words: self.registers.get(11),
                result_base: self.registers.get(12),
                result_words: self.registers.get(13),
            },
            self.memory.stack_top(),
        )?;
        self.registers.set(31, self.memory.stack_top());
        self.registers.set_tag(31, false);
        self.registers.set(1, self.memory.code_len());
        self.registers.set_tag(1, false);
        self.contract_outer_return_pc = Some(self.memory.code_len());
        self.validate_call_tables(callable, true)?;
        self.memory.call_frames.enter_prepared_root(prepared_frame);
        Ok(())
    }

    pub(super) fn begin_child_call(&mut self, target: u64) -> Result<(), VMError> {
        if !self.strict_return_integrity {
            return Ok(());
        }
        let index = self.callable_index(target)?;
        let interface = self
            .contract_interface
            .clone()
            .ok_or(VMError::DecodeError)?;
        let callable = &interface.callables[index];
        // Fund the protected return slot before validating tables debits gas
        // or entering the child frame mutates its ownership bitmap.
        self.preflight_contract_return()?;
        let prepared_frame = self.memory.call_frames.prepare_child(
            self.registers.get(31),
            callable,
            crate::call_frame::CallTables {
                argument_base: self.registers.get(10),
                argument_words: self.registers.get(11),
                result_base: self.registers.get(12),
                result_words: self.registers.get(13),
            },
            self.memory.stack_top(),
        )?;
        self.validate_call_tables(callable, false)?;
        self.memory.call_frames.enter_prepared_child(prepared_frame);
        Ok(())
    }

    fn validate_call_tables(
        &mut self,
        callable: &EmbeddedCallableV1,
        root: bool,
    ) -> Result<(), VMError> {
        self.zk_require_public_trap_operands(&[10, 11, 12, 13, 31])?;
        let argument = self.registers.get(10);
        let results = self.registers.get(12);
        if self.registers.get(11) != callable.argument_words.len() as u64
            || self.registers.get(13) != callable.result_words.len() as u64
            || !argument.is_multiple_of(8)
            || !results.is_multiple_of(8)
            || (callable.argument_words.is_empty() && argument != 0)
        {
            return Err(VMError::AssertionFailed);
        }
        if root {
            if !callable.argument_words.is_empty() {
                self.ensure_owned_heap_range(argument, callable.argument_words.len() as u64 * 8)?;
            }
            self.ensure_owned_heap_range(results, callable.result_words.len() as u64 * 8)?;
        }
        // Reserve deterministic bitmap work before the frame owner allocates it.
        self.debit_gas(crate::call_gas::frame(
            callable.frame_bytes,
            callable.result_words.len(),
        )?)?;
        for (index, role) in callable.argument_words.iter().copied().enumerate() {
            self.debit_gas(crate::call_gas::WORD)?;
            let address = argument
                .checked_add(index as u64 * 8)
                .ok_or(VMError::MemoryOutOfBounds)?;
            let word = self.memory.load_u64(address)?;
            self.validate_call_word(address, word, role)?;
        }
        Ok(())
    }

    pub(super) fn finish_call(&mut self) -> Result<(), VMError> {
        self.zk_require_public_trap_operands(&[10, 11, 31])?;
        let entry_pc = self.memory.call_frames.entry_pc()?;
        let interface = self
            .contract_interface
            .clone()
            .ok_or(VMError::DecodeError)?;
        let index = interface
            .callables
            .binary_search_by_key(&entry_pc, |entry| entry.entry_pc)
            .map_err(|_| VMError::DecodeError)?;
        let callable = &interface.callables[index];
        if self.registers.get(11) != callable.result_words.len() as u64 {
            return Err(VMError::AssertionFailed);
        }
        for (index, role) in callable.result_words.iter().copied().enumerate() {
            self.debit_gas(crate::call_gas::WORD)?;
            let (address, word) = self.memory.active_call_result(index)?;
            self.validate_call_word(address, word, role)?;
        }
        self.memory.call_frames.finish(
            self.registers.get(31),
            self.registers.get(10),
            self.registers.get(11),
        )
    }

    fn validate_call_word(
        &mut self,
        address: u64,
        word: u64,
        role: CallWordV1,
    ) -> Result<(), VMError> {
        if self.memory_load_privacy_tag(address, 8)? != role.is_private() {
            return Err(VMError::PrivacyViolation);
        }
        match role {
            CallWordV1::Unit if word == 0 => Ok(()),
            CallWordV1::Bool if word <= 1 => Ok(()),
            CallWordV1::Error if u32::try_from(word).is_ok() => Ok(()),
            CallWordV1::Pointer(expected) => self.validate_call_pointer(word, Some(expected)),
            CallWordV1::StateCursor => self.validate_call_cursor_pointer(word),
            CallWordV1::StateRoot => self.validate_call_pointer(word, None),
            CallWordV1::SecretNumeric(expected) => {
                self.validate_secret_call_pointer(word, expected)
            }
            CallWordV1::Sum => {
                self.debit_gas(crate::call_gas::WORD)?;
                self.ensure_owned_heap_range(word, 8)?;
                if self.load_u64(word)? > 1 {
                    return Err(VMError::DecodeError);
                }
                Ok(())
            }
            CallWordV1::List => {
                self.debit_gas(2 * crate::call_gas::WORD)?;
                self.ensure_owned_heap_range(word, 16)?;
                let length = self.load_u64(word)?;
                let capacity = self.load_u64(word + 8)?;
                if !(1..=64).contains(&capacity) || length > capacity {
                    return Err(VMError::DecodeError);
                }
                Ok(())
            }
            _ => Err(VMError::DecodeError),
        }
    }

    fn validate_call_pointer(&mut self, word: u64, expected: Option<u16>) -> Result<(), VMError> {
        self.validate_call_pointer_role(word, expected, false)
    }

    fn validate_call_cursor_pointer(&mut self, word: u64) -> Result<(), VMError> {
        self.validate_call_pointer_role(word, Some(PointerType::NoritoBytes as u16), true)
    }

    fn validate_call_pointer_role(
        &mut self,
        word: u64,
        expected: Option<u16>,
        cursor: bool,
    ) -> Result<(), VMError> {
        let (payload, _) = self.inspect_owned_public_tlv_header(word)?;
        self.debit_gas(crate::call_gas::pointer(payload)?)?;
        let tlv = self.validate_tlv(word)?;
        if expected.is_some_and(|expected| tlv.type_id as u16 != expected)
            || (expected.is_none()
                && !matches!(tlv.type_id, PointerType::Name | PointerType::NoritoBytes))
        {
            return Err(VMError::NoritoInvalid);
        }
        if cursor {
            // TODO: Include the declared key kind in CallWordV1 so helper calls
            // can also reject a structurally valid cursor for the wrong map-key type.
            iroha_data_model::smart_contract::state_cursor::StateCursorV1::decode_frame(
                tlv.payload,
            )
            .map_err(|_| VMError::NoritoInvalid)?;
        }
        if matches!(
            tlv.type_id,
            PointerType::Int | PointerType::Decimal | PointerType::Quantity
        ) {
            // Numeric word roles bind the entire canonical numeric frame, not merely its type tag.
            let envelope = self.memory.load_region(word, 39 + payload)?;
            crate::private_input::validate_private_numeric_envelope(envelope)?;
        }
        Ok(())
    }

    fn validate_secret_call_pointer(&mut self, word: u64, expected: u16) -> Result<(), VMError> {
        use iroha_primitives::numeric_abi::{
            MAX_DECIMAL_FRAME_BYTES_V1, MAX_INT_FRAME_BYTES_V1, MAX_QUANTITY_FRAME_BYTES_V1,
        };
        let maximum = match PointerType::from_u16(expected) {
            Some(PointerType::Int) => MAX_INT_FRAME_BYTES_V1,
            Some(PointerType::Decimal) => MAX_DECIMAL_FRAME_BYTES_V1,
            Some(PointerType::Quantity) => MAX_QUANTITY_FRAME_BYTES_V1,
            _ => return Err(VMError::DecodeError),
        } as u64
            + 39;
        // The public declared numeric type determines the charge, never secret envelope length.
        self.debit_gas(crate::call_gas::pointer(maximum - 39)?)?;
        self.ensure_owned_tlv_range(word, 7)?;
        let header = self.memory.load_region(word, 7)?;
        let total = u64::from(u32::from_be_bytes([
            header[3], header[4], header[5], header[6],
        ])) + 39;
        if total > maximum {
            return Err(VMError::NoritoInvalid);
        }
        self.ensure_owned_tlv_range(word, total)?;
        if self
            .private_memory_bytes
            .intersection_len(Self::memory_privacy_range(word, total)?)
            != total
        {
            return Err(VMError::PrivacyViolation);
        }
        let envelope = self.memory.load_region(word, total)?;
        if crate::private_input::validate_private_numeric_envelope(envelope)?.pointer_type() as u16
            != expected
        {
            return Err(VMError::NoritoInvalid);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{KotodamaCompiler, ProgramMetadata, host::DefaultHost};

    fn boolean_root(gas: u64) -> IVM {
        let (bytes, _) = KotodamaCompiler::new()
            .compile_source_with_manifest("seiyaku Tables { view fn main() -> bool { true } }")
            .unwrap();
        let parsed = ProgramMetadata::parse(&bytes).unwrap();
        let entry = parsed
            .contract_interface
            .as_ref()
            .unwrap()
            .entrypoints
            .iter()
            .find(|entry| entry.name == "main")
            .unwrap();
        let pc = parsed.prefix_len() as u64 + entry.entry_pc;
        let mut vm = IVM::new(gas);
        vm.load_program(&bytes).unwrap();
        vm.set_program_counter(pc).unwrap();
        vm
    }

    #[test]
    fn completed_results_use_owned_state_and_reset_discards_authority() {
        let mut vm = boolean_root(100_000);
        assert!(vm.call_result_word_count().is_err());
        vm.run().unwrap();
        assert_eq!(vm.call_result_word_count(), Ok(1));
        assert_eq!(vm.public_call_result_word(0), Ok(1));
        assert!(vm.public_call_result_word(1).is_err());
        vm.set_register(10, u64::MAX);
        vm.set_register(11, 8192);
        assert_eq!(vm.call_result_word_count(), Ok(1));
        assert_eq!(vm.public_call_result_word(0), Ok(1));
        vm.reset();
        assert!(vm.call_result_word_count().is_err());
        assert!(vm.public_call_result_word(0).is_err());
    }

    #[test]
    fn public_selection_and_transaction_dispatch_require_exact_selectors() {
        let code = KotodamaCompiler::new().compile_source(
            "seiyaku Selectors { fn helper() -> bool { true } view fn main() -> bool { helper() } }"
        ).unwrap();
        let mut vm = IVM::new(100_000);
        vm.load_program(&code).unwrap();
        assert_eq!(
            vm.select_entrypoint("helper"),
            Err(VMError::PermissionDenied)
        );
        assert_eq!(
            vm.select_entrypoint("missing"),
            Err(VMError::PermissionDenied)
        );
        vm.select_entrypoint("main").unwrap();
        vm.run().unwrap();
        assert_eq!(vm.public_call_result_word(0), Ok(1));
        for (entrypoint, success) in [
            (None, false),
            (Some("helper"), false),
            (Some("missing"), false),
            (Some("main"), true),
        ] {
            let mut invocation_vm = IVM::new(100_000);
            invocation_vm.load_program(&code).unwrap();
            let selected =
                entrypoint.is_some_and(|name| invocation_vm.select_entrypoint(name).is_ok());
            let result = selected && invocation_vm.run().is_ok();
            assert_eq!(result, success, "selector {entrypoint:?}");
        }
    }

    #[test]
    fn return_checks_initialization_canonical_role_base_and_count() {
        let mut vm = boolean_root(100_000);
        vm.begin_root_call(&mut DefaultHost::default()).unwrap();
        let result = vm.registers.get(12);
        vm.registers.set(10, result);
        vm.registers.set(11, 1);
        assert_eq!(vm.finish_call(), Err(VMError::AssertionFailed));
        vm.store_u64(result, 2).unwrap();
        assert_eq!(vm.finish_call(), Err(VMError::DecodeError));
        vm.store_u64(result, 1).unwrap();
        vm.registers.set(10, result + 8);
        assert_eq!(vm.finish_call(), Err(VMError::AssertionFailed));
        vm.registers.set(10, result);
        vm.registers.set(11, 0);
        assert_eq!(vm.finish_call(), Err(VMError::AssertionFailed));
        vm.registers.set(11, 1);
        vm.finish_call().unwrap();
        assert_eq!(vm.public_call_result_word(0), Ok(1));
    }

    #[test]
    fn root_preparation_and_result_validation_debit_before_work() {
        let mut vm = boolean_root(7);
        assert_eq!(vm.run(), Err(VMError::OutOfGas));
        assert_eq!(vm.memory.heap_allocated_len(), 0);
        assert!(vm.call_result_word_count().is_err());

        let mut vm = boolean_root(100_000);
        vm.begin_root_call(&mut DefaultHost::default()).unwrap();
        let result = vm.registers.get(12);
        vm.store_u64(result, 1).unwrap();
        vm.registers.set(10, result);
        vm.registers.set(11, 1);
        vm.set_gas_limit(7);
        assert_eq!(vm.finish_call(), Err(VMError::OutOfGas));
        assert!(vm.call_result_word_count().is_err());
    }

    #[test]
    fn pointer_validation_rejects_wrong_roles_and_uses_public_work_gas() {
        let mut vm = IVM::new(100_000);
        let pointer = vm
            .alloc_host_tlv(
                &crate::numeric_tlv::encode_envelope(PointerType::Blob, b"value").unwrap(),
            )
            .unwrap();
        let before = vm.remaining_gas();
        vm.validate_call_pointer(pointer, Some(PointerType::Blob as u16))
            .unwrap();
        assert_eq!(before - vm.remaining_gas(), 16 + 39 + 5);
        assert_eq!(
            vm.validate_call_pointer(pointer, Some(PointerType::Int as u16)),
            Err(VMError::NoritoInvalid)
        );
        vm.set_gas_limit(16 + 39 + 4);
        assert_eq!(
            vm.validate_call_pointer(pointer, Some(PointerType::Blob as u16)),
            Err(VMError::OutOfGas)
        );
        let pointer = vm
            .alloc_host_tlv(&crate::numeric_tlv::encode_envelope(PointerType::Int, b"bad").unwrap())
            .unwrap();
        vm.set_gas_limit(100_000);
        assert_eq!(
            vm.validate_call_pointer(pointer, Some(PointerType::Int as u16)),
            Err(VMError::NoritoInvalid)
        );
    }

    #[test]
    fn state_cursor_call_word_requires_a_canonical_cursor_frame() {
        use iroha_data_model::smart_contract::{
            entrypoint::EntrypointValueKindV1, state_cursor::StateCursorV1,
        };

        let mut vm = IVM::new(100_000);
        let slot = vm.alloc_heap(8).unwrap();
        let invalid_payload = b"not a cursor";
        let invalid_pointer = vm
            .alloc_host_tlv(
                &crate::numeric_tlv::encode_envelope(PointerType::NoritoBytes, invalid_payload)
                    .unwrap(),
            )
            .unwrap();
        vm.store_u64(slot, invalid_pointer).unwrap();
        let before = vm.remaining_gas();
        assert_eq!(
            vm.validate_call_word(slot, invalid_pointer, CallWordV1::StateCursor),
            Err(VMError::NoritoInvalid)
        );
        assert_eq!(
            before - vm.remaining_gas(),
            crate::call_gas::pointer(invalid_payload.len() as u64).unwrap()
        );

        let cursor = StateCursorV1 {
            instance: "contract::instance".into(),
            map: "balances".parse().unwrap(),
            schema_hash: [7; 32],
            key_type: EntrypointValueKindV1::Int,
            last_key: "balances/00".parse().unwrap(),
        };
        let valid_payload = cursor.encode_frame().unwrap();
        let valid_pointer = vm
            .alloc_host_tlv(
                &crate::numeric_tlv::encode_envelope(PointerType::NoritoBytes, &valid_payload)
                    .unwrap(),
            )
            .unwrap();
        vm.store_u64(slot, valid_pointer).unwrap();
        let before = vm.remaining_gas();
        assert_eq!(
            vm.validate_call_word(slot, valid_pointer, CallWordV1::StateCursor),
            Ok(())
        );
        assert_eq!(
            before - vm.remaining_gas(),
            crate::call_gas::pointer(valid_payload.len() as u64).unwrap()
        );
    }

    #[test]
    fn private_numeric_call_checks_use_fixed_type_cost_and_preserve_privacy() {
        use iroha_primitives::{bigint::BigInt, numeric_abi::MAX_INT_FRAME_BYTES_V1};
        let mut vm = IVM::new(100_000);
        vm.set_zk_mode(true);
        let slot = vm.memory.stack_top() - 8;
        let mut costs = Vec::new();
        for value in [BigInt::from(0), BigInt::pow10(120).unwrap()] {
            let record = crate::private_input::int_record(value).unwrap();
            let envelope =
                crate::numeric_tlv::encode_envelope(PointerType::Int, &record.payload).unwrap();
            let pointer = vm.alloc_host_private_tlv(&envelope).unwrap();
            vm.memory.store_u64(slot, pointer).unwrap();
            vm.record_memory_store_privacy(slot, 8, true);
            let before = vm.remaining_gas();
            vm.validate_call_word(
                slot,
                pointer,
                CallWordV1::SecretNumeric(PointerType::Int as u16),
            )
            .unwrap();
            costs.push(before - vm.remaining_gas());
            assert_eq!(
                vm.validate_call_word(slot, pointer, CallWordV1::Pointer(PointerType::Int as u16)),
                Err(VMError::PrivacyViolation)
            );
            assert_eq!(
                vm.validate_call_pointer(pointer, Some(PointerType::Int as u16)),
                Err(VMError::PrivacyViolation)
            );
        }
        assert_eq!(costs, vec![16 + 39 + MAX_INT_FRAME_BYTES_V1 as u64; 2]);
        vm.store_u8(slot, 0).unwrap();
        assert_eq!(
            vm.validate_call_word(slot, 0, CallWordV1::Unit),
            Err(VMError::PrivacyViolation)
        );
    }
}
