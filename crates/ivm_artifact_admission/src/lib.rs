//! Canonical, host-independent admission for deployable IVM contract artifacts.
//!
//! This crate is the single policy implementation used by the native IVM and
//! browser WebAssembly. It deliberately depends on the stable `ivm_abi`
//! surface, not on the VM runtime, caches, proof systems, or host integrations.

use std::{error::Error as StdError, fmt};

use iroha_crypto::Hash;
use iroha_data_model::smart_contract::manifest::{ContractManifest, StateDescriptor};
use ivm_abi::{
    SyscallPolicy, VMError,
    metadata::{
        EmbeddedContractInterfaceV1, EmbeddedStateDescriptor, EmbeddedStateType, HEADER_SIZE,
        ParsedLiteralSection, ParsedProgramMetadata, ProgramMetadata, contract_code_hash, mode,
    },
};

mod policy;

/// Maximum executable-image bytes admitted by IVM code memory.
pub const MAX_CONTRACT_IMAGE_BYTES: u64 = 0x0010_0000;
/// Maximum browser input accepted by the exported WebAssembly boundary.
pub const MAX_BROWSER_ARTIFACT_BYTES: usize = 4 * 1024 * 1024;

/// One fixed-width decoded instruction in the executable stream.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DecodedOp {
    pub(crate) pc: u64,
    pub(crate) inst: u32,
    pub(crate) len: u32,
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

/// Stable failure returned when a deployable artifact is malformed or unsafe.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractArtifactError {
    message: String,
    abi_hash_mismatch: Option<([u8; 32], [u8; 32])>,
}

impl ContractArtifactError {
    /// Construct an ordinary admission error. Public for the native preparation
    /// adapter; artifact callers should receive errors from verification.
    #[doc(hidden)]
    pub fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: format!("invalid contract artifact: {}", message.into()),
            abi_hash_mismatch: None,
        }
    }

    /// Construct the ABI-descriptor mismatch variant.
    #[doc(hidden)]
    pub fn abi_hash_mismatch(expected: [u8; 32], actual: [u8; 32]) -> Self {
        Self {
            message: "invalid contract artifact: contract interface abi_hash does not match the runtime ABI descriptor".to_owned(),
            abi_hash_mismatch: Some((expected, actual)),
        }
    }

    /// Convert admission failure into the stable VM error surface.
    #[must_use]
    pub fn into_vm_error(self) -> VMError {
        self.abi_hash_mismatch
            .map_or(VMError::InvalidMetadata, |(expected, actual)| {
                VMError::ArtifactAbiHashMismatch { expected, actual }
            })
    }
}

impl fmt::Display for ContractArtifactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for ContractArtifactError {}

/// Verify a self-describing IVM 1.1 artifact and derive its canonical manifest.
pub fn verify_contract_artifact(
    artifact: &[u8],
) -> Result<VerifiedContractArtifact, ContractArtifactError> {
    let parsed = parse_contract_metadata(artifact)?;
    let contract_interface = validate_contract_envelope(artifact, &parsed)?;
    let code = artifact.get(parsed.code_offset..).ok_or_else(|| {
        ContractArtifactError::invalid("executable stream offset exceeds artifact length")
    })?;
    let decoded = decode_instruction_stream(code)?;
    policy::validate_contract_interface(
        &parsed.metadata,
        &contract_interface,
        &decoded,
        policy::ValidationProfile::Production,
    )?;
    validate_literal_table(artifact, &parsed, &decoded)?;

    Ok(verified_from_parts(artifact, parsed, contract_interface))
}

/// Verify a compiler-produced generic IVM 1.0 Kotodama test harness against
/// its compiler-owned interface sidecar.
///
/// This is intentionally hidden from ordinary artifact consumers. Native IVM
/// preparation uses it so production and local-test profiles still share one
/// policy implementation.
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
    validate_literal_table(artifact, &parsed, &decoded)?;

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
        .map(|entrypoint| entrypoint.to_manifest_descriptor())
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
        error_codes: (!contract_interface.error_codes.is_empty())
            .then_some(contract_interface.error_codes.clone()),
        kotoba: (!contract_interface.kotoba.is_empty()).then_some(contract_interface.kotoba.clone()),
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
        other => ContractArtifactError::invalid(format!("metadata parse failed: {other}")),
    })
}

fn validate_contract_envelope(
    artifact: &[u8],
    parsed: &ParsedProgramMetadata,
) -> Result<EmbeddedContractInterfaceV1, ContractArtifactError> {
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
        .clone()
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
            len: 4,
        });
    }
    Ok(decoded)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DecodedLiteral {
    Pointer,
    I64,
}

fn validate_literal_table(
    artifact: &[u8],
    parsed: &ParsedProgramMetadata,
    decoded: &[DecodedOp],
) -> Result<(), ContractArtifactError> {
    let literals = decode_literal_table(
        artifact,
        parsed.header_len,
        parsed.literal_section,
        SyscallPolicy::AbiV1,
    )
    .map_err(|error| {
        ContractArtifactError::invalid(format!("literal index validation failed: {error}"))
    })?;
    for op in decoded {
        let expects_i64 = match ivm_abi::instruction::wide::opcode(op.inst) {
            ivm_abi::instruction::wide::memory::LDLIT => Some(false),
            ivm_abi::instruction::wide::memory::LDI64 => Some(true),
            _ => None,
        };
        if let Some(expects_i64) = expects_i64 {
            let literal = literals
                .get(ivm_abi::instruction::wide::literal_index(op.inst))
                .ok_or_else(|| {
                    ContractArtifactError::invalid(
                        "literal instruction validation failed: invalid metadata",
                    )
                })?;
            if matches!(literal, DecodedLiteral::I64) != expects_i64 {
                return Err(ContractArtifactError::invalid(
                    "literal instruction validation failed: invalid metadata",
                ));
            }
        }
    }
    Ok(())
}

fn decode_literal_table(
    program: &[u8],
    header_len: usize,
    section: Option<ParsedLiteralSection>,
    policy: SyscallPolicy,
) -> Result<Vec<DecodedLiteral>, VMError> {
    use ivm_abi::metadata::{LiteralKindV1, decode_literal_descriptor};

    let Some(section) = section else {
        return Ok(Vec::new());
    };
    if section.count > usize::from(u16::MAX) + 1 {
        return Err(VMError::InvalidMetadata);
    }
    let mut descriptors = Vec::with_capacity(section.count);
    let mut previous_target = None;
    for index in 0..section.count {
        let entry_start = section
            .entries_start
            .checked_add(index.checked_mul(8).ok_or(VMError::InvalidMetadata)?)
            .ok_or(VMError::InvalidMetadata)?;
        let entry_end = entry_start.checked_add(8).ok_or(VMError::InvalidMetadata)?;
        let raw = u64::from_le_bytes(
            program
                .get(entry_start..entry_end)
                .ok_or(VMError::InvalidMetadata)?
                .try_into()
                .map_err(|_| VMError::InvalidMetadata)?,
        );
        let (kind, relative) = decode_literal_descriptor(raw)?;
        let target = section
            .start
            .checked_add(usize::try_from(relative).map_err(|_| VMError::InvalidMetadata)?)
            .ok_or(VMError::InvalidMetadata)?;
        if target < section.data_start || target >= section.data_end {
            return Err(VMError::InvalidMetadata);
        }
        if previous_target.is_some_and(|previous| target <= previous) {
            return Err(VMError::InvalidMetadata);
        }
        previous_target = Some(target);
        descriptors.push((kind, target));
    }
    if descriptors.is_empty() {
        return (section.data_start == section.data_end)
            .then(Vec::new)
            .ok_or(VMError::InvalidMetadata);
    }
    if descriptors.first().map(|(_, target)| *target) != Some(section.data_start) {
        return Err(VMError::InvalidMetadata);
    }
    let mut entries = Vec::with_capacity(descriptors.len());
    for (index, (kind, target)) in descriptors.iter().copied().enumerate() {
        let end = descriptors
            .get(index + 1)
            .map_or(section.data_end, |(_, target)| *target);
        let bytes = program.get(target..end).ok_or(VMError::InvalidMetadata)?;
        match kind {
            LiteralKindV1::PointerTlv => {
                let tlv = ivm_abi::pointer_abi::validate_tlv_bytes(bytes)
                    .map_err(|_| VMError::InvalidMetadata)?;
                let exact_len = 7usize
                    .checked_add(tlv.payload.len())
                    .and_then(|len| len.checked_add(iroha_crypto::Hash::LENGTH))
                    .ok_or(VMError::InvalidMetadata)?;
                if bytes.len() != exact_len
                    || !ivm_abi::pointer_abi::is_type_allowed_for_policy(policy, tlv.type_id)
                    || target.checked_sub(header_len).is_none()
                {
                    return Err(VMError::InvalidMetadata);
                }
                entries.push(DecodedLiteral::Pointer);
            }
            LiteralKindV1::I64 => {
                let _: [u8; 8] = bytes.try_into().map_err(|_| VMError::InvalidMetadata)?;
                entries.push(DecodedLiteral::I64);
            }
        }
    }
    Ok(entries)
}

fn manifest_state_descriptors(states: &[EmbeddedStateDescriptor]) -> Vec<StateDescriptor> {
    states
        .iter()
        .map(|state| StateDescriptor {
            name: state.name.clone(),
            type_name: manifest_state_type_name(&state.ty),
        })
        .collect()
}

fn manifest_state_type_name(ty: &EmbeddedStateType) -> String {
    match ty {
        EmbeddedStateType::Int => "int".to_owned(),
        EmbeddedStateType::Decimal => "decimal".to_owned(),
        EmbeddedStateType::Quantity => "quantity".to_owned(),
        EmbeddedStateType::Bool => "bool".to_owned(),
        EmbeddedStateType::String => "string".to_owned(),
        EmbeddedStateType::Bytes => "bytes".to_owned(),
        EmbeddedStateType::DataSpaceId => "DataSpaceId".to_owned(),
        EmbeddedStateType::AccountId => "AccountId".to_owned(),
        EmbeddedStateType::AssetDefinitionId => "AssetDefinitionId".to_owned(),
        EmbeddedStateType::AssetId => "AssetId".to_owned(),
        EmbeddedStateType::NftId => "NftId".to_owned(),
        EmbeddedStateType::DomainId => "DomainId".to_owned(),
        EmbeddedStateType::Name => "Name".to_owned(),
        EmbeddedStateType::Json => "Json".to_owned(),
        EmbeddedStateType::Tuple(items) => format!(
            "({})",
            items
                .iter()
                .map(manifest_state_type_name)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        EmbeddedStateType::Struct { name, fields } => format!(
            "{name}{{{}}}",
            fields
                .iter()
                .map(|field| format!("{}: {}", field.name, manifest_state_type_name(&field.ty)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        EmbeddedStateType::StateMap { key, value } => format!(
            "StateMap<{}, {}>",
            manifest_state_type_name(key),
            manifest_state_type_name(value)
        ),
        EmbeddedStateType::Option(value) => {
            format!("Option<{}>", manifest_state_type_name(value))
        }
        EmbeddedStateType::Result { ok, err } => format!(
            "Result<{}, {}>",
            manifest_state_type_name(ok),
            manifest_state_type_name(err)
        ),
        EmbeddedStateType::List { element, capacity } => {
            format!("List<{}, {capacity}>", manifest_state_type_name(element))
        }
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

/// Deterministic JSON admission result used by the raw browser-WASM boundary.
#[must_use]
pub fn verify_contract_artifact_json(artifact: &[u8]) -> String {
    match verify_contract_artifact(artifact) {
        Ok(verified) => {
            let manifest = norito::json::to_json(&verified.manifest)
                .expect("validated contract manifest must serialize");
            format!(
                "{{\"ok\":true,\"code_hash_hex\":\"{}\",\"abi_hash_hex\":\"{}\",\"header_len\":{},\"code_offset\":{},\"entrypoint_count\":{},\"manifest\":{manifest}}}",
                hex::encode(verified.code_hash.as_ref()),
                hex::encode(verified.abi_hash.as_ref()),
                verified.header_len,
                verified.code_offset,
                verified.contract_interface.entrypoints.len(),
            )
        }
        Err(error) => {
            let encoded = norito::json::to_json(&error.to_string())
                .expect("artifact error string must serialize");
            format!("{{\"ok\":false,\"error\":{encoded}}}")
        }
    }
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    static INPUT: OnceLock<Mutex<Box<[u8; MAX_BROWSER_ARTIFACT_BYTES]>>> = OnceLock::new();
    static OUTPUT: OnceLock<Mutex<Vec<u8>>> = OnceLock::new();

    fn input() -> &'static Mutex<Box<[u8; MAX_BROWSER_ARTIFACT_BYTES]>> {
        INPUT.get_or_init(|| Mutex::new(Box::new([0; MAX_BROWSER_ARTIFACT_BYTES])))
    }

    fn output() -> &'static Mutex<Vec<u8>> {
        OUTPUT.get_or_init(|| Mutex::new(Vec::new()))
    }

    /// Address of the fixed browser input buffer in exported linear memory.
    #[unsafe(no_mangle)]
    pub extern "C" fn iroha_ivm_artifact_admission_input_ptr() -> *mut u8 {
        input()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_mut_ptr()
    }

    /// Verify the first `len` input bytes and retain deterministic JSON output.
    #[unsafe(no_mangle)]
    pub extern "C" fn iroha_ivm_artifact_admission_verify(len: u32) -> u32 {
        let len = usize::try_from(len).unwrap_or(usize::MAX);
        let result = if len == 0 || len > MAX_BROWSER_ARTIFACT_BYTES {
            "{\"ok\":false,\"error\":\"invalid contract artifact: browser input length is outside 1..=4194304\"}".to_owned()
        } else {
            let input = input()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            verify_contract_artifact_json(&input[..len])
        };
        let success = result.starts_with("{\"ok\":true,");
        *output()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = result.into_bytes();
        u32::from(success)
    }

    /// Address of the retained JSON result in exported linear memory.
    #[unsafe(no_mangle)]
    pub extern "C" fn iroha_ivm_artifact_admission_output_ptr() -> *const u8 {
        output()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ptr()
    }

    /// Byte length of the retained JSON result.
    #[unsafe(no_mangle)]
    pub extern "C" fn iroha_ivm_artifact_admission_output_len() -> u32 {
        u32::try_from(
            output()
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .len(),
        )
        .expect("bounded admission JSON fits u32")
    }
}
