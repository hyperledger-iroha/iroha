//! Canonical Core TLV emission and prepaid nested-return transport ownership.
//!
//! A nested call admits the public affordable-record bound plus the complete TLV envelope
//! before entering its child. Encoding partitions exact byte backing from that original State
//! credit and never waits for another admission. The source record and destination guest image
//! coexist with this scratch owner; their allocations are separate responsibilities. Guest
//! alignment padding lives in the VM image, while this byte-array allocation has alignment one.
//!
//! TODO: Fund typed return-record construction, nested journals and DefaultHost checkpoints
//! before enforcing the active pool over the complete nested execution path.

use super::{CoreHostImpl, QueryStateAccess};
use iroha_allocation::AllocationBudget;
use iroha_crypto::Hash;
use iroha_data_model::smart_contract::entrypoint::{
    ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1, MAX_ENTRYPOINT_RETURN_RECORD_BYTES,
};
use ivm::{
    IVM, PointerType, VMError,
    execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan},
};

fn envelope_bytes(payload_bytes: usize) -> Result<usize, VMError> {
    u32::try_from(payload_bytes).map_err(|_| VMError::NoritoInvalid)?;
    payload_bytes
        .checked_add(ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1)
        .ok_or(VMError::NoritoInvalid)
}

/// Emit the sole Core TLV layout directly into the caller's backing owner.
fn write_tlv_payload(
    pointer_type: PointerType,
    payload: &[u8],
    mut append: impl FnMut(&[u8]) -> Result<(), VMError>,
) -> Result<(), VMError> {
    let payload_len = u32::try_from(payload.len()).map_err(|_| VMError::NoritoInvalid)?;
    let mut header = [0_u8; 7];
    header[..2].copy_from_slice(&(pointer_type as u16).to_be_bytes());
    header[2] = 1;
    header[3..].copy_from_slice(&payload_len.to_be_bytes());
    append(&header)?;
    append(payload)?;
    append(Hash::new(payload).as_ref())
}

impl<QS: Default + QueryStateAccess> CoreHostImpl<QS> {
    pub(super) fn alloc_tlv_payload(
        vm: &mut IVM,
        pointer_type: PointerType,
        payload: &[u8],
    ) -> Result<u64, VMError> {
        let out = Self::encode_tlv_payload(pointer_type, payload)?;
        vm.alloc_host_tlv(&out)
    }

    pub(super) fn encode_tlv_payload(
        pointer_type: PointerType,
        payload: &[u8],
    ) -> Result<Vec<u8>, VMError> {
        let mut out = Vec::with_capacity(envelope_bytes(payload.len())?);
        write_tlv_payload(pointer_type, payload, |part| {
            out.extend_from_slice(part);
            Ok(())
        })?;
        Ok(out)
    }

    pub(super) fn alloc_norito_bytes(vm: &mut IVM, payload: &[u8]) -> Result<u64, VMError> {
        Self::alloc_tlv_payload(vm, PointerType::NoritoBytes, payload)
    }
}

/// Move-only child allowance. No private value is consulted when choosing its bound.
pub(super) struct NestedReturnTransport {
    max_record_bytes: usize,
    lease: ExecutionMemoryLease,
}

impl NestedReturnTransport {
    /// Reserve before child effects. Only a checked public ABI/gas bound is admitted.
    pub(super) fn reserve(
        budget: &AllocationBudget,
        max_record_bytes: usize,
    ) -> Result<Self, VMError> {
        if max_record_bytes > MAX_ENTRYPOINT_RETURN_RECORD_BYTES {
            return Err(VMError::DecodeError);
        }
        let plan = ExecutionMemoryPlan::array::<u8>(envelope_bytes(max_record_bytes)?)
            .map_err(VMError::AllocationDeferred)?;
        let lease =
            ExecutionMemoryLease::reserve(budget, plan).map_err(VMError::AllocationDeferred)?;
        Ok(Self {
            max_record_bytes,
            lease,
        })
    }

    /// Emit directly into exact prepaid backing; no conversion Vec overlaps the scratch buffer.
    fn encode(mut self, record: &[u8]) -> Result<ExecutionBuffer<u8>, VMError> {
        if record.len() > self.max_record_bytes {
            return Err(VMError::DecodeError);
        }
        let mut envelope = ExecutionBuffer::new(envelope_bytes(record.len())?, &mut self.lease)
            .map_err(|_| {
                VMError::ExecutionDeferred(ivm::error::ExecutionDeferral::AllocationUnavailable)
            })?;
        write_tlv_payload(PointerType::NoritoBytes, record, |part| {
            // The checked complete layout above covers every write. Refusing a write remains
            // fail-closed if that invariant changes; it never requests an unfunded growth.
            envelope.append(part).map_err(|_| VMError::DecodeError)
        })?;
        Ok(envelope)
    }

    /// Keep scratch charged until the guest copy finishes or fails, then release its final owner.
    pub(super) fn publish(self, vm: &mut IVM, record: &[u8]) -> Result<u64, VMError> {
        let envelope = self.encode(record)?;
        vm.alloc_host_tlv(envelope.as_slice())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn prepaid_return_refuses_zero_and_invalid_bounds_without_charge() {
        let budget = AllocationBudget::new(0);
        let error = NestedReturnTransport::reserve(&budget, 0)
            .err()
            .expect("even an empty record requires its envelope");
        assert!(matches!(error, VMError::AllocationDeferred(_)));
        assert_eq!(VMError::metered(500, error).metered_gas(), None);
        assert!(matches!(
            NestedReturnTransport::reserve(&budget, MAX_ENTRYPOINT_RETURN_RECORD_BYTES + 1),
            Err(VMError::DecodeError)
        ));
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn prepaid_return_uses_exact_backing_after_shrink_and_keeps_final_owner_charge() {
        let budget = AllocationBudget::new(1024 + ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1);
        let transport = NestedReturnTransport::reserve(&budget, 1024).unwrap();
        assert_eq!(budget.reserved_bytes(), budget.limit_bytes());
        // Credit admitted before child entry stays usable even if an operator shrinks the pool.
        budget.set_limit_bytes(0);
        let envelope = transport.encode(&[1, 2, 3]).unwrap();
        let exact_bytes = 3 + ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1;
        assert_eq!(envelope.capacity(), exact_bytes);
        assert_eq!(envelope.as_slice().len(), exact_bytes);
        assert_eq!(budget.reserved_bytes(), exact_bytes);
        // This test's outer Arc is a separate owner; only the exact byte backing is in this plan.
        let owner = Arc::new(envelope);
        let borrower = Arc::clone(&owner);
        drop(owner);
        assert_eq!(budget.reserved_bytes(), exact_bytes);
        drop(borrower);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn prepaid_return_matches_canonical_tlv_and_rejects_overrun() {
        let budget = AllocationBudget::new(4096);
        for payload in [&[][..], &[0, 1, 127, 255][..]] {
            let transport = NestedReturnTransport::reserve(&budget, payload.len()).unwrap();
            let envelope = transport.encode(payload).unwrap();
            let expected =
                super::super::CoreHost::encode_tlv_payload(PointerType::NoritoBytes, payload)
                    .unwrap();
            assert_eq!(envelope.as_slice(), expected);
            let tlv = ivm::pointer_abi::validate_tlv_bytes(envelope.as_slice()).unwrap();
            assert_eq!(tlv.payload, payload);
            assert_eq!(tlv.type_id, PointerType::NoritoBytes);
        }
        assert_eq!(budget.reserved_bytes(), 0);
        let transport = NestedReturnTransport::reserve(&budget, 2).unwrap();
        assert!(matches!(
            transport.encode(&[1, 2, 3]),
            Err(VMError::DecodeError)
        ));
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn prepaid_nested_allowance_refuses_without_waiting_or_refunding_parent() {
        let bytes = 64 + ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1;
        let budget = AllocationBudget::new(bytes);
        let parent = NestedReturnTransport::reserve(&budget, 64).unwrap();
        assert!(matches!(
            NestedReturnTransport::reserve(&budget, 64),
            Err(VMError::AllocationDeferred(_))
        ));
        assert_eq!(budget.reserved_bytes(), bytes);
        drop(parent);
        assert_eq!(budget.reserved_bytes(), 0);
        let retry = NestedReturnTransport::reserve(&budget, 64).unwrap();
        drop(retry);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn prepaid_return_guest_copy_failure_releases_scratch_without_gas() {
        let bytes = MAX_ENTRYPOINT_RETURN_RECORD_BYTES + ENTRYPOINT_RETURN_TLV_ENVELOPE_BYTES_V1;
        let budget = AllocationBudget::new(bytes);
        let transport =
            NestedReturnTransport::reserve(&budget, MAX_ENTRYPOINT_RETURN_RECORD_BYTES).unwrap();
        let record = vec![0x5A; MAX_ENTRYPOINT_RETURN_RECORD_BYTES];
        let mut vm = IVM::new(10_000);
        vm.alloc_heap(1).unwrap();
        assert_eq!(
            transport.publish(&mut vm, &record),
            Err(VMError::OutOfMemory)
        );
        assert_eq!(vm.remaining_gas(), 10_000);
        assert_eq!(budget.reserved_bytes(), 0);
    }

    #[test]
    fn prepaid_return_owner_releases_on_unwind() {
        let budget = AllocationBudget::new(4096);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let transport = NestedReturnTransport::reserve(&budget, 1024).unwrap();
            let _envelope = transport.encode(&[7; 512]).unwrap();
            panic!("unwind while return scratch is live");
        }));
        assert!(unwind.is_err());
        assert_eq!(budget.reserved_bytes(), 0);
    }
}
