//! Canonical, host-independent admission for deployable IVM contract artifacts.
//!
//! This crate is the single policy implementation for native IVM artifact admission.
//! It deliberately depends on the stable `ivm_abi` surface and canonical primitive codecs, not on
//! the VM runtime, caches, proof systems, or host integrations.
use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    asset::id::{AssetDefinitionId, AssetId},
    nexus::AxtAnchoredSpendV1,
    nft::NftId,
    prelude::{DecimalValueV1, IntValueV1, Json, QuantityValueV1},
    smart_contract::manifest::{ContractManifest, StateDescriptor},
    soracloud::{SoracloudHostRequestEnvelopeV1, SoracloudHostResponseEnvelopeV1},
};
use iroha_model_base::domain::DomainId;
use iroha_model_base::name::Name;
use iroha_model_base::topology::DataSpaceId;
use ivm_abi::{
    SyscallPolicy, VMError,
    axt::{AxtDescriptor, ProofBlob, validate_descriptor, validate_proof_blob},
    metadata::{
        EmbeddedContractInterfaceV1, EmbeddedEntrypointDescriptor, EmbeddedStateDescriptor,
        EmbeddedStateType, HEADER_SIZE, MAX_EMBEDDED_STATE_TYPE_DEPTH_V1, ParsedProgramMetadata,
        ProgramMetadata, contract_code_hash, mode,
    },
};
#[cfg(test)]
use norito::NoritoSerialize;
use std::fmt::Write as _;
mod policy;
/// Maximum executable-image bytes admitted by IVM code memory.
pub const MAX_CONTRACT_IMAGE_BYTES: u64 = ivm_abi::metadata::MAX_PROGRAM_IMAGE_BYTES_V1 as u64;
/// One fixed-width decoded instruction in the executable stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DecodedOp {
    pub(crate) pc: u64,
    pub(crate) inst: u32,
}
/// Admission outputs derived from the artifact itself.
#[derive(Clone, Debug)]
pub struct VerifiedContractArtifact {
    /// Validated fixed-header execution metadata.
    pub metadata: ProgramMetadata,
    /// Fixed metadata header length in artifact bytes.
    pub header_len: usize,
    /// Absolute executable-stream offset in the artifact.
    pub code_offset: usize,
    /// Domain-separated identity of the complete deployable artifact.
    pub code_hash: Hash,
    /// ABI descriptor hash authenticated by the embedded interface.
    pub abi_hash: Hash,
    /// Decoded and admission-validated embedded contract interface.
    pub contract_interface: EmbeddedContractInterfaceV1,
    /// Canonical unsigned on-chain manifest derived from the interface.
    pub manifest: ContractManifest,
}
mod error;
pub use error::ContractArtifactError;
/// Verify a self-describing IVM 1.1 artifact and derive its canonical manifest.
///
/// # Errors
///
/// Returns a stable admission error when metadata, the embedded contract
/// interface, bytecode policy, or literal-table bindings are invalid.
pub fn verify_contract_artifact(
    artifact: &[u8],
) -> Result<VerifiedContractArtifact, ContractArtifactError> {
    verify_contract_artifact_owned(artifact, None)
}
/// Verify with temporary instruction-array custody in the original State pool.
///
/// The canonical policy checker is shared with diagnostic admission. Literal
/// directories borrow the original artifact; metadata, typed payload decoding
/// and control-flow analysis allocations require separate custody.
///
/// # Errors
/// Returns the same protocol errors or an original local allocation refusal.
pub fn verify_contract_artifact_with_memory_budget(
    artifact: &[u8],
    budget: &iroha_allocation::AllocationBudget,
) -> Result<VerifiedContractArtifact, ContractArtifactError> {
    verify_contract_artifact_owned(artifact, Some(budget))
}
fn verify_contract_artifact_owned(
    artifact: &[u8],
    budget: Option<&iroha_allocation::AllocationBudget>,
) -> Result<VerifiedContractArtifact, ContractArtifactError> {
    let mut parsed = parse_contract_metadata(artifact)?;
    let contract_interface = validate_contract_envelope(artifact, &parsed)?;
    let code = artifact.get(parsed.code_offset..).ok_or_else(|| {
        ContractArtifactError::invalid("executable stream offset exceeds artifact length")
    })?;
    let decoded = decoded::instructions(code, budget)?;
    policy::validate_contract_interface(
        &parsed.metadata,
        contract_interface,
        &decoded,
        policy::ValidationProfile::Production,
    )?;
    literal::validate_literal_table(artifact, &parsed, &decoded)?;
    let contract_interface = parsed
        .contract_interface
        .take()
        .expect("validated contract envelope retains its CNTR interface");
    Ok(verified_from_parts(artifact, parsed, contract_interface))
}
/// Verify a compiler-produced generic IVM 1.0 Kotodama test harness against
/// its compiler-owned interface sidecar.
///
/// This is intentionally hidden from ordinary artifact consumers. Native IVM preparation uses it so
/// production and local-test profiles still share one policy implementation.
#[doc(hidden)]
pub fn verify_koto_test_artifact(
    artifact: &[u8],
    contract_interface: EmbeddedContractInterfaceV1,
) -> Result<VerifiedContractArtifact, ContractArtifactError> {
    let parsed = parse_contract_metadata(artifact)?;
    validate_koto_test_envelope(artifact, &parsed, &contract_interface)?;
    let code = artifact.get(parsed.code_offset..).ok_or_else(|| {
        ContractArtifactError::invalid("executable stream offset exceeds artifact length")
    })?;
    let decoded = decode_instruction_stream(code)?;
    policy::validate_contract_interface(
        &parsed.metadata,
        &contract_interface,
        &decoded,
        policy::ValidationProfile::KotoTest,
    )?;
    literal::validate_literal_table(artifact, &parsed, &decoded)?;
    Ok(verified_from_parts(artifact, parsed, contract_interface))
}
fn verified_from_parts(
    artifact: &[u8],
    parsed: ParsedProgramMetadata,
    contract_interface: EmbeddedContractInterfaceV1,
) -> VerifiedContractArtifact {
    let code_hash = contract_code_hash(artifact);
    let abi_hash = Hash::prehashed(contract_interface.abi_hash);
    let entrypoints = contract_interface
        .entrypoints
        .iter()
        .map(EmbeddedEntrypointDescriptor::to_manifest_descriptor)
        .collect::<Vec<_>>();
    let manifest = ContractManifest {
        seiyaku_name: Some(contract_interface.seiyaku_name.clone()),
        code_hash: Some(code_hash),
        abi_hash: Some(abi_hash),
        compiler_fingerprint: Some(contract_interface.compiler_fingerprint.clone()),
        features_bitmap: Some(contract_interface.features_bitmap),
        access_set_hints: contract_interface.access_set_hints.clone(),
        entrypoints: Some(entrypoints),
        states: Some(manifest_state_descriptors(&contract_interface.states)),
        error_types: (!contract_interface.error_types.is_empty())
            .then_some(contract_interface.error_types.clone()),
        error_messages: (!contract_interface.error_messages.is_empty())
            .then_some(contract_interface.error_messages.clone()),
        kotoba: (!contract_interface.kotoba.is_empty())
            .then_some(contract_interface.kotoba.clone()),
        provenance: None,
    };
    VerifiedContractArtifact {
        metadata: parsed.metadata,
        header_len: parsed.header_len,
        code_offset: parsed.code_offset,
        code_hash,
        abi_hash,
        contract_interface,
        manifest,
    }
}
fn parse_contract_metadata(
    artifact: &[u8],
) -> Result<ParsedProgramMetadata, ContractArtifactError> {
    ProgramMetadata::parse(artifact).map_err(|error| match error {
        VMError::ArtifactAbiHashMismatch { expected, actual } => {
            ContractArtifactError::abi_hash_mismatch(expected, actual)
        }
        _ if header_declares_contract_minor_one(artifact) && cntr_section_missing(artifact) => {
            ContractArtifactError::invalid("missing required CNTR section")
        }
        other if other.execution_deferral().is_some() => {
            ContractArtifactError::preparation("metadata parse", other)
        }
        other => ContractArtifactError::invalid(format!("metadata parse failed: {other}")),
    })
}
fn validate_contract_envelope<'a>(
    artifact: &[u8],
    parsed: &'a ParsedProgramMetadata,
) -> Result<&'a EmbeddedContractInterfaceV1, ContractArtifactError> {
    let metadata = &parsed.metadata;
    if metadata.version_major != 1 || metadata.version_minor != 1 {
        return Err(ContractArtifactError::invalid(format!(
            "expected IVM 1.1 contract artifact, got {}.{}",
            metadata.version_major, metadata.version_minor
        )));
    }
    if metadata.mode & !(mode::ZK | mode::VECTOR) != 0 {
        return Err(ContractArtifactError::invalid(format!(
            "unsupported contract execution mode bits 0x{:02x}",
            metadata.mode
        )));
    }
    if artifact.len() < HEADER_SIZE {
        return Err(ContractArtifactError::invalid(
            "artifact shorter than fixed IVM header",
        ));
    }
    let code_region_len = artifact
        .len()
        .checked_sub(parsed.header_len)
        .and_then(|len| u64::try_from(len).ok())
        .ok_or_else(|| ContractArtifactError::invalid("contract image length is invalid"))?;
    if code_region_len > MAX_CONTRACT_IMAGE_BYTES {
        return Err(ContractArtifactError::invalid(
            "contract image exceeds IVM code memory",
        ));
    }
    if parsed.contract_debug.is_some() {
        return Err(ContractArtifactError::invalid(
            "embedded DBG1 debug metadata is forbidden; publish source maps as hash-keyed sidecars",
        ));
    }
    let contract_interface = parsed
        .contract_interface
        .as_ref()
        .ok_or_else(|| ContractArtifactError::invalid("missing required CNTR section"))?;
    let policy = match metadata.abi_version {
        1 => SyscallPolicy::AbiV1,
        other => {
            return Err(ContractArtifactError::invalid(format!(
                "unsupported abi_version {other}; expected 1"
            )));
        }
    };
    let expected_abi_hash = ivm_abi::syscalls::compute_abi_hash(policy);
    if contract_interface.abi_hash != expected_abi_hash {
        return Err(ContractArtifactError::abi_hash_mismatch(
            expected_abi_hash,
            contract_interface.abi_hash,
        ));
    }
    Ok(contract_interface)
}
fn validate_koto_test_envelope(
    artifact: &[u8],
    parsed: &ParsedProgramMetadata,
    contract_interface: &EmbeddedContractInterfaceV1,
) -> Result<(), ContractArtifactError> {
    let metadata = &parsed.metadata;
    if metadata.version_major != 1 || metadata.version_minor != 0 {
        return Err(ContractArtifactError::invalid(format!(
            "expected generic IVM 1.0 Kotodama test harness, got {}.{}",
            metadata.version_major, metadata.version_minor
        )));
    }
    if metadata.mode & !(mode::ZK | mode::VECTOR) != 0 {
        return Err(ContractArtifactError::invalid(format!(
            "unsupported Kotodama test execution mode bits 0x{:02x}",
            metadata.mode
        )));
    }
    if metadata.vector_length != 0 {
        return Err(ContractArtifactError::invalid(
            "Kotodama test harness must use the compiler-owned default vector length",
        ));
    }
    if artifact.len() < HEADER_SIZE {
        return Err(ContractArtifactError::invalid(
            "artifact shorter than fixed IVM header",
        ));
    }
    let code_region_len = artifact
        .len()
        .checked_sub(parsed.header_len)
        .and_then(|len| u64::try_from(len).ok())
        .ok_or_else(|| ContractArtifactError::invalid("test harness image length is invalid"))?;
    if code_region_len > MAX_CONTRACT_IMAGE_BYTES {
        return Err(ContractArtifactError::invalid(
            "Kotodama test harness exceeds IVM code memory",
        ));
    }
    if parsed.contract_interface.is_some() {
        return Err(ContractArtifactError::invalid(
            "generic IVM 1.0 Kotodama test harness must not embed a CNTR section",
        ));
    }
    if parsed.contract_debug.is_some() {
        return Err(ContractArtifactError::invalid(
            "generic IVM 1.0 Kotodama test harness must not embed DBG1 metadata",
        ));
    }
    let expected_abi_hash = ivm_abi::syscalls::compute_abi_hash(SyscallPolicy::AbiV1);
    if contract_interface.abi_hash != expected_abi_hash {
        return Err(ContractArtifactError::abi_hash_mismatch(
            expected_abi_hash,
            contract_interface.abi_hash,
        ));
    }
    Ok(())
}
fn decode_instruction_stream(code: &[u8]) -> Result<Vec<DecodedOp>, ContractArtifactError> {
    if code.len() as u64 > MAX_CONTRACT_IMAGE_BYTES || !code.len().is_multiple_of(4) {
        return Err(ContractArtifactError::invalid(
            "instruction decode failed for executable stream: decode error",
        ));
    }
    let mut decoded = Vec::with_capacity(code.len() / 4);
    for (index, bytes) in code.chunks_exact(4).enumerate() {
        let pc = u64::try_from(index)
            .ok()
            .and_then(|index| index.checked_mul(4))
            .ok_or_else(|| ContractArtifactError::invalid("instruction pc overflows"))?;
        decoded.push(DecodedOp {
            pc,
            inst: u32::from_le_bytes(bytes.try_into().expect("four-byte instruction")),
        });
    }
    Ok(decoded)
}
mod decoded;
mod literal;
fn manifest_state_descriptors(states: &[EmbeddedStateDescriptor]) -> Vec<StateDescriptor> {
    states
        .iter()
        .map(|state| StateDescriptor {
            name: state.name.clone(),
            type_name: manifest_state_type_name(&state.ty),
        })
        .collect()
}
enum ManifestStateTypeFragment<'a> {
    Type {
        ty: &'a EmbeddedStateType,
        depth: usize,
    },
    Text(&'a str),
    Capacity(u8),
}
fn manifest_state_type_name(ty: &EmbeddedStateType) -> String {
    let mut output = String::new();
    let mut pending = vec![ManifestStateTypeFragment::Type { ty, depth: 1 }];
    while let Some(fragment) = pending.pop() {
        match fragment {
            ManifestStateTypeFragment::Text(text) => output.push_str(text),
            ManifestStateTypeFragment::Capacity(capacity) => {
                write!(&mut output, "{capacity}").expect("writing to a String cannot fail");
            }
            ManifestStateTypeFragment::Type { ty, depth } => {
                schedule_manifest_state_type_name(ty, depth, &mut output, &mut pending);
            }
        }
    }
    output
}
fn manifest_scalar_state_type_name(ty: &EmbeddedStateType) -> Option<&str> {
    match ty {
        EmbeddedStateType::Unit => Some("()"),
        EmbeddedStateType::Error(error) => Some(&error.identity),
        EmbeddedStateType::Int => Some("int"),
        EmbeddedStateType::Decimal => Some("decimal"),
        EmbeddedStateType::Quantity => Some("quantity"),
        EmbeddedStateType::Bool => Some("bool"),
        EmbeddedStateType::String => Some("string"),
        EmbeddedStateType::Bytes => Some("bytes"),
        EmbeddedStateType::DataSpaceId => Some("DataSpaceId"),
        EmbeddedStateType::AccountId => Some("AccountId"),
        EmbeddedStateType::AssetDefinitionId => Some("AssetDefinitionId"),
        EmbeddedStateType::AssetId => Some("AssetId"),
        EmbeddedStateType::NftId => Some("NftId"),
        EmbeddedStateType::DomainId => Some("DomainId"),
        EmbeddedStateType::Name => Some("Name"),
        EmbeddedStateType::Json => Some("Json"),
        EmbeddedStateType::Tuple(_)
        | EmbeddedStateType::Struct { .. }
        | EmbeddedStateType::StateMap { .. }
        | EmbeddedStateType::Option(_)
        | EmbeddedStateType::Result { .. }
        | EmbeddedStateType::List { .. }
        | EmbeddedStateType::StateCursor(_) => None,
    }
}
fn schedule_manifest_state_type_name<'a>(
    ty: &'a EmbeddedStateType,
    depth: usize,
    output: &mut String,
    pending: &mut Vec<ManifestStateTypeFragment<'a>>,
) {
    assert!(
        depth <= MAX_EMBEDDED_STATE_TYPE_DEPTH_V1,
        "validated embedded state type exceeds the V1 nesting limit"
    );
    if let Some(name) = manifest_scalar_state_type_name(ty) {
        output.push_str(name);
        return;
    }
    let child_depth = depth
        .checked_add(1)
        .expect("validated embedded state type depth cannot overflow");
    match ty {
        EmbeddedStateType::StateCursor(key) => {
            let schema = iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeV1 {
                nodes: vec![iroha_data_model::smart_contract::entrypoint::EntrypointValueTypeNodeV1::StateCursor(*key)],
            };
            output.push_str(
                &schema
                    .canonical_type_name()
                    .expect("validated cursor key type"),
            );
        }
        EmbeddedStateType::Tuple(items) => {
            output.push('(');
            pending.push(ManifestStateTypeFragment::Text(")"));
            for (index, item) in items.iter().enumerate().rev() {
                pending.push(ManifestStateTypeFragment::Type {
                    ty: item,
                    depth: child_depth,
                });
                if index != 0 {
                    pending.push(ManifestStateTypeFragment::Text(", "));
                }
            }
        }
        EmbeddedStateType::Struct { name, fields } => {
            output.push_str(name);
            output.push('{');
            pending.push(ManifestStateTypeFragment::Text("}"));
            for (index, field) in fields.iter().enumerate().rev() {
                pending.push(ManifestStateTypeFragment::Type {
                    ty: &field.ty,
                    depth: child_depth,
                });
                pending.push(ManifestStateTypeFragment::Text(": "));
                pending.push(ManifestStateTypeFragment::Text(&field.name));
                if index != 0 {
                    pending.push(ManifestStateTypeFragment::Text(", "));
                }
            }
        }
        EmbeddedStateType::StateMap { key, value } => {
            output.push_str("StateMap<");
            pending.push(ManifestStateTypeFragment::Text(">"));
            pending.push(ManifestStateTypeFragment::Type {
                ty: value,
                depth: child_depth,
            });
            pending.push(ManifestStateTypeFragment::Text(", "));
            pending.push(ManifestStateTypeFragment::Type {
                ty: key,
                depth: child_depth,
            });
        }
        EmbeddedStateType::Option(value) => {
            output.push_str("Option<");
            pending.push(ManifestStateTypeFragment::Text(">"));
            pending.push(ManifestStateTypeFragment::Type {
                ty: value,
                depth: child_depth,
            });
        }
        EmbeddedStateType::Result { ok, err } => {
            output.push_str("Result<");
            pending.push(ManifestStateTypeFragment::Text(">"));
            pending.push(ManifestStateTypeFragment::Type {
                ty: err,
                depth: child_depth,
            });
            pending.push(ManifestStateTypeFragment::Text(", "));
            pending.push(ManifestStateTypeFragment::Type {
                ty: ok,
                depth: child_depth,
            });
        }
        EmbeddedStateType::List { element, capacity } => {
            output.push_str("List<");
            pending.push(ManifestStateTypeFragment::Text(">"));
            pending.push(ManifestStateTypeFragment::Capacity(*capacity));
            pending.push(ManifestStateTypeFragment::Text(", "));
            pending.push(ManifestStateTypeFragment::Type {
                ty: element,
                depth: child_depth,
            });
        }
        _ => unreachable!("scalar embedded state types returned before compound formatting"),
    }
}
fn header_declares_contract_minor_one(artifact: &[u8]) -> bool {
    artifact.len() >= HEADER_SIZE && artifact[4] == 1 && artifact[5] == 1
}
fn cntr_section_missing(artifact: &[u8]) -> bool {
    artifact.len() < HEADER_SIZE + 4
        || artifact[HEADER_SIZE..HEADER_SIZE + 4]
            != ivm_abi::metadata::CONTRACT_INTERFACE_SECTION_MAGIC
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_data_model::smart_contract::manifest::EntryPointKind;
    use ivm_abi::{
        axt::{AxtDescriptor, AxtTouchSpec, ProofBlob},
        metadata::EmbeddedStateFieldDescriptor,
        pointer_abi::PointerType,
    };
    fn encoded(descriptor: &AxtDescriptor) -> Vec<u8> {
        norito::to_bytes(descriptor).expect("encode canonical AXT descriptor")
    }
    fn contract_artifact_with_state_type(ty: EmbeddedStateType) -> Vec<u8> {
        use ivm_abi::{encoding::wide as enc, instruction::wide};

        let entrypoint = EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: EntryPointKind::Kotoage,
            params: Vec::new(),
            argument_schema: None,
            return_type: Some("()".to_owned()),
            return_schema: Some(ivm_abi::entrypoint::EntrypointValueTypeV1 {
                nodes: vec![ivm_abi::entrypoint::EntrypointValueTypeNodeV1::Unit],
            }),
            permission: Some("Execute".to_owned()),
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        };
        let interface = EmbeddedContractInterfaceV1 {
            callables: vec![ivm_abi::call::EmbeddedCallableV1 {
                entry_pc: 0,
                frame_bytes: 0,
                arguments: ivm_abi::call::CallSchemaV1::empty(),
                results: ivm_abi::call::CallSchemaV1::unit(),
            }],
            seiyaku_name: "DeepManifest".to_owned(),
            compiler_fingerprint: "ivm-artifact-admission-tests".to_owned(),
            abi_hash: ivm_abi::syscalls::compute_abi_hash(SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![entrypoint],
            states: vec![EmbeddedStateDescriptor {
                name: "deep_state".to_owned(),
                ty,
            }],
            error_messages: Vec::new(),
            error_types: Vec::new(),
        };
        let mut artifact = ProgramMetadata::default().encode();
        artifact.extend_from_slice(&interface.encode_section());
        for word in [
            enc::encode_store(wide::memory::STORE64, 12, 0, 0),
            enc::encode_ri(wide::arithmetic::ADDI, 10, 12, 0),
            enc::encode_ri(wide::arithmetic::ADDI, 11, 0, 1),
            enc::encode_rr(wide::control::JALR, 0, 1, 0),
        ] {
            artifact.extend_from_slice(&word.to_le_bytes());
        }
        artifact
    }
    #[test]
    fn funded_artifact_admission_retains_policy_and_releases_temporary_instructions() {
        let artifact = contract_artifact_with_state_type(EmbeddedStateType::Bool);
        let baseline = verify_contract_artifact(&artifact).unwrap();
        let budget = iroha_allocation::AllocationBudget::new(0);
        let error = verify_contract_artifact_with_memory_budget(&artifact, &budget).unwrap_err();
        assert!(matches!(
            error.local_vm_error(),
            Some(VMError::AllocationDeferred(_))
        ));
        assert_eq!(budget.reserved_bytes(), 0);
        budget.set_limit_bytes(1024 * 1024);
        let verified = verify_contract_artifact_with_memory_budget(&artifact, &budget).unwrap();
        assert_eq!(verified.code_hash, baseline.code_hash);
        assert_eq!(verified.manifest, baseline.manifest);
        assert!(budget.peak_reserved_bytes() > 0);
        assert_eq!(
            budget.reserved_bytes(),
            0,
            "temporary decode backing was reclaimed"
        );
        let mut malformed = artifact;
        malformed[baseline.code_offset..baseline.code_offset + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        assert_eq!(
            verify_contract_artifact_with_memory_budget(&malformed, &budget).unwrap_err(),
            verify_contract_artifact(&malformed).unwrap_err(),
        );
        assert_eq!(
            budget.reserved_bytes(),
            0,
            "policy failure releases the admitted array"
        );
    }
    #[test]
    fn canonical_metadata_refusal_survives_artifact_admission_and_retries() {
        let artifact = contract_artifact_with_state_type(EmbeddedStateType::Bool);
        let original = artifact.clone();
        let baseline = verify_contract_artifact(&artifact).expect("valid canonical artifact");
        let refusal = norito::core::with_decode_limits_scope(
            norito::core::DecodeLimits::new(usize::MAX, usize::MAX, usize::MAX, 0, usize::MAX),
            || verify_contract_artifact(&artifact),
        )
        .expect_err("enclosing allocation refusal must reach admission");
        assert_eq!(
            refusal.into_vm_error(),
            VMError::ExecutionDeferred(ivm_abi::error::ExecutionDeferral::ActiveMemoryCapacity)
        );
        let retry =
            verify_contract_artifact(&artifact).expect("retry unchanged canonical artifact");
        assert_eq!(retry.code_hash, baseline.code_hash);
        assert_eq!(retry.abi_hash, baseline.abi_hash);
        assert_eq!(retry.contract_interface, baseline.contract_interface);
        assert_eq!(retry.manifest, baseline.manifest);
        assert_eq!(artifact, original);
    }

    #[test]
    fn manifest_state_type_names_preserve_variant_spelling_and_order() {
        let scalar_cases = [
            (EmbeddedStateType::Int, "int"),
            (EmbeddedStateType::Decimal, "decimal"),
            (EmbeddedStateType::Quantity, "quantity"),
            (EmbeddedStateType::Bool, "bool"),
            (EmbeddedStateType::String, "string"),
            (EmbeddedStateType::Bytes, "bytes"),
            (EmbeddedStateType::DataSpaceId, "DataSpaceId"),
            (EmbeddedStateType::AccountId, "AccountId"),
            (EmbeddedStateType::AssetDefinitionId, "AssetDefinitionId"),
            (EmbeddedStateType::AssetId, "AssetId"),
            (EmbeddedStateType::NftId, "NftId"),
            (EmbeddedStateType::DomainId, "DomainId"),
            (EmbeddedStateType::Name, "Name"),
            (EmbeddedStateType::Json, "Json"),
            (
                EmbeddedStateType::StateCursor(
                    iroha_data_model::smart_contract::entrypoint::EntrypointValueKindV1::Int,
                ),
                "StateCursor<int>",
            ),
        ];
        for (ty, expected) in scalar_cases {
            assert_eq!(manifest_state_type_name(&ty), expected);
        }
        let composite = EmbeddedStateType::Struct {
            name: "Envelope".to_owned(),
            fields: vec![
                EmbeddedStateFieldDescriptor {
                    name: "ordered_tuple".to_owned(),
                    ty: EmbeddedStateType::Tuple(vec![
                        EmbeddedStateType::Int,
                        EmbeddedStateType::Decimal,
                    ]),
                },
                EmbeddedStateFieldDescriptor {
                    name: "ordered_map".to_owned(),
                    ty: EmbeddedStateType::StateMap {
                        key: Box::new(EmbeddedStateType::Name),
                        value: Box::new(EmbeddedStateType::Result {
                            ok: Box::new(EmbeddedStateType::Option(Box::new(
                                EmbeddedStateType::Quantity,
                            ))),
                            err: Box::new(EmbeddedStateType::List {
                                element: Box::new(EmbeddedStateType::Bytes),
                                capacity: 64,
                            }),
                        }),
                    },
                },
            ],
        };
        assert_eq!(
            manifest_state_type_name(&composite),
            "Envelope{ordered_tuple: (int, decimal), ordered_map: StateMap<Name, Result<Option<quantity>, List<bytes, 64>>>}"
        );
    }
    #[test]
    fn depth_255_state_admission_and_manifest_formatting_are_stack_safe() {
        std::thread::Builder::new()
            .name("artifact-admission-manifest-depth-boundary".to_owned())
            .stack_size(128 * 1024)
            .spawn(|| {
                let wrappers = MAX_EMBEDDED_STATE_TYPE_DEPTH_V1 - 1;
                let ty = (0..wrappers).fold(EmbeddedStateType::Bool, |ty, _| {
                    EmbeddedStateType::Option(Box::new(ty))
                });
                let artifact = contract_artifact_with_state_type(ty);
                let verified = verify_contract_artifact(&artifact)
                    .expect("the exact state-type nesting budget must pass admission");
                let states = verified
                    .manifest
                    .states
                    .as_deref()
                    .expect("verified manifest retains its state descriptors");
                assert_eq!(states.len(), 1);
                assert_eq!(states[0].name, "deep_state");
                let mut expected = "Option<".repeat(wrappers);
                expected.push_str("bool");
                expected.push_str(&">".repeat(wrappers));
                assert_eq!(states[0].type_name, expected);
                assert_eq!(verified.contract_interface.states.len(), 1);
            })
            .expect("spawn constrained-stack artifact admission test")
            .join()
            .expect("depth-255 admission and formatting must not overflow the native stack");
    }
    #[test]
    fn axt_descriptor_literal_validation_matches_host_invariants() {
        let dsid = DataSpaceId::new(7);
        let other = DataSpaceId::new(11);
        let touch = AxtTouchSpec {
            dsid,
            read: vec!["orders".to_owned()],
            write: vec!["ledger".to_owned()],
        };
        let valid = AxtDescriptor {
            dsids: vec![dsid],
            touches: vec![touch.clone()],
        };
        assert_eq!(
            literal::validate_literal_payload(PointerType::AxtDescriptor, &encoded(&valid)),
            Ok(())
        );
        let invalid = [
            AxtDescriptor {
                dsids: Vec::new(),
                touches: Vec::new(),
            },
            AxtDescriptor {
                dsids: vec![dsid, dsid],
                touches: Vec::new(),
            },
            AxtDescriptor {
                dsids: vec![other],
                touches: vec![touch.clone()],
            },
            AxtDescriptor {
                dsids: vec![dsid],
                touches: vec![touch.clone(), touch],
            },
            AxtDescriptor {
                dsids: vec![other, dsid],
                touches: Vec::new(),
            },
            AxtDescriptor {
                dsids: vec![dsid, other],
                touches: vec![AxtTouchSpec {
                    dsid,
                    read: vec![String::new()],
                    write: Vec::new(),
                }],
            },
        ];
        for descriptor in invalid {
            assert_eq!(
                literal::validate_literal_payload(
                    PointerType::AxtDescriptor,
                    &encoded(&descriptor)
                ),
                Err(VMError::InvalidMetadata),
                "invalid descriptor must fail shared artifact admission: {descriptor:?}"
            );
        }
    }
    #[test]
    fn anchored_spend_literal_requires_one_canonical_statically_bound_wire() {
        let fixture: norito::json::Value = norito::json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../iroha_data_model/tests/fixtures/axt_envelope_multi_ds.json"
        )))
        .expect("current signed-spend fixture");
        let spend: AxtAnchoredSpendV1 =
            norito::json::from_value(fixture["spends"]["happy"][0].clone())
                .expect("signed-spend fixture");
        let canonical =
            ivm_abi::codec::encode_canonical_norito(&spend).expect("canonical signed-spend frame");
        assert_eq!(
            literal::validate_literal_payload(PointerType::AxtAnchoredSpendV1, &canonical),
            Ok(())
        );
        let mut malformed = spend;
        malformed.draft.amount = Some(iroha_data_model::prelude::Quantity::from(99_u64));
        let malformed =
            ivm_abi::codec::encode_canonical_norito(&malformed).expect("canonical malformed frame");
        assert_eq!(
            literal::validate_literal_payload(PointerType::AxtAnchoredSpendV1, &malformed),
            Err(VMError::InvalidMetadata)
        );
        assert_eq!(
            PointerType::from_u16(0x000C),
            None,
            "retired standalone handle pointer type is unassigned"
        );
    }
    #[test]
    fn proof_blob_literal_validation_rejects_empty_payload() {
        let valid_proof = ProofBlob {
            payload: vec![1],
            expiry_slot: None,
        };
        assert_eq!(
            literal::validate_literal_payload(PointerType::ProofBlob, &encoded_value(&valid_proof)),
            Ok(())
        );
        let empty_proof = ProofBlob {
            payload: Vec::new(),
            expiry_slot: None,
        };
        assert_eq!(
            literal::validate_literal_payload(PointerType::ProofBlob, &encoded_value(&empty_proof)),
            Err(VMError::InvalidMetadata)
        );
    }
    fn encoded_value<T: NoritoSerialize>(value: &T) -> Vec<u8> {
        norito::to_bytes(value).expect("encode canonical capability value")
    }
}
