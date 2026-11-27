//! Private descriptor invocation capabilities retained by the ordinary ordered host queue.
//!
//! A guest addresses a declared dependency by its index, never by a supplied World field,
//! namespace or definition id. The original transaction remains borrowed for the complete run.

use super::*;
use crate::state::generic_carrier_storage::{
    CarrierStorageMode, RetainedCarrierBytes, RetainedCarrierKey,
};
use iroha_allocation::{AllocationBudget, AllocationReservation};
use iroha_data_model::runtime::{
    carrier::{CarrierStorageKeyV1, MAX_CARRIER_KEY_BYTES, validated_carrier_key_v1},
    definition::{
        DefinitionDescriptorV1, DefinitionId, RootSelectionV1, SELECTION_ID_DOMAIN, SelectionId,
    },
};

const MAX_DEPENDENCIES: usize = 16;

/// A preflighted typed group; only the consuming carrier artifact boundary constructs it.
pub(crate) struct Admission {
    items: usize,
    bytes: u64,
}
impl Admission {
    pub(crate) fn items(&self) -> usize {
        self.items
    }
    pub(crate) fn bytes(&self) -> u64 {
        self.bytes
    }
}

impl HostExecutionArtifacts {
    pub(super) fn apply_carrier(
        mut self,
        state: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
    ) -> Result<ExecutedHostInstructions, ValidationFail> {
        let reject = || {
            ValidationFail::NotPermitted(
                "carrier artifact group contains a foreign effect or authority".into(),
            )
        };
        if self.carrier_queue_backing.is_none()
            || self.x509_queue_backing.is_some()
            || self.entrypoint_authorization.is_some()
            || self.confidential_gas_delta != 0
            || !self.completed_axt.is_empty()
            || !self.durable_state_overlay.is_empty()
            || !self.durable_state_authorizations.is_empty()
        {
            return Err(reject());
        }
        self.carrier_queue_backing
            .as_ref()
            .expect("checked original carrier queue")
            .invocation
            .validate(state)
            .map_err(|error| {
                state.vm_error_to_validation_fail(error, |error| {
                    ValidationFail::NotPermitted(error.to_string())
                })
            })?;
        let mut bytes = 0_u64;
        for effect in &mut self.queued {
            if &effect.authority != authority
                || effect.contract_runtime_context.is_some()
                || effect.entrypoint_authorization.is_some()
            {
                return Err(reject());
            }
            let QueuedEffectPayload::CarrierWrite(write) = &mut effect.payload else {
                return Err(reject());
            };
            write.validate(state).map_err(|error| {
                state.vm_error_to_validation_fail(error, |error| {
                    ValidationFail::NotPermitted(error.to_string())
                })
            })?;
            write.prepared = Some(
                crate::runtime::generic_carriers::storage::prepare(
                    &write.key,
                    write.value.as_deref(),
                    &state.execution_budget(),
                )
                .map_err(|original| {
                    state.vm_error_to_validation_fail(original.into_vm_error(), |error| {
                        ValidationFail::NotPermitted(error.to_string())
                    })
                })?,
            );
            let len = 64_usize
                .checked_add(write.key.key.len())
                .and_then(|len| len.checked_add(write.value.as_ref().map_or(0, Vec::len)))
                .ok_or_else(reject)?;
            bytes = bytes
                .checked_add(u64::try_from(len).map_err(|_| reject())?)
                .ok_or_else(reject)?;
        }
        state.admit_carrier_execution_effects(Admission {
            items: self.queued.len(),
            bytes,
        })?;
        // Every fallible admission precedes the first write. Consume the original ordered queue
        // directly into the existing World MV owner; no secondary overlay or publication path.
        for effect in self.queued {
            if let QueuedEffectPayload::CarrierWrite(write) = effect.payload {
                write.apply(state)?;
            }
        }
        Ok(ExecutedHostInstructions::default())
    }
}

fn denied() -> ivm::VMError {
    ivm::VMError::Trap(VmTrap::PermissionDenied)
}

fn codec_error(original: norito::core::DecodeAttemptError) -> ivm::VMError {
    match crate::execution_attempt::canonical_decode_attempt_error(original, |_| {
        ivm::VMError::Trap(VmTrap::NoritoInvalid)
    }) {
        crate::execution_attempt::ExecutionAttemptError::Rejected(error) => error,
        crate::execution_attempt::ExecutionAttemptError::Deferred(original) => {
            original.into_vm_error()
        }
    }
}

pub(crate) fn carrier_error(error: crate::runtime::generic_carriers::Error) -> ivm::VMError {
    match error {
        crate::runtime::generic_carriers::Error::Codec(original) => codec_error(original),
        _ => denied(),
    }
}

pub(crate) fn copy_account(account: &AccountId) -> Result<AccountId, ivm::VMError> {
    norito::core::classify_decode_attempt(|| account.try_clone_for_admission()).map_err(codec_error)
}

fn selection_id(selection: &RootSelectionV1) -> Result<SelectionId, ivm::VMError> {
    norito::core::classify_decode_attempt(|| {
        let mut encoded = Ok(());
        let hash = Hash::new_from_writer(|writer| {
            writer.write_all(SELECTION_ID_DOMAIN)?;
            encoded = norito::core::write_canonical_to_writer(selection, writer);
            Ok(())
        })
        .map_err(norito::Error::Io)?;
        encoded?;
        Ok(SelectionId(hash.into()))
    })
    .map_err(codec_error)
}

/// Fixed-size original selected dependency lists; only this module constructs the value.
#[derive(Clone, Copy, Debug)]
struct Invocation {
    source: SourceIdentity,
    selection: SelectionId,
    interface: DefinitionId,
    implementation: DefinitionId,
    reads: [Option<DefinitionId>; MAX_DEPENDENCIES],
    writes: [Option<DefinitionId>; MAX_DEPENDENCIES],
    max_effects: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceIdentity {
    signed: HashOf<iroha_data_model::transaction::SignedTransaction>,
    call: Hash,
    proposal: HashOf<iroha_data_model::block::BlockHeader>,
    network: iroha_data_model::NetworkId,
    dataspace: iroha_model_base::topology::DataSpaceId,
    index: Option<u64>,
}

impl SourceIdentity {
    fn capture(state: &StateTransaction<'_, '_>) -> Result<Self, ivm::VMError> {
        if state.execution_callback_active()
            || state.completing_obligation.is_some()
            || !state.deferred_expectations.is_empty()
            || state.native_execution.is_some()
            || state.world.runtime_selection.get() != state.world.runtime_selection.canonical()
            || state.current_dataspace_id != state.world.current_dataspace_id
        {
            return Err(denied());
        }
        Ok(Self {
            signed: state.current_tx_hash.ok_or_else(denied)?,
            call: state.tx_call_hash.ok_or_else(denied)?,
            proposal: state._curr_block.hash(),
            network: *state.network_id(),
            dataspace: state.current_dataspace_id.ok_or_else(denied)?,
            index: state.current_entrypoint_index,
        })
    }
}

impl Invocation {
    fn capture(
        state: &StateTransaction<'_, '_>,
        instruction: &InstructionBox,
    ) -> Result<Self, ivm::VMError> {
        let catalog = crate::runtime::release_catalog().map_err(|_| denied())?;
        let published =
            crate::runtime::change_set::PublishedDefinitions(&state.world.runtime_definitions);
        let decoded = crate::runtime::generic_carriers::decode_selected(
            catalog,
            state.world.runtime_selection.get(),
            &published,
            state.block_height(),
            instruction,
        )
        .map_err(carrier_error)?
        .ok_or_else(denied)?;
        let mut reads = [None; MAX_DEPENDENCIES];
        let mut writes = [None; MAX_DEPENDENCIES];
        if decoded.implementation.reads.len() > reads.len()
            || decoded.implementation.writes.len() > writes.len()
        {
            return Err(denied());
        }
        for (slot, id) in reads.iter_mut().zip(&decoded.implementation.reads) {
            *slot = Some(*id);
        }
        for (slot, id) in writes.iter_mut().zip(&decoded.implementation.writes) {
            *slot = Some(*id);
        }
        Ok(Self {
            source: SourceIdentity::capture(state)?,
            selection: selection_id(state.world.runtime_selection.get())?,
            interface: decoded.interface_id,
            implementation: decoded.implementation_id,
            reads,
            writes,
            max_effects: decoded.implementation.limits.max_effects,
        })
    }

    fn owner(&self, index: u64, write: bool) -> Result<DefinitionId, ivm::VMError> {
        let index = usize::try_from(index).map_err(|_| denied())?;
        let entries = if write { &self.writes } else { &self.reads };
        entries.get(index).copied().flatten().ok_or_else(denied)
    }

    fn validate(&self, state: &StateTransaction<'_, '_>) -> Result<(), ivm::VMError> {
        self.validate_selected(
            SourceIdentity::capture(state)?,
            state.world.runtime_selection.get(),
            &state.world.runtime_definitions,
        )
    }

    fn validate_selected(
        &self,
        source: SourceIdentity,
        selection: &RootSelectionV1,
        definitions: &impl StorageReadOnly<DefinitionId, DefinitionDescriptorV1>,
    ) -> Result<(), ivm::VMError> {
        if source != self.source {
            return Err(denied());
        }
        if selection_id(selection)? != self.selection {
            return Err(denied());
        }
        for id in [self.interface, self.implementation] {
            let definition = definitions.get(&id).ok_or_else(denied)?;
            if iroha_data_model::runtime::carrier::carrier_definition_id_v1(definition)
                .map_err(codec_error)?
                != id
            {
                return Err(denied());
            }
        }
        Ok(())
    }
}

/// Immutable original cursor borrows, with no State transaction, copied graph or callback.
/// The original transaction cannot mutate while the host holds these views. Artifact application
/// separately rechecks the complete original source against the actual mutable State transaction.
struct CarrierSource<'run> {
    identity: SourceIdentity,
    selection: &'run RootSelectionV1,
    definitions: mv::storage::View<'run, DefinitionId, DefinitionDescriptorV1>,
    values: mv::storage::View<'run, RetainedCarrierKey, RetainedCarrierBytes, CarrierStorageMode>,
    budget: AllocationBudget,
}

impl<'run> CarrierSource<'run> {
    fn capture(state: &'run StateTransaction<'_, '_>) -> Result<Self, ivm::VMError> {
        Ok(Self {
            identity: SourceIdentity::capture(state)?,
            selection: state.world.runtime_selection.get(),
            definitions: state.world.runtime_definitions.view(),
            values: state.world.runtime_generic_values.view(),
            budget: state.execution_budget(),
        })
    }

    fn budget(&self) -> AllocationBudget {
        self.budget.clone()
    }
    fn validate_key(
        &self,
        invocation: &Invocation,
        key: &CarrierStorageKeyV1,
        value: Option<&[u8]>,
    ) -> Result<(), ivm::VMError> {
        invocation.validate_selected(self.identity, self.selection, &self.definitions)?;
        let published = crate::runtime::change_set::PublishedDefinitions(&self.definitions);
        let (key_schema, _) =
            crate::runtime::generic_carriers::storage::row_schema(self.selection, &published, key)
                .map_err(carrier_error)?;
        validated_carrier_key_v1(key_schema, &key.key).map_err(codec_error)?;
        if let Some(value) = value {
            crate::runtime::generic_carriers::storage::validate_row(
                self.selection,
                &published,
                key,
                value,
            )
            .map_err(carrier_error)?;
        }
        Ok(())
    }
    fn value<'a>(&'a self, key: &CarrierStorageKeyV1) -> Option<&'a [u8]> {
        self.values.get(key).map(|value| value.as_slice())
    }
}

pub(super) struct CarrierContext<'run> {
    invocation: Invocation,
    source: CarrierSource<'run>,
}

/// The invocation survives extraction even when the successful run emitted no writes.
pub(super) struct QueueBacking {
    invocation: Invocation,
    _allocation: AllocationReservation,
}

/// An exact capability-authorized write; bytes are destroyed before their original allowance.
#[derive(Debug)]
pub(crate) struct OwnedWrite {
    invocation: Invocation,
    key: CarrierStorageKeyV1,
    value: Option<Vec<u8>>,
    prepared: Option<crate::runtime::generic_carriers::storage::PreparedWrite>,
    _backing: AllocationReservation,
}

impl OwnedWrite {
    pub(super) fn validate(&self, state: &StateTransaction<'_, '_>) -> Result<(), ivm::VMError> {
        self.invocation.validate(state)?;
        let world = &*state.world;
        if !self.invocation.writes.contains(&Some(self.key.storage)) {
            return Err(denied());
        }
        let published =
            crate::runtime::change_set::PublishedDefinitions(world.runtime_definitions());
        let (key_schema, _) = crate::runtime::generic_carriers::storage::row_schema(
            world.runtime_selection(),
            &published,
            &self.key,
        )
        .map_err(carrier_error)?;
        validated_carrier_key_v1(key_schema, &self.key.key).map_err(codec_error)?;
        if let Some(value) = &self.value {
            crate::runtime::generic_carriers::storage::validate_row(
                world.runtime_selection(),
                &published,
                &self.key,
                value,
            )
            .map_err(carrier_error)?;
        }
        Ok(())
    }

    pub(super) fn apply(
        mut self,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), ValidationFail> {
        self.prepared
            .take()
            .ok_or_else(|| {
                ValidationFail::NotPermitted(
                    "carrier write has no original prepared storage owner".into(),
                )
            })?
            .apply(state)
    }
}

fn reserve(budget: &AllocationBudget, bytes: usize) -> Result<AllocationReservation, ivm::VMError> {
    budget
        .try_reserve_bytes(bytes)
        .map_err(|original| ivm::VMError::Deferred(VmDeferral::AllocationDeferred(original)))
}

fn copy(bytes: &[u8]) -> Result<Vec<u8>, ivm::VMError> {
    let layout = std::alloc::Layout::array::<u8>(bytes.len()).map_err(|_| {
        ivm::VMError::Deferred(VmDeferral::AllocationDeferred(
            iroha_allocation::AllocationRefusal::DemandOverflow,
        ))
    })?;
    let mut output = Vec::new();
    output.try_reserve_exact(bytes.len()).map_err(|original| {
        ivm::VMError::Deferred(VmDeferral::AllocatorDeferred(
            iroha_allocation::AllocatorRefusal::from_vec_reserve(original, layout),
        ))
    })?;
    output.extend_from_slice(bytes);
    Ok(output)
}

impl<'run> CoreHostImpl<'run, '_, NoQueryState> {
    /// The only carrier constructor takes the actual original transaction and envelope.
    pub(crate) fn for_carrier(
        authority: AccountId,
        state: &'run StateTransaction<'_, '_>,
        instruction: &InstructionBox,
    ) -> Result<Self, ivm::VMError> {
        let invocation = Invocation::capture(state, instruction)?;
        let mut host = Self::new(authority);
        host.execution_class = HostExecutionClass::Carrier;
        host.set_output_limits_from_parameters(state.world.parameters.get().smart_contract());
        host.restrict_output_limits(HostOutputLimits {
            max_items: u64::from(invocation.max_effects),
            max_bytes: u64::from(invocation.max_effects) * (1024 * 1024 + 4096 + 64),
        });
        let count =
            usize::try_from(host.instruction_queue_limits.max_items).map_err(|_| denied())?;
        let layout = std::alloc::Layout::array::<QueuedEffect>(count).map_err(|_| {
            ivm::VMError::Deferred(VmDeferral::AllocationDeferred(
                iroha_allocation::AllocationRefusal::DemandOverflow,
            ))
        })?;
        let budget = state.execution_budget();
        let mut account_bytes = 0;
        crate::executor::add_account_clone_bytes(&mut account_bytes, &host.authority)?;
        let bytes = account_bytes
            .checked_mul(count)
            .and_then(|bytes| bytes.checked_add(layout.size()))
            .ok_or_else(|| {
                ivm::VMError::Deferred(VmDeferral::AllocationDeferred(
                    iroha_allocation::AllocationRefusal::DemandOverflow,
                ))
            })?;
        // Whole-effect backing outlives every queued authority, including abort/drop paths.
        let backing = reserve(&budget, bytes)?;
        host.queued.try_reserve_exact(count).map_err(|original| {
            ivm::VMError::Deferred(VmDeferral::AllocatorDeferred(
                iroha_allocation::AllocatorRefusal::from_vec_reserve(original, layout),
            ))
        })?;
        host.carrier_queue_backing = Some(QueueBacking {
            invocation,
            _allocation: backing,
        });
        host.carrier = Some(CarrierContext {
            invocation,
            source: CarrierSource::capture(state)?,
        });
        Ok(host)
    }
}

impl<QS: Default + QueryStateAccess> CoreHostImpl<'_, '_, QS> {
    pub(super) fn carrier_quote(
        &self,
        number: u32,
        vm: ivm::host::SyscallQuote<'_>,
    ) -> Result<Option<u64>, ivm::VMError> {
        if vm.syscall_policy() != ivm::SyscallPolicy::AbiV1GenericCarrier {
            return if self.carrier.is_some() {
                Err(denied())
            } else {
                Ok(None)
            };
        }
        let context = self.carrier.as_ref().ok_or_else(denied)?;
        if matches!(
            number,
            ivm::syscalls::SYSCALL_STATE_GET
                | ivm::syscalls::SYSCALL_STATE_SET
                | ivm::syscalls::SYSCALL_STATE_DEL
        ) {
            context.invocation.owner(
                vm.public_register(10)?,
                number != ivm::syscalls::SYSCALL_STATE_GET,
            )?;
            let key_len =
                vm.quote_tlv_payload_len_at(vm.public_register(11)?, PointerType::NoritoBytes)?;
            if key_len > MAX_CARRIER_KEY_BYTES {
                return Err(ivm::VMError::Trap(VmTrap::NoritoInvalid));
            }
            let gas = match number {
                ivm::syscalls::SYSCALL_STATE_GET => {
                    vm.reserve_available_syscall_gas_at_least(ivm::host::state_path_gas(key_len))?
                }
                ivm::syscalls::SYSCALL_STATE_SET => {
                    let value_len = vm.quote_tlv_payload_len_at(
                        vm.public_register(12)?,
                        PointerType::NoritoBytes,
                    )?;
                    if value_len > 1024 * 1024 {
                        return Err(ivm::VMError::Trap(VmTrap::NoritoInvalid));
                    }
                    ivm::host::state_value_gas(key_len, value_len)
                }
                _ => ivm::host::state_path_gas(key_len),
            };
            return Ok(Some(gas));
        }
        Ok(None)
    }

    pub(super) fn carrier_syscall(
        &mut self,
        number: u32,
        vm: &mut dyn ivm::host::SyscallContext,
    ) -> Result<Option<u64>, ivm::VMError> {
        if vm.quote().syscall_policy() != ivm::SyscallPolicy::AbiV1GenericCarrier {
            return if self.carrier.is_some() {
                Err(denied())
            } else {
                Ok(None)
            };
        }
        let context = self.carrier.as_ref().ok_or_else(denied)?;
        let invocation = context.invocation;
        let source = &context.source;
        if !matches!(
            number,
            ivm::syscalls::SYSCALL_STATE_GET
                | ivm::syscalls::SYSCALL_STATE_SET
                | ivm::syscalls::SYSCALL_STATE_DEL
        ) {
            return Ok(None);
        }
        let write = number != ivm::syscalls::SYSCALL_STATE_GET;
        let owner = invocation.owner(vm.public_register(10)?, write)?;
        let key_tlv = Self::expect_tlv(vm, vm.public_register(11)?, PointerType::NoritoBytes)?;
        if key_tlv.payload.len() > MAX_CARRIER_KEY_BYTES {
            return Err(ivm::VMError::Trap(VmTrap::NoritoInvalid));
        }
        let key_len = key_tlv.payload.len();
        let value = if number == ivm::syscalls::SYSCALL_STATE_SET {
            let value =
                Self::expect_tlv(vm, vm.public_register(12)?, PointerType::NoritoBytes)?.payload;
            if value.len() > 1024 * 1024 {
                return Err(ivm::VMError::Trap(VmTrap::NoritoInvalid));
            }
            Some(value)
        } else {
            None
        };
        let mut bytes = key_len;
        if let Some(value) = value {
            crate::executor::add_clone_bytes(&mut bytes, value.len())?;
        }
        let budget = source.budget();
        let backing = reserve(&budget, bytes)?;
        let key = CarrierStorageKeyV1 {
            storage: owner,
            key: copy(key_tlv.payload)?,
        };
        source.validate_key(&invocation, &key, value)?;
        if !write {
            let staged = self
                .queued
                .iter()
                .rev()
                .find_map(|effect| match &effect.payload {
                    QueuedEffectPayload::CarrierWrite(write) if write.key == key => {
                        Some(write.value.as_deref())
                    }
                    _ => None,
                });
            let stored = staged.unwrap_or_else(|| source.value(&key));
            let gas = stored.map_or_else(
                || ivm::host::state_path_gas(key_len),
                |value| ivm::host::state_value_gas(key_len, value.len()),
            );
            ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
            if let Some(value) = stored {
                source.validate_key(&invocation, &key, Some(value))?;
                let _tlv_backing =
                    reserve(&budget, value.len().checked_add(39).ok_or_else(denied)?)?;
                let tlv = ivm_abi::pointer_abi::encode_tlv_with_allocator_custody(
                    PointerType::NoritoBytes,
                    value,
                )?;
                let pointer = vm.alloc_encoded_host_tlv(&tlv)?;
                vm.write_register(10, pointer)?;
            } else {
                vm.write_register(10, 0)?;
            }
            drop(key);
            drop(backing);
            return Ok(Some(gas));
        }
        let gas = value.map_or_else(
            || ivm::host::state_path_gas(key_len),
            |value| ivm::host::state_value_gas(key_len, value.len()),
        );
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        let value = value.map(copy).transpose()?;
        let authority = copy_account(&self.authority)?;
        let output_bytes = key_len
            .checked_add(value.as_ref().map_or(0, Vec::len))
            .and_then(|n| n.checked_add(64))
            .ok_or_else(denied)?;
        if !self.try_reserve_output(1, output_bytes) {
            self.ensure_output_budget()?;
            return Err(denied());
        }
        if self.queued.len() >= self.queued.capacity() {
            return Err(denied());
        }
        self.queued.push(QueuedEffect {
            payload: QueuedEffectPayload::CarrierWrite(OwnedWrite {
                invocation,
                key,
                value,
                prepared: None,
                _backing: backing,
            }),
            authority,
            contract_runtime_context: None,
            entrypoint_authorization: None,
        });
        vm.write_register(10, 0)?;
        Ok(Some(gas))
    }
}

#[cfg(test)]
pub(crate) mod tests;
