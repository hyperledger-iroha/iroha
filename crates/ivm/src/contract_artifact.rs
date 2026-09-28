//! Native preparation adapter for the shared artifact-admission crate.
use crate::{
    ProgramMetadata, SyscallPolicy,
    ivm::{
        decode_literal_table, prepare_instruction_stream, validate_indexed_literal_instructions,
    },
    ivm_cache::global_get,
    metadata::{EmbeddedContractInterfaceV1, ParsedProgramMetadata},
    prepared::{PreparedContract, PreparedContractParts, PreparedControlFlow},
};
pub use ivm_artifact_admission::{
    ContractArtifactError, VerifiedContractArtifact, verify_contract_artifact,
};
use std::sync::Arc;
/// Prepare a validated self-describing contract for repeated VM loading.
///
/// Admission is delegated to [`ivm_artifact_admission`]. This module only constructs native cache
/// and execution structures after that shared policy has accepted the immutable artifact bytes.
pub fn prepare_contract(artifact: Arc<[u8]>) -> Result<PreparedContract, ContractArtifactError> {
    PreparedContract::prepare(artifact)
}
/// Prepare a compiler-produced Kotodama test-suite artifact for local execution.
pub(crate) fn prepare_koto_test_contract(
    artifact: Arc<[u8]>,
    contract_interface: EmbeddedContractInterfaceV1,
) -> Result<PreparedContract, ContractArtifactError> {
    PreparedContract::prepare_koto_test_harness(artifact, contract_interface)
}
impl PreparedContract {
    /// Admit through the shared production verifier, then build native runtime indexes.
    pub fn prepare(artifact: Arc<[u8]>) -> Result<Self, ContractArtifactError> {
        let verified = ivm_artifact_admission::verify_contract_artifact(artifact.as_ref())?;
        Self::prepare_shared_verified(artifact, verified)
    }
    fn prepare_koto_test_harness(
        artifact: Arc<[u8]>,
        contract_interface: EmbeddedContractInterfaceV1,
    ) -> Result<Self, ContractArtifactError> {
        let verified = ivm_artifact_admission::verify_koto_test_artifact(
            artifact.as_ref(),
            contract_interface,
        )?;
        Self::prepare_shared_verified(artifact, verified)
    }
    fn prepare_shared_verified(
        artifact: Arc<[u8]>,
        verified: VerifiedContractArtifact,
    ) -> Result<Self, ContractArtifactError> {
        // Reparse only to recover native preparation ranges. Consensus policy
        // and all artifact-derived outputs above came from the shared verifier.
        let parsed = ProgramMetadata::parse(artifact.as_ref()).map_err(|error| {
            ContractArtifactError::preparation("metadata reparse after shared admission", error)
        })?;
        ensure_shared_offsets_match(&parsed, &verified)?;
        let decoded = decode_instruction_stream(artifact.as_ref(), &parsed)?;
        let instruction_region = artifact.get(parsed.code_offset..).ok_or_else(|| {
            ContractArtifactError::invalid("executable stream offset exceeds artifact length")
        })?;
        let literal_table = decode_literal_table(
            artifact.as_ref(),
            parsed.header_len,
            parsed.literal_section,
            SyscallPolicy::AbiV1,
        )
        .map_err(|error| {
            ContractArtifactError::preparation(
                "literal index preparation after shared admission",
                error,
            )
        })?;
        validate_indexed_literal_instructions(decoded.as_ref(), literal_table.entries()).map_err(
            |error| {
                ContractArtifactError::preparation(
                    "literal instruction preparation after shared admission",
                    error,
                )
            },
        )?;
        let instruction_entry_pc = u64::try_from(parsed.prefix_len()).map_err(|_| {
            ContractArtifactError::invalid("executable stream offset does not fit a VM address")
        })?;
        let prepared_program = prepare_instruction_stream(
            instruction_region,
            decoded.as_ref(),
            instruction_entry_pc,
            literal_table.entries(),
        )
        .map_err(|error| ContractArtifactError::preparation("instruction preparation", error))?;
        let control_flow =
            PreparedControlFlow::from_decoded(decoded.as_ref()).map_err(|error| {
                ContractArtifactError::preparation("control-flow preparation", error)
            })?;
        PreparedContract::from_parts(PreparedContractParts {
            // Take our own byte allocation; an input Arc may have unrelated owners
            // whose lifetimes cannot be governed by this preparation reservation.
            artifact: crate::cache_memory::SharedAllocation::from(artifact.as_ref().to_vec()),
            metadata: verified.metadata,
            manifest: verified.manifest,
            header_len: verified.header_len,
            code_offset: verified.code_offset,
            code_hash: verified.code_hash,
            contract_interface: {
                let exclusively_owned = verified
                    .contract_interface
                    .entrypoints
                    .iter()
                    .all(|entry| entry.triggers.is_empty());
                crate::prepared::shared_metadata(verified.contract_interface, exclusively_owned)
            },
            literal_table,
            decoded,
            prepared_program,
            control_flow,
        })
        .map_err(|error| ContractArtifactError::preparation("contract indexing", error))
    }
}
fn ensure_shared_offsets_match(
    parsed: &ParsedProgramMetadata,
    verified: &VerifiedContractArtifact,
) -> Result<(), ContractArtifactError> {
    if parsed.header_len != verified.header_len || parsed.code_offset != verified.code_offset {
        return Err(ContractArtifactError::invalid(
            "native metadata ranges diverge from shared artifact admission",
        ));
    }
    Ok(())
}
fn decode_instruction_stream(
    artifact: &[u8],
    parsed: &ParsedProgramMetadata,
) -> Result<crate::ivm_cache::DecodedStream, ContractArtifactError> {
    let instruction_region = artifact.get(parsed.code_offset..).ok_or_else(|| {
        ContractArtifactError::invalid("executable stream offset exceeds artifact length")
    })?;
    global_get(instruction_region).map_err(|error| {
        ContractArtifactError::preparation("instruction decode after shared admission", error)
    })
}

#[cfg(test)]
mod preparation_deferral_tests {
    use super::*;
    use crate::{VMError, error::ExecutionDeferral};
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
    };

    #[test]
    fn cold_native_preparation_defers_allocator_refusal_then_retries_same_artifact() {
        let artifact: Arc<[u8]> = crate::KotodamaCompiler::new()
            .compile_source(r#"seiyaku PreparationRefusal { kotoage fn main() -> int authorize("Entry") { return 701; } }"#)
            .expect("compile an admitted first-release artifact")
            .into();
        let admitted = verify_contract_artifact(&artifact).expect("valid independent admission");
        let _limits = crate::ivm_cache::CacheLimitsGuard::new(crate::ivm_cache::CacheLimits {
            capacity: 0,
            max_bytes: 0,
            max_decoded_ops: 0,
        });
        let error = crate::cache_memory::with_refused_shared_allocation_for_test(|| {
            prepare_contract(Arc::clone(&artifact))
        })
        .expect_err("cold native decode must expose local allocator refusal");
        let error = error.into_vm_error();
        assert_eq!(
            error,
            VMError::ExecutionDeferred(ExecutionDeferral::AllocationUnavailable)
        );
        assert_eq!(error.metered_gas(), None);
        let retried =
            prepare_contract(Arc::clone(&artifact)).expect("retry the same valid artifact");
        assert_eq!(retried.code_hash(), admitted.code_hash);
        assert_eq!(retried.artifact(), artifact.as_ref());
    }

    #[test]
    fn preparation_keeps_exact_pool_release_observation_through_error_conversion() {
        let budget = mv::allocation::AllocationBudget::new(8);
        let occupied = budget.try_reserve_bytes(8).unwrap();
        let original = budget.try_reserve_bytes(1).unwrap_err();
        let deferred = VMError::Metered {
            gas: 31,
            source: Box::new(VMError::AllocationDeferred(original.clone())),
        };
        let error =
            ContractArtifactError::preparation("prepared instructions", deferred).into_vm_error();
        assert_eq!(error, VMError::AllocationDeferred(original));
        assert_eq!(
            error.execution_deferral(),
            Some(ExecutionDeferral::ActiveMemoryCapacity)
        );
        assert_eq!(error.metered_gas(), None);
        let VMError::AllocationDeferred(mv::allocation::AllocationRefusal::Capacity {
            release,
            ..
        }) = error
        else {
            panic!("preparation must preserve the capacity owner's observation");
        };
        let mut wait = release.wait_for_release();
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(Pin::new(&mut wait).poll(&mut cx), Poll::Pending);
        // A refund from another pool cannot make this failed attempt ready.
        let other = mv::allocation::AllocationBudget::new(8);
        drop(other.try_reserve_bytes(8).unwrap());
        assert_eq!(Pin::new(&mut wait).poll(&mut cx), Poll::Pending);
        drop(occupied);
        assert_eq!(Pin::new(&mut wait).poll(&mut cx), Poll::Ready(()));
        assert!(budget.try_reserve_bytes(8).is_ok());
    }
}
