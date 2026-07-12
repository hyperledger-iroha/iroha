use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error as StdError,
    fmt,
    sync::Arc,
};

use iroha_crypto::Hash;
use iroha_data_model::smart_contract::manifest::{
    AccessSetHints, ContractManifest, DynamicAccessHint, EntryPointKind, KotobaTranslationEntry,
    StateDescriptor,
};

use crate::{
    ProgramMetadata, SyscallPolicy,
    ivm::{
        decode_literal_table, prepare_instruction_stream, validate_indexed_literal_instructions,
    },
    ivm_cache::{DecodedOp, IvmCache},
    metadata::{
        CONTRACT_FEATURE_BIT_VECTOR, CONTRACT_FEATURE_BIT_ZK, CONTRACT_FEATURE_KNOWN_BITS,
        EmbeddedContractInterfaceV1, EmbeddedStateType, HEADER_SIZE, ParsedProgramMetadata,
        contract_code_hash, mode,
    },
    prepared::{PreparedContract, PreparedContractParts, PreparedControlFlow},
};

#[derive(Clone, Copy)]
enum ArtifactAdmissionPolicy {
    Production,
    KotodamaTest,
}

impl ArtifactAdmissionPolicy {
    fn allows_syscall(self, syscall_policy: SyscallPolicy, number: u32) -> bool {
        crate::syscalls::is_syscall_allowed(syscall_policy, number)
            || matches!(self, Self::KotodamaTest) && crate::syscalls::is_koto_test_syscall(number)
    }
}

/// Structurally validated contract artifact details derived from a self-describing `.to` image.
///
/// The compiler fingerprint is informational and is never treated as an
/// attestation of compiler provenance. Scheduler hints remain subject to
/// independent completeness and bytecode-safety checks.
#[derive(Clone, Debug)]
pub struct VerifiedContractArtifact {
    pub metadata: ProgramMetadata,
    pub header_len: usize,
    pub code_offset: usize,
    pub code_hash: Hash,
    pub abi_hash: Hash,
    pub contract_interface: EmbeddedContractInterfaceV1,
    pub manifest: ContractManifest,
}

/// Validation failure returned when a contract artifact is malformed or inconsistent.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContractArtifactError {
    message: String,
    abi_hash_mismatch: Option<([u8; 32], [u8; 32])>,
}

impl ContractArtifactError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: format!("invalid contract artifact: {}", message.into()),
            abi_hash_mismatch: None,
        }
    }

    fn abi_hash_mismatch(expected: [u8; 32], actual: [u8; 32]) -> Self {
        Self {
            message: "invalid contract artifact: embedded CNTR abi_hash does not match the runtime ABI descriptor".to_owned(),
            abi_hash_mismatch: Some((expected, actual)),
        }
    }

    /// Convert this artifact-admission failure into the stable VM error surface.
    #[must_use]
    pub fn into_vm_error(self) -> crate::VMError {
        self.abi_hash_mismatch
            .map_or(crate::VMError::InvalidMetadata, |(expected, actual)| {
                crate::VMError::ArtifactAbiHashMismatch { expected, actual }
            })
    }
}

impl fmt::Display for ContractArtifactError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl StdError for ContractArtifactError {}

/// Verify a self-describing contract artifact and derive the canonical on-chain manifest from it.
pub fn verify_contract_artifact(
    artifact: &[u8],
) -> Result<VerifiedContractArtifact, ContractArtifactError> {
    verify_contract_artifact_with_policy(artifact, ArtifactAdmissionPolicy::Production)
}

fn verify_contract_artifact_with_policy(
    artifact: &[u8],
    admission_policy: ArtifactAdmissionPolicy,
) -> Result<VerifiedContractArtifact, ContractArtifactError> {
    let parsed = parse_contract_metadata(artifact)?;
    let envelope = validate_contract_envelope(artifact, &parsed)?;
    let decoded = IvmCache::decode_stream(&artifact[parsed.code_offset..]).map_err(|err| {
        ContractArtifactError::invalid(format!(
            "instruction decode failed for executable stream: {err}"
        ))
    })?;
    let verified = verify_decoded_contract_artifact(
        artifact,
        &parsed,
        envelope,
        decoded.as_ref(),
        admission_policy,
    )?;
    let literal_table = decode_literal_table(
        artifact,
        parsed.header_len,
        parsed.literal_section,
        SyscallPolicy::AbiV1,
    )
    .map_err(|err| {
        ContractArtifactError::invalid(format!("literal index validation failed: {err}"))
    })?;
    validate_indexed_literal_instructions(decoded.as_ref(), literal_table.entries()).map_err(
        |err| {
            ContractArtifactError::invalid(format!("literal instruction validation failed: {err}"))
        },
    )?;
    Ok(verified)
}

/// Prepare a validated self-describing contract for repeated VM loading.
///
/// The input [`Arc`] becomes the immutable canonical artifact retained by the
/// returned contract, so callers that already own shared bytecode do not need
/// to copy it.
pub fn prepare_contract(artifact: Arc<[u8]>) -> Result<PreparedContract, ContractArtifactError> {
    PreparedContract::prepare(artifact)
}

pub(crate) fn prepare_kotodama_test_contract(
    artifact: Arc<[u8]>,
) -> Result<PreparedContract, ContractArtifactError> {
    PreparedContract::prepare_with_policy(artifact, ArtifactAdmissionPolicy::KotodamaTest)
}

impl PreparedContract {
    /// Parse, validate, index, and predecode a canonical deployable contract artifact.
    pub fn prepare(artifact: Arc<[u8]>) -> Result<Self, ContractArtifactError> {
        Self::prepare_with_policy(artifact, ArtifactAdmissionPolicy::Production)
    }

    fn prepare_with_policy(
        artifact: Arc<[u8]>,
        admission_policy: ArtifactAdmissionPolicy,
    ) -> Result<Self, ContractArtifactError> {
        let parsed = parse_contract_metadata(artifact.as_ref())?;
        let envelope = validate_contract_envelope(artifact.as_ref(), &parsed)?;
        let instruction_region = artifact.get(parsed.code_offset..).ok_or_else(|| {
            ContractArtifactError::invalid("executable stream offset exceeds artifact length")
        })?;
        let decoded =
            crate::ivm_cache::global_get_with_meta(instruction_region, &envelope.metadata)
                .map_err(|err| {
                    ContractArtifactError::invalid(format!(
                        "instruction decode failed for executable stream: {err}"
                    ))
                })?;
        let verified = verify_decoded_contract_artifact(
            artifact.as_ref(),
            &parsed,
            envelope,
            decoded.as_ref(),
            admission_policy,
        )?;
        let literal_table = decode_literal_table(
            artifact.as_ref(),
            parsed.header_len,
            parsed.literal_section,
            SyscallPolicy::AbiV1,
        )
        .map_err(|err| {
            ContractArtifactError::invalid(format!("literal index validation failed: {err}"))
        })?;
        validate_indexed_literal_instructions(decoded.as_ref(), literal_table.entries()).map_err(
            |err| {
                ContractArtifactError::invalid(format!(
                    "literal instruction validation failed: {err}"
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
        .map_err(|err| {
            ContractArtifactError::invalid(format!("instruction preparation failed: {err}"))
        })?;
        let control_flow = PreparedControlFlow::from_decoded(decoded.as_ref()).map_err(|err| {
            ContractArtifactError::invalid(format!("control-flow preparation failed: {err}"))
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
        .map_err(|err| ContractArtifactError::invalid(format!("contract indexing failed: {err}")))
    }
}

struct ValidatedContractEnvelope {
    metadata: ProgramMetadata,
    contract_interface: EmbeddedContractInterfaceV1,
}

fn parse_contract_metadata(
    artifact: &[u8],
) -> Result<ParsedProgramMetadata, ContractArtifactError> {
    ProgramMetadata::parse(artifact).map_err(|err| match err {
        crate::VMError::ArtifactAbiHashMismatch { expected, actual } => {
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
) -> Result<ValidatedContractEnvelope, ContractArtifactError> {
    let metadata = parsed.metadata.clone();
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
    if code_region_len > crate::memory::Memory::HEAP_START {
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
    let syscall_policy = match metadata.abi_version {
        1 => SyscallPolicy::AbiV1,
        other => {
            return Err(ContractArtifactError::invalid(format!(
                "unsupported abi_version {other}; expected 1"
            )));
        }
    };
    let expected_abi_hash = crate::syscalls::compute_abi_hash(syscall_policy);
    if contract_interface.abi_hash != expected_abi_hash {
        return Err(ContractArtifactError::abi_hash_mismatch(
            expected_abi_hash,
            contract_interface.abi_hash,
        ));
    }

    Ok(ValidatedContractEnvelope {
        metadata,
        contract_interface,
    })
}

fn verify_decoded_contract_artifact(
    artifact: &[u8],
    parsed: &ParsedProgramMetadata,
    envelope: ValidatedContractEnvelope,
    decoded: &[DecodedOp],
    admission_policy: ArtifactAdmissionPolicy,
) -> Result<VerifiedContractArtifact, ContractArtifactError> {
    validate_contract_interface(
        &envelope.metadata,
        &envelope.contract_interface,
        decoded,
        admission_policy,
    )?;

    let code_hash = contract_code_hash(artifact);
    let abi_hash = Hash::prehashed(envelope.contract_interface.abi_hash);
    let entrypoints = envelope
        .contract_interface
        .entrypoints
        .iter()
        .map(|entrypoint| entrypoint.to_manifest_descriptor())
        .collect::<Vec<_>>();
    let manifest = ContractManifest {
        seiyaku_name: Some(envelope.contract_interface.seiyaku_name.clone()),
        code_hash: Some(code_hash),
        abi_hash: Some(abi_hash),
        compiler_fingerprint: Some(envelope.contract_interface.compiler_fingerprint.clone()),
        features_bitmap: Some(envelope.contract_interface.features_bitmap),
        access_set_hints: envelope.contract_interface.access_set_hints.clone(),
        entrypoints: Some(entrypoints),
        states: Some(manifest_state_descriptors(
            &envelope.contract_interface.states,
        )),
        error_codes: (!envelope.contract_interface.error_codes.is_empty())
            .then_some(envelope.contract_interface.error_codes.clone()),
        kotoba: (!envelope.contract_interface.kotoba.is_empty())
            .then_some(envelope.contract_interface.kotoba.clone()),
        provenance: None,
    };

    Ok(VerifiedContractArtifact {
        metadata: envelope.metadata,
        header_len: parsed.header_len,
        code_offset: parsed.code_offset,
        code_hash,
        abi_hash,
        contract_interface: envelope.contract_interface,
        manifest,
    })
}

fn manifest_state_descriptors(
    states: &[crate::metadata::EmbeddedStateDescriptor],
) -> Vec<StateDescriptor> {
    states
        .iter()
        .map(|state| StateDescriptor {
            name: state.name.clone(),
            type_name: manifest_state_type_name(&state.ty),
        })
        .collect()
}

fn manifest_state_type_name(ty: &crate::metadata::EmbeddedStateType) -> String {
    use crate::metadata::EmbeddedStateType;

    match ty {
        EmbeddedStateType::Int => "int".to_string(),
        EmbeddedStateType::Decimal => "decimal".to_string(),
        EmbeddedStateType::Quantity => "quantity".to_string(),
        EmbeddedStateType::Bool => "bool".to_string(),
        EmbeddedStateType::String => "string".to_string(),
        EmbeddedStateType::Bytes => "bytes".to_string(),
        EmbeddedStateType::DataSpaceId => "DataSpaceId".to_string(),
        EmbeddedStateType::AccountId => "AccountId".to_string(),
        EmbeddedStateType::AssetDefinitionId => "AssetDefinitionId".to_string(),
        EmbeddedStateType::AssetId => "AssetId".to_string(),
        EmbeddedStateType::NftId => "NftId".to_string(),
        EmbeddedStateType::DomainId => "DomainId".to_string(),
        EmbeddedStateType::Name => "Name".to_string(),
        EmbeddedStateType::Json => "Json".to_string(),
        EmbeddedStateType::Tuple(items) => {
            let items = items
                .iter()
                .map(manifest_state_type_name)
                .collect::<Vec<_>>()
                .join(", ");
            format!("({items})")
        }
        EmbeddedStateType::Struct { name, fields } => {
            let fields = fields
                .iter()
                .map(|field| format!("{}: {}", field.name, manifest_state_type_name(&field.ty)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{name}{{{fields}}}")
        }
        EmbeddedStateType::StateMap { key, value } => {
            format!(
                "StateMap<{}, {}>",
                manifest_state_type_name(key),
                manifest_state_type_name(value)
            )
        }
        EmbeddedStateType::Option(value) => {
            format!("Option<{}>", manifest_state_type_name(value))
        }
        EmbeddedStateType::Result { ok, err } => {
            format!(
                "Result<{}, {}>",
                manifest_state_type_name(ok),
                manifest_state_type_name(err)
            )
        }
        EmbeddedStateType::List { element, capacity } => {
            format!("List<{}, {capacity}>", manifest_state_type_name(element))
        }
    }
}

fn validate_contract_interface(
    metadata: &ProgramMetadata,
    contract_interface: &EmbeddedContractInterfaceV1,
    decoded: &[DecodedOp],
    admission_policy: ArtifactAdmissionPolicy,
) -> Result<(), ContractArtifactError> {
    if !is_canonical_seiyaku_name(&contract_interface.seiyaku_name) {
        return Err(ContractArtifactError::invalid(
            "CNTR seiyaku_name must be a canonical Kotodama V1 identifier",
        ));
    }
    let fingerprint = contract_interface.compiler_fingerprint.trim();
    if fingerprint.is_empty() {
        return Err(ContractArtifactError::invalid(
            "CNTR compiler_fingerprint must not be empty",
        ));
    }

    let features_bitmap = contract_interface.features_bitmap;
    if features_bitmap & !CONTRACT_FEATURE_KNOWN_BITS != 0 {
        return Err(ContractArtifactError::invalid(format!(
            "CNTR features_bitmap contains unsupported bits 0x{:x}",
            features_bitmap & !CONTRACT_FEATURE_KNOWN_BITS
        )));
    }
    let zk_declared = features_bitmap & CONTRACT_FEATURE_BIT_ZK != 0;
    let vector_declared = features_bitmap & CONTRACT_FEATURE_BIT_VECTOR != 0;
    let zk_enabled = metadata.mode & mode::ZK != 0;
    let vector_enabled = metadata.mode & mode::VECTOR != 0;
    if zk_declared != zk_enabled {
        return Err(ContractArtifactError::invalid(
            "CNTR features_bitmap does not match metadata ZK mode",
        ));
    }
    if vector_declared != vector_enabled {
        return Err(ContractArtifactError::invalid(
            "CNTR features_bitmap does not match metadata VECTOR mode",
        ));
    }

    validate_access_set_hints(contract_interface.access_set_hints.as_ref())?;
    validate_kotoba_entries(&contract_interface.kotoba)?;
    validate_state_descriptors(contract_interface)?;
    validate_error_codes(contract_interface)?;

    if contract_interface.entrypoints.is_empty() {
        return Err(ContractArtifactError::invalid(
            "CNTR must declare at least one entrypoint",
        ));
    }

    validate_bytecode_security(decoded, zk_enabled, admission_policy)?;
    let valid_pcs = decoded.iter().map(|op| op.pc).collect::<BTreeSet<_>>();
    let mut entrypoint_names = BTreeSet::new();
    let mut entrypoint_kinds = BTreeMap::new();
    let mut entrypoint_pcs = BTreeSet::new();
    let mut entrypoint_reachability = BTreeMap::new();
    let mut hajimari_seen = false;
    let mut kaizen_seen = false;

    for entrypoint in &contract_interface.entrypoints {
        validate_entrypoint_name(&entrypoint.name)?;
        if !entrypoint_names.insert(entrypoint.name.clone()) {
            return Err(ContractArtifactError::invalid(format!(
                "duplicate entrypoint `{}`",
                entrypoint.name
            )));
        }
        entrypoint_kinds.insert(entrypoint.name.clone(), entrypoint.kind);
        if !valid_pcs.contains(&entrypoint.entry_pc) {
            return Err(ContractArtifactError::invalid(format!(
                "entrypoint `{}` has invalid entry_pc {}",
                entrypoint.name, entrypoint.entry_pc
            )));
        }
        if !entrypoint_pcs.insert(entrypoint.entry_pc) {
            return Err(ContractArtifactError::invalid(format!(
                "entrypoint `{}` reuses entry_pc {}",
                entrypoint.name, entrypoint.entry_pc
            )));
        }
        let reachability =
            reachable_syscalls(decoded, entrypoint.entry_pc, entrypoint.name.as_str())?;
        match (&entrypoint.params[..], entrypoint.argument_schema.as_ref()) {
            ([], None) => {}
            ([], Some(_)) => {
                return Err(ContractArtifactError::invalid(format!(
                    "zero-parameter entrypoint `{}` must not declare an argument schema",
                    entrypoint.name
                )));
            }
            (params, Some(schema))
                if schema.validate()
                    && schema.fields.len() == params.len()
                    && schema.fields.iter().zip(params).all(|(field, param)| {
                        field.name == param.name
                            && field.ty.canonical_type_name().as_deref()
                                == Some(param.type_name.as_str())
                    })
                    && reachability
                        .syscalls
                        .contains(&crate::syscalls::SYSCALL_DECODE_ARGUMENT_RECORD) => {}
            (_, Some(_)) => {
                return Err(ContractArtifactError::invalid(format!(
                    "entrypoint `{}` has an invalid argument schema or does not decode it",
                    entrypoint.name
                )));
            }
            (_, None) => {
                return Err(ContractArtifactError::invalid(format!(
                    "parameterized entrypoint `{}` is missing its argument schema",
                    entrypoint.name
                )));
            }
        }
        match (
            entrypoint.return_type.as_deref(),
            entrypoint.return_schema.as_ref(),
        ) {
            (None, None) => {}
            (Some(type_name), Some(schema))
                if schema.validate()
                    && schema.canonical_type_name().as_deref() == Some(type_name)
                    && schema.word_count().is_some_and(|words| {
                        words <= ivm_abi::entrypoint::MAX_ENTRYPOINT_RETURN_WORDS
                    }) => {}
            (Some(_), Some(_)) => {
                return Err(ContractArtifactError::invalid(format!(
                    "entrypoint `{}` has a return schema that does not match its declared type or exceeds the ABI v1 public register window",
                    entrypoint.name
                )));
            }
            (Some(_), None) => {
                return Err(ContractArtifactError::invalid(format!(
                    "entrypoint `{}` is missing its exact return schema",
                    entrypoint.name
                )));
            }
            (None, Some(_)) => {
                return Err(ContractArtifactError::invalid(format!(
                    "unit entrypoint `{}` must not declare a return schema",
                    entrypoint.name
                )));
            }
        }
        if entrypoint.kind == EntryPointKind::View {
            validate_view_effects(&entrypoint.name, &reachability.syscalls)?;
        }
        if entrypoint.kind == EntryPointKind::Kotoage && entrypoint.permission.is_none() {
            return Err(ContractArtifactError::invalid(format!(
                "`kotoage`/`言挙げ` entrypoint `{}` is missing caller authorization",
                entrypoint.name
            )));
        }
        if matches!(
            entrypoint.kind,
            EntryPointKind::Hajimari | EntryPointKind::Kaizen
        ) && entrypoint.permission.is_some()
        {
            return Err(ContractArtifactError::invalid(format!(
                "`hajimari`/`始まり` and `kaizen`/`改善` entrypoint `{}` must use runtime-defined authorization",
                entrypoint.name
            )));
        }
        let canonical_lifecycle_kind = match entrypoint.name.as_str() {
            "hajimari" | "始まり" => Some(EntryPointKind::Hajimari),
            "kaizen" | "改善" => Some(EntryPointKind::Kaizen),
            _ => None,
        };
        match (entrypoint.kind, canonical_lifecycle_kind) {
            (EntryPointKind::Hajimari, Some(EntryPointKind::Hajimari))
            | (EntryPointKind::Kaizen, Some(EntryPointKind::Kaizen))
            | (EntryPointKind::Kotoage | EntryPointKind::View, None) => {}
            (EntryPointKind::Hajimari, _) => {
                return Err(ContractArtifactError::invalid(format!(
                    "hajimari/始まり entrypoint `{}` must use the reserved `hajimari` or `始まり` selector",
                    entrypoint.name
                )));
            }
            (EntryPointKind::Kaizen, _) => {
                return Err(ContractArtifactError::invalid(format!(
                    "kaizen/改善 entrypoint `{}` must use the reserved `kaizen` or `改善` selector",
                    entrypoint.name
                )));
            }
            (EntryPointKind::Kotoage | EntryPointKind::View, Some(_)) => {
                return Err(ContractArtifactError::invalid(format!(
                    "reserved hajimari/始まり or kaizen/改善 selector `{}` has the wrong entrypoint kind",
                    entrypoint.name
                )));
            }
        }
        if let Some(permission) = entrypoint.permission.as_deref()
            && permission.trim().is_empty()
        {
            return Err(ContractArtifactError::invalid(format!(
                "entrypoint `{}` has an empty permission hint",
                entrypoint.name
            )));
        }
        validate_access_keys(&entrypoint.name, "read_keys", &entrypoint.read_keys)?;
        validate_access_keys(&entrypoint.name, "write_keys", &entrypoint.write_keys)?;
        for reason in &entrypoint.access_hints_skipped {
            if reason.trim().is_empty() {
                return Err(ContractArtifactError::invalid(format!(
                    "entrypoint `{}` contains an empty access_hints_skipped reason",
                    entrypoint.name
                )));
            }
        }
        match entrypoint.access_hints_complete {
            Some(true) if !entrypoint.access_hints_skipped.is_empty() => {
                return Err(ContractArtifactError::invalid(format!(
                    "entrypoint `{}` marks access hints complete but records skipped reasons",
                    entrypoint.name
                )));
            }
            Some(false) if entrypoint.access_hints_skipped.is_empty() => {
                return Err(ContractArtifactError::invalid(format!(
                    "entrypoint `{}` marks access hints incomplete without a reason",
                    entrypoint.name
                )));
            }
            _ => {}
        }
        validate_entrypoint_access_claims(entrypoint, &reachability.syscalls)?;
        match entrypoint.kind {
            EntryPointKind::Hajimari if hajimari_seen => {
                return Err(ContractArtifactError::invalid(
                    "CNTR declares more than one hajimari entrypoint",
                ));
            }
            EntryPointKind::Hajimari => hajimari_seen = true,
            EntryPointKind::Kaizen if kaizen_seen => {
                return Err(ContractArtifactError::invalid(
                    "CNTR declares more than one kaizen entrypoint",
                ));
            }
            EntryPointKind::Kaizen => kaizen_seen = true,
            EntryPointKind::Kotoage | EntryPointKind::View => {}
        }
        entrypoint_reachability.insert(entrypoint.name.clone(), reachability);
    }

    // Entrypoint authorization is enforced by dispatch metadata, not by code at
    // the target PC. Raw control flow into a distinct entrypoint would bypass
    // that target's `authorize` permission or its runtime-defined lifecycle
    // authorization. Shared implementation must therefore live in private
    // helpers; cross-contract calls use the reauthorizing host boundary.
    for caller in &contract_interface.entrypoints {
        let reachability = entrypoint_reachability
            .get(&caller.name)
            .expect("validated entrypoint reachability is retained");
        for target in &contract_interface.entrypoints {
            if caller.name != target.name && reachability.pcs.contains(&target.entry_pc) {
                return Err(ContractArtifactError::invalid(format!(
                    "entrypoint `{}` reaches distinct entrypoint `{}` at pc {}; raw cross-entrypoint control flow bypasses dispatch authorization",
                    caller.name, target.name, target.entry_pc
                )));
            }
        }
    }

    let mut trigger_ids = BTreeSet::new();
    for entrypoint in &contract_interface.entrypoints {
        for trigger in &entrypoint.triggers {
            if !trigger_ids.insert(trigger.id.clone()) {
                return Err(ContractArtifactError::invalid(format!(
                    "duplicate trigger `{}`",
                    trigger.id
                )));
            }
            if let Some(namespace) = trigger.callback.namespace.as_deref()
                && namespace.trim().is_empty()
            {
                return Err(ContractArtifactError::invalid(format!(
                    "entrypoint `{}` has a trigger with an empty callback namespace",
                    entrypoint.name
                )));
            }
            validate_entrypoint_name(&trigger.callback.entrypoint)?;
            if trigger.callback.namespace.is_none() {
                let Some(kind) = entrypoint_kinds.get(&trigger.callback.entrypoint) else {
                    return Err(ContractArtifactError::invalid(format!(
                        "trigger `{}` callback target `{}` is not a declared entrypoint",
                        trigger.id, trigger.callback.entrypoint
                    )));
                };
                if *kind != EntryPointKind::Kotoage {
                    return Err(ContractArtifactError::invalid(format!(
                        "trigger `{}` callback target `{}` must be a `kotoage`/`言挙げ` entrypoint",
                        trigger.id, trigger.callback.entrypoint
                    )));
                }
            }
        }
    }

    Ok(())
}

fn is_canonical_ascii_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    matches!(chars.next(), Some(first) if first == '_' || first.is_ascii_alphabetic())
        && chars.all(|ch| ch == '_' || ch.is_ascii_alphanumeric())
}

fn is_canonical_source_identifier(name: &str) -> bool {
    is_canonical_ascii_identifier(name) && !kotodama_lang::lexer::V1_KEYWORDS.contains(&name)
}

fn is_canonical_source_declaration_name(name: &str, is_function: bool) -> bool {
    is_canonical_source_identifier(name)
        && !kotodama_lang::semantic::is_reserved_source_declaration(name, is_function)
}

fn is_canonical_entrypoint_name(name: &str) -> bool {
    matches!(name, "hajimari" | "始まり" | "kaizen" | "改善")
        || is_canonical_source_declaration_name(name, true)
}

fn is_canonical_seiyaku_name(name: &str) -> bool {
    is_canonical_source_declaration_name(name, false)
}

fn validate_bytecode_security(
    decoded: &[crate::ivm_cache::DecodedOp],
    zk_enabled: bool,
    admission_policy: ArtifactAdmissionPolicy,
) -> Result<(), ContractArtifactError> {
    use crate::instruction::wide;

    let instruction_boundaries = decoded.iter().map(|op| op.pc).collect::<BTreeSet<_>>();
    for op in decoded {
        let opcode = wide::opcode(op.inst);
        if opcode == wide::control::JR {
            return Err(ContractArtifactError::invalid(format!(
                "unverifiable indirect control flow at pc {}",
                op.pc
            )));
        }
        if opcode == wide::control::JALR {
            let (_, rd, base, immediate) = crate::encoding::wide::decode_rr(op.inst);
            if !(rd == 0 && base == 1 && immediate == 0) {
                return Err(ContractArtifactError::invalid(format!(
                    "unverifiable indirect control flow at pc {}",
                    op.pc
                )));
            }
        }
        if opcode == wide::control::JAL && !matches!(wide::rd(op.inst), 0 | 1) {
            return Err(ContractArtifactError::invalid(format!(
                "direct call at pc {} uses unsupported link register r{}",
                op.pc,
                wide::rd(op.inst)
            )));
        }
        let syscall = decoded_syscall_number(op.inst);
        if syscall == Some(crate::syscalls::SYSCALL_GET_PRIVATE_INPUT) && !zk_enabled {
            return Err(ContractArtifactError::invalid(format!(
                "private-input syscall at pc {} requires ZK execution mode",
                op.pc
            )));
        }
        if let Some(number) = syscall
            && !admission_policy.allows_syscall(SyscallPolicy::AbiV1, number)
        {
            return Err(ContractArtifactError::invalid(format!(
                "disallowed syscall 0x{number:06x} at pc {}",
                op.pc
            )));
        }
        let offset_words = match opcode {
            wide::control::BEQ
            | wide::control::BNE
            | wide::control::BLT
            | wide::control::BGE
            | wide::control::BLTU
            | wide::control::BGEU => i64::from(wide::imm8(op.inst)),
            wide::control::JAL => i64::from(wide::imm16(op.inst)),
            wide::control::JMP | wide::control::JALS => i64::from(wide::imm24(op.inst)),
            _ => continue,
        };
        let Some(byte_offset) = offset_words.checked_mul(4) else {
            return Err(ContractArtifactError::invalid(format!(
                "control-flow offset overflows at pc {}",
                op.pc
            )));
        };
        let Some(target) = i128::from(op.pc)
            .checked_add(i128::from(byte_offset))
            .and_then(|target| u64::try_from(target).ok())
        else {
            return Err(ContractArtifactError::invalid(format!(
                "control-flow target is outside the executable stream at pc {}",
                op.pc
            )));
        };
        if !instruction_boundaries.contains(&target) {
            return Err(ContractArtifactError::invalid(format!(
                "control-flow target {target} from pc {} is not an instruction boundary",
                op.pc
            )));
        }

        let requires_fallthrough = matches!(
            opcode,
            wide::control::BEQ
                | wide::control::BNE
                | wide::control::BLT
                | wide::control::BGE
                | wide::control::BLTU
                | wide::control::BGEU
                | wide::control::JALS
        ) || (opcode == wide::control::JAL && wide::rd(op.inst) != 0);
        if requires_fallthrough {
            let fallthrough = op.pc.checked_add(u64::from(op.len)).ok_or_else(|| {
                ContractArtifactError::invalid(format!(
                    "control-flow fallthrough overflows at pc {}",
                    op.pc
                ))
            })?;
            if !instruction_boundaries.contains(&fallthrough) {
                return Err(ContractArtifactError::invalid(format!(
                    "control-flow fallthrough {fallthrough} from pc {} is not an instruction boundary",
                    op.pc
                )));
            }
        }
    }
    Ok(())
}

fn decoded_syscall_number(instruction: u32) -> Option<u32> {
    use crate::instruction::wide;

    match wide::opcode(instruction) {
        wide::system::SCALL => Some(u32::from(wide::imm8(instruction) as u8)),
        wide::system::SYSTEM => Some(crate::encoding::wide::decode_syscallx(instruction)),
        _ => None,
    }
}

fn direct_control_flow_target(op: &crate::ivm_cache::DecodedOp) -> Option<u64> {
    use crate::instruction::wide;

    let offset_words = match wide::opcode(op.inst) {
        wide::control::BEQ
        | wide::control::BNE
        | wide::control::BLT
        | wide::control::BGE
        | wide::control::BLTU
        | wide::control::BGEU => i64::from(wide::imm8(op.inst)),
        wide::control::JAL => i64::from(wide::imm16(op.inst)),
        wide::control::JMP | wide::control::JALS => i64::from(wide::imm24(op.inst)),
        _ => return None,
    };
    let byte_offset = offset_words.checked_mul(4)?;
    i128::from(op.pc)
        .checked_add(i128::from(byte_offset))
        .and_then(|target| u64::try_from(target).ok())
}

struct Reachability {
    syscalls: BTreeSet<u32>,
    pcs: BTreeSet<u64>,
}

fn reachable_syscalls(
    decoded: &[crate::ivm_cache::DecodedOp],
    entry_pc: u64,
    entrypoint_name: &str,
) -> Result<Reachability, ContractArtifactError> {
    use crate::instruction::wide;

    let instructions = decoded
        .iter()
        .map(|op| (op.pc, op))
        .collect::<BTreeMap<_, _>>();
    let mut pending = VecDeque::from([entry_pc]);
    let mut visited = BTreeSet::new();
    let mut syscalls = BTreeSet::new();

    while let Some(pc) = pending.pop_front() {
        if !visited.insert(pc) {
            continue;
        }
        let op = instructions.get(&pc).ok_or_else(|| {
            ContractArtifactError::invalid(format!(
                "entrypoint `{entrypoint_name}` reaches non-instruction pc {pc}"
            ))
        })?;
        if let Some(number) = decoded_syscall_number(op.inst) {
            syscalls.insert(number);
        }

        let opcode = wide::opcode(op.inst);
        let fallthrough = op.pc.checked_add(u64::from(op.len));
        match opcode {
            wide::control::HALT => {}
            wide::control::BEQ
            | wide::control::BNE
            | wide::control::BLT
            | wide::control::BGE
            | wide::control::BLTU
            | wide::control::BGEU => {
                pending.push_back(direct_control_flow_target(op).ok_or_else(|| {
                    ContractArtifactError::invalid(format!(
                        "entrypoint `{entrypoint_name}` has an invalid branch at pc {}",
                        op.pc
                    ))
                })?);
                pending.push_back(fallthrough.ok_or_else(|| {
                    ContractArtifactError::invalid("control-flow fallthrough overflows")
                })?);
            }
            wide::control::JAL => {
                pending.push_back(direct_control_flow_target(op).ok_or_else(|| {
                    ContractArtifactError::invalid(format!(
                        "entrypoint `{entrypoint_name}` has an invalid jump at pc {}",
                        op.pc
                    ))
                })?);
                if wide::rd(op.inst) != 0 {
                    pending.push_back(fallthrough.ok_or_else(|| {
                        ContractArtifactError::invalid("call fallthrough overflows")
                    })?);
                }
            }
            wide::control::JMP => {
                pending.push_back(direct_control_flow_target(op).ok_or_else(|| {
                    ContractArtifactError::invalid(format!(
                        "entrypoint `{entrypoint_name}` has an invalid jump at pc {}",
                        op.pc
                    ))
                })?);
            }
            wide::control::JALS => {
                pending.push_back(direct_control_flow_target(op).ok_or_else(|| {
                    ContractArtifactError::invalid(format!(
                        "entrypoint `{entrypoint_name}` has an invalid call at pc {}",
                        op.pc
                    ))
                })?);
                pending.push_back(
                    fallthrough.ok_or_else(|| {
                        ContractArtifactError::invalid("call fallthrough overflows")
                    })?,
                );
            }
            wide::control::JALR => {
                let (_, rd, base, immediate) = crate::encoding::wide::decode_rr(op.inst);
                if !(rd == 0 && base == 1 && immediate == 0) {
                    return Err(ContractArtifactError::invalid(format!(
                        "entrypoint `{entrypoint_name}` reaches unverifiable indirect control flow at pc {}",
                        op.pc
                    )));
                }
                // Deployable contracts execute with IVM return-address
                // integrity enabled. A canonical return therefore terminates
                // this static path; its dynamic target must match the protected
                // direct-call stack (or a trusted outer-invocation sentinel).
            }
            wide::control::JR => {
                return Err(ContractArtifactError::invalid(format!(
                    "entrypoint `{entrypoint_name}` reaches unverifiable indirect control flow at pc {}",
                    op.pc
                )));
            }
            _ => {
                pending.push_back(fallthrough.ok_or_else(|| {
                    ContractArtifactError::invalid("control-flow fallthrough overflows")
                })?);
            }
        }
    }
    Ok(Reachability {
        syscalls,
        pcs: visited,
    })
}

fn validate_view_effects(
    entrypoint_name: &str,
    syscalls: &BTreeSet<u32>,
) -> Result<(), ContractArtifactError> {
    for number in syscalls {
        if matches!(
            crate::syscalls::syscall_access(*number),
            crate::syscalls::SyscallAccess::StateWrite
                | crate::syscalls::SyscallAccess::LedgerWrite
                | crate::syscalls::SyscallAccess::Dynamic
        ) {
            return Err(ContractArtifactError::invalid(format!(
                "view entrypoint `{entrypoint_name}` transitively reaches effectful syscall 0x{number:06x}"
            )));
        }
    }
    Ok(())
}

fn validate_entrypoint_access_claims(
    entrypoint: &crate::metadata::EmbeddedEntrypointDescriptor,
    syscalls: &BTreeSet<u32>,
) -> Result<(), ContractArtifactError> {
    if entrypoint.access_hints_complete != Some(true) {
        return Ok(());
    }

    // Bytecode admission can prove the coarse access class but not the concrete
    // host key carried in a runtime register. Reject cross-class omissions here;
    // exact keys remain advisory and the scheduler must independently prove
    // them or select its conservative wildcard fence.
    for number in syscalls {
        let access = crate::syscalls::syscall_access(*number);
        if !entrypoint_claim_covers_access(entrypoint, access) {
            return Err(ContractArtifactError::invalid(format!(
                "entrypoint `{}` marks access hints complete but under-reports transitively reachable {access:?} syscall 0x{number:06x}",
                entrypoint.name
            )));
        }
    }
    Ok(())
}

fn entrypoint_claim_covers_access(
    entrypoint: &crate::metadata::EmbeddedEntrypointDescriptor,
    access: crate::syscalls::SyscallAccess,
) -> bool {
    use crate::syscalls::SyscallAccess;

    let is_state_key = |key: &str| key.starts_with("state:");
    let global_read = entrypoint.read_keys.iter().any(|key| key == "*")
        || entrypoint.write_keys.iter().any(|key| key == "*");
    let global_write = entrypoint.write_keys.iter().any(|key| key == "*");
    let state_read = entrypoint.read_keys.iter().any(|key| is_state_key(key))
        || entrypoint.write_keys.iter().any(|key| is_state_key(key));
    let state_write = entrypoint.write_keys.iter().any(|key| is_state_key(key));
    let ledger_read = entrypoint
        .read_keys
        .iter()
        .chain(&entrypoint.write_keys)
        .any(|key| key != "*" && !is_state_key(key));
    let ledger_write = entrypoint
        .write_keys
        .iter()
        .any(|key| key != "*" && !is_state_key(key));

    match access {
        SyscallAccess::None => true,
        SyscallAccess::StateRead => global_read || state_read,
        SyscallAccess::StateWrite => global_write || state_write,
        SyscallAccess::LedgerRead => global_read || ledger_read,
        SyscallAccess::LedgerWrite => global_write || ledger_write,
        SyscallAccess::Dynamic => global_write,
    }
}

fn validate_access_set_hints(
    access_set_hints: Option<&AccessSetHints>,
) -> Result<(), ContractArtifactError> {
    let Some(access_set_hints) = access_set_hints else {
        return Ok(());
    };
    validate_access_keys(
        "contract",
        "access_set_hints.read_keys",
        &access_set_hints.read_keys,
    )?;
    validate_access_keys(
        "contract",
        "access_set_hints.write_keys",
        &access_set_hints.write_keys,
    )?;
    validate_dynamic_access_hints(
        "contract",
        "access_set_hints.dynamic_reads",
        &access_set_hints.dynamic_reads,
    )?;
    validate_dynamic_access_hints(
        "contract",
        "access_set_hints.dynamic_writes",
        &access_set_hints.dynamic_writes,
    )?;
    Ok(())
}

fn validate_access_keys(
    owner: &str,
    field: &str,
    keys: &[String],
) -> Result<(), ContractArtifactError> {
    for key in keys {
        if key.trim().is_empty() {
            return Err(ContractArtifactError::invalid(format!(
                "{owner} contains an empty {field} entry"
            )));
        }
    }
    Ok(())
}

fn validate_dynamic_access_hints(
    owner: &str,
    field: &str,
    hints: &[DynamicAccessHint],
) -> Result<(), ContractArtifactError> {
    for hint in hints {
        if hint.base_key.trim().is_empty() {
            return Err(ContractArtifactError::invalid(format!(
                "{owner} contains an empty {field}.base_key entry"
            )));
        }
        if !hint.base_key.starts_with("state:") || hint.base_key == "state:*" {
            return Err(ContractArtifactError::invalid(format!(
                "{owner} contains unsupported dynamic access base `{}`",
                hint.base_key
            )));
        }
        if hint.max_keys == 0 {
            return Err(ContractArtifactError::invalid(format!(
                "{owner} contains zero-bound dynamic access hint `{}`",
                hint.base_key
            )));
        }
    }
    Ok(())
}

fn validate_kotoba_entries(
    entries: &[KotobaTranslationEntry],
) -> Result<(), ContractArtifactError> {
    let mut msg_ids = BTreeSet::new();
    for entry in entries {
        if entry.msg_id.trim().is_empty() {
            return Err(ContractArtifactError::invalid(
                "CNTR kotoba entries must not contain an empty msg_id",
            ));
        }
        if !msg_ids.insert(entry.msg_id.clone()) {
            return Err(ContractArtifactError::invalid(format!(
                "duplicate kotoba msg_id `{}`",
                entry.msg_id
            )));
        }
        let mut langs = BTreeSet::new();
        for translation in &entry.translations {
            if translation.lang.trim().is_empty() {
                return Err(ContractArtifactError::invalid(format!(
                    "kotoba entry `{}` contains an empty language tag",
                    entry.msg_id
                )));
            }
            if !langs.insert(translation.lang.clone()) {
                return Err(ContractArtifactError::invalid(format!(
                    "kotoba entry `{}` declares duplicate language `{}`",
                    entry.msg_id, translation.lang
                )));
            }
        }
    }
    Ok(())
}

fn validate_entrypoint_name(name: &str) -> Result<(), ContractArtifactError> {
    if name.is_empty() {
        return Err(ContractArtifactError::invalid(
            "entrypoint names must not be empty",
        ));
    }
    if !is_canonical_entrypoint_name(name) {
        return Err(ContractArtifactError::invalid(format!(
            "entrypoint `{name}` is not a canonical Kotodama V1 identifier or branded lifecycle selector"
        )));
    }
    Ok(())
}

fn validate_state_descriptors(
    contract_interface: &EmbeddedContractInterfaceV1,
) -> Result<(), ContractArtifactError> {
    let mut names = BTreeSet::new();
    for state in &contract_interface.states {
        if state.name.trim().is_empty() {
            return Err(ContractArtifactError::invalid(
                "CNTR state descriptors must not use an empty name",
            ));
        }
        if !is_canonical_source_declaration_name(&state.name, false) {
            return Err(ContractArtifactError::invalid(format!(
                "state descriptor `{}` is not a canonical Kotodama V1 identifier",
                state.name
            )));
        }
        if !names.insert(state.name.clone()) {
            return Err(ContractArtifactError::invalid(format!(
                "duplicate state descriptor `{}`",
                state.name
            )));
        }
        validate_state_type(&state.ty, true)?;
    }
    Ok(())
}

fn validate_error_codes(
    contract_interface: &EmbeddedContractInterfaceV1,
) -> Result<(), ContractArtifactError> {
    let mut paths = BTreeSet::new();
    let mut codes = BTreeSet::new();
    for error in &contract_interface.error_codes {
        if !is_canonical_source_declaration_name(&error.namespace, false)
            || !is_canonical_source_identifier(&error.name)
        {
            return Err(ContractArtifactError::invalid(
                "CNTR error code namespace and name must be canonical Kotodama V1 identifiers",
            ));
        }
        let path = format!("{}::{}", error.namespace, error.name);
        if !paths.insert(path.clone()) {
            return Err(ContractArtifactError::invalid(format!(
                "duplicate error code descriptor `{path}`"
            )));
        }
        if error.code == 0 {
            return Err(ContractArtifactError::invalid(format!(
                "error code descriptor `{path}` uses reserved code 0"
            )));
        }
        if !codes.insert(error.code) {
            return Err(ContractArtifactError::invalid(format!(
                "duplicate numeric error code {}",
                error.code
            )));
        }
    }
    Ok(())
}

fn is_supported_state_map_key(ty: &EmbeddedStateType) -> bool {
    matches!(
        ty,
        EmbeddedStateType::Int
            | EmbeddedStateType::Decimal
            | EmbeddedStateType::Quantity
            | EmbeddedStateType::Bool
            | EmbeddedStateType::String
            | EmbeddedStateType::Bytes
            | EmbeddedStateType::DataSpaceId
            | EmbeddedStateType::AccountId
            | EmbeddedStateType::AssetDefinitionId
            | EmbeddedStateType::AssetId
            | EmbeddedStateType::NftId
            | EmbeddedStateType::DomainId
            | EmbeddedStateType::Name
    )
}

fn validate_state_type(
    ty: &EmbeddedStateType,
    allow_state_map: bool,
) -> Result<(), ContractArtifactError> {
    match ty {
        EmbeddedStateType::Tuple(items) => {
            if items.len() < 2 {
                return Err(ContractArtifactError::invalid(
                    "CNTR durable tuples require at least two elements",
                ));
            }
            for item in items {
                validate_state_type(item, false)?;
            }
        }
        EmbeddedStateType::Struct { name, fields } => {
            if !is_canonical_source_declaration_name(name, false) {
                return Err(ContractArtifactError::invalid(format!(
                    "CNTR struct `{name}` is not a canonical Kotodama V1 identifier"
                )));
            }
            if fields.is_empty() {
                return Err(ContractArtifactError::invalid(format!(
                    "CNTR struct `{name}` must contain at least one field"
                )));
            }
            let mut field_names = BTreeSet::new();
            for field in fields {
                if !is_canonical_source_identifier(&field.name) {
                    return Err(ContractArtifactError::invalid(format!(
                        "CNTR struct `{name}` contains noncanonical field `{}`",
                        field.name
                    )));
                }
                if !field_names.insert(field.name.clone()) {
                    return Err(ContractArtifactError::invalid(format!(
                        "CNTR struct `{name}` contains duplicate field `{}`",
                        field.name
                    )));
                }
                validate_state_type(&field.ty, false)?;
            }
        }
        EmbeddedStateType::StateMap { key, value } => {
            if !allow_state_map {
                return Err(ContractArtifactError::invalid(
                    "CNTR StateMap is a top-level durable collection and cannot be nested",
                ));
            }
            if !is_supported_state_map_key(key) {
                return Err(ContractArtifactError::invalid(
                    "CNTR StateMap key must be a supported canonical scalar type",
                ));
            }
            validate_state_type(value, false)?;
        }
        EmbeddedStateType::Option(value) => validate_state_type(value, false)?,
        EmbeddedStateType::Result { ok, err } => {
            validate_state_type(ok, false)?;
            validate_state_type(err, false)?;
        }
        EmbeddedStateType::List { element, capacity } => {
            if !(1..=64).contains(capacity) {
                return Err(ContractArtifactError::invalid(
                    "CNTR List capacity must be in 1..=64",
                ));
            }
            validate_state_type(element, false)?;
        }
        _ => {}
    }
    Ok(())
}

fn header_declares_contract_minor_one(artifact: &[u8]) -> bool {
    artifact.len() >= HEADER_SIZE && artifact[4] == 1 && artifact[5] == 1
}

fn cntr_section_missing(artifact: &[u8]) -> bool {
    artifact.len() < HEADER_SIZE + 4
        || artifact[HEADER_SIZE..HEADER_SIZE + 4]
            != crate::metadata::CONTRACT_INTERFACE_SECTION_MAGIC
}

#[cfg(test)]
mod tests {
    use super::*;

    fn prepared_fixture(max_cycles: u64) -> Arc<[u8]> {
        let metadata = ProgramMetadata {
            max_cycles,
            ..ProgramMetadata::default()
        };
        let interface = EmbeddedContractInterfaceV1 {
            seiyaku_name: "PreparedFixture".to_owned(),
            compiler_fingerprint: "ivm-unit-tests".to_owned(),
            abi_hash: crate::syscalls::compute_abi_hash(crate::SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![crate::metadata::EmbeddedEntrypointDescriptor {
                name: "inspect".to_owned(),
                kind: EntryPointKind::View,
                params: Vec::new(),
                argument_schema: None,
                return_type: None,
                return_schema: None,
                permission: None,
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
            }],
            error_codes: Vec::new(),
            states: Vec::new(),
        };
        let mut artifact = metadata.encode();
        artifact.extend_from_slice(&interface.encode_section());
        artifact.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
        Arc::from(artifact.into_boxed_slice())
    }

    fn indexed_literal_contract_fixture(
        kind: crate::metadata::LiteralKindV1,
        data: &[u8],
        opcode: u8,
    ) -> Vec<u8> {
        let metadata = ProgramMetadata::default();
        let interface = EmbeddedContractInterfaceV1 {
            seiyaku_name: "LiteralAdmission".to_owned(),
            compiler_fingerprint: "ivm-unit-tests".to_owned(),
            abi_hash: crate::syscalls::compute_abi_hash(crate::SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: vec![crate::metadata::EmbeddedEntrypointDescriptor {
                name: "inspect".to_owned(),
                kind: EntryPointKind::View,
                params: Vec::new(),
                argument_schema: None,
                return_type: None,
                return_schema: None,
                permission: None,
                read_keys: Vec::new(),
                write_keys: Vec::new(),
                access_hints_complete: Some(true),
                access_hints_skipped: Vec::new(),
                triggers: Vec::new(),
                entry_pc: 0,
            }],
            error_codes: Vec::new(),
            states: Vec::new(),
        };
        let interface = interface.encode_section();
        let unpadded = interface.len() + 16 + 8 + data.len();
        let post_pad = (4 - (unpadded % 4)) % 4;
        let descriptor =
            crate::metadata::encode_literal_descriptor(kind, 24).expect("small literal descriptor");

        let mut artifact = metadata.encode();
        artifact.extend_from_slice(&interface);
        artifact.extend_from_slice(&crate::metadata::LITERAL_SECTION_MAGIC);
        artifact.extend_from_slice(&1_u32.to_le_bytes());
        artifact.extend_from_slice(&(post_pad as u32).to_le_bytes());
        artifact.extend_from_slice(&(data.len() as u32).to_le_bytes());
        artifact.extend_from_slice(&descriptor.to_le_bytes());
        artifact.extend_from_slice(data);
        artifact.extend(std::iter::repeat_n(0, post_pad));
        artifact
            .extend_from_slice(&crate::encoding::wide::encode_literal(opcode, 5, 0).to_le_bytes());
        artifact.extend_from_slice(&crate::encoding::wide::encode_halt().to_le_bytes());
        artifact
    }

    fn interface_with_state(ty: EmbeddedStateType) -> EmbeddedContractInterfaceV1 {
        EmbeddedContractInterfaceV1 {
            seiyaku_name: "StateAdmission".to_owned(),
            compiler_fingerprint: "ivm-unit-tests".to_owned(),
            abi_hash: crate::syscalls::compute_abi_hash(crate::SyscallPolicy::AbiV1),
            features_bitmap: 0,
            access_set_hints: None,
            kotoba: Vec::new(),
            entrypoints: Vec::new(),
            states: vec![crate::metadata::EmbeddedStateDescriptor {
                name: "value".to_owned(),
                ty,
            }],
            error_codes: Vec::new(),
        }
    }

    #[test]
    fn cntr_state_admission_rejects_unexecutable_type_trees() {
        let valid = EmbeddedStateType::StateMap {
            key: Box::new(EmbeddedStateType::Decimal),
            value: Box::new(EmbeddedStateType::List {
                element: Box::new(EmbeddedStateType::Quantity),
                capacity: 64,
            }),
        };
        validate_state_descriptors(&interface_with_state(valid))
            .expect("canonical map and aggregate value remain admissible");

        let invalid = [
            EmbeddedStateType::StateMap {
                key: Box::new(EmbeddedStateType::Json),
                value: Box::new(EmbeddedStateType::Int),
            },
            EmbeddedStateType::Option(Box::new(EmbeddedStateType::StateMap {
                key: Box::new(EmbeddedStateType::Int),
                value: Box::new(EmbeddedStateType::Int),
            })),
            EmbeddedStateType::Tuple(vec![EmbeddedStateType::Int]),
            EmbeddedStateType::Struct {
                name: "Empty".to_owned(),
                fields: Vec::new(),
            },
            EmbeddedStateType::List {
                element: Box::new(EmbeddedStateType::Int),
                capacity: 0,
            },
        ];
        for ty in invalid {
            let error = validate_state_descriptors(&interface_with_state(ty))
                .expect_err("unexecutable CNTR state type must fail admission");
            assert!(error.to_string().contains("CNTR"), "{error}");
        }
    }

    #[test]
    fn cntr_state_map_admission_accepts_every_runtime_key_domain() {
        let keys = [
            EmbeddedStateType::Int,
            EmbeddedStateType::Decimal,
            EmbeddedStateType::Quantity,
            EmbeddedStateType::Bool,
            EmbeddedStateType::String,
            EmbeddedStateType::Bytes,
            EmbeddedStateType::DataSpaceId,
            EmbeddedStateType::AccountId,
            EmbeddedStateType::AssetDefinitionId,
            EmbeddedStateType::AssetId,
            EmbeddedStateType::NftId,
            EmbeddedStateType::DomainId,
            EmbeddedStateType::Name,
        ];
        for key in keys {
            let map = EmbeddedStateType::StateMap {
                key: Box::new(key),
                value: Box::new(EmbeddedStateType::Bytes),
            };
            validate_state_descriptors(&interface_with_state(map))
                .expect("runtime-supported map key must pass admission");
        }
    }

    #[test]
    fn repeated_preparation_reuses_instruction_storage_and_indexes_entrypoints() {
        let artifact = prepared_fixture(0);
        let original_artifact = Arc::clone(&artifact);
        let first = prepare_contract(Arc::clone(&artifact)).expect("first preparation succeeds");
        let second = prepare_contract(artifact).expect("second preparation succeeds");
        let retained_artifact = first.shared_artifact();

        assert_eq!(first.code_hash(), second.code_hash());
        assert!(Arc::ptr_eq(&retained_artifact, &original_artifact));
        assert!(Arc::ptr_eq(&retained_artifact, &first.shared_artifact()));
        assert!(first.shares_prepared_program_with(&second));
        assert_eq!(first.instruction_boundaries(), &[0]);
        assert_eq!(first.control_flow_successors(0), Some(&[][..]));
        assert_eq!(
            first.entrypoint_pc("inspect"),
            Some(first.instruction_entry_pc())
        );
        assert_eq!(
            first
                .entrypoint_descriptor("inspect")
                .map(|entrypoint| entrypoint.kind),
            Some(EntryPointKind::View)
        );
    }

    #[test]
    fn admission_rejects_indexed_literal_type_confusion_and_scalar_length_mismatch() {
        let scalar = 7_i64.to_le_bytes();
        let valid = indexed_literal_contract_fixture(
            crate::metadata::LiteralKindV1::I64,
            &scalar,
            crate::instruction::wide::memory::LDI64,
        );
        verify_contract_artifact(&valid).expect("canonical LDI64 artifact verifies");
        let prepared = prepare_contract(Arc::from(valid.into_boxed_slice()))
            .expect("canonical LDI64 artifact prepares");
        let mut vm = crate::IVM::new(1);
        vm.load_prepared(&prepared)
            .expect("prepared LDI64 artifact loads");
        vm.run().expect("prepared LDI64 artifact executes");
        assert_eq!(vm.register(5), 7);

        let wrong_opcode = indexed_literal_contract_fixture(
            crate::metadata::LiteralKindV1::I64,
            &scalar,
            crate::instruction::wide::memory::LDLIT,
        );
        let wrong_length = indexed_literal_contract_fixture(
            crate::metadata::LiteralKindV1::I64,
            &scalar[..7],
            crate::instruction::wide::memory::LDI64,
        );
        for artifact in [wrong_opcode, wrong_length] {
            let error = verify_contract_artifact(&artifact)
                .expect_err("malicious indexed literal artifact must fail admission");
            assert!(error.to_string().contains("literal"), "{error}");
            let error = prepare_contract(Arc::from(artifact.into_boxed_slice()))
                .expect_err("malicious indexed literal artifact must not prepare");
            assert!(error.to_string().contains("literal"), "{error}");
        }
    }

    #[test]
    fn preparation_identity_binds_execution_header_fields() {
        let original = prepared_fixture(7);
        let mut mutated = original.to_vec();
        mutated[8..16].copy_from_slice(&11_u64.to_le_bytes());
        let mutated: Arc<[u8]> = Arc::from(mutated.into_boxed_slice());

        let original = prepare_contract(original).expect("original preparation succeeds");
        let mutated = prepare_contract(mutated).expect("mutated preparation succeeds");

        assert_eq!(original.metadata().max_cycles, 7);
        assert_eq!(mutated.metadata().max_cycles, 11);
        assert_ne!(original.code_hash(), mutated.code_hash());
    }

    #[test]
    fn prepared_load_matches_raw_load_without_a_parse_attempt() {
        crate::ivm::set_banner_enabled(false);
        let artifact = prepared_fixture(0);
        let prepared = prepare_contract(Arc::clone(&artifact)).expect("preparation succeeds");
        let mut raw = crate::IVM::new(u64::MAX);
        let mut warm = crate::IVM::new(u64::MAX);

        raw.load_program(artifact.as_ref())
            .expect("raw load succeeds");
        warm.load_prepared(&prepared)
            .expect("prepared load succeeds");

        assert_eq!(raw.program_parse_attempts(), 1);
        assert_eq!(warm.program_parse_attempts(), 0);
        assert_eq!(warm.prepared_loads(), 1);
        assert_eq!(raw.code_hash(), warm.code_hash());
        assert_eq!(raw.pc(), warm.pc());
        assert_eq!(raw.metadata().max_cycles, warm.metadata().max_cycles);
        let code_len = u64::try_from(prepared.artifact().len() - prepared.header_len())
            .expect("fixture length fits VM address");
        assert_eq!(
            raw.memory.load_region(0, code_len).expect("raw code image"),
            warm.memory
                .load_region(0, code_len)
                .expect("prepared code image")
        );

        raw.run().expect("raw program runs");
        warm.reset_predecode_misses();
        warm.run().expect("prepared program runs");
        assert_eq!(warm.predecode_misses(), 0);
        assert_eq!(raw.pc(), warm.pc());
        assert_eq!(raw.remaining_gas(), warm.remaining_gas());
        assert_eq!(raw.register(10), warm.register(10));

        warm.load_prepared(&prepared)
            .expect("prepared reload succeeds");
        assert_eq!(warm.program_parse_attempts(), 0);
        assert_eq!(warm.prepared_loads(), 2);
    }
}
