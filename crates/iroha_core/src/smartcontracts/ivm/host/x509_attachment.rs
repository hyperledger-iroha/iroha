//! Funded X.509 attachment payloads stay owned through queued application.

use super::*;
use crate::executor::attachment_custody::x509::{X509Binding, X509Error};
use iroha_allocation::{
    AllocationBudget, AllocationReservation, ChargedBuffer, ChargedBufferError,
};
use iroha_data_model::{
    isi::privacy::{PrivacyZkX509RequestErrorV1, SubmitPrivacyZkX509CertificateV1},
    transaction::signed_attachments::{SIGNED_ATTACHMENTS_MAX_ACTIONS_V1, SignedAttachmentErrorV1},
};

/// Queue backing and all added metadata copies drop after every queued effect.
#[derive(Debug)]
pub(super) struct X509QueueBacking {
    _backing: AllocationReservation,
    metadata: [Option<AllocationReservation>; SIGNED_ATTACHMENTS_MAX_ACTIONS_V1],
}
impl X509QueueBacking {
    fn retain_metadata(&mut self, owner: AllocationReservation) {
        *self
            .metadata
            .iter_mut()
            .find(|slot| slot.is_none())
            .expect("one metadata owner per consumed signed action") = Some(owner);
    }
}

/// The canonical decoded request drops before the original conservative decode allowance.
struct DecodedRequest {
    instruction: InstructionBox,
    allowance: AllocationReservation,
}
impl DecodedRequest {
    fn decode(vm: &IVM) -> Result<Self, ivm::VMError> {
        let tlv = vm.validate_tlv(vm.register(10))?;
        if tlv.type_id != PointerType::NoritoBytes
            || tlv.payload.len()
                > SubmitPrivacyZkX509CertificateV1::canonical_instruction_box_frame_byte_limit_v1()
        {
            return Err(ivm::VMError::NoritoInvalid);
        }
        let budget = vm
            .allocation_budget()
            .ok_or(ivm::VMError::ExecutionDeferred(
                ivm::ExecutionDeferral::LocalInvariantViolation,
            ))?;
        let limits = norito::canonical_decode_limits(tlv.payload.len());
        let demand = limits
            .max_total_allocated_bytes()
            .checked_add(norito::core::DecodeBudgetContext::allocation_layout().size())
            .ok_or(ivm::VMError::AllocationDeferred(
                iroha_allocation::AllocationRefusal::DemandOverflow,
            ))?;
        let mut allowance = budget
            .try_reserve_bytes(demand)
            .map_err(ivm::VMError::AllocationDeferred)?;
        let scope = norito::core::DecodeBudgetContext::from_reservation(limits, &mut allowance)
            .map_err(|error| match error {
                iroha_allocation::PrepaidSharedError::Reservation(_) => {
                    ivm::VMError::ExecutionDeferred(ivm::ExecutionDeferral::LocalInvariantViolation)
                }
                iroha_allocation::PrepaidSharedError::Allocator { .. } => {
                    ivm::VMError::ExecutionDeferred(ivm::ExecutionDeferral::AllocationUnavailable)
                }
            })?;
        let instruction: InstructionBox = scope
            .with(|| {
                norito::core::classify_decode_attempt(|| {
                    norito::decode_canonical_with_limits(tlv.payload, limits)
                })
            })
            .map_err(|error| {
                crate::execution_attempt::canonical_decode_attempt_error(error, |_| {
                    ivm::VMError::NoritoInvalid
                })
                .into_vm_error(std::convert::identity)
            })?;
        if !instruction
            .as_any()
            .is::<SubmitPrivacyZkX509CertificateV1>()
        {
            return Err(ivm::VMError::PermissionDenied);
        }
        drop(scope);
        Ok(Self {
            instruction,
            allowance,
        })
    }
}

/// One private queued request and its exact original proof identity/backing.
pub(crate) struct OwnedX509Action {
    binding: X509Binding,
    proof: ChargedBuffer<u8>,
    request: DecodedRequest,
}
impl std::fmt::Debug for OwnedX509Action {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OwnedX509Action")
            .field("request", &self.request.instruction)
            .field("proof_bytes", &self.proof.as_slice().len())
            .finish_non_exhaustive()
    }
}
impl OwnedX509Action {
    pub(crate) fn instruction(&self) -> &InstructionBox {
        &self.request.instruction
    }
    pub(super) fn execute(
        &self,
        executor: &crate::executor::Executor,
        state: &mut StateTransaction<'_, '_>,
        authority: &AccountId,
        runtime: Option<&ContractRuntimeExecutionContext>,
    ) -> Result<(), ValidationFail> {
        executor.execute_queued_x509_instruction(
            state,
            authority,
            self.instruction(),
            runtime,
            &self.binding,
            self.proof.as_slice(),
        )
    }
}

/// Applied instruction observations retain request decode custody until they are dropped.
/// Proof bytes are not receipts and remain under their signed input owner.
pub struct ExecutedHostInstructions {
    instructions: Vec<InstructionBox>,
    backing: Option<AllocationReservation>,
    allowances: [Option<AllocationReservation>; SIGNED_ATTACHMENTS_MAX_ACTIONS_V1],
    retained: usize,
}
impl Default for ExecutedHostInstructions {
    fn default() -> Self {
        Self {
            instructions: Vec::new(),
            backing: None,
            allowances: std::array::from_fn(|_| None),
            retained: 0,
        }
    }
}
impl std::fmt::Debug for ExecutedHostInstructions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutedHostInstructions")
            .field("instructions", &self.instructions)
            .field("retained_requests", &self.retained)
            .finish()
    }
}
impl std::ops::Deref for ExecutedHostInstructions {
    type Target = [InstructionBox];
    fn deref(&self) -> &Self::Target {
        &self.instructions
    }
}
impl ExecutedHostInstructions {
    pub(super) fn try_with_capacity(
        count: usize,
        budget: &AllocationBudget,
    ) -> Result<Self, ivm::VMError> {
        let bytes = std::alloc::Layout::array::<InstructionBox>(count)
            .map_err(|_| {
                ivm::VMError::AllocationDeferred(
                    iroha_allocation::AllocationRefusal::DemandOverflow,
                )
            })?
            .size();
        let backing = budget
            .try_reserve_bytes(bytes)
            .map_err(ivm::VMError::AllocationDeferred)?;
        let mut instructions = Vec::new();
        instructions.try_reserve_exact(count).map_err(|_| {
            ivm::VMError::ExecutionDeferred(ivm::ExecutionDeferral::AllocationUnavailable)
        })?;
        Ok(Self {
            instructions,
            backing: Some(backing),
            allowances: std::array::from_fn(|_| None),
            retained: 0,
        })
    }

    /// Borrow the applied proof-free instructions without releasing their custody.
    pub fn as_slice(&self) -> &[InstructionBox] {
        &self.instructions
    }
    pub(super) fn push(&mut self, instruction: InstructionBox) {
        if self.backing.is_some() {
            assert!(
                self.instructions.len() < self.instructions.capacity(),
                "prepaid receipt bound"
            );
        }
        self.instructions.push(instruction);
    }
    pub(super) fn push_attached(&mut self, action: OwnedX509Action) {
        // Signed-attachment validation admits at most this fixed number. The
        // queue consumes every occurrence only once, before this apply loop.
        assert!(
            self.retained < self.allowances.len(),
            "validated attachment action bound"
        );
        let OwnedX509Action {
            request,
            binding,
            proof,
        } = action;
        let DecodedRequest {
            instruction,
            allowance,
        } = request;
        assert!(
            self.backing.is_some() && self.instructions.len() < self.instructions.capacity(),
            "prepaid attached receipt"
        );
        self.instructions.push(instruction);
        self.allowances[self.retained] = Some(allowance);
        self.retained += 1;
        drop(binding);
        drop(proof);
    }
    /// Trigger execution has no original signed attachment root and cannot export one.
    pub(crate) fn into_trigger_instructions(self) -> Result<Vec<InstructionBox>, ValidationFail> {
        if self.retained != 0 || self.backing.is_some() {
            return Err(ValidationFail::InternalError(
                "trigger attempted to extract original attachment-backed instructions".into(),
            ));
        }
        Ok(self.instructions)
    }
}

fn x509_vm_error(error: X509Error) -> ivm::VMError {
    match error {
        X509Error::Capacity(original) => ivm::VMError::AllocationDeferred(original),
        X509Error::AllocationUnavailable => {
            ivm::VMError::ExecutionDeferred(ivm::ExecutionDeferral::AllocationUnavailable)
        }
        X509Error::Custody(crate::executor::attachment_custody::CustodyError::Declaration(
            SignedAttachmentErrorV1::LocalResource(original),
        ))
        | X509Error::Request(PrivacyZkX509RequestErrorV1::Attachment(
            SignedAttachmentErrorV1::LocalResource(original),
        )) => ivm::VMError::DecodeDeferred(original),
        _ => ivm::VMError::PermissionDenied,
    }
}
fn copy_proof(proof: &[u8], budget: &AllocationBudget) -> Result<ChargedBuffer<u8>, X509Error> {
    let mut owner = ChargedBuffer::new(proof.len(), budget).map_err(|error| match error {
        ChargedBufferError::Admission(original) => X509Error::Capacity(original),
        ChargedBufferError::Allocator { .. } => X509Error::AllocationUnavailable,
    })?;
    for byte in proof {
        owner.push_reserved(*byte);
    }
    Ok(owner)
}

impl<'run, 'tx, QS: Default + QueryStateAccess> CoreHostImpl<'run, 'tx, QS> {
    pub(super) fn reserve_x509_queue_backing(
        &mut self,
        budget: &AllocationBudget,
    ) -> Result<(), ivm::VMError> {
        if self.x509_queue_backing.is_some() {
            return Ok(());
        }
        let limit = usize::try_from(self.instruction_queue_limits.max_items).map_err(|_| {
            ivm::VMError::AllocationDeferred(iroha_allocation::AllocationRefusal::DemandOverflow)
        })?;
        let capacity = limit.max(self.queued.capacity());
        let bytes = std::alloc::Layout::array::<QueuedEffect>(capacity)
            .map_err(|_| {
                ivm::VMError::AllocationDeferred(
                    iroha_allocation::AllocationRefusal::DemandOverflow,
                )
            })?
            .size();
        let backing = budget
            .try_reserve_bytes(bytes)
            .map_err(ivm::VMError::AllocationDeferred)?;
        self.queued
            .try_reserve_exact(capacity.saturating_sub(self.queued.len()))
            .map_err(|_| {
                ivm::VMError::ExecutionDeferred(ivm::ExecutionDeferral::AllocationUnavailable)
            })?;
        // Preallocate the configured output-item bound once. Child rollback
        // truncates elements but retains this capacity and its original charge.
        self.x509_queue_backing = Some(X509QueueBacking {
            _backing: backing,
            metadata: std::array::from_fn(|_| None),
        });
        Ok(())
    }

    pub(super) fn x509_metadata_clone_bytes(&self) -> Result<usize, ivm::VMError> {
        use crate::executor::{add_account_clone_bytes, add_clone_bytes};
        let mut bytes = 0usize;
        let authority = self
            .current_contract_runtime_context
            .as_ref()
            .map_or(&self.authority, |context| &context.contract_subject);
        add_account_clone_bytes(&mut bytes, authority)?;
        if let Some(context) = &self.current_contract_runtime_context {
            add_account_clone_bytes(&mut bytes, &context.contract_subject)?;
            for len in [
                context.contract_address.as_str().len(),
                context.entrypoint.len(),
                context
                    .contract_alias
                    .as_ref()
                    .map_or(0, |alias| AsRef::<str>::as_ref(alias).len()),
            ] {
                add_clone_bytes(&mut bytes, len)?;
            }
        }
        if let Some(authorization) = &self.current_entrypoint_authorization {
            add_clone_bytes(&mut bytes, authorization.queued_clone_bytes()?)?;
        }
        Ok(bytes)
    }

    pub(super) fn queue_x509_attachment(&mut self, vm: &mut IVM) -> Result<u64, ivm::VMError> {
        let request = DecodedRequest::decode(vm)?;
        self.ensure_instruction_queue_allowed(&request.instruction)?;
        let typed = request
            .instruction
            .as_any()
            .downcast_ref::<SubmitPrivacyZkX509CertificateV1>()
            .ok_or(ivm::VMError::PermissionDenied)?;
        let ordinal = typed.proof_ref.action_index;
        let limits = self
            .query_state
            .get()
            .ok_or(ivm::VMError::PermissionDenied)?
            .privacy_consensus_limits();
        let source_byte_limit = self
            .query_state
            .get()
            .ok_or(ivm::VMError::PermissionDenied)?
            .transaction_source_byte_limit();
        let budget = vm
            .allocation_budget()
            .ok_or(ivm::VMError::ExecutionDeferred(
                ivm::ExecutionDeferral::LocalInvariantViolation,
            ))?
            .clone();
        let gas = crate::gas::meter_instruction(&request.instruction, &self.confidential_gas)
            .map_err(ivm::VMError::VerificationDeferred)?;
        ivm::host::preflight_reserved_syscall_gas(vm, gas)?;
        if !self.try_reserve_serialized_output(&request.instruction, 1) {
            self.abort_attachment_attempt();
            return self.ensure_output_budget().map(|()| gas);
        }
        self.reserve_x509_queue_backing(&budget)?;
        let metadata = budget
            .try_reserve_bytes(self.x509_metadata_clone_bytes()?)
            .map_err(ivm::VMError::AllocationDeferred)?;
        let (binding, proof) = self
            .with_attachment_invocation(|invocation| {
                invocation.with_x509_action(
                    typed,
                    &limits,
                    source_byte_limit,
                    &budget,
                    |binding, proof| Ok((binding, copy_proof(proof, &budget)?)),
                )
            })
            .map_err(x509_vm_error)?;
        self.queued.push(QueuedEffect {
            payload: QueuedEffectPayload::AttachedX509(OwnedX509Action {
                request,
                binding,
                proof,
            }),
            authority: self.effect_authority(),
            contract_runtime_context: self.current_contract_runtime_context.clone(),
            entrypoint_authorization: self.current_entrypoint_authorization.clone(),
        });
        self.x509_queue_backing
            .as_mut()
            .expect("prepaid queue")
            .retain_metadata(metadata);
        // This ordinal observes only successful queue insertion. It is neither
        // a proof result nor authority to branch on an unapplied ledger effect.
        vm.set_register(10, u64::from(ordinal));
        Ok(gas)
    }
}

#[cfg(test)]
mod allocation_tests {
    use super::*;
    use iroha_test_samples::ALICE_ID;

    #[test]
    fn queued_backing_refuses_before_allocation_and_survives_truncation() {
        let budget = AllocationBudget::new(1);
        let mut host = CoreHost::new(ALICE_ID.clone());
        assert!(matches!(
            host.reserve_x509_queue_backing(&budget),
            Err(ivm::VMError::AllocationDeferred(_))
        ));
        assert_eq!(host.queued.capacity(), 0);
        assert!(host.x509_queue_backing.is_none());
        assert_eq!(budget.reserved_bytes(), 0);
        budget.set_limit_bytes(64 * 1024 * 1024);
        host.reserve_x509_queue_backing(&budget).unwrap();
        assert!(host.queued.capacity() >= host.instruction_queue_limits.max_items as usize);
        let retained = budget.reserved_bytes();
        assert!(retained >= host.queued.capacity() * std::mem::size_of::<QueuedEffect>());
        host.queued.truncate(0);
        assert_eq!(
            budget.reserved_bytes(),
            retained,
            "rollback keeps the backing allocation alive"
        );
        assert!(
            host.drain_queued_effects_with_contract_runtime_context(None)
                .is_err()
        );
        drop(host);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn receipt_backing_is_prepaid_and_cannot_escape_through_trigger_extraction() {
        let budget = AllocationBudget::new(1024);
        let mut receipt = ExecutedHostInstructions::try_with_capacity(2, &budget).unwrap();
        assert_eq!(
            budget.reserved_bytes(),
            2 * std::mem::size_of::<InstructionBox>()
        );
        receipt.push(
            iroha_data_model::isi::Log::new(iroha_data_model::Level::INFO, "first".into()).into(),
        );
        receipt.push(
            iroha_data_model::isi::Log::new(iroha_data_model::Level::INFO, "second".into()).into(),
        );
        assert_eq!(receipt.len(), 2);
        assert!(receipt.into_trigger_instructions().is_err());
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
