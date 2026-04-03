#![cfg(feature = "app_api")]

use std::{
    collections::BTreeSet,
    fs, io,
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use axum::{http::StatusCode, response::IntoResponse};
use iroha_core::state::{State as CoreState, StateReadOnly, WorldReadOnly};
use iroha_crypto::Hash;
use iroha_data_model::{
    ValidationFail,
    isi::{
        InstructionBox,
        smart_contract_code::{
            ActivateContractInstance, RegisterSmartContractBytes, RegisterSmartContractCode,
        },
    },
    query::error::QueryExecutionFail,
    smart_contract::manifest::{ContractManifest, EntryPointKind, EntrypointDescriptor},
    transaction::{TransactionEntrypoint, executable::Executable},
};
use ivm::analysis::ProgramAnalysis;
use mv::storage::StorageReadOnly;
use sorafs_car::CarBuildPlan;
use sorafs_chunker::ChunkProfile;
use sorafs_manifest::{BLAKE3_256_MULTIHASH_CODE, DagCodecId, ManifestBuilder, PinPolicy};

use crate::{Error, JsonBody, data_dir};

const VERIFIED_SOURCE_VERSION: u32 = 1;
const VERIFIED_SOURCE_LANGUAGE_KOTODAMA: &str = "kotodama";
const RENDERED_SOURCE_VERIFIED: &str = "verified_source";
const RENDERED_SOURCE_PSEUDO: &str = "pseudo_source";
const RENDERED_SOURCE_MANIFEST_STUB: &str = "manifest_stub";

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct ContractViewAccessHintsDto {
    pub read_keys: Vec<String>,
    pub write_keys: Vec<String>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct ContractViewEntrypointParamDto {
    pub name: String,
    pub type_name: String,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct ContractViewEntrypointDto {
    pub name: String,
    pub kind: String,
    pub params: Vec<ContractViewEntrypointParamDto>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub return_type: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub permission: Option<String>,
    pub read_keys: Vec<String>,
    pub write_keys: Vec<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub access_hints_complete: Option<bool>,
    pub access_hints_skipped: Vec<String>,
    pub triggers: Vec<String>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct ContractViewSyscallDto {
    pub number: u8,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub count: u64,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct ContractViewMemoryDto {
    pub load64: u64,
    pub store64: u64,
    pub load128: u64,
    pub store128: u64,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct ContractViewAnalysisDto {
    pub instruction_count: u64,
    pub memory: ContractViewMemoryDto,
    pub syscalls: Vec<ContractViewSyscallDto>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
)]
pub struct ContractVerifiedSourceRefDto {
    pub language: String,
    #[norito(default)]
    pub source_name: Option<String>,
    pub submitted_at: String,
    #[norito(default)]
    pub manifest_id_hex: Option<String>,
    #[norito(default)]
    pub payload_digest_hex: Option<String>,
    #[norito(default)]
    pub content_length: Option<u64>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoSerialize,
    norito::derive::NoritoDeserialize,
)]
pub struct ContractCodeViewDto {
    pub code_hash: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub declared_code_hash: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub abi_hash: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub compiler_fingerprint: Option<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub byte_len: Option<u64>,
    pub permissions: Vec<String>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub access_hints: Option<ContractViewAccessHintsDto>,
    pub entrypoints: Vec<ContractViewEntrypointDto>,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub analysis: Option<ContractViewAnalysisDto>,
    pub warnings: Vec<String>,
    pub rendered_source_kind: String,
    pub rendered_source_text: String,
    #[norito(skip_serializing_if = "Option::is_none")]
    pub verified_source_ref: Option<ContractVerifiedSourceRefDto>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
)]
pub struct SubmitVerifiedContractSourceDto {
    pub language: String,
    #[norito(default)]
    pub source_name: Option<String>,
    pub source_text: String,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
)]
pub struct ContractVerifiedSourceJobResponseDto {
    pub job_id: String,
    pub code_hash: String,
    pub status: String,
    pub submitted_at: String,
    #[norito(default)]
    pub completed_at: Option<String>,
    #[norito(default)]
    pub message: Option<String>,
    #[norito(default)]
    pub actual_code_hash: Option<String>,
    #[norito(default)]
    pub verified_source_ref: Option<ContractVerifiedSourceRefDto>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
)]
struct StoredVerifiedSourceRecord {
    version: u32,
    code_hash: String,
    #[norito(default)]
    abi_hash: Option<String>,
    #[norito(default)]
    compiler_fingerprint: Option<String>,
    language: String,
    #[norito(default)]
    source_name: Option<String>,
    source_text: String,
    submitted_at: String,
    #[norito(default)]
    manifest_id_hex: Option<String>,
    #[norito(default)]
    payload_digest_hex: Option<String>,
    #[norito(default)]
    content_length: Option<u64>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
)]
struct StoredVerifiedSourceJob {
    version: u32,
    job_id: String,
    code_hash: String,
    status: String,
    submitted_at: String,
    #[norito(default)]
    completed_at: Option<String>,
    #[norito(default)]
    message: Option<String>,
    #[norito(default)]
    actual_code_hash: Option<String>,
    #[norito(default)]
    verified_source_ref: Option<ContractVerifiedSourceRefDto>,
}

#[derive(
    Debug,
    Clone,
    crate::json_macros::JsonSerialize,
    norito::derive::NoritoSerialize,
    crate::json_macros::JsonDeserialize,
    norito::derive::NoritoDeserialize,
)]
struct VerifiedSourceBundle {
    version: u32,
    language: String,
    #[norito(default)]
    source_name: Option<String>,
    source_text: String,
    code_hash: String,
    #[norito(default)]
    abi_hash: Option<String>,
    #[norito(default)]
    compiler_fingerprint: Option<String>,
    submitted_at: String,
}

struct ContractViewBuildInput {
    code_hash: Option<String>,
    declared_code_hash: Option<String>,
    manifest: Option<ContractManifest>,
    code_bytes: Option<Vec<u8>>,
    warnings: Vec<String>,
}

fn not_found() -> Error {
    Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::NotFound))
}

fn conversion_error(message: impl Into<String>) -> Error {
    Error::Query(ValidationFail::QueryFailed(QueryExecutionFail::Conversion(
        message.into(),
    )))
}

fn map_io_error(error: io::Error, context: &str) -> Error {
    conversion_error(format!("{context}: {error}"))
}

fn hash_hex(hash: &Hash) -> String {
    hex::encode(hash.as_ref())
}

fn canonical_code_hash(code_bytes: &[u8]) -> Result<Hash, Error> {
    let parsed = ivm::ProgramMetadata::parse(code_bytes)
        .map_err(|err| conversion_error(format!("invalid contract artifact header: {err}")))?;
    if parsed.header_len > code_bytes.len() {
        return Err(conversion_error(
            "contract artifact header length exceeds code length",
        ));
    }
    Ok(Hash::new(&code_bytes[parsed.header_len..]))
}

fn manifest_from_verified_artifact(
    verified: &ivm::VerifiedContractArtifact,
    code_hash: Hash,
) -> ContractManifest {
    ContractManifest {
        code_hash: Some(code_hash),
        abi_hash: Some(verified.abi_hash),
        compiler_fingerprint: Some(verified.contract_interface.compiler_fingerprint.clone()),
        features_bitmap: Some(verified.contract_interface.features_bitmap),
        access_set_hints: verified.contract_interface.access_set_hints.clone(),
        entrypoints: Some(
            verified
                .contract_interface
                .entrypoints
                .iter()
                .map(|entrypoint| entrypoint.to_manifest_descriptor())
                .collect(),
        ),
        kotoba: (!verified.contract_interface.kotoba.is_empty())
            .then_some(verified.contract_interface.kotoba.clone()),
        provenance: None,
    }
}

fn parse_code_hash_hex(raw: &str) -> Result<Hash, Error> {
    let bytes =
        hex::decode(raw).map_err(|err| conversion_error(format!("invalid code hash: {err}")))?;
    if bytes.len() != 32 {
        return Err(conversion_error(format!(
            "invalid code hash length {}; expected 32 bytes",
            bytes.len()
        )));
    }
    let mut array = [0_u8; 32];
    array.copy_from_slice(&bytes);
    Ok(Hash::prehashed(array))
}

fn now_rfc3339() -> String {
    crate::explorer::now_rfc3339()
}

fn unique_suffix() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    format!("{nanos:x}")
}

fn contracts_dir() -> PathBuf {
    data_dir::base_dir().join("contracts")
}

fn verified_source_record_path(code_hash: &str) -> PathBuf {
    contracts_dir()
        .join("verified_sources")
        .join(format!("{code_hash}.json"))
}

fn verified_source_job_path(code_hash: &str, job_id: &str) -> PathBuf {
    contracts_dir()
        .join("verified_source_jobs")
        .join(code_hash)
        .join(format!("{job_id}.json"))
}

fn write_json_file_atomic<T: norito::json::JsonSerialize>(
    path: &Path,
    value: &T,
) -> Result<(), Error> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| map_io_error(err, "failed to create contract source directory"))?;
    }
    let bytes = norito::json::to_vec(value).map_err(|err| {
        conversion_error(format!(
            "failed to serialize contract source payload: {err}"
        ))
    })?;
    let temp_path = path.with_extension(format!("tmp-{}", unique_suffix()));
    fs::write(&temp_path, bytes)
        .map_err(|err| map_io_error(err, "failed to write contract source file"))?;
    fs::rename(&temp_path, path)
        .map_err(|err| map_io_error(err, "failed to persist contract source file"))?;
    Ok(())
}

fn read_json_file<T: norito::json::JsonDeserializeOwned>(path: &Path) -> Result<Option<T>, Error> {
    match fs::read(path) {
        Ok(bytes) => norito::json::from_slice(&bytes).map(Some).map_err(|err| {
            conversion_error(format!("failed to decode contract source file: {err}"))
        }),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(map_io_error(err, "failed to read contract source file")),
    }
}

fn load_verified_source_record(
    code_hash: &str,
) -> Result<Option<StoredVerifiedSourceRecord>, Error> {
    read_json_file(&verified_source_record_path(code_hash))
}

fn load_verified_source_job(
    code_hash: &str,
    job_id: &str,
) -> Result<Option<ContractVerifiedSourceJobResponseDto>, Error> {
    read_json_file::<StoredVerifiedSourceJob>(&verified_source_job_path(code_hash, job_id))
        .map(|maybe| maybe.map(Into::into))
}

fn persist_verified_source_record(record: &StoredVerifiedSourceRecord) -> Result<(), Error> {
    write_json_file_atomic(&verified_source_record_path(&record.code_hash), record)
}

fn persist_verified_source_job(job: &StoredVerifiedSourceJob) -> Result<(), Error> {
    write_json_file_atomic(&verified_source_job_path(&job.code_hash, &job.job_id), job)
}

fn verified_source_ref_from_record(
    record: &StoredVerifiedSourceRecord,
) -> Option<ContractVerifiedSourceRefDto> {
    if record.manifest_id_hex.is_none()
        && record.payload_digest_hex.is_none()
        && record.content_length.is_none()
    {
        return None;
    }

    Some(ContractVerifiedSourceRefDto {
        language: record.language.clone(),
        source_name: record.source_name.clone(),
        submitted_at: record.submitted_at.clone(),
        manifest_id_hex: record.manifest_id_hex.clone(),
        payload_digest_hex: record.payload_digest_hex.clone(),
        content_length: record.content_length,
    })
}

impl From<StoredVerifiedSourceJob> for ContractVerifiedSourceJobResponseDto {
    fn from(value: StoredVerifiedSourceJob) -> Self {
        Self {
            job_id: value.job_id,
            code_hash: value.code_hash,
            status: value.status,
            submitted_at: value.submitted_at,
            completed_at: value.completed_at,
            message: value.message,
            actual_code_hash: value.actual_code_hash,
            verified_source_ref: value.verified_source_ref,
        }
    }
}

fn persist_job_response(
    job: ContractVerifiedSourceJobResponseDto,
) -> Result<ContractVerifiedSourceJobResponseDto, Error> {
    let stored = StoredVerifiedSourceJob {
        version: VERIFIED_SOURCE_VERSION,
        job_id: job.job_id.clone(),
        code_hash: job.code_hash.clone(),
        status: job.status.clone(),
        submitted_at: job.submitted_at.clone(),
        completed_at: job.completed_at.clone(),
        message: job.message.clone(),
        actual_code_hash: job.actual_code_hash.clone(),
        verified_source_ref: job.verified_source_ref.clone(),
    };
    persist_verified_source_job(&stored)?;
    Ok(job)
}

fn entrypoint_kind_label(kind: EntryPointKind) -> &'static str {
    match kind {
        EntryPointKind::Public => "public",
        EntryPointKind::View => "view",
        EntryPointKind::Hajimari => "hajimari",
        EntryPointKind::Kaizen => "kaizen",
    }
}

fn entrypoint_signature(entrypoint: &EntrypointDescriptor) -> String {
    let params = entrypoint
        .params
        .iter()
        .map(|param| format!("{}: {}", param.name, param.type_name))
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = entrypoint
        .return_type
        .as_ref()
        .map(|value| format!(" -> {value}"))
        .unwrap_or_default();
    format!(
        "{} fn {}({}){}",
        entrypoint_kind_label(entrypoint.kind),
        entrypoint.name,
        params,
        return_type
    )
}

fn render_program_syscalls(analysis: &ProgramAnalysis) -> String {
    if analysis.syscalls.is_empty() {
        return "none".to_owned();
    }
    analysis
        .syscalls
        .iter()
        .map(|entry| {
            let name = ivm::syscalls::syscall_name(entry.number.into()).unwrap_or("UNKNOWN");
            format!("{name} x{}", entry.count)
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn trigger_label(
    trigger: &iroha_data_model::smart_contract::manifest::TriggerDescriptor,
) -> String {
    let callback = match &trigger.callback.namespace {
        Some(namespace) => format!("{namespace}.{}", trigger.callback.entrypoint),
        None => trigger.callback.entrypoint.clone(),
    };
    format!("{} -> {}", trigger.id, callback)
}

fn render_pseudo_source(
    code_hash: &str,
    manifest: Option<&ContractManifest>,
    analysis: Option<&ProgramAnalysis>,
) -> String {
    let contract_name = format!(
        "Contract_{}",
        &code_hash.chars().take(8).collect::<String>()
    );
    let mut lines = Vec::new();
    lines.push(format!("contract {contract_name} {{"));
    lines.push(
        "  // Decompiled pseudo-source derived from contract bytes and manifest hints.".to_owned(),
    );
    lines.push(format!("  // code_hash: {code_hash}"));

    if let Some(manifest) = manifest {
        if let Some(abi_hash) = manifest.abi_hash.as_ref() {
            lines.push(format!("  // abi_hash: {}", hash_hex(abi_hash)));
        }
        if let Some(compiler) = manifest.compiler_fingerprint.as_ref() {
            lines.push(format!("  // compiler_fingerprint: {compiler}"));
        }
        if let Some(features) = manifest.features_bitmap {
            lines.push(format!("  // features_bitmap: 0x{features:x}"));
        }
    }

    if let Some(analysis) = analysis {
        lines.push(format!(
            "  // static analysis: {} instructions; memory(load64={}, store64={}, load128={}, store128={})",
            analysis.instruction_count,
            analysis.memory.load64,
            analysis.memory.store64,
            analysis.memory.load128,
            analysis.memory.store128
        ));
        lines.push(format!(
            "  // static syscalls: {}",
            render_program_syscalls(analysis)
        ));
    }

    if let Some(manifest) = manifest {
        if let Some(entrypoints) = manifest.entrypoints.as_ref() {
            for entrypoint in entrypoints {
                lines.push(String::new());
                lines.push(format!("  {} {{", entrypoint_signature(entrypoint)));
                if let Some(permission) = entrypoint.permission.as_ref() {
                    lines.push(format!("    // permission: {permission}"));
                }
                if !entrypoint.read_keys.is_empty() {
                    lines.push(format!("    // reads: {}", entrypoint.read_keys.join(", ")));
                }
                if !entrypoint.write_keys.is_empty() {
                    lines.push(format!(
                        "    // writes: {}",
                        entrypoint.write_keys.join(", ")
                    ));
                }
                if !entrypoint.triggers.is_empty() {
                    let triggers = entrypoint
                        .triggers
                        .iter()
                        .map(trigger_label)
                        .collect::<Vec<_>>()
                        .join(", ");
                    lines.push(format!("    // triggers: {triggers}"));
                }
                if !entrypoint.access_hints_skipped.is_empty() {
                    lines.push(format!(
                        "    // skipped access hints: {}",
                        entrypoint.access_hints_skipped.join(", ")
                    ));
                }
                lines.push(
                    "    // body omitted; output reconstructed from static metadata.".to_owned(),
                );
                lines.push("  }".to_owned());
            }
        } else {
            lines.push(String::new());
            lines.push("  // No entrypoint metadata was embedded in the manifest.".to_owned());
        }
    } else {
        lines.push(String::new());
        lines.push("  // Manifest metadata is unavailable for this artifact.".to_owned());
    }

    lines.push("}".to_owned());
    lines.join("\n")
}

fn render_manifest_stub(
    code_hash: &str,
    manifest: Option<&ContractManifest>,
    warnings: &[String],
) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "contract ManifestStub_{} {{",
        &code_hash[..code_hash.len().min(8)]
    ));
    lines.push(
        "  // Decompiled code bytes are unavailable; showing manifest-derived hints only."
            .to_owned(),
    );
    lines.push(format!("  // code_hash: {code_hash}"));
    if let Some(manifest) = manifest {
        if let Some(abi_hash) = manifest.abi_hash.as_ref() {
            lines.push(format!("  // abi_hash: {}", hash_hex(abi_hash)));
        }
        if let Some(compiler) = manifest.compiler_fingerprint.as_ref() {
            lines.push(format!("  // compiler_fingerprint: {compiler}"));
        }
        if let Some(entrypoints) = manifest.entrypoints.as_ref() {
            for entrypoint in entrypoints {
                lines.push(format!(
                    "  // entrypoint: {}",
                    entrypoint_signature(entrypoint)
                ));
            }
        }
    }
    for warning in warnings {
        lines.push(format!("  // warning: {warning}"));
    }
    lines.push("}".to_owned());
    lines.join("\n")
}

fn aggregate_permissions(manifest: Option<&ContractManifest>) -> Vec<String> {
    let mut values = BTreeSet::new();
    if let Some(entrypoints) = manifest.and_then(|value| value.entrypoints.as_ref()) {
        for entrypoint in entrypoints {
            if let Some(permission) = entrypoint.permission.as_ref() {
                values.insert(permission.clone());
            }
        }
    }
    values.into_iter().collect()
}

fn to_entrypoint_dto(entrypoint: &EntrypointDescriptor) -> ContractViewEntrypointDto {
    ContractViewEntrypointDto {
        name: entrypoint.name.clone(),
        kind: entrypoint_kind_label(entrypoint.kind).to_owned(),
        params: entrypoint
            .params
            .iter()
            .map(|param| ContractViewEntrypointParamDto {
                name: param.name.clone(),
                type_name: param.type_name.clone(),
            })
            .collect(),
        return_type: entrypoint.return_type.clone(),
        permission: entrypoint.permission.clone(),
        read_keys: entrypoint.read_keys.clone(),
        write_keys: entrypoint.write_keys.clone(),
        access_hints_complete: entrypoint.access_hints_complete,
        access_hints_skipped: entrypoint.access_hints_skipped.clone(),
        triggers: entrypoint.triggers.iter().map(trigger_label).collect(),
    }
}

fn to_analysis_dto(analysis: &ProgramAnalysis) -> ContractViewAnalysisDto {
    ContractViewAnalysisDto {
        instruction_count: analysis.instruction_count as u64,
        memory: ContractViewMemoryDto {
            load64: analysis.memory.load64,
            store64: analysis.memory.store64,
            load128: analysis.memory.load128,
            store128: analysis.memory.store128,
        },
        syscalls: analysis
            .syscalls
            .iter()
            .map(|entry| ContractViewSyscallDto {
                number: entry.number,
                name: ivm::syscalls::syscall_name(entry.number.into()).map(ToOwned::to_owned),
                count: entry.count,
            })
            .collect(),
    }
}

fn locate_instruction_box(
    state: &CoreState,
    transaction_hash: &str,
    index: u64,
) -> Result<InstructionBox, Error> {
    let start_height = state.committed_height() as u64;
    if start_height == 0 {
        return Err(not_found());
    }

    let target: iroha_crypto::HashOf<TransactionEntrypoint> = transaction_hash
        .trim()
        .parse()
        .map_err(|_| conversion_error("invalid transaction hash".to_owned()))?;
    let lookup_index: usize = index
        .try_into()
        .map_err(|_| conversion_error("instruction index exceeds host pointer width"))?;

    let mut height = start_height;
    loop {
        let Some(nonzero_height) = NonZeroUsize::new(height as usize) else {
            break;
        };
        if let Some(block) = state.block_by_height(nonzero_height) {
            let block_ref = block.as_ref();
            let external_total = block_ref.external_transactions().len();
            for tx in block_ref.external_transactions().take(external_total) {
                if tx.hash_as_entrypoint() != target {
                    continue;
                }
                let Executable::Instructions(instructions) = tx.instructions() else {
                    return Err(not_found());
                };
                let instruction = instructions.get(lookup_index).ok_or_else(not_found)?;
                return Ok(instruction.clone());
            }
        }
        if height == 1 {
            break;
        }
        height -= 1;
    }

    Err(not_found())
}

fn build_contract_view(mut input: ContractViewBuildInput) -> Result<ContractCodeViewDto, Error> {
    let mut code_hash = input.code_hash.clone().unwrap_or_default();
    let mut declared_code_hash = input.declared_code_hash.clone();
    let mut manifest = input.manifest.clone();
    let mut compiler_fingerprint = manifest
        .as_ref()
        .and_then(|value| value.compiler_fingerprint.clone());
    let mut abi_hash = manifest
        .as_ref()
        .and_then(|value| value.abi_hash.as_ref().map(hash_hex));
    let mut analysis = None;
    let byte_len = input.code_bytes.as_ref().map(|bytes| bytes.len() as u64);

    if let Some(code_bytes) = input.code_bytes.as_ref() {
        let canonical_hash = canonical_code_hash(code_bytes)?;
        match ivm::verify_contract_artifact(code_bytes) {
            Ok(verified) => {
                let verified_hash = hash_hex(&canonical_hash);
                if declared_code_hash.is_none() {
                    declared_code_hash = input.code_hash.clone();
                }
                if code_hash.is_empty() {
                    code_hash = verified_hash.clone();
                } else if code_hash != verified_hash {
                    input.warnings.push(format!(
                        "Declared code hash {code_hash} does not match verified artifact hash {verified_hash}; showing the verified artifact hash."
                    ));
                    declared_code_hash = Some(code_hash.clone());
                    code_hash = verified_hash.clone();
                }

                let verified_manifest = manifest_from_verified_artifact(&verified, canonical_hash);
                match manifest.as_ref() {
                    Some(existing)
                        if existing.signature_payload()
                            == verified_manifest.signature_payload() => {}
                    Some(_) => {
                        input.warnings.push(
                            "Stored manifest does not match the verified artifact; using metadata embedded in the contract bytes.".to_owned(),
                        );
                        manifest = Some(verified_manifest);
                    }
                    None => {
                        manifest = Some(verified_manifest);
                    }
                }

                abi_hash = Some(hash_hex(&verified.abi_hash));
                compiler_fingerprint = Some(verified.contract_interface.compiler_fingerprint);

                match ivm::analysis::analyze_program(code_bytes) {
                    Ok(value) => analysis = Some(value),
                    Err(err) => input
                        .warnings
                        .push(format!("Static analysis unavailable: {err}")),
                }
            }
            Err(err) => {
                input
                    .warnings
                    .push(format!("Contract artifact verification failed: {err}"));
            }
        }
    }

    if code_hash.is_empty() {
        if let Some(manifest_hash) = manifest
            .as_ref()
            .and_then(|value| value.code_hash.as_ref().map(hash_hex))
        {
            code_hash = manifest_hash;
        } else {
            return Err(not_found());
        }
    }

    let verified_source_record = load_verified_source_record(&code_hash)?;
    let verified_source_ref = verified_source_record
        .as_ref()
        .and_then(verified_source_ref_from_record);

    let rendered_source_kind;
    let rendered_source_text;

    if let Some(record) = verified_source_record.as_ref() {
        rendered_source_kind = RENDERED_SOURCE_VERIFIED.to_owned();
        rendered_source_text = record.source_text.clone();
    } else if input.code_bytes.is_some() {
        rendered_source_kind = RENDERED_SOURCE_PSEUDO.to_owned();
        rendered_source_text =
            render_pseudo_source(&code_hash, manifest.as_ref(), analysis.as_ref());
    } else {
        rendered_source_kind = RENDERED_SOURCE_MANIFEST_STUB.to_owned();
        rendered_source_text = render_manifest_stub(&code_hash, manifest.as_ref(), &input.warnings);
    }

    Ok(ContractCodeViewDto {
        code_hash,
        declared_code_hash,
        abi_hash,
        compiler_fingerprint,
        byte_len,
        permissions: aggregate_permissions(manifest.as_ref()),
        access_hints: manifest
            .as_ref()
            .and_then(|value| value.access_set_hints.as_ref())
            .map(|hints| ContractViewAccessHintsDto {
                read_keys: hints.read_keys.clone(),
                write_keys: hints.write_keys.clone(),
            }),
        entrypoints: manifest
            .as_ref()
            .and_then(|value| value.entrypoints.as_ref())
            .map(|entrypoints| entrypoints.iter().map(to_entrypoint_dto).collect())
            .unwrap_or_default(),
        analysis: analysis.as_ref().map(to_analysis_dto),
        warnings: input.warnings,
        rendered_source_kind,
        rendered_source_text,
        verified_source_ref,
    })
}

fn resolve_contract_view_input_for_code_hash(
    state: &CoreState,
    code_hash_hex: &str,
) -> Result<ContractViewBuildInput, Error> {
    let code_hash = parse_code_hash_hex(code_hash_hex)?;
    let world = state.world_view();
    let manifest = world.contract_manifests().get(&code_hash).cloned();
    let code_bytes = world.contract_code().get(&code_hash).cloned();
    if manifest.is_none() && code_bytes.is_none() {
        return Err(not_found());
    }

    Ok(ContractViewBuildInput {
        code_hash: Some(code_hash_hex.to_owned()),
        declared_code_hash: None,
        manifest,
        code_bytes,
        warnings: Vec::new(),
    })
}

fn resolve_contract_view_input_for_instruction(
    instruction: &InstructionBox,
    state: &CoreState,
) -> Result<ContractViewBuildInput, Error> {
    let any = instruction.as_any();

    if let Some(register_bytes) = any.downcast_ref::<RegisterSmartContractBytes>() {
        return Ok(ContractViewBuildInput {
            code_hash: Some(hash_hex(&register_bytes.code_hash)),
            declared_code_hash: Some(hash_hex(&register_bytes.code_hash)),
            manifest: None,
            code_bytes: Some(register_bytes.code.clone()),
            warnings: vec![
                "This view was reconstructed from historical instruction bytes; the deployment may have been rejected and the artifact may not exist on-chain.".to_owned(),
            ],
        });
    }

    if let Some(register_code) = any.downcast_ref::<RegisterSmartContractCode>() {
        let declared = register_code.manifest.code_hash.as_ref().map(hash_hex);
        return Ok(ContractViewBuildInput {
            code_hash: declared.clone(),
            declared_code_hash: declared,
            manifest: Some(register_code.manifest.clone()),
            code_bytes: None,
            warnings: vec![
                "Only manifest metadata is available for this instruction; on-chain contract bytes were not included.".to_owned(),
            ],
        });
    }

    if let Some(activate) = any.downcast_ref::<ActivateContractInstance>() {
        let code_hash_hex = hash_hex(&activate.code_hash);
        let mut input = resolve_contract_view_input_for_code_hash(state, &code_hash_hex)?;
        input.warnings.push(format!(
            "Showing the contract currently bound to {}.",
            activate.contract_address
        ));
        return Ok(input);
    }

    Err(not_found())
}

fn persist_verified_source_bundle(
    bundle: &VerifiedSourceBundle,
    sorafs_node: &sorafs_node::NodeHandle,
) -> Result<Option<ContractVerifiedSourceRefDto>, Error> {
    if !sorafs_node.is_enabled() {
        return Ok(None);
    }

    let payload = norito::json::to_vec(bundle).map_err(|err| {
        conversion_error(format!("failed to encode verified source bundle: {err}"))
    })?;
    let plan = CarBuildPlan::single_file_with_profile(&payload, ChunkProfile::DEFAULT)
        .map_err(|err| conversion_error(format!("failed to derive SoraFS chunk plan: {err}")))?;
    let digest = blake3::hash(&payload);
    let manifest = ManifestBuilder::new()
        .root_cid(digest.as_bytes().to_vec())
        .dag_codec(DagCodecId(0x71))
        .chunking_from_profile(ChunkProfile::DEFAULT, BLAKE3_256_MULTIHASH_CODE)
        .content_length(plan.content_length)
        .car_digest(digest.into())
        .car_size(plan.content_length)
        .pin_policy(PinPolicy::default())
        .build()
        .map_err(|err| conversion_error(format!("failed to build SoraFS manifest: {err}")))?;
    let mut reader = payload.as_slice();
    let manifest_id = sorafs_node
        .ingest_manifest(&manifest, &plan, &mut reader)
        .map_err(|err| {
            conversion_error(format!("failed to store verified source in SoraFS: {err}"))
        })?;

    Ok(Some(ContractVerifiedSourceRefDto {
        language: bundle.language.clone(),
        source_name: bundle.source_name.clone(),
        submitted_at: bundle.submitted_at.clone(),
        manifest_id_hex: Some(manifest_id),
        payload_digest_hex: Some(hex::encode(plan.payload_digest.as_bytes())),
        content_length: Some(plan.content_length),
    }))
}

fn new_job_id(code_hash: &str, source_text: &str) -> String {
    let digest = Hash::new(format!("{code_hash}:{}:{source_text}", unique_suffix()).as_bytes());
    hex::encode(digest.as_ref())
}

pub async fn handle_get_instruction_contract_view(
    state: Arc<CoreState>,
    transaction_hash: String,
    index: u64,
) -> Result<impl IntoResponse, Error> {
    let instruction = locate_instruction_box(state.as_ref(), &transaction_hash, index)?;
    let input = resolve_contract_view_input_for_instruction(&instruction, state.as_ref())?;
    let view = build_contract_view(input)?;
    Ok(JsonBody(view))
}

pub async fn handle_get_contract_code_view(
    state: Arc<CoreState>,
    code_hash_hex: String,
) -> Result<impl IntoResponse, Error> {
    let input = resolve_contract_view_input_for_code_hash(state.as_ref(), &code_hash_hex)?;
    let view = build_contract_view(input)?;
    Ok(JsonBody(view))
}

pub async fn handle_post_verified_source_job(
    code_hash_hex: String,
    request: SubmitVerifiedContractSourceDto,
    sorafs_node: sorafs_node::NodeHandle,
) -> Result<(StatusCode, JsonBody<ContractVerifiedSourceJobResponseDto>), Error> {
    let requested_hash = parse_code_hash_hex(&code_hash_hex)?;
    let submitted_at = now_rfc3339();
    let language = request.language.trim().to_ascii_lowercase();
    let job_id = new_job_id(&code_hash_hex, &request.source_text);

    if language != VERIFIED_SOURCE_LANGUAGE_KOTODAMA {
        let response = ContractVerifiedSourceJobResponseDto {
            job_id,
            code_hash: code_hash_hex.clone(),
            status: "error".to_owned(),
            submitted_at,
            completed_at: Some(now_rfc3339()),
            message: Some(format!(
                "unsupported verified source language `{language}`; only `{VERIFIED_SOURCE_LANGUAGE_KOTODAMA}` is accepted"
            )),
            actual_code_hash: None,
            verified_source_ref: None,
        };
        let persisted = persist_job_response(response)?;
        return Ok((StatusCode::BAD_REQUEST, JsonBody(persisted)));
    }

    let source_text = request.source_text;
    if source_text.trim().is_empty() {
        let response = ContractVerifiedSourceJobResponseDto {
            job_id,
            code_hash: code_hash_hex.clone(),
            status: "error".to_owned(),
            submitted_at,
            completed_at: Some(now_rfc3339()),
            message: Some("source_text must not be empty".to_owned()),
            actual_code_hash: None,
            verified_source_ref: None,
        };
        let persisted = persist_job_response(response)?;
        return Ok((StatusCode::BAD_REQUEST, JsonBody(persisted)));
    }

    let compile_result =
        ivm::KotodamaCompiler::new().compile_source_with_manifest_and_report(&source_text);

    let response = match compile_result {
        Ok((code_bytes, _manifest, _report)) => {
            let actual_hash = canonical_code_hash(&code_bytes)?;
            let verified = ivm::verify_contract_artifact(&code_bytes).map_err(|err| {
                conversion_error(format!(
                    "compiled source did not produce a valid contract artifact: {err}"
                ))
            })?;
            let actual_code_hash = hash_hex(&actual_hash);
            if actual_hash != requested_hash {
                ContractVerifiedSourceJobResponseDto {
                    job_id,
                    code_hash: code_hash_hex.clone(),
                    status: "mismatch".to_owned(),
                    submitted_at,
                    completed_at: Some(now_rfc3339()),
                    message: Some(
                        "compiled source does not match the requested code hash".to_owned(),
                    ),
                    actual_code_hash: Some(actual_code_hash),
                    verified_source_ref: None,
                }
            } else if let Some(existing) = load_verified_source_record(&code_hash_hex)? {
                if existing.source_text == source_text {
                    ContractVerifiedSourceJobResponseDto {
                        job_id,
                        code_hash: code_hash_hex.clone(),
                        status: "accepted".to_owned(),
                        submitted_at,
                        completed_at: Some(now_rfc3339()),
                        message: Some(
                            "verified source already stored for this code hash".to_owned(),
                        ),
                        actual_code_hash: Some(actual_code_hash),
                        verified_source_ref: verified_source_ref_from_record(&existing),
                    }
                } else {
                    ContractVerifiedSourceJobResponseDto {
                        job_id,
                        code_hash: code_hash_hex.clone(),
                        status: "conflict".to_owned(),
                        submitted_at,
                        completed_at: Some(now_rfc3339()),
                        message: Some(
                            "a different verified source is already stored for this code hash"
                                .to_owned(),
                        ),
                        actual_code_hash: Some(actual_code_hash),
                        verified_source_ref: verified_source_ref_from_record(&existing),
                    }
                }
            } else {
                let bundle = VerifiedSourceBundle {
                    version: VERIFIED_SOURCE_VERSION,
                    language: language.clone(),
                    source_name: request.source_name.clone(),
                    source_text: source_text.clone(),
                    code_hash: code_hash_hex.clone(),
                    abi_hash: Some(hash_hex(&verified.abi_hash)),
                    compiler_fingerprint: verified.manifest.compiler_fingerprint.clone(),
                    submitted_at: submitted_at.clone(),
                };
                let verified_source_ref = persist_verified_source_bundle(&bundle, &sorafs_node)?;
                let record = StoredVerifiedSourceRecord {
                    version: VERIFIED_SOURCE_VERSION,
                    code_hash: code_hash_hex.clone(),
                    abi_hash: Some(hash_hex(&verified.abi_hash)),
                    compiler_fingerprint: verified.manifest.compiler_fingerprint.clone(),
                    language,
                    source_name: request.source_name.clone(),
                    source_text,
                    submitted_at: submitted_at.clone(),
                    manifest_id_hex: verified_source_ref
                        .as_ref()
                        .and_then(|value| value.manifest_id_hex.clone()),
                    payload_digest_hex: verified_source_ref
                        .as_ref()
                        .and_then(|value| value.payload_digest_hex.clone()),
                    content_length: verified_source_ref
                        .as_ref()
                        .and_then(|value| value.content_length),
                };
                persist_verified_source_record(&record)?;

                ContractVerifiedSourceJobResponseDto {
                    job_id,
                    code_hash: code_hash_hex.clone(),
                    status: "accepted".to_owned(),
                    submitted_at,
                    completed_at: Some(now_rfc3339()),
                    message: Some("verified source stored".to_owned()),
                    actual_code_hash: Some(actual_code_hash),
                    verified_source_ref,
                }
            }
        }
        Err(err) => ContractVerifiedSourceJobResponseDto {
            job_id,
            code_hash: code_hash_hex.clone(),
            status: "compile_error".to_owned(),
            submitted_at,
            completed_at: Some(now_rfc3339()),
            message: Some(err.to_string()),
            actual_code_hash: None,
            verified_source_ref: None,
        },
    };

    let status_code = match response.status.as_str() {
        "accepted" => StatusCode::ACCEPTED,
        "mismatch" | "compile_error" | "conflict" | "error" => StatusCode::BAD_REQUEST,
        _ => StatusCode::ACCEPTED,
    };
    let persisted = persist_job_response(response)?;
    Ok((status_code, JsonBody(persisted)))
}

pub async fn handle_get_verified_source_job(
    code_hash_hex: String,
    job_id: String,
) -> Result<impl IntoResponse, Error> {
    parse_code_hash_hex(&code_hash_hex)?;
    let job = load_verified_source_job(&code_hash_hex, &job_id)?.ok_or_else(not_found)?;
    Ok(JsonBody(job))
}

#[cfg(test)]
mod tests {
    use std::{borrow::Cow, num::NonZeroU64, time::Duration};

    use iroha_core::{
        block::{BlockBuilder, ValidBlock},
        kura::Kura,
        query::store::LiveQueryStore,
        smartcontracts::Execute,
        smartcontracts::code::{activate_instance, register_code_bytes, register_manifest},
        state::{State, World},
        tx::AcceptedTransaction,
    };
    use iroha_crypto::{Algorithm, HashOf, KeyPair};
    use iroha_data_model::{account::AccountId, permission, prelude as dm};
    use iroha_executor_data_model::permission::{
        governance::CanEnactGovernance, smart_contract::CanRegisterSmartContractCode,
    };

    use super::*;
    use crate::test_utils::TestDataDirGuard;

    fn build_state_with_single_transaction(
        instructions: Vec<dm::InstructionBox>,
    ) -> (Arc<State>, HashOf<TransactionEntrypoint>) {
        let kura = Kura::blank_kura_for_testing();
        let query = LiveQueryStore::start_test();
        let state = Arc::new(State::new_for_testing(
            World::default(),
            kura.clone(),
            query,
        ));

        let chain: dm::ChainId = "test-chain".parse().expect("chain");
        let authority_key = KeyPair::random_with_algorithm(Algorithm::Ed25519);
        let authority = dm::AccountId::new(authority_key.public_key().clone());
        let mut builder = dm::TransactionBuilder::new(chain, authority);
        builder.set_creation_time(Duration::from_millis(1_710_000_000_000));
        let signed = builder
            .with_instructions(instructions)
            .sign(authority_key.private_key());
        let target_hash = signed.hash_as_entrypoint();

        let leader = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let block = BlockBuilder::new(vec![AcceptedTransaction::new_unchecked(Cow::Owned(signed))])
            .chain(0, state.view().latest_block().as_deref())
            .sign(leader.private_key())
            .unpack(|_| {});
        let mut state_block = state.block(block.header());
        let valid: ValidBlock = block
            .validate_and_record_transactions(&mut state_block)
            .unpack(|_| {});
        let committed = valid.commit_unchecked().unpack(|_| {});
        crate::test_utils::finalize_committed_block(&state, state_block, committed);

        (state, target_hash)
    }

    fn install_contract_instance(
        state: &State,
        authority: &AccountId,
        authority_keypair: &KeyPair,
        contract_address: &dm::ContractAddress,
        code: Vec<u8>,
    ) -> Hash {
        let mut block = state.block(dm::BlockHeader::new(
            NonZeroU64::new(1).expect("height"),
            None,
            None,
            None,
            0,
            0,
        ));
        let mut stx = block.transaction();

        let register_permission: permission::Permission = CanRegisterSmartContractCode.into();
        dm::Grant::account_permission(register_permission, authority.clone())
            .execute(authority, &mut stx)
            .expect("grant CanRegisterSmartContractCode");

        let enact_permission: permission::Permission = CanEnactGovernance.into();
        dm::Grant::account_permission(enact_permission, authority.clone())
            .execute(authority, &mut stx)
            .expect("grant CanEnactGovernance");

        let verified = ivm::verify_contract_artifact(&code).expect("verify contract artifact");
        let code_hash =
            register_code_bytes(authority, code, &mut stx).expect("register contract bytes");
        let manifest = verified.manifest.signed(authority_keypair);
        register_manifest(authority, manifest, &mut stx).expect("register manifest");
        activate_instance(authority, contract_address.clone(), code_hash, &mut stx)
            .expect("activate instance");

        stx.apply();
        block.commit().expect("commit block");
        code_hash
    }

    #[tokio::test]
    async fn instruction_contract_view_renders_pseudo_source_for_register_bytes() {
        let _guard = TestDataDirGuard::new();
        let program = crate::test_utils::minimal_ivm_program(1);
        let code_hash = canonical_code_hash(&program).expect("canonical hash");
        let instruction = dm::InstructionBox::from(RegisterSmartContractBytes {
            code_hash,
            code: program,
        });
        let (state, hash) = build_state_with_single_transaction(vec![instruction]);

        let response = handle_get_instruction_contract_view(state, hash.to_string(), 0)
            .await
            .expect("contract view response")
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: ContractCodeViewDto =
            norito::json::from_slice(&body).expect("decode contract view");
        assert_eq!(payload.rendered_source_kind, RENDERED_SOURCE_PSEUDO);
        assert!(payload.rendered_source_text.contains("public fn main()"));
        assert!(!payload.entrypoints.is_empty());
    }

    #[tokio::test]
    async fn code_hash_contract_view_prefers_verified_source_record() {
        let _guard = TestDataDirGuard::new();
        let authority_keypair = KeyPair::random();
        let authority = dm::AccountId::new(authority_keypair.public_key().clone());
        let world = crate::test_utils::world_with_authority(&authority);
        let state = Arc::new(State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));

        let code = crate::test_utils::minimal_ivm_program(1);
        let contract_address = dm::ContractAddress::derive(0, &authority, 0, dm::DataSpaceId::GLOBAL)
            .expect("contract address");
        let code_hash = install_contract_instance(
            state.as_ref(),
            &authority,
            &authority_keypair,
            &contract_address,
            code,
        );
        let code_hash_hex = hash_hex(&code_hash);
        let record = StoredVerifiedSourceRecord {
            version: VERIFIED_SOURCE_VERSION,
            code_hash: code_hash_hex.clone(),
            abi_hash: None,
            compiler_fingerprint: Some("torii-tests".to_owned()),
            language: VERIFIED_SOURCE_LANGUAGE_KOTODAMA.to_owned(),
            source_name: Some("demo.ko".to_owned()),
            source_text: "kotoage fn main() {}".to_owned(),
            submitted_at: now_rfc3339(),
            manifest_id_hex: Some("aa".repeat(16)),
            payload_digest_hex: Some("bb".repeat(32)),
            content_length: Some(24),
        };
        persist_verified_source_record(&record).expect("persist verified source");

        let response = handle_get_contract_code_view(state, code_hash_hex)
            .await
            .expect("contract view response")
            .into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("body");
        let payload: ContractCodeViewDto =
            norito::json::from_slice(&body).expect("decode contract view");
        assert_eq!(payload.rendered_source_kind, RENDERED_SOURCE_VERIFIED);
        assert_eq!(payload.rendered_source_text, "kotoage fn main() {}");
        assert!(payload.verified_source_ref.is_some());
    }

    #[tokio::test]
    async fn verified_source_job_accepts_exact_match_and_persists_record() {
        let _guard = TestDataDirGuard::new();
        let source = r#"
kotoage fn main() {}
"#;

        let (compiled, _, _) = ivm::KotodamaCompiler::new()
            .compile_source_with_manifest_and_report(source)
            .expect("compile contract");
        let code_hash_hex = hash_hex(&canonical_code_hash(&compiled).expect("canonical hash"));
        let node = sorafs_node::NodeHandle::new(
            sorafs_node::config::StorageConfig::builder()
                .enabled(true)
                .data_dir(_guard.path().join("sorafs"))
                .build(),
        );

        let (status, JsonBody(response)) = handle_post_verified_source_job(
            code_hash_hex.clone(),
            SubmitVerifiedContractSourceDto {
                language: VERIFIED_SOURCE_LANGUAGE_KOTODAMA.to_owned(),
                source_name: Some("demo.ko".to_owned()),
                source_text: source.to_owned(),
            },
            node,
        )
        .await
        .expect("submit verified source");

        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(response.status, "accepted");
        assert!(response.verified_source_ref.is_some());

        let record = load_verified_source_record(&code_hash_hex)
            .expect("load record")
            .expect("record exists");
        assert_eq!(record.source_text.trim(), source.trim());
        assert_eq!(record.language, VERIFIED_SOURCE_LANGUAGE_KOTODAMA);
    }

    #[tokio::test]
    async fn verified_source_job_reports_hash_mismatch() {
        let _guard = TestDataDirGuard::new();
        let source = "kotoage fn main() {}";
        let wrong_hash = "11".repeat(32);
        let node = sorafs_node::NodeHandle::new(sorafs_node::config::StorageConfig::default());

        let (status, JsonBody(response)) = handle_post_verified_source_job(
            wrong_hash.clone(),
            SubmitVerifiedContractSourceDto {
                language: VERIFIED_SOURCE_LANGUAGE_KOTODAMA.to_owned(),
                source_name: None,
                source_text: source.to_owned(),
            },
            node,
        )
        .await
        .expect("submit mismatch");

        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(response.status, "mismatch");
        assert!(response.actual_code_hash.is_some());

        let stored = load_verified_source_job(&wrong_hash, &response.job_id)
            .expect("load job")
            .expect("job exists");
        assert_eq!(stored.status, "mismatch");
    }
}
