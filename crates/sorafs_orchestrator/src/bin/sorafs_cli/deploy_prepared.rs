//! Pinned prepared-package adapter for the existing native publication flow.
//!
//! Manifest policy is preserved byte-for-byte. Native CAR/PoR verification binds the
//! reviewed file layout and payload before any registration or publisher source upload.
//!
//! `--prepared-package=PATH --prepared-package-blake3-hex=HEX32` selects a bounded
//! `sorafs.prepared_deployment.v1` JSON object with `manifest`, `chunk_plan`, `files`,
//! and `payload` entries. Each entry contains an absolute `path`, `sha256_hex`, and
//! `blake3_hex`. The chunk plan is a native fetch-plan envelope or toolkit pack report;
//! `files` is the ordered `storage prepare` path/size array (or `null` for a raw file).
//! `payload` is its flattened payload file, at most 4 GiB. Publication uses the supplied
//! canonical manifest, including its original retention epoch and replication policy.

use super::*;
use sha2::{Digest as _, Sha256};
use sorafs_car::{
    fetch_plan::{
        CHUNK_FETCH_PLAN_SCHEMA_V1, TOOLKIT_PACK_REPORT_SCHEMA_V1,
        chunk_fetch_specs_from_embedded_array, expected_payload_digest_from_json,
    },
    payload_verifier::verify_payload_reader,
    streaming_input::{NativePayloadReader, PreparedCarInput},
};
use std::io::{Seek, SeekFrom};

const SCHEMA: &str = "sorafs.prepared_deployment.v1";
const MAX_METADATA_BYTES: u64 = 16 * 1024 * 1024;

pub(super) enum DeployPayload {
    Memory(Cursor<Vec<u8>>),
    Prepared(Box<NativePayloadReader>),
}
impl Read for DeployPayload {
    fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
        match self {
            Self::Memory(reader) => reader.read(bytes),
            Self::Prepared(reader) => reader.read(bytes),
        }
    }
}
impl Seek for DeployPayload {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        match self {
            Self::Memory(reader) => reader.seek(position),
            Self::Prepared(reader) => reader.seek(position),
        }
    }
}

#[derive(norito::derive::JsonDeserialize, norito::derive::JsonSerialize)]
#[norito(deny_unknown_fields)]
struct PinnedInput {
    path: PathBuf,
    sha256_hex: String,
    blake3_hex: String,
}
impl PinnedInput {
    fn validate(&self) -> Result<(), String> {
        if !self.path.is_absolute() {
            return Err("prepared input paths must be absolute".to_owned());
        }
        digest(&self.sha256_hex)?;
        digest(&self.blake3_hex)?;
        Ok(())
    }
    fn read(&self, maximum: u64, label: &str) -> Result<Vec<u8>, String> {
        self.validate()?;
        let bytes = read_file_bounded(&self.path, maximum, label)?;
        if Sha256::digest(&bytes).as_slice() != digest(&self.sha256_hex)?
            || blake3::hash(&bytes).as_bytes() != &digest(&self.blake3_hex)?
        {
            return Err(format!(
                "prepared {label} differs from reviewed SHA-256/BLAKE3 pins"
            ));
        }
        Ok(bytes)
    }
}

#[derive(norito::derive::JsonDeserialize, norito::derive::JsonSerialize)]
#[norito(deny_unknown_fields)]
pub(super) struct PreparedPackage {
    schema: String,
    manifest: PinnedInput,
    chunk_plan: PinnedInput,
    files: PinnedInput,
    payload: PinnedInput,
}

#[derive(norito::derive::JsonDeserialize, norito::derive::JsonSerialize)]
#[norito(deny_unknown_fields)]
struct FileEntry {
    path: Vec<String>,
    size: u64,
}

fn digest(value: &str) -> Result<[u8; 32], String> {
    sorafs_car::fetch_plan::parse_digest_hex(value).map_err(|error| error.to_string())
}

pub(super) fn select(
    path: Option<PathBuf>,
    pin: Option<String>,
    payload: Option<&Path>,
) -> Result<Option<PreparedPackage>, String> {
    match (path, pin, payload) {
        (None, None, _) => Ok(None),
        (Some(path), Some(pin), None) => {
            let expected = digest(&pin)?;
            let bytes = read_file_bounded(&path, 64 * 1024, "prepared package request")?;
            if blake3::hash(&bytes).as_bytes() != &expected {
                return Err("prepared package request differs from reviewed BLAKE3 pin".to_owned());
            }
            let package: PreparedPackage = norito::json::from_slice(&bytes)
                .map_err(|error| format!("invalid prepared package request: {error}"))?;
            if package.schema != SCHEMA {
                return Err("prepared package schema must be sorafs.prepared_deployment.v1".to_owned());
            }
            for input in [&package.manifest, &package.chunk_plan, &package.files, &package.payload] {
                input.validate()?;
            }
            Ok(Some(package))
        }
        _ => Err("--prepared-package and --prepared-package-blake3-hex must appear together and exclude --payload".to_owned()),
    }
}

impl PreparedPackage {
    pub(super) fn payload_path(&self) -> PathBuf {
        self.payload.path.clone()
    }
    pub(super) fn receipt(&self) -> Value {
        to_value(self).expect("public pinned package is JSON serializable")
    }
    pub(super) fn load(&self) -> Result<DeployPackArtifacts, String> {
        let manifest_bytes = self
            .manifest
            .read(MAX_MANIFEST_ENCODED_BYTES as u64, "manifest")?;
        let manifest = decode_manifest_v1_canonical(&manifest_bytes)
            .map_err(|error| format!("invalid canonical prepared manifest: {error}"))?;
        let profile = chunk_profile_from_manifest(&manifest)?;
        let plan_bytes = self.chunk_plan.read(MAX_METADATA_BYTES, "chunk plan")?;
        let plan_value: Value = from_slice(&plan_bytes).map_err(|error| error.to_string())?;
        let (payload_digest, specs) = match plan_value.get("schema").and_then(Value::as_str) {
            Some(CHUNK_FETCH_PLAN_SCHEMA_V1) => {
                let parsed =
                    chunk_fetch_plan_from_json(&plan_value).map_err(|error| error.to_string())?;
                (parsed.payload_digest, parsed.chunk_fetch_specs)
            }
            Some(TOOLKIT_PACK_REPORT_SCHEMA_V1) => {
                let payload = expected_payload_digest_from_json(&plan_value)
                    .map_err(|error| error.to_string())?
                    .ok_or_else(|| "toolkit report lacks exact payload digest".to_owned())?;
                let specs = chunk_fetch_specs_from_embedded_array(
                    plan_value
                        .get("chunk_fetch_specs")
                        .ok_or_else(|| "toolkit report lacks chunk specs".to_owned())?,
                )
                .map_err(|error| error.to_string())?;
                (payload, specs)
            }
            _ => {
                return Err(
                    "prepared chunk plan must be a native fetch plan or toolkit pack report"
                        .to_owned(),
                );
            }
        };
        if payload_digest != digest(&self.payload.blake3_hex)? {
            return Err("prepared plan and reviewed payload digest differ".to_owned());
        }
        if specs
            .iter()
            .enumerate()
            .any(|(index, spec)| index != spec.chunk_index)
        {
            return Err(
                "prepared chunk indices must be in exact contiguous native order".to_owned(),
            );
        }
        let files_bytes = self.files.read(MAX_METADATA_BYTES, "files")?;
        // Native storage-prepare emits null for a raw single file, otherwise its ordered table.
        let entries: Option<Vec<FileEntry>> =
            from_slice(&files_bytes).map_err(|error| error.to_string())?;
        let mut plan = CarBuildPlan {
            chunk_profile: profile,
            payload_digest: blake3::Hash::from_bytes(payload_digest),
            content_length: manifest.content_length,
            chunks: specs
                .into_iter()
                .map(|spec| CarChunk {
                    offset: spec.offset,
                    length: spec.length,
                    digest: spec.digest,
                })
                .collect(),
            files: Vec::new(),
        };
        let is_directory = entries.is_some();
        let entries = entries.unwrap_or_else(|| {
            vec![FileEntry {
                path: Vec::new(),
                size: manifest.content_length,
            }]
        });
        let mut cursor = 0;
        let mut offset = 0_u64;
        for entry in entries {
            let end = offset
                .checked_add(entry.size)
                .ok_or_else(|| "prepared file length overflow".to_owned())?;
            let first = cursor;
            while offset < end {
                let chunk = plan
                    .chunks
                    .get(cursor)
                    .ok_or_else(|| "prepared file table exceeds chunks".to_owned())?;
                if chunk.offset != offset {
                    return Err("prepared chunk offsets are not contiguous".to_owned());
                }
                offset = offset
                    .checked_add(u64::from(chunk.length))
                    .ok_or_else(|| "prepared chunk offset overflow".to_owned())?;
                if offset > end {
                    return Err("prepared file boundary splits a native chunk".to_owned());
                }
                cursor += 1;
            }
            plan.files.push(FilePlan {
                path: entry.path,
                first_chunk: first,
                chunk_count: cursor - first,
                size: entry.size,
            });
        }
        if cursor != plan.chunks.len() || offset != plan.content_length {
            return Err("prepared files and chunks must cover the exact payload".to_owned());
        }
        plan.verify_manifest_metadata(&manifest)
            .map_err(|error| error.to_string())?;
        // Retain the flattened native payload using its reviewed multi-file plan. Rechunking
        // this file would discard file boundaries and change the package's DAG authority.
        let PreparedCarInput { plan, mut payload } =
            PreparedCarInput::open(&self.payload.path, plan).map_err(|error| error.to_string())?;
        verify_payload_reader(&manifest, &plan, &mut payload).map_err(|error| error.to_string())?;
        let gateway_expectations =
            file_expectations(&plan, &mut payload, &self.payload.sha256_hex, is_directory)?;
        payload.revalidate().map_err(|error| error.to_string())?;
        let manifest_digest_hex = hex_encode(
            manifest
                .digest()
                .map_err(|error| error.to_string())?
                .as_bytes(),
        );
        let root_cid_hex = hex_encode(&manifest.root_cid);
        let root_cid_base32 = encode_content_cid_base32(&manifest.root_cid);
        Ok(DeployPackArtifacts {
            plan,
            payload: DeployPayload::Prepared(Box::new(payload)),
            manifest,
            manifest_digest_hex,
            root_cid_hex,
            root_cid_base32,
            gateway_expectations,
            payload_kind: if is_directory {
                "prepared-directory"
            } else {
                "prepared-file"
            },
            payload_digest_hex: hex_encode(payload_digest),
        })
    }
}

fn file_expectations(
    plan: &CarBuildPlan,
    reader: &mut NativePayloadReader,
    sha256_hex: &str,
    directory: bool,
) -> Result<Vec<GatewayExpectation>, String> {
    reader.rewind().map_err(|error| error.to_string())?;
    let mut sha = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    let mut expectations = Vec::new();
    for file in &plan.files {
        let mut remaining = file.size;
        let mut digest = blake3::Hasher::new();
        while remaining > 0 {
            let count = usize::try_from(remaining.min(buffer.len() as u64))
                .map_err(|error| error.to_string())?;
            reader
                .read_exact(&mut buffer[..count])
                .map_err(|error| error.to_string())?;
            sha.update(&buffer[..count]);
            digest.update(&buffer[..count]);
            remaining -= count as u64;
        }
        let expected = GatewayExpectation {
            path: directory.then(|| file.path.clone()),
            bytes: file.size,
            blake3_hex: digest.finalize().to_hex().to_string(),
        };
        if directory && file.path.as_slice() == ["index.html"] {
            expectations.insert(
                0,
                GatewayExpectation {
                    path: None,
                    ..expected.clone()
                },
            );
        }
        expectations.push(expected);
    }
    if reader
        .read(&mut buffer[..1])
        .map_err(|error| error.to_string())?
        != 0
        || sha.finalize().as_slice() != digest(sha256_hex)?
    {
        return Err("prepared payload differs from reviewed SHA-256 pin or exact EOF".to_owned());
    }
    reader.rewind().map_err(|error| error.to_string())?;
    Ok(expectations)
}

#[cfg(test)]
#[path = "deploy_prepared/tests.rs"]
mod tests;
