#![cfg(feature = "app_api")]
use std::{
    collections::BTreeSet,
    fmt::{self, Write as _},
    fs,
    io::{self, Read as _},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};
use crate::{Error, JsonBody, data_dir};
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
    transaction::TransactionEntrypoint,
};
use ivm::analysis::ProgramAnalysis;
use mv::storage::StorageReadOnly;
const VERIFIED_SOURCE_VERSION: u32 = 1;
const VERIFIED_SOURCE_LANGUAGE_KOTODAMA: &str = "kotodama";
const FIXED_HEX_COMPONENT_BYTES_V1: usize = 32;
const FIXED_HEX_COMPONENT_CHARS_V1: usize = FIXED_HEX_COMPONENT_BYTES_V1 * 2;
const VERIFIED_SOURCE_JSON_MAX_ESCAPE_BYTES_PER_INPUT_BYTE_V1: usize = 6;
const VERIFIED_SOURCE_SUBMISSION_JSON_STRUCTURAL_BYTES_V1: usize = 1024;
/// First-release HTTP envelope ceiling for a verified-source submission.
///
/// This derives from Kotodama's canonical source and logical-path limits and
/// admits the worst-case six-byte JSON escape for every UTF-8 input byte. The
/// route therefore bounds transport memory without reducing the language's V1
/// source surface.
pub(crate) const VERIFIED_SOURCE_SUBMISSION_MAX_HTTP_BODY_BYTES_V1: usize =
    VERIFIED_SOURCE_JSON_MAX_ESCAPE_BYTES_PER_INPUT_BYTE_V1
        * (VERIFIED_SOURCE_TEXT_MAX_BYTES_V1
            + VERIFIED_SOURCE_NAME_MAX_BYTES_V1
            + VERIFIED_SOURCE_LANGUAGE_MAX_BYTES_V1)
        + VERIFIED_SOURCE_SUBMISSION_JSON_STRUCTURAL_BYTES_V1;
const VERIFIED_SOURCE_TEXT_MAX_BYTES_V1: usize = ivm::kotodama::source::MAX_SOURCE_BYTES;
const VERIFIED_SOURCE_NAME_MAX_BYTES_V1: usize =
    ivm::kotodama::linker::MAX_LOGICAL_SOURCE_PATH_BYTES;
const VERIFIED_SOURCE_LANGUAGE_MAX_BYTES_V1: usize = 32;
const VERIFIED_SOURCE_RECORD_MAX_BYTES_V1: usize =
    VERIFIED_SOURCE_SUBMISSION_MAX_HTTP_BODY_BYTES_V1 + 256 * 1024;
const VERIFIED_SOURCE_JOB_MAX_BYTES_V1: usize = 256 * 1024;
const VERIFIED_SOURCE_JOB_MESSAGE_MAX_BYTES_V1: usize = 16 * 1024;
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
    pub number: u32,
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
    Ok(ivm::contract_code_hash(code_bytes))
}
fn manifest_from_verified_artifact(
    verified: &ivm::VerifiedContractArtifact,
    code_hash: Hash,
) -> ContractManifest {
    let mut manifest = verified.manifest.clone();
    manifest.code_hash = Some(code_hash);
    manifest
}
fn parse_code_hash_hex(raw: &str) -> Result<(Hash, String), Error> {
    if raw.len() != FIXED_HEX_COMPONENT_CHARS_V1 {
        return Err(conversion_error(format!(
            "invalid code hash length {}; expected {FIXED_HEX_COMPONENT_CHARS_V1} hexadecimal characters",
            raw.len()
        )));
    }
    let mut array = [0_u8; FIXED_HEX_COMPONENT_BYTES_V1];
    hex::decode_to_slice(raw, &mut array)
        .map_err(|err| conversion_error(format!("invalid code hash: {err}")))?;
    let hash = Hash::prehashed(array);
    let canonical = hash_hex(&hash);
    Ok((hash, canonical))
}
fn canonical_verified_source_job_id(raw: &str) -> Result<String, Error> {
    if raw.len() != FIXED_HEX_COMPONENT_CHARS_V1 {
        return Err(conversion_error(format!(
            "invalid verified-source job id length {}; expected {FIXED_HEX_COMPONENT_CHARS_V1} hexadecimal characters",
            raw.len()
        )));
    }
    let mut bytes = [0_u8; FIXED_HEX_COMPONENT_BYTES_V1];
    hex::decode_to_slice(raw, &mut bytes)
        .map_err(|err| conversion_error(format!("invalid verified-source job id: {err}")))?;
    Ok(hex::encode(bytes))
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
fn decimal_u64_encoded_len(mut value: u64) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}
fn json_string_encoded_len(value: &str) -> Option<usize> {
    value.as_bytes().iter().try_fold(2_usize, |length, byte| {
        let encoded = match *byte {
            b'"' | b'\\' | b'\n' | b'\r' | b'\t' | 0x08 | 0x0c => 2,
            byte if byte < 0x20 => 6,
            _ => 1,
        };
        length.checked_add(encoded)
    })
}
fn optional_json_string_encoded_len(value: Option<&str>) -> Option<usize> {
    value.map_or(Some(4), json_string_encoded_len)
}
fn optional_json_u64_encoded_len(value: Option<u64>) -> usize {
    value.map_or(4, decimal_u64_encoded_len)
}
fn json_object_encoded_len(fields: &[(&str, usize)]) -> Option<usize> {
    let mut length = 2_usize;
    for (index, (key, value_len)) in fields.iter().enumerate() {
        if index != 0 {
            length = length.checked_add(1)?;
        }
        length = length
            .checked_add(json_string_encoded_len(key)?)?
            .checked_add(1)?
            .checked_add(*value_len)?;
    }
    Some(length)
}
fn verified_source_ref_json_encoded_len(value: &ContractVerifiedSourceRefDto) -> Option<usize> {
    let fields = [
        ("language", json_string_encoded_len(&value.language)?),
        (
            "source_name",
            optional_json_string_encoded_len(value.source_name.as_deref())?,
        ),
        (
            "submitted_at",
            json_string_encoded_len(&value.submitted_at)?,
        ),
        (
            "manifest_id_hex",
            optional_json_string_encoded_len(value.manifest_id_hex.as_deref())?,
        ),
        (
            "payload_digest_hex",
            optional_json_string_encoded_len(value.payload_digest_hex.as_deref())?,
        ),
        (
            "content_length",
            optional_json_u64_encoded_len(value.content_length),
        ),
    ];
    json_object_encoded_len(&fields)
}
fn optional_verified_source_ref_json_encoded_len(
    value: Option<&ContractVerifiedSourceRefDto>,
) -> Option<usize> {
    value.map_or(Some(4), verified_source_ref_json_encoded_len)
}
trait PersistedJsonEncodedLen {
    fn persisted_json_encoded_len(&self) -> Option<usize>;
}
impl PersistedJsonEncodedLen for StoredVerifiedSourceRecord {
    fn persisted_json_encoded_len(&self) -> Option<usize> {
        let fields = [
            ("version", decimal_u64_encoded_len(u64::from(self.version))),
            ("code_hash", json_string_encoded_len(&self.code_hash)?),
            (
                "abi_hash",
                optional_json_string_encoded_len(self.abi_hash.as_deref())?,
            ),
            (
                "compiler_fingerprint",
                optional_json_string_encoded_len(self.compiler_fingerprint.as_deref())?,
            ),
            ("language", json_string_encoded_len(&self.language)?),
            (
                "source_name",
                optional_json_string_encoded_len(self.source_name.as_deref())?,
            ),
            ("source_text", json_string_encoded_len(&self.source_text)?),
            ("submitted_at", json_string_encoded_len(&self.submitted_at)?),
            (
                "manifest_id_hex",
                optional_json_string_encoded_len(self.manifest_id_hex.as_deref())?,
            ),
            (
                "payload_digest_hex",
                optional_json_string_encoded_len(self.payload_digest_hex.as_deref())?,
            ),
            (
                "content_length",
                optional_json_u64_encoded_len(self.content_length),
            ),
        ];
        json_object_encoded_len(&fields)
    }
}
impl PersistedJsonEncodedLen for StoredVerifiedSourceJob {
    fn persisted_json_encoded_len(&self) -> Option<usize> {
        let fields = [
            ("version", decimal_u64_encoded_len(u64::from(self.version))),
            ("job_id", json_string_encoded_len(&self.job_id)?),
            ("code_hash", json_string_encoded_len(&self.code_hash)?),
            ("status", json_string_encoded_len(&self.status)?),
            ("submitted_at", json_string_encoded_len(&self.submitted_at)?),
            (
                "completed_at",
                optional_json_string_encoded_len(self.completed_at.as_deref())?,
            ),
            (
                "message",
                optional_json_string_encoded_len(self.message.as_deref())?,
            ),
            (
                "actual_code_hash",
                optional_json_string_encoded_len(self.actual_code_hash.as_deref())?,
            ),
            (
                "verified_source_ref",
                optional_verified_source_ref_json_encoded_len(self.verified_source_ref.as_ref())?,
            ),
        ];
        json_object_encoded_len(&fields)
    }
}
fn write_json_file_atomic<T: norito::json::JsonSerialize + PersistedJsonEncodedLen>(
    path: &Path,
    value: &T,
    maximum_bytes: usize,
    label: &str,
) -> Result<(), Error> {
    let encoded_len = value.persisted_json_encoded_len().ok_or_else(|| {
        conversion_error(format!("failed to size {label}: encoded length overflow"))
    })?;
    if encoded_len > maximum_bytes {
        return Err(conversion_error(format!(
            "{label} encoding is {} bytes; first-release maximum is {maximum_bytes} bytes",
            encoded_len
        )));
    }
    let mut encoded = String::new();
    encoded.try_reserve_exact(encoded_len).map_err(|err| {
        conversion_error(format!(
            "failed to reserve bounded {label} encoding ({encoded_len} bytes): {err}"
        ))
    })?;
    norito::json::JsonSerialize::json_serialize(value, &mut encoded);
    if encoded.len() != encoded_len {
        return Err(conversion_error(format!(
            "{label} encoded length {} differs from its bounded {encoded_len}-byte preflight",
            encoded.len()
        )));
    }
    let bytes = encoded.into_bytes();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|err| map_io_error(err, "failed to create contract source directory"))?;
    }
    let temp_path = path.with_extension(format!("tmp-{}", unique_suffix()));
    fs::write(&temp_path, bytes)
        .map_err(|err| map_io_error(err, "failed to write contract source file"))?;
    fs::rename(&temp_path, path)
        .map_err(|err| map_io_error(err, "failed to persist contract source file"))?;
    Ok(())
}
fn read_json_file<T: norito::json::JsonDeserializeOwned>(
    path: &Path,
    maximum_bytes: usize,
    label: &str,
) -> Result<Option<T>, Error> {
    let mut file = match fs::File::open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(map_io_error(err, "failed to open contract source file")),
    };
    let metadata = file
        .metadata()
        .map_err(|err| map_io_error(err, "failed to inspect contract source file"))?;
    if !metadata.is_file() {
        return Err(conversion_error(format!(
            "{label} path is not a regular file: {}",
            path.display()
        )));
    }
    if metadata.len() > u64::try_from(maximum_bytes).unwrap_or(u64::MAX) {
        return Err(conversion_error(format!(
            "{label} at {} exceeds the first-release {maximum_bytes}-byte maximum",
            path.display()
        )));
    }
    let file_len = usize::try_from(metadata.len()).map_err(|_| {
        conversion_error(format!(
            "{label} at {} has a length that does not fit this platform",
            path.display()
        ))
    })?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(file_len).map_err(|err| {
        conversion_error(format!(
            "failed to reserve bounded {label} read ({file_len} bytes): {err}"
        ))
    })?;
    bytes.resize(file_len, 0);
    file.read_exact(&mut bytes)
        .map_err(|err| map_io_error(err, "failed to read exact contract source file length"))?;
    let mut growth_probe = [0_u8; 1];
    if file
        .read(&mut growth_probe)
        .map_err(|err| map_io_error(err, "failed to verify contract source file length"))?
        != 0
    {
        return Err(conversion_error(format!(
            "{label} at {} grew after its bounded length preflight",
            path.display()
        )));
    }
    norito::json::from_slice(&bytes)
        .map(Some)
        .map_err(|err| conversion_error(format!("failed to decode bounded {label}: {err}")))
}
fn load_verified_source_record(
    code_hash: &str,
) -> Result<Option<StoredVerifiedSourceRecord>, Error> {
    read_json_file(
        &verified_source_record_path(code_hash),
        VERIFIED_SOURCE_RECORD_MAX_BYTES_V1,
        "verified-source record",
    )
}
fn load_verified_source_job(
    code_hash: &str,
    job_id: &str,
) -> Result<Option<ContractVerifiedSourceJobResponseDto>, Error> {
    read_json_file::<StoredVerifiedSourceJob>(
        &verified_source_job_path(code_hash, job_id),
        VERIFIED_SOURCE_JOB_MAX_BYTES_V1,
        "verified-source job",
    )
    .map(|maybe| maybe.map(Into::into))
}
fn persist_verified_source_record(record: &StoredVerifiedSourceRecord) -> Result<(), Error> {
    write_json_file_atomic(
        &verified_source_record_path(&record.code_hash),
        record,
        VERIFIED_SOURCE_RECORD_MAX_BYTES_V1,
        "verified-source record",
    )
}
fn persist_verified_source_job(job: &StoredVerifiedSourceJob) -> Result<(), Error> {
    write_json_file_atomic(
        &verified_source_job_path(&job.code_hash, &job.job_id),
        job,
        VERIFIED_SOURCE_JOB_MAX_BYTES_V1,
        "verified-source job",
    )
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
        EntryPointKind::Kotoage => "kotoage",
        EntryPointKind::View => "view",
        EntryPointKind::Hajimari => "hajimari",
        EntryPointKind::Kaizen => "kaizen",
    }
}
fn kotodama_string_literal(value: &str) -> String {
    let mut rendered = String::with_capacity(value.len().saturating_add(2));
    rendered.push('"');
    for character in value.chars() {
        match character {
            '\\' => rendered.push_str("\\\\"),
            '"' => rendered.push_str("\\\""),
            '\n' => rendered.push_str("\\n"),
            '\r' => rendered.push_str("\\r"),
            '\t' => rendered.push_str("\\t"),
            '\0' => rendered.push_str("\\0"),
            character if character.is_control() => {
                rendered.push_str(&format!("\\u{{{:x}}}", u32::from(character)));
            }
            character => rendered.push(character),
        }
    }
    rendered.push('"');
    rendered
}
fn entrypoint_signature(entrypoint: &EntrypointDescriptor) -> Result<String, &'static str> {
    let params = entrypoint
        .params
        .iter()
        .map(|param| format!("{} {}", param.type_name, param.name))
        .collect::<Vec<_>>()
        .join(", ");
    let return_type = entrypoint
        .return_type
        .as_ref()
        .map(|value| format!(" -> {value}"))
        .unwrap_or_default();
    let authorization = match entrypoint.kind {
        EntryPointKind::Kotoage => {
            let permission = entrypoint
                .permission
                .as_deref()
                .filter(|permission| !permission.trim().is_empty())
                .ok_or("kotoage entrypoint is missing caller authorization")?;
            format!(" authorize({})", kotodama_string_literal(permission))
        }
        EntryPointKind::View => match entrypoint.permission.as_deref() {
            Some(permission) if permission.trim().is_empty() => {
                return Err("view entrypoint declares an empty caller authorization");
            }
            Some(permission) => {
                format!(" authorize({})", kotodama_string_literal(permission))
            }
            None => String::new(),
        },
        EntryPointKind::Hajimari | EntryPointKind::Kaizen => {
            if entrypoint.permission.is_some() {
                return Err("lifecycle entrypoint declares forbidden source authorization");
            }
            String::new()
        }
    };
    Ok(match entrypoint.kind {
        EntryPointKind::Kotoage | EntryPointKind::View => format!(
            "{} fn {}({}){}{}",
            entrypoint_kind_label(entrypoint.kind),
            entrypoint.name,
            params,
            return_type,
            authorization
        ),
        EntryPointKind::Hajimari | EntryPointKind::Kaizen => format!(
            "{}({}){}",
            entrypoint_kind_label(entrypoint.kind),
            params,
            return_type
        ),
    })
}
fn render_program_syscalls(analysis: &ProgramAnalysis) -> String {
    if analysis.syscalls.is_empty() {
        return "none".to_owned();
    }
    analysis
        .syscalls
        .iter()
        .map(|entry| {
            let name = ivm::syscalls::syscall_name(entry.number).unwrap_or("UNKNOWN");
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
    let seiyaku_name = format!(
        "Contract_{}",
        &code_hash.chars().take(8).collect::<String>()
    );
    let mut lines = Vec::new();
    lines.push(format!("seiyaku {seiyaku_name} {{"));
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
                let Ok(signature) = entrypoint_signature(entrypoint) else {
                    lines.push(String::new());
                    lines.push(
                        "  // Invalid entrypoint descriptor omitted from pseudo-source.".to_owned(),
                    );
                    continue;
                };
                lines.push(String::new());
                lines.push(format!("  {signature} {{"));
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
        "seiyaku ManifestStub_{} {{",
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
                match entrypoint_signature(entrypoint) {
                    Ok(signature) => lines.push(format!("  // entrypoint: {signature}")),
                    Err(_) => lines.push(
                        "  // Invalid entrypoint descriptor omitted from manifest stub.".to_owned(),
                    ),
                }
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
                name: ivm::syscalls::syscall_name(entry.number).map(ToOwned::to_owned),
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
            for (entrypoint_index, entrypoint, _) in block_ref.entrypoint_results() {
                if entrypoint_index >= block_ref.external_entrypoint_count() {
                    break;
                }
                if entrypoint.hash() != target {
                    continue;
                }
                let tx = match entrypoint {
                    TransactionEntrypoint::External(tx) => tx,
                    TransactionEntrypoint::SealedReveal(reveal) => {
                        reveal.signed_transaction().clone()
                    }
                    TransactionEntrypoint::SealedCommitment(_) | TransactionEntrypoint::Time(_) => {
                        return Err(not_found());
                    }
                };
                let instruction = tx
                    .instructions()
                    .explicit_instructions()
                    .nth(lookup_index)
                    .ok_or_else(not_found)?;
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
    let verified_source_record = if let Some(record) = load_verified_source_record(&code_hash)? {
        Some(record)
    } else if let Some(declared) = declared_code_hash
        .as_ref()
        .filter(|declared| declared.as_str() != code_hash.as_str())
    {
        load_verified_source_record(declared)?
    } else {
        None
    };
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
    let (code_hash, code_hash_hex) = parse_code_hash_hex(code_hash_hex)?;
    let world = state.world_view();
    let manifest = world.contract_manifests().get(&code_hash).cloned();
    let code_bytes = world.contract_code().get(&code_hash).cloned();
    if manifest.is_none() && code_bytes.is_none() {
        return Err(not_found());
    }
    Ok(ContractViewBuildInput {
        code_hash: Some(code_hash_hex),
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
fn new_job_id(code_hash: &str, source_text: &str) -> String {
    let mut hasher = blake3::Hasher::new();
    hasher.update(b"iroha.contract.verified-source-job.v1\0");
    hasher.update(code_hash.as_bytes());
    hasher.update(b"\0");
    hasher.update(unique_suffix().as_bytes());
    hasher.update(b"\0");
    hasher.update(source_text.as_bytes());
    hasher.finalize().to_hex().to_string()
}
fn verified_source_request_bound_error(
    request: &SubmitVerifiedContractSourceDto,
) -> Option<&'static str> {
    if request.language.len() > VERIFIED_SOURCE_LANGUAGE_MAX_BYTES_V1 {
        return Some("language exceeds the first-release 32-byte maximum");
    }
    if request
        .source_name
        .as_ref()
        .is_some_and(|name| name.len() > VERIFIED_SOURCE_NAME_MAX_BYTES_V1)
    {
        return Some("source_name exceeds the Kotodama V1 4096-byte maximum");
    }
    if request.source_text.len() > VERIFIED_SOURCE_TEXT_MAX_BYTES_V1 {
        return Some("source_text exceeds the Kotodama V1 1048576-byte maximum");
    }
    None
}
struct BoundedDiagnosticText {
    text: String,
    maximum_bytes: usize,
    truncated: bool,
}
impl BoundedDiagnosticText {
    fn try_new(maximum_bytes: usize) -> Result<Self, std::collections::TryReserveError> {
        let mut text = String::new();
        text.try_reserve_exact(maximum_bytes)?;
        Ok(Self {
            text,
            maximum_bytes,
            truncated: false,
        })
    }
    fn finish(mut self) -> String {
        if self.truncated {
            let ellipsis = '…';
            if self.maximum_bytes >= ellipsis.len_utf8() {
                let mut end = self
                    .text
                    .len()
                    .min(self.maximum_bytes - ellipsis.len_utf8());
                while !self.text.is_char_boundary(end) {
                    end = end.saturating_sub(1);
                }
                self.text.truncate(end);
                self.text.push(ellipsis);
            } else {
                self.text.clear();
            }
        }
        self.text
    }
}
impl fmt::Write for BoundedDiagnosticText {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        if self.truncated {
            return Err(fmt::Error);
        }
        let remaining = self.maximum_bytes.saturating_sub(self.text.len());
        if value.len() <= remaining {
            self.text.push_str(value);
            return Ok(());
        }
        let mut end = remaining;
        while !value.is_char_boundary(end) {
            end = end.saturating_sub(1);
        }
        self.text.push_str(&value[..end]);
        self.truncated = true;
        Err(fmt::Error)
    }
}
fn write_diagnostic_source(
    output: &mut BoundedDiagnosticText,
    span: &ivm::kotodama::diagnostic::SourceSpan,
) -> fmt::Result {
    if let Some(package) = span.package_identity.as_deref() {
        write!(output, "{package}::")?;
    }
    output.write_str(span.source.as_deref().unwrap_or("<source>"))
}
/// Render compiler diagnostics directly into a fixed-size UTF-8 buffer.
///
/// In particular, this must not call `DiagnosticBundle::to_string` or
/// `render_human`: both first allocate the complete attacker-influenced
/// rendering before a caller can truncate it.
fn bounded_verified_source_diagnostic_message(
    bundle: &ivm::kotodama::diagnostic::DiagnosticBundle,
) -> String {
    let Ok(mut output) = BoundedDiagnosticText::try_new(VERIFIED_SOURCE_JOB_MESSAGE_MAX_BYTES_V1)
    else {
        return "Kotodama compilation failed (diagnostic allocation failed)".to_owned();
    };
    for (index, diagnostic) in bundle.diagnostics.iter().enumerate() {
        let rendered = (|| -> fmt::Result {
            if index != 0 {
                output.write_char('\n')?;
            }
            write!(
                output,
                "{}[{}] {}: {}",
                diagnostic.severity.as_str(),
                diagnostic.code,
                diagnostic.phase.as_str(),
                diagnostic.message
            )?;
            if let Some(span) = &diagnostic.primary_span {
                output.write_str("\n  --> ")?;
                write_diagnostic_source(&mut output, span)?;
                write!(
                    output,
                    ":{}:{}-{}:{}",
                    span.start.line, span.start.column, span.end.line, span.end.column
                )?;
                if let Some(range) = span.byte_range {
                    write!(output, " [bytes {}..{}]", range.start, range.end)?;
                }
            }
            for label in &diagnostic.labels {
                output.write_str("\n  = label: ")?;
                write_diagnostic_source(&mut output, &label.span)?;
                write!(
                    output,
                    ":{}:{}-{}:{}: {}",
                    label.span.start.line,
                    label.span.start.column,
                    label.span.end.line,
                    label.span.end.column,
                    label.message
                )?;
                if let Some(range) = label.span.byte_range {
                    write!(output, " [bytes {}..{}]", range.start, range.end)?;
                }
            }
            for note in &diagnostic.notes {
                write!(output, "\n  = note: {note}")?;
            }
            if let Some(help) = &diagnostic.help {
                write!(output, "\n  = help: {help}")?;
            }
            if let Some(fix) = &diagnostic.fix {
                output.write_str("\n  = fix: replace ")?;
                write_diagnostic_source(&mut output, &fix.span)?;
                write!(
                    output,
                    ":{}:{}-{}:{} with {:?}",
                    fix.span.start.line,
                    fix.span.start.column,
                    fix.span.end.line,
                    fix.span.end.column,
                    fix.replacement
                )?;
                if let Some(range) = fix.span.byte_range {
                    write!(output, " [bytes {}..{}]", range.start, range.end)?;
                }
            }
            Ok(())
        })();
        if rendered.is_err() {
            break;
        }
    }
    if output.text.is_empty() {
        let _ = output.write_str("Kotodama compilation failed without diagnostics");
    }
    output.finish()
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
pub fn handle_post_verified_source_job(
    code_hash_hex: String,
    request: SubmitVerifiedContractSourceDto,
    _sorafs_node: sorafs_node::NodeHandle,
) -> Result<(StatusCode, JsonBody<ContractVerifiedSourceJobResponseDto>), Error> {
    let (requested_hash, code_hash_hex) = parse_code_hash_hex(&code_hash_hex)?;
    let submitted_at = now_rfc3339();
    let job_id = new_job_id(&code_hash_hex, &request.source_text);
    if let Some(message) = verified_source_request_bound_error(&request) {
        let response = ContractVerifiedSourceJobResponseDto {
            job_id,
            code_hash: code_hash_hex,
            status: "error".to_owned(),
            submitted_at,
            completed_at: Some(now_rfc3339()),
            message: Some(message.to_owned()),
            actual_code_hash: None,
            verified_source_ref: None,
        };
        let persisted = persist_job_response(response)?;
        return Ok((StatusCode::BAD_REQUEST, JsonBody(persisted)));
    }
    let language = request.language.trim().to_ascii_lowercase();
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
    let source_name = request.source_name;
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
    let compile_result = ivm::kotodama::session::CompilerSession::default().build(
        ivm::kotodama::session::CompileRequest {
            source: &source_text,
            source_name: source_name.as_deref(),
        },
    );
    let response = match compile_result {
        Ok(output) => {
            let ivm::kotodama::session::CompileOutput {
                artifact: code_bytes,
                contract_interface,
                manifest,
                report,
            } = output;
            drop((contract_interface, manifest, report));
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
                // HTTP submission persists only the verified-source job record.
                // Provider storage is populated exclusively by the finalized-ledger
                // ingest outbox, never as a side effect of this route.
                let verified_source_ref: Option<ContractVerifiedSourceRefDto> = None;
                let record = StoredVerifiedSourceRecord {
                    version: VERIFIED_SOURCE_VERSION,
                    code_hash: code_hash_hex.clone(),
                    abi_hash: Some(hash_hex(&verified.abi_hash)),
                    compiler_fingerprint: verified.manifest.compiler_fingerprint.clone(),
                    language,
                    source_name: source_name.clone(),
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
            message: Some(bounded_verified_source_diagnostic_message(&err)),
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
    let (_, code_hash_hex) = parse_code_hash_hex(&code_hash_hex)?;
    let job_id = canonical_verified_source_job_id(&job_id)?;
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
    #[test]
    fn verified_source_request_bounds_accept_exact_and_reject_first_overflow() {
        assert_eq!(
            VERIFIED_SOURCE_TEXT_MAX_BYTES_V1,
            ivm::kotodama::source::MAX_SOURCE_BYTES
        );
        assert_eq!(
            VERIFIED_SOURCE_NAME_MAX_BYTES_V1,
            ivm::kotodama::linker::MAX_LOGICAL_SOURCE_PATH_BYTES
        );
        let mut request = SubmitVerifiedContractSourceDto {
            language: "k".repeat(VERIFIED_SOURCE_LANGUAGE_MAX_BYTES_V1),
            source_name: Some("n".repeat(VERIFIED_SOURCE_NAME_MAX_BYTES_V1)),
            source_text: "s".repeat(VERIFIED_SOURCE_TEXT_MAX_BYTES_V1),
        };
        assert_eq!(verified_source_request_bound_error(&request), None);
        request.source_text.push('s');
        assert_eq!(
            verified_source_request_bound_error(&request),
            Some("source_text exceeds the Kotodama V1 1048576-byte maximum")
        );
        request.source_text.pop();
        request.source_name.as_mut().expect("source name").push('n');
        assert_eq!(
            verified_source_request_bound_error(&request),
            Some("source_name exceeds the Kotodama V1 4096-byte maximum")
        );
        request.source_name.as_mut().expect("source name").pop();
        request.language.push('k');
        assert_eq!(
            verified_source_request_bound_error(&request),
            Some("language exceeds the first-release 32-byte maximum")
        );
    }
    #[test]
    fn verified_source_diagnostic_message_is_utf8_safe_and_bounded() {
        let short = ivm::kotodama::diagnostic::DiagnosticBundle::single(
            ivm::kotodama::diagnostic::Diagnostic::error(
                "KTEST",
                ivm::kotodama::diagnostic::DiagnosticPhase::Parse,
                "short diagnostic",
                None,
            ),
        );
        assert_eq!(
            bounded_verified_source_diagnostic_message(&short),
            short.render_human()
        );
        let long = ivm::kotodama::diagnostic::DiagnosticBundle::single(
            ivm::kotodama::diagnostic::Diagnostic::error(
                "KTEST",
                ivm::kotodama::diagnostic::DiagnosticPhase::Parse,
                "界".repeat(VERIFIED_SOURCE_JOB_MESSAGE_MAX_BYTES_V1),
                None,
            ),
        );
        let bounded = bounded_verified_source_diagnostic_message(&long);
        assert!(bounded.len() <= VERIFIED_SOURCE_JOB_MESSAGE_MAX_BYTES_V1);
        assert!(bounded.ends_with('…'));
    }
    #[test]
    fn fixed_hex_path_components_are_validated_before_decode_and_canonicalized() {
        let uppercase = "AB".repeat(FIXED_HEX_COMPONENT_BYTES_V1);
        let (_, canonical_hash) = parse_code_hash_hex(&uppercase).expect("valid code hash");
        assert_eq!(canonical_hash, "ab".repeat(FIXED_HEX_COMPONENT_BYTES_V1));
        assert_eq!(
            canonical_verified_source_job_id(&uppercase).expect("valid job id"),
            "ab".repeat(FIXED_HEX_COMPONENT_BYTES_V1)
        );
        for invalid in [
            "a".repeat(FIXED_HEX_COMPONENT_CHARS_V1 - 1),
            "a".repeat(FIXED_HEX_COMPONENT_CHARS_V1 + 1),
            "g".repeat(FIXED_HEX_COMPONENT_CHARS_V1),
            format!("../{}", "a".repeat(FIXED_HEX_COMPONENT_CHARS_V1 - 3)),
        ] {
            assert!(parse_code_hash_hex(&invalid).is_err());
            assert!(canonical_verified_source_job_id(&invalid).is_err());
        }
    }
    fn persisted_json_size_fixture() -> StoredVerifiedSourceRecord {
        StoredVerifiedSourceRecord {
            version: VERIFIED_SOURCE_VERSION,
            code_hash: "ab".repeat(FIXED_HEX_COMPONENT_BYTES_V1),
            abi_hash: Some("\"\\\n界".to_owned()),
            compiler_fingerprint: None,
            language: VERIFIED_SOURCE_LANGUAGE_KOTODAMA.to_owned(),
            source_name: Some("\u{0001}/契約.ko".to_owned()),
            source_text: "seiyaku \"quoted\" { \\ }\n".to_owned(),
            submitted_at: "2026-08-11T00:00:00Z".to_owned(),
            manifest_id_hex: Some("cd".repeat(FIXED_HEX_COMPONENT_BYTES_V1)),
            payload_digest_hex: None,
            content_length: Some(u64::MAX),
        }
    }
    #[test]
    fn persisted_json_preflight_matches_compact_norito_encoding() {
        let record = persisted_json_size_fixture();
        let record_bytes = norito::json::to_vec(&record).expect("encode source record");
        assert_eq!(
            record.persisted_json_encoded_len(),
            Some(record_bytes.len())
        );
        let job = StoredVerifiedSourceJob {
            version: VERIFIED_SOURCE_VERSION,
            job_id: "ef".repeat(FIXED_HEX_COMPONENT_BYTES_V1),
            code_hash: record.code_hash.clone(),
            status: "accepted".to_owned(),
            submitted_at: record.submitted_at.clone(),
            completed_at: Some(record.submitted_at.clone()),
            message: Some("stored\nwith warning".to_owned()),
            actual_code_hash: Some(record.code_hash.clone()),
            verified_source_ref: Some(ContractVerifiedSourceRefDto {
                language: record.language.clone(),
                source_name: record.source_name.clone(),
                submitted_at: record.submitted_at.clone(),
                manifest_id_hex: record.manifest_id_hex.clone(),
                payload_digest_hex: record.payload_digest_hex.clone(),
                content_length: record.content_length,
            }),
        };
        let job_bytes = norito::json::to_vec(&job).expect("encode source job");
        assert_eq!(job.persisted_json_encoded_len(), Some(job_bytes.len()));
    }
    #[test]
    fn persisted_json_writer_rejects_overflow_before_filesystem_mutation() {
        let directory = tempfile::tempdir().expect("verified-source writer directory");
        let record = persisted_json_size_fixture();
        let encoded_len = record
            .persisted_json_encoded_len()
            .expect("bounded record length");
        let exact_path = directory.path().join("exact").join("record.json");
        write_json_file_atomic(&exact_path, &record, encoded_len, "source writer test")
            .expect("exact-size record is admitted");
        assert_eq!(
            fs::read(&exact_path).expect("read persisted record").len(),
            encoded_len
        );
        let rejected_parent = directory.path().join("must-not-be-created");
        let rejected_path = rejected_parent.join("record.json");
        assert!(
            write_json_file_atomic(
                &rejected_path,
                &record,
                encoded_len - 1,
                "source writer test"
            )
            .is_err()
        );
        assert!(!rejected_parent.exists());
    }
    #[test]
    fn verified_source_file_reader_rejects_size_before_json_decode() {
        let directory = tempfile::tempdir().expect("verified-source reader directory");
        let exact_path = directory.path().join("exact.json");
        fs::write(&exact_path, b"null").expect("write exact JSON");
        let exact =
            read_json_file::<norito::json::Value>(&exact_path, 4, "verified-source reader test")
                .expect("exact-size JSON is admitted");
        assert!(exact.is_some());
        let overflow_path = directory.path().join("overflow.json");
        fs::write(&overflow_path, b"null ").expect("write oversized JSON");
        assert!(
            read_json_file::<norito::json::Value>(&overflow_path, 4, "verified-source reader test")
                .is_err()
        );
    }
    #[test]
    fn verified_source_job_id_has_fixed_hex_size() {
        let job_id = new_job_id(&"00".repeat(32), "seiyaku test {}");
        assert_eq!(job_id.len(), FIXED_HEX_COMPONENT_CHARS_V1);
        assert!(job_id.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
    fn checked_contract_sources_key_fixture(algorithm: Algorithm) -> KeyPair {
        KeyPair::try_random_with_algorithm(algorithm)
            .expect("generate checked contract source fixture key")
    }
    #[test]
    fn contract_sources_fixture_uses_checked_random_key_generation() {
        for algorithm in [Algorithm::Ed25519, Algorithm::BlsNormal] {
            let key_pair = checked_contract_sources_key_fixture(algorithm);
            let actual = key_pair
                .public_key()
                .try_algorithm()
                .expect("contract source fixture key advertises a valid algorithm");
            assert_eq!(actual, algorithm);
        }
    }
    #[test]
    fn pseudo_source_uses_branded_entrypoint_syntax() {
        let descriptor = |name: &str, kind| EntrypointDescriptor {
            name: name.to_owned(),
            kind,
            params: Vec::new(),
            argument_schema: None,
            return_type: None,
            return_schema: None,
            permission: (kind == EntryPointKind::Kotoage).then(|| "Run".to_owned()),
            read_keys: Vec::new(),
            write_keys: Vec::new(),
            access_hints_complete: Some(true),
            access_hints_skipped: Vec::new(),
            triggers: Vec::new(),
        };
        let mut run = descriptor("run", EntryPointKind::Kotoage);
        run.params.push(
            iroha_data_model::smart_contract::manifest::EntrypointParamDescriptor {
                name: "amount".to_owned(),
                type_name: "quantity".to_owned(),
            },
        );
        assert_eq!(
            entrypoint_signature(&run).expect("canonical kotoage signature"),
            "kotoage fn run(quantity amount) authorize(\"Run\")",
        );
        assert_eq!(
            entrypoint_signature(&descriptor("read", EntryPointKind::View))
                .expect("canonical view signature"),
            "view fn read()",
        );
        assert_eq!(
            entrypoint_signature(&descriptor("hajimari", EntryPointKind::Hajimari))
                .expect("canonical hajimari signature"),
            "hajimari()",
        );
        assert_eq!(
            entrypoint_signature(&descriptor("kaizen", EntryPointKind::Kaizen))
                .expect("canonical kaizen signature"),
            "kaizen()",
        );
        let mut typed = descriptor("write", EntryPointKind::Kotoage);
        typed.params = vec![
            iroha_data_model::smart_contract::manifest::EntrypointParamDescriptor {
                name: "amount".to_owned(),
                type_name: "quantity".to_owned(),
            },
            iroha_data_model::smart_contract::manifest::EntrypointParamDescriptor {
                name: "memo".to_owned(),
                type_name: "string".to_owned(),
            },
        ];
        typed.permission = Some("CanWrite\"Memo\\Ledger\n".to_owned());
        assert_eq!(
            entrypoint_signature(&typed).expect("escaped typed kotoage signature"),
            "kotoage fn write(quantity amount, string memo) authorize(\"CanWrite\\\"Memo\\\\Ledger\\n\")"
        );
        let manifest = ContractManifest {
            seiyaku_name: Some("Demo".to_owned()),
            code_hash: None,
            abi_hash: None,
            compiler_fingerprint: None,
            features_bitmap: None,
            access_set_hints: None,
            entrypoints: Some(vec![typed]),
            states: None,
            error_codes: None,
            kotoba: None,
            provenance: None,
        };
        let rendered = render_pseudo_source("00", Some(&manifest), None);
        assert!(rendered.contains(
            "kotoage fn write(quantity amount, string memo) authorize(\"CanWrite\\\"Memo\\\\Ledger\\n\")"
        ));
        assert!(!rendered.contains("// permission:"));
        assert_eq!(rendered.matches("CanWrite").count(), 1);
        let mut missing_authorization = descriptor("write", EntryPointKind::Kotoage);
        missing_authorization.permission = None;
        assert_eq!(
            entrypoint_signature(&missing_authorization),
            Err("kotoage entrypoint is missing caller authorization")
        );
        missing_authorization.permission = Some(" \t\n".to_owned());
        assert_eq!(
            entrypoint_signature(&missing_authorization),
            Err("kotoage entrypoint is missing caller authorization")
        );
        let mut empty_view_authorization = descriptor("read", EntryPointKind::View);
        empty_view_authorization.permission = Some(" \t\n".to_owned());
        assert_eq!(
            entrypoint_signature(&empty_view_authorization),
            Err("view entrypoint declares an empty caller authorization")
        );
        let mut forbidden_lifecycle_authorization =
            descriptor("hajimari", EntryPointKind::Hajimari);
        forbidden_lifecycle_authorization.permission = Some("Admin".to_owned());
        assert_eq!(
            entrypoint_signature(&forbidden_lifecycle_authorization),
            Err("lifecycle entrypoint declares forbidden source authorization")
        );
    }
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
        let authority_key = checked_contract_sources_key_fixture(Algorithm::Ed25519);
        let authority = dm::AccountId::new(authority_key.public_key().clone());
        let mut builder = dm::TransactionBuilder::new(
            *state.network_id_ref(),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        );
        builder.set_creation_time(Duration::from_millis(1_710_000_000_000));
        let signed = builder
            .with_instructions(instructions)
            .sign(authority_key.private_key());
        let target_hash = signed.hash_as_entrypoint();
        let leader = checked_contract_sources_key_fixture(Algorithm::BlsNormal);
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
        assert!(payload.rendered_source_text.contains("seiyaku Contract_"));
        assert!(payload.rendered_source_text.contains("view fn main()"));
        assert!(!payload.rendered_source_text.contains("public fn"));
        assert!(!payload.rendered_source_text.contains("main:"));
        assert!(!payload.entrypoints.is_empty());
    }
    #[tokio::test]
    async fn code_hash_contract_view_prefers_verified_source_record() {
        let _guard = TestDataDirGuard::new();
        let authority_keypair = checked_contract_sources_key_fixture(Algorithm::default());
        let authority = dm::AccountId::new(authority_keypair.public_key().clone());
        let world = crate::test_utils::world_with_authority(&authority);
        let state = Arc::new(State::new_for_testing(
            world,
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        ));
        let code = crate::test_utils::minimal_ivm_program(1);
        let network_id = *state.network_id_ref();
        let contract_address =
            dm::ContractAddress::derive(&network_id, &authority, 0, dm::DataSpaceId::UNIVERSAL)
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
            source_text: "seiyaku Demo { kotoage fn main() authorize(\"Run\") {} }".to_owned(),
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
        assert_eq!(
            payload.rendered_source_text,
            "seiyaku Demo { kotoage fn main() authorize(\"Run\") {} }"
        );
        assert!(payload.verified_source_ref.is_some());
    }
    #[test]
    fn verified_source_job_accepts_exact_match_and_persists_record() {
        let _guard = TestDataDirGuard::new();
        let source = r#"
seiyaku Demo { kotoage fn main() authorize("Run") {} }
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
        .expect("submit verified source");
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(response.status, "accepted");
        assert!(response.verified_source_ref.is_none());
        let record = load_verified_source_record(&code_hash_hex)
            .expect("load record")
            .expect("record exists");
        assert_eq!(record.source_text.trim(), source.trim());
        assert_eq!(record.language, VERIFIED_SOURCE_LANGUAGE_KOTODAMA);
    }
    #[test]
    fn verified_source_job_does_not_mutate_provider_storage() {
        let _guard = TestDataDirGuard::new();
        let source = "seiyaku Demo { kotoage fn main() authorize(\"Run\") {} }";
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
        let inspect_node = node.clone();
        let (status, JsonBody(response)) = handle_post_verified_source_job(
            code_hash_hex,
            SubmitVerifiedContractSourceDto {
                language: VERIFIED_SOURCE_LANGUAGE_KOTODAMA.to_owned(),
                source_name: Some("demo.ko".to_owned()),
                source_text: source.to_owned(),
            },
            node,
        )
        .expect("submit verified source");
        assert_eq!(status, StatusCode::ACCEPTED);
        assert!(response.verified_source_ref.is_none());
        assert!(
            inspect_node
                .stored_manifests()
                .expect("inspect provider storage")
                .is_empty(),
            "HTTP verified-source submission must not mutate provider storage"
        );
    }
    #[test]
    fn verified_source_job_reports_hash_mismatch() {
        let _guard = TestDataDirGuard::new();
        let source = "seiyaku Demo { kotoage fn main() authorize(\"Run\") {} }";
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
