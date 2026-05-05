use std::{collections::BTreeSet, error::Error as StdError, fmt};

use iroha_crypto::Hash;
use iroha_data_model::smart_contract::manifest::{
    AccessSetHints, ContractManifest, EntryPointKind, KotobaTranslationEntry,
};

use crate::{
    ProgramMetadata, SyscallPolicy,
    ivm_cache::IvmCache,
    metadata::{
        CONTRACT_FEATURE_BIT_VECTOR, CONTRACT_FEATURE_BIT_ZK, CONTRACT_FEATURE_KNOWN_BITS,
        EmbeddedContractInterfaceV1, EmbeddedStateType, HEADER_SIZE, mode,
    },
};

/// Verified contract artifact details derived from a self-describing `.to` image.
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
}

impl ContractArtifactError {
    fn invalid(message: impl Into<String>) -> Self {
        Self {
            message: format!("invalid contract artifact: {}", message.into()),
        }
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
    let parsed = ProgramMetadata::parse(artifact).map_err(|err| {
        if header_declares_contract_minor_one(artifact) && cntr_section_missing(artifact) {
            ContractArtifactError::invalid("missing required CNTR section")
        } else {
            ContractArtifactError::invalid(format!("metadata parse failed: {err}"))
        }
    })?;
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

    validate_contract_interface(
        &metadata,
        &contract_interface,
        &artifact[parsed.code_offset..],
    )?;

    let code_hash = Hash::new(&artifact[HEADER_SIZE..]);
    let abi_hash = Hash::prehashed(crate::syscalls::compute_abi_hash(syscall_policy));
    let entrypoints = contract_interface
        .entrypoints
        .iter()
        .map(|entrypoint| entrypoint.to_manifest_descriptor())
        .collect::<Vec<_>>();
    let manifest = ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(abi_hash),
        compiler_fingerprint: Some(contract_interface.compiler_fingerprint.clone()),
        features_bitmap: Some(contract_interface.features_bitmap),
        access_set_hints: contract_interface.access_set_hints.clone(),
        entrypoints: Some(entrypoints),
        kotoba: (!contract_interface.kotoba.is_empty())
            .then_some(contract_interface.kotoba.clone()),
        provenance: None,
    };

    Ok(VerifiedContractArtifact {
        metadata,
        header_len: parsed.header_len,
        code_offset: parsed.code_offset,
        code_hash,
        abi_hash,
        contract_interface,
        manifest,
    })
}

fn validate_contract_interface(
    metadata: &ProgramMetadata,
    contract_interface: &EmbeddedContractInterfaceV1,
    code: &[u8],
) -> Result<(), ContractArtifactError> {
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

    if contract_interface.entrypoints.is_empty() {
        return Err(ContractArtifactError::invalid(
            "CNTR must declare at least one entrypoint",
        ));
    }

    let decoded = IvmCache::decode_stream(code).map_err(|err| {
        ContractArtifactError::invalid(format!(
            "instruction decode failed for executable stream: {err}"
        ))
    })?;
    let valid_pcs = decoded.iter().map(|op| op.pc).collect::<BTreeSet<_>>();
    let mut entrypoint_names = BTreeSet::new();
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
        if !valid_pcs.contains(&entrypoint.entry_pc) {
            return Err(ContractArtifactError::invalid(format!(
                "entrypoint `{}` has invalid entry_pc {}",
                entrypoint.name, entrypoint.entry_pc
            )));
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
            EntryPointKind::Public | EntryPointKind::View => {}
        }
    }

    for entrypoint in &contract_interface.entrypoints {
        for trigger in &entrypoint.triggers {
            if let Some(namespace) = trigger.callback.namespace.as_deref()
                && namespace.trim().is_empty()
            {
                return Err(ContractArtifactError::invalid(format!(
                    "entrypoint `{}` has a trigger with an empty callback namespace",
                    entrypoint.name
                )));
            }
            validate_entrypoint_name(&trigger.callback.entrypoint)?;
            if !entrypoint_names.contains(&trigger.callback.entrypoint) {
                return Err(ContractArtifactError::invalid(format!(
                    "trigger `{}` callback target `{}` is not a declared entrypoint",
                    trigger.id, trigger.callback.entrypoint
                )));
            }
        }
    }

    Ok(())
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
    if !name.chars().all(|ch| ch.is_alphanumeric() || ch == '_') {
        return Err(ContractArtifactError::invalid(format!(
            "entrypoint `{name}` contains unsupported characters"
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
        if !names.insert(state.name.clone()) {
            return Err(ContractArtifactError::invalid(format!(
                "duplicate state descriptor `{}`",
                state.name
            )));
        }
        validate_state_type(&state.ty)?;
    }
    Ok(())
}

fn validate_state_type(ty: &EmbeddedStateType) -> Result<(), ContractArtifactError> {
    match ty {
        EmbeddedStateType::Tuple(items) => {
            for item in items {
                validate_state_type(item)?;
            }
        }
        EmbeddedStateType::Struct { name, fields } => {
            if name.trim().is_empty() {
                return Err(ContractArtifactError::invalid(
                    "CNTR struct state type must not use an empty name",
                ));
            }
            let mut field_names = BTreeSet::new();
            for field in fields {
                if field.name.trim().is_empty() {
                    return Err(ContractArtifactError::invalid(format!(
                        "CNTR struct `{name}` contains an empty field name"
                    )));
                }
                if !field_names.insert(field.name.clone()) {
                    return Err(ContractArtifactError::invalid(format!(
                        "CNTR struct `{name}` contains duplicate field `{}`",
                        field.name
                    )));
                }
                validate_state_type(&field.ty)?;
            }
        }
        EmbeddedStateType::Map { key, value } => {
            validate_state_type(key)?;
            validate_state_type(value)?;
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
