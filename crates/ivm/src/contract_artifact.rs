//! Native preparation adapter for the shared artifact-admission crate.
use std::sync::Arc;
pub use ivm_artifact_admission::{
    ContractArtifactError, VerifiedContractArtifact, verify_contract_artifact,
};
use crate::{
    ProgramMetadata, SyscallPolicy,
    ivm::{
        decode_literal_table, prepare_instruction_stream, validate_indexed_literal_instructions,
    },
    ivm_cache::{DecodedOp, global_get_with_meta},
    metadata::{EmbeddedContractInterfaceV1, ParsedProgramMetadata},
    prepared::{PreparedContract, PreparedContractParts, PreparedControlFlow},
};
/// Prepare a validated self-describing contract for repeated VM loading.
///
/// Admission is delegated to [`ivm_artifact_admission`]. This module only
/// constructs native cache and execution structures after that shared policy
/// has accepted the immutable artifact bytes.
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
            ContractArtifactError::invalid(format!(
                "metadata reparse failed after shared admission: {error}"
            ))
        })?;
        ensure_shared_offsets_match(&parsed, &verified)?;
        let decoded = decode_instruction_stream(artifact.as_ref(), &parsed, &verified.metadata)?;
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
            ContractArtifactError::invalid(format!(
                "literal index preparation failed after shared admission: {error}"
            ))
        })?;
        validate_indexed_literal_instructions(decoded.as_ref(), literal_table.entries()).map_err(
            |error| {
                ContractArtifactError::invalid(format!(
                    "literal instruction preparation failed after shared admission: {error}"
                ))
            },
        )?;
        let instruction_entry_pc = u64::try_from(parsed.prefix_len()).map_err(|_| {
            ContractArtifactError::invalid("executable stream offset does not fit a VM address")
        })?;
        let prepared_program = prepare_instruction_stream(
            instruction_region,
            &verified.metadata,
            decoded.as_ref(),
            instruction_entry_pc,
            literal_table.entries(),
        )
        .map_err(|error| {
            ContractArtifactError::invalid(format!("instruction preparation failed: {error}"))
        })?;
        let control_flow =
            PreparedControlFlow::from_decoded(decoded.as_ref()).map_err(|error| {
                ContractArtifactError::invalid(format!("control-flow preparation failed: {error}"))
            })?;
        PreparedContract::from_parts(PreparedContractParts {
            artifact,
            metadata: verified.metadata,
            manifest: verified.manifest,
            header_len: verified.header_len,
            code_offset: verified.code_offset,
            code_hash: verified.code_hash,
            contract_interface: Arc::new(verified.contract_interface),
            literal_table,
            decoded,
            prepared_program,
            control_flow,
        })
        .map_err(|error| {
            ContractArtifactError::invalid(format!("contract indexing failed: {error}"))
        })
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
    metadata: &ProgramMetadata,
) -> Result<Arc<[DecodedOp]>, ContractArtifactError> {
    let instruction_region = artifact.get(parsed.code_offset..).ok_or_else(|| {
        ContractArtifactError::invalid("executable stream offset exceeds artifact length")
    })?;
    global_get_with_meta(instruction_region, metadata).map_err(|error| {
        ContractArtifactError::invalid(format!(
            "instruction preparation decode failed after shared admission: {error}"
        ))
    })
}
