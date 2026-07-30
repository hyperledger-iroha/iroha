//! Canonical, host-independent admission for deployable IVM contract artifacts.
//!
//! This crate is the single policy implementation used by the native IVM and
//! browser WebAssembly. It deliberately depends on the stable `ivm_abi`
//! surface and canonical primitive codecs, not on the VM runtime, caches,
//! proof systems, or host integrations.

use std::{error::Error as StdError, fmt, fmt::Write as _};

use iroha_crypto::Hash;
use iroha_data_model::{
    account::AccountId,
    asset::id::{AssetDefinitionId, AssetId},
    domain::DomainId,
    name::Name,
    nexus::DataSpaceId,
    nft::NftId,
    prelude::{DecimalValueV1, IntValueV1, Json, QuantityValueV1},
    smart_contract::manifest::{ContractManifest, StateDescriptor},
    soracloud::{SoracloudHostRequestEnvelopeV1, SoracloudHostResponseEnvelopeV1},
};
use ivm_abi::{
    SyscallPolicy, VMError,
    axt::{
        AssetHandle, AxtDescriptor, ProofBlob, validate_asset_handle, validate_descriptor,
        validate_proof_blob,
    },
    codec::decode_canonical_norito,
    metadata::{
        EmbeddedContractInterfaceV1, EmbeddedEntrypointDescriptor, EmbeddedStateDescriptor,
        EmbeddedStateType, HEADER_SIZE, MAX_EMBEDDED_STATE_TYPE_DEPTH_V1, ParsedLiteralSection,
        ParsedProgramMetadata, ProgramMetadata, contract_code_hash, mode,
    },
};
#[cfg(test)]
use norito::NoritoSerialize;
use norito::codec::{Decode, Encode};

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
///
/// # Errors
///
/// Returns a stable admission error when metadata, the embedded contract
/// interface, bytecode policy, or literal-table bindings are invalid.
pub fn verify_contract_artifact(
    artifact: &[u8],
) -> Result<VerifiedContractArtifact, ContractArtifactError> {
    let mut parsed = parse_contract_metadata(artifact)?;
    let contract_interface = validate_contract_envelope(artifact, &parsed)?;
    let code = artifact.get(parsed.code_offset..).ok_or_else(|| {
        ContractArtifactError::invalid("executable stream offset exceeds artifact length")
    })?;
    let decoded = decode_instruction_stream(code)?;
    policy::validate_contract_interface(
        &parsed.metadata,
        contract_interface,
        &decoded,
        policy::ValidationProfile::Production,
    )?;
    validate_literal_table(artifact, &parsed, &decoded)?;
    let contract_interface = parsed
        .contract_interface
        .take()
        .expect("validated contract envelope retains its CNTR interface");

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
        error_codes: (!contract_interface.error_codes.is_empty())
            .then_some(contract_interface.error_codes.clone()),
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
                validate_literal_payload(tlv.type_id, tlv.payload)?;
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

fn decode_canonical_literal_payload<T>(payload: &[u8]) -> Result<T, VMError>
where
    T: Decode + Encode,
{
    decode_canonical_norito(payload).map_err(|_| VMError::InvalidMetadata)
}

fn validate_literal_payload(
    type_id: ivm_abi::pointer_abi::PointerType,
    payload: &[u8],
) -> Result<(), VMError> {
    use ivm_abi::pointer_abi::PointerType;

    // A literal pointer's nominal type is part of the authenticated artifact
    // contract. Validate every compiler-structured payload at admission rather
    // than deferring malformed frames to whichever syscall first consumes
    // them. Blob and NoritoBytes deliberately remain opaque byte containers.
    //
    // Keep codec details behind the same deterministic metadata failure used
    // for every malformed literal-table binding.
    match type_id {
        PointerType::AccountId => decode_canonical_literal_payload::<AccountId>(payload).map(drop),
        PointerType::AssetDefinitionId => {
            decode_canonical_literal_payload::<AssetDefinitionId>(payload).map(drop)
        }
        PointerType::Name => decode_canonical_literal_payload::<Name>(payload).map(drop),
        PointerType::Json => decode_canonical_literal_payload::<Json>(payload).map(drop),
        PointerType::NftId => decode_canonical_literal_payload::<NftId>(payload).map(drop),
        PointerType::Blob | PointerType::NoritoBytes => Ok(()),
        PointerType::AssetId => decode_canonical_literal_payload::<AssetId>(payload).map(drop),
        PointerType::DomainId => decode_canonical_literal_payload::<DomainId>(payload).map(drop),
        PointerType::DataSpaceId => {
            decode_canonical_literal_payload::<DataSpaceId>(payload).map(drop)
        }
        PointerType::AxtDescriptor => {
            let descriptor = decode_canonical_literal_payload::<AxtDescriptor>(payload)?;
            validate_descriptor(&descriptor).map_err(|_| VMError::InvalidMetadata)
        }
        PointerType::AssetHandle => {
            let handle = decode_canonical_literal_payload::<AssetHandle>(payload)?;
            validate_asset_handle(&handle).map_err(|_| VMError::InvalidMetadata)
        }
        PointerType::ProofBlob => {
            let proof = decode_canonical_literal_payload::<ProofBlob>(payload)?;
            validate_proof_blob(&proof).map_err(|_| VMError::InvalidMetadata)
        }
        PointerType::SoracloudRequest => {
            let request =
                decode_canonical_literal_payload::<SoracloudHostRequestEnvelopeV1>(payload)?;
            request.validate().map_err(|_| VMError::InvalidMetadata)
        }
        PointerType::SoracloudResponse => {
            let response =
                decode_canonical_literal_payload::<SoracloudHostResponseEnvelopeV1>(payload)?;
            response.validate().map_err(|_| VMError::InvalidMetadata)
        }
        PointerType::Int => IntValueV1::decode_frame(payload)
            .map(drop)
            .map_err(|_| VMError::InvalidMetadata),
        PointerType::Decimal => DecimalValueV1::decode_frame(payload)
            .map(drop)
            .map_err(|_| VMError::InvalidMetadata),
        PointerType::Quantity => QuantityValueV1::decode_frame(payload)
            .map(drop)
            .map_err(|_| VMError::InvalidMetadata),
    }
    .map_err(|_| VMError::InvalidMetadata)
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
    enum Fragment<'a> {
        Type {
            ty: &'a EmbeddedStateType,
            depth: usize,
        },
        Text(&'a str),
        Capacity(u8),
    }

    let mut output = String::new();
    let mut pending = vec![Fragment::Type { ty, depth: 1 }];
    while let Some(fragment) = pending.pop() {
        match fragment {
            Fragment::Text(text) => output.push_str(text),
            Fragment::Capacity(capacity) => {
                write!(&mut output, "{capacity}").expect("writing to a String cannot fail");
            }
            Fragment::Type { ty, depth } => {
                assert!(
                    depth <= MAX_EMBEDDED_STATE_TYPE_DEPTH_V1,
                    "validated embedded state type exceeds the V1 nesting limit"
                );
                let child_depth = depth
                    .checked_add(1)
                    .expect("validated embedded state type depth cannot overflow");
                match ty {
                    EmbeddedStateType::Int => output.push_str("int"),
                    EmbeddedStateType::Decimal => output.push_str("decimal"),
                    EmbeddedStateType::Quantity => output.push_str("quantity"),
                    EmbeddedStateType::Bool => output.push_str("bool"),
                    EmbeddedStateType::String => output.push_str("string"),
                    EmbeddedStateType::Bytes => output.push_str("bytes"),
                    EmbeddedStateType::DataSpaceId => output.push_str("DataSpaceId"),
                    EmbeddedStateType::AccountId => output.push_str("AccountId"),
                    EmbeddedStateType::AssetDefinitionId => {
                        output.push_str("AssetDefinitionId");
                    }
                    EmbeddedStateType::AssetId => output.push_str("AssetId"),
                    EmbeddedStateType::NftId => output.push_str("NftId"),
                    EmbeddedStateType::DomainId => output.push_str("DomainId"),
                    EmbeddedStateType::Name => output.push_str("Name"),
                    EmbeddedStateType::Json => output.push_str("Json"),
                    EmbeddedStateType::Tuple(items) => {
                        output.push('(');
                        pending.push(Fragment::Text(")"));
                        for (index, item) in items.iter().enumerate().rev() {
                            pending.push(Fragment::Type {
                                ty: item,
                                depth: child_depth,
                            });
                            if index != 0 {
                                pending.push(Fragment::Text(", "));
                            }
                        }
                    }
                    EmbeddedStateType::Struct { name, fields } => {
                        output.push_str(name);
                        output.push('{');
                        pending.push(Fragment::Text("}"));
                        for (index, field) in fields.iter().enumerate().rev() {
                            pending.push(Fragment::Type {
                                ty: &field.ty,
                                depth: child_depth,
                            });
                            pending.push(Fragment::Text(": "));
                            pending.push(Fragment::Text(&field.name));
                            if index != 0 {
                                pending.push(Fragment::Text(", "));
                            }
                        }
                    }
                    EmbeddedStateType::StateMap { key, value } => {
                        output.push_str("StateMap<");
                        pending.push(Fragment::Text(">"));
                        pending.push(Fragment::Type {
                            ty: value,
                            depth: child_depth,
                        });
                        pending.push(Fragment::Text(", "));
                        pending.push(Fragment::Type {
                            ty: key,
                            depth: child_depth,
                        });
                    }
                    EmbeddedStateType::Option(value) => {
                        output.push_str("Option<");
                        pending.push(Fragment::Text(">"));
                        pending.push(Fragment::Type {
                            ty: value,
                            depth: child_depth,
                        });
                    }
                    EmbeddedStateType::Result { ok, err } => {
                        output.push_str("Result<");
                        pending.push(Fragment::Text(">"));
                        pending.push(Fragment::Type {
                            ty: err,
                            depth: child_depth,
                        });
                        pending.push(Fragment::Text(", "));
                        pending.push(Fragment::Type {
                            ty: ok,
                            depth: child_depth,
                        });
                    }
                    EmbeddedStateType::List { element, capacity } => {
                        output.push_str("List<");
                        pending.push(Fragment::Text(">"));
                        pending.push(Fragment::Capacity(*capacity));
                        pending.push(Fragment::Text(", "));
                        pending.push(Fragment::Type {
                            ty: element,
                            depth: child_depth,
                        });
                    }
                }
            }
        }
    }
    output
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

#[cfg(test)]
mod tests {
    use iroha_data_model::{nexus::LaneId, smart_contract::manifest::EntryPointKind};
    use ivm_abi::{
        axt::{
            AssetHandle, AxtDescriptor, AxtTouchSpec, GroupBinding, HandleBudget, HandleSubject,
            ProofBlob,
        },
        metadata::EmbeddedStateFieldDescriptor,
        pointer_abi::PointerType,
    };

    use super::*;

    fn encoded(descriptor: &AxtDescriptor) -> Vec<u8> {
        norito::to_bytes(descriptor).expect("encode canonical AXT descriptor")
    }

    fn contract_artifact_with_state_type(ty: EmbeddedStateType) -> Vec<u8> {
        let entrypoint = EmbeddedEntrypointDescriptor {
            name: "main".to_owned(),
            kind: EntryPointKind::Kotoage,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: Some("Execute".to_owned()),
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
            entry_pc: 0,
        };
        let interface = EmbeddedContractInterfaceV1 {
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
            error_codes: Vec::new(),
        };
        let mut artifact = ProgramMetadata::default().encode();
        artifact.extend_from_slice(&interface.encode_section());
        artifact.extend_from_slice(&ivm_abi::encoding::encode_halt().to_le_bytes());
        artifact
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
            validate_literal_payload(PointerType::AxtDescriptor, &encoded(&valid)),
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
                validate_literal_payload(PointerType::AxtDescriptor, &encoded(&descriptor)),
                Err(VMError::InvalidMetadata),
                "invalid descriptor must fail shared artifact admission: {descriptor:?}"
            );
        }
    }

    #[test]
    fn capability_literal_validation_rejects_context_free_faults() {
        let valid_handle = AssetHandle {
            scope: vec!["transfer".to_owned()],
            subject: HandleSubject {
                account: "subject".to_owned(),
                origin_dsid: Some(DataSpaceId::new(7)),
            },
            budget: HandleBudget {
                remaining: "1".parse().expect("canonical quantity"),
                per_use: None,
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![1],
                epoch_id: 1,
            },
            target_lane: LaneId::new(0),
            axt_binding: vec![1; 32],
            manifest_view_root: vec![2; 32],
            expiry_slot: 1,
            max_clock_skew_ms: None,
        };
        let canonical_handle = encoded_value(&valid_handle);
        assert_eq!(
            validate_literal_payload(PointerType::AssetHandle, &canonical_handle),
            Ok(())
        );

        let alternate_flags =
            norito::core::default_encode_flags() ^ norito::core::header_flags::COMPACT_LEN;
        let alternate_handle = {
            let _alternate = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            encoded_value(&valid_handle)
        };
        assert_ne!(alternate_handle, canonical_handle);
        assert_eq!(
            norito::decode_from_bytes::<AssetHandle>(&alternate_handle)
                .expect("ordinary Norito accepts its advertised alternate layout"),
            valid_handle
        );
        assert_eq!(
            validate_literal_payload(PointerType::AssetHandle, &alternate_handle),
            Err(VMError::InvalidMetadata)
        );

        let mut malformed_handle = valid_handle.clone();
        malformed_handle.axt_binding.pop();
        assert_eq!(
            validate_literal_payload(PointerType::AssetHandle, &encoded_value(&malformed_handle)),
            Err(VMError::InvalidMetadata)
        );
        let mut unusable_handle = valid_handle.clone();
        unusable_handle.budget.per_use = Some("0".parse().expect("zero quantity"));
        assert_eq!(
            validate_literal_payload(PointerType::AssetHandle, &encoded_value(&unusable_handle)),
            Err(VMError::InvalidMetadata)
        );
        let mut malformed_handle = valid_handle.clone();
        malformed_handle.scope.push("transfer".to_owned());
        assert_eq!(
            validate_literal_payload(PointerType::AssetHandle, &encoded_value(&malformed_handle)),
            Err(VMError::InvalidMetadata)
        );
        let mut unusable_handle = valid_handle.clone();
        unusable_handle.subject.account.clear();
        assert_eq!(
            validate_literal_payload(PointerType::AssetHandle, &encoded_value(&unusable_handle)),
            Err(VMError::InvalidMetadata)
        );

        {
            let _ambient = norito::core::DecodeFlagsGuard::enter(alternate_flags);
            let ambient_before = encoded_value(&valid_handle);
            assert_eq!(
                validate_literal_payload(PointerType::AssetHandle, &canonical_handle),
                Ok(())
            );
            assert_eq!(
                encoded_value(&valid_handle),
                ambient_before,
                "admission must restore the caller's ambient Norito layout"
            );
        }

        let valid_proof = ProofBlob {
            payload: vec![1],
            expiry_slot: None,
        };
        assert_eq!(
            validate_literal_payload(PointerType::ProofBlob, &encoded_value(&valid_proof)),
            Ok(())
        );
        let empty_proof = ProofBlob {
            payload: Vec::new(),
            expiry_slot: None,
        };
        assert_eq!(
            validate_literal_payload(PointerType::ProofBlob, &encoded_value(&empty_proof)),
            Err(VMError::InvalidMetadata)
        );
    }

    fn encoded_value<T: NoritoSerialize>(value: &T) -> Vec<u8> {
        norito::to_bytes(value).expect("encode canonical capability value")
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
