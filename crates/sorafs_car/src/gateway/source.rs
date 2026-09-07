//! Bounded complete native plan and payload acquisition over the existing gateway protocol.
use super::*;
use crate::{CarBuildPlan, CarChunk, CarStreamingWriter, FilePlan};
use std::io;

/// Immutable resource limits for one authenticated provider source fetch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GatewaySourceLimitsV1 {
    /// Maximum retained unchunked payload; at most 64 MiB.
    pub max_payload_bytes: u64,
    /// Maximum complete file inventory; at most 4096 entries.
    pub max_files: usize,
    /// Maximum complete chunk inventory; at most 4096 entries.
    pub max_chunks: usize,
    /// Maximum bytes in each plan metadata response; at most one MiB.
    pub max_page_bytes: usize,
    /// Maximum metadata pages; at most 256.
    pub max_pages: usize,
    /// Requested file and chunk entries per page; at most 64.
    pub page_entries: usize,
}
impl GatewaySourceLimitsV1 {
    /// Reject zero, excessive or internally inconsistent configured bounds.
    pub fn validate(self) -> Result<(), GatewaySourceErrorV1> {
        if self.max_payload_bytes == 0
            || self.max_payload_bytes > 64 * 1024 * 1024
            || self.max_files == 0
            || self.max_files > 4096
            || self.max_chunks == 0
            || self.max_chunks > 4096
            || self.max_page_bytes == 0
            || self.max_page_bytes > 1024 * 1024
            || self.max_pages == 0
            || self.max_pages > 256
            || self.page_entries == 0
            || self.page_entries > 64
            || self.max_page_bytes * self.max_pages > 16 * 1024 * 1024
            || self.max_pages * self.page_entries < self.max_files.max(self.max_chunks)
        {
            return Err(GatewaySourceErrorV1::Bounds);
        }
        Ok(())
    }
}
/// Payload-free error categories; endpoint URLs, credentials and bytes never appear.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum GatewaySourceErrorV1 {
    /// Configured or observed resource limits were exceeded.
    #[error("provider source exceeded configured bounds")]
    Bounds,
    /// Remote transport was unavailable or returned a non-success status.
    #[error("provider source transport unavailable")]
    Unavailable,
    /// Source metadata or bytes do not match the exact native manifest.
    #[error("provider source content rejected")]
    ContentRejected,
}
/// Complete native source bytes, verified before any caller can read them.
///
/// Retains one bounded payload buffer, never a second complete CAR buffer. Acquisition consumes
/// every HTTP response through exact EOF; the native writer then reproduces the complete CAR
/// commitment and PoR. The caller remains responsible for current governed source authorization.
pub struct GatewayVerifiedPayloadV1 {
    manifest: ManifestV1,
    plan: CarBuildPlan,
    payload: Vec<u8>,
}
impl fmt::Debug for GatewayVerifiedPayloadV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GatewayVerifiedPayloadV1")
            .field("content_length", &self.plan.content_length)
            .finish_non_exhaustive()
    }
}
impl GatewayVerifiedPayloadV1 {
    /// Transfer the verified manifest, exact complete plan and retained public payload.
    #[must_use]
    pub fn into_parts(self) -> (ManifestV1, CarBuildPlan, Vec<u8>) {
        (self.manifest, self.plan, self.payload)
    }
}
impl GatewayFetchContext {
    /// Fetch every paginated file/chunk entry and bind the complete native plan to `manifest`.
    ///
    /// Exactly one provider must be configured: authenticated source retry belongs to the
    /// governed source pool. Metadata pages may never switch provider or manifest identity.
    pub async fn fetch_bound_plan_v1(
        &self,
        manifest: &ManifestV1,
        limits: GatewaySourceLimitsV1,
    ) -> Result<CarBuildPlan, GatewaySourceErrorV1> {
        limits.validate()?;
        let rejected = GatewaySourceErrorV1::ContentRejected;
        sorafs_manifest::validate_manifest(
            manifest,
            &sorafs_manifest::PinPolicyConstraints::default(),
        )
        .map_err(|_| rejected)?;
        if manifest.content_length > limits.max_payload_bytes {
            return Err(GatewaySourceErrorV1::Bounds);
        }
        let inner = &self.fetcher.inner;
        if inner.providers.len() != 1 {
            return Err(rejected);
        }
        if hex::encode(manifest.digest().map_err(|_| rejected)?.as_bytes()) != inner.manifest_id_hex
        {
            return Err(rejected);
        }
        let descriptor = sorafs_manifest::validate_registered_chunker_profile(&manifest.chunking)
            .map_err(|_| rejected)?;
        let profile_handle = format!(
            "{}.{}@{}",
            descriptor.namespace, descriptor.name, descriptor.semver
        );
        if inner.chunker_header.to_str().ok() != Some(profile_handle.as_str()) {
            return Err(rejected);
        }
        let runtime = inner.providers.values().next().ok_or(rejected)?;
        let mut chunks = Vec::new();
        let mut files = Vec::new();
        let mut totals = None;
        let mut payload_digest = None;
        let mut file_offset = 0_u64;
        for page in 0..limits.max_pages {
            let offset = page
                .checked_mul(limits.page_entries)
                .ok_or(GatewaySourceErrorV1::Bounds)?;
            let url = runtime
                .base_url
                .join(&format!(
                    "v1/sorafs/storage/plan/{}?offset={offset}&limit={}",
                    inner.manifest_id_hex, limits.page_entries,
                ))
                .map_err(|_| rejected)?;
            let mut headers = HeaderMap::new();
            if let Some(envelope) = &inner.manifest_envelope {
                headers.insert(
                    HeaderName::from_static(HEADER_SORA_MANIFEST_ENVELOPE),
                    envelope.clone(),
                );
            }
            if let Some(client) = &inner.client_header {
                headers.insert(HeaderName::from_static(HEADER_SORA_CLIENT), client.clone());
            }
            let response = inner
                .engine
                .get(HttpRequest {
                    url,
                    headers,
                    max_response_bytes: limits.max_page_bytes,
                })
                .await
                .map_err(|_| GatewaySourceErrorV1::Unavailable)?;
            if response.status != StatusCode::OK {
                return Err(GatewaySourceErrorV1::Unavailable);
            }
            if response.body.len() > limits.max_page_bytes {
                return Err(GatewaySourceErrorV1::Bounds);
            }
            if let Some(expected) = &inner.cache_version
                && observed_cache_version(&response.headers).as_deref() != Some(expected.as_str())
            {
                return Err(rejected);
            }
            let root: Value = json::from_slice(&response.body).map_err(|_| rejected)?;
            exact_fields(&root, &["manifest_id_hex", "plan"])?;
            if root.get("manifest_id_hex").and_then(Value::as_str)
                != Some(inner.manifest_id_hex.as_str())
            {
                return Err(rejected);
            }
            let plan = root.get("plan").ok_or(rejected)?;
            exact_fields(
                plan,
                &[
                    "chunk_count",
                    "offset",
                    "returned_chunk_count",
                    "limit",
                    "truncated_chunks",
                    "content_length",
                    "payload_digest_blake3",
                    "chunk_profile_handle",
                    "file_count",
                    "returned_file_count",
                    "truncated_files",
                    "files",
                    "chunk_digest_count",
                    "returned_chunk_digest_count",
                    "truncated_chunk_digests",
                    "chunk_digests_blake3",
                    "chunks",
                ],
            )?;
            let chunk_count = usize_field(plan, "chunk_count")?;
            let file_count = usize_field(plan, "file_count")?;
            if chunk_count > limits.max_chunks || file_count > limits.max_files {
                return Err(GatewaySourceErrorV1::Bounds);
            }
            let digest = digest_field(plan, "payload_digest_blake3")?;
            if totals.is_some_and(|expected| expected != (chunk_count, file_count))
                || payload_digest.is_some_and(|expected| expected != digest)
                || usize_field(plan, "offset")? != offset
                || usize_field(plan, "limit")? != limits.page_entries
                || number_field(plan, "content_length")? != manifest.content_length
                || plan.get("chunk_profile_handle").and_then(Value::as_str)
                    != Some(profile_handle.as_str())
                || usize_field(plan, "chunk_digest_count")? != chunk_count
            {
                return Err(rejected);
            }
            totals = Some((chunk_count, file_count));
            payload_digest = Some(digest);
            let page_chunks = array_field(plan, "chunks")?;
            let page_files = array_field(plan, "files")?;
            let page_digests = array_field(plan, "chunk_digests_blake3")?;
            let expected_chunks = chunk_count.saturating_sub(offset).min(limits.page_entries);
            let expected_files = file_count.saturating_sub(offset).min(limits.page_entries);
            if page_chunks.len() != expected_chunks
                || page_files.len() != expected_files
                || page_digests.len() != expected_chunks
                || usize_field(plan, "returned_chunk_count")? != expected_chunks
                || usize_field(plan, "returned_file_count")? != expected_files
                || usize_field(plan, "returned_chunk_digest_count")? != expected_chunks
                || boolean_field(plan, "truncated_chunks")?
                    != (offset + expected_chunks < chunk_count)
                || boolean_field(plan, "truncated_files")? != (offset + expected_files < file_count)
                || boolean_field(plan, "truncated_chunk_digests")?
                    != (offset + expected_chunks < chunk_count)
            {
                return Err(rejected);
            }
            for (entry, digest_entry) in page_chunks.iter().zip(page_digests) {
                exact_fields(entry, &["chunk_index", "offset", "length", "digest_blake3"])?;
                let digest = digest_field(entry, "digest_blake3")?;
                if usize_field(entry, "chunk_index")? != chunks.len()
                    || digest_entry.as_str() != Some(hex::encode(digest).as_str())
                {
                    return Err(rejected);
                }
                chunks.push(CarChunk {
                    offset: number_field(entry, "offset")?,
                    length: u32::try_from(number_field(entry, "length")?).map_err(|_| rejected)?,
                    digest,
                });
            }
            for entry in page_files {
                exact_fields(
                    entry,
                    &["path", "offset", "size", "first_chunk", "chunk_count"],
                )?;
                let size = number_field(entry, "size")?;
                if number_field(entry, "offset")? != file_offset {
                    return Err(rejected);
                }
                file_offset = file_offset.checked_add(size).ok_or(rejected)?;
                let path = array_field(entry, "path")?
                    .iter()
                    .map(|part| part.as_str().map(str::to_owned).ok_or(rejected))
                    .collect::<Result<Vec<_>, _>>()?;
                files.push(FilePlan {
                    path,
                    first_chunk: usize_field(entry, "first_chunk")?,
                    chunk_count: usize_field(entry, "chunk_count")?,
                    size,
                });
            }
            if chunks.len() == chunk_count && files.len() == file_count {
                let plan = CarBuildPlan {
                    chunk_profile: descriptor.profile,
                    payload_digest: blake3::Hash::from(digest),
                    content_length: manifest.content_length,
                    chunks,
                    files,
                };
                plan.validate_for_ingest().map_err(|_| rejected)?;
                if crate::compute_chunk_plan_digest_sha3(&plan.chunks)
                    != manifest.chunk_digest_sha3_256
                {
                    return Err(rejected);
                }
                return Ok(plan);
            }
        }
        Err(GatewaySourceErrorV1::Bounds)
    }
    /// Acquire one exact native payload with complete HTTP EOF and all manifest commitments checked.
    ///
    /// This intentionally retains a bounded payload before exposing it. No completion may rely
    /// merely on metadata or successful HTTP status. The outer runtime supplies an absolute deadline.
    pub async fn fetch_verified_payload_v1(
        &self,
        expected_manifest: &ManifestV1,
        limits: GatewaySourceLimitsV1,
    ) -> Result<GatewayVerifiedPayloadV1, GatewaySourceErrorV1> {
        limits.validate()?;
        if expected_manifest.content_length > limits.max_payload_bytes {
            return Err(GatewaySourceErrorV1::Bounds);
        }
        let rejected = GatewaySourceErrorV1::ContentRejected;
        let fetched = self.fetch_manifest().await.map_err(|error| match error {
            GatewayManifestError::Request { .. } | GatewayManifestError::Status { .. } => {
                GatewaySourceErrorV1::Unavailable
            }
            _ => GatewaySourceErrorV1::ContentRejected,
        })?;
        if fetched.manifest != *expected_manifest {
            return Err(rejected);
        }
        let plan = self.fetch_bound_plan_v1(expected_manifest, limits).await?;
        if fetched.payload_digest != plan.payload_digest
            || fetched.chunk_count != plan.chunks.len() as u64
        {
            return Err(rejected);
        }
        let capacity =
            usize::try_from(plan.content_length).map_err(|_| GatewaySourceErrorV1::Bounds)?;
        let mut payload = Vec::new();
        payload
            .try_reserve_exact(capacity)
            .map_err(|_| GatewaySourceErrorV1::Bounds)?;
        let provider = Arc::new(self.providers.first().ok_or(rejected)?.clone());
        for spec in plan.try_chunk_fetch_specs().map_err(|_| rejected)? {
            let response = self
                .fetcher
                .fetch(FetchRequest {
                    provider: Arc::clone(&provider),
                    spec: spec.clone(),
                    attempt: 1,
                })
                .await
                .map_err(|_| GatewaySourceErrorV1::Unavailable)?;
            if response.bytes.len() != spec.length as usize
                || blake3::hash(&response.bytes).as_bytes() != &spec.digest
            {
                return Err(rejected);
            }
            if payload
                .len()
                .checked_add(response.bytes.len())
                .filter(|len| *len <= capacity)
                .is_none()
            {
                return Err(rejected);
            }
            payload.extend_from_slice(&response.bytes);
        }
        if payload.len() != capacity
            || blake3::hash(&payload) != plan.payload_digest
            || crate::compute_por_root(&payload, &plan).map_err(|_| rejected)?
                != expected_manifest.por_root
        {
            return Err(rejected);
        }
        let stats = CarStreamingWriter::new(&plan)
            .write_from_reader(&mut payload.as_slice(), &mut io::sink())
            .map_err(|_| rejected)?;
        if stats.root_cids.as_slice() != [expected_manifest.root_cid.clone()]
            || stats.dag_codec != expected_manifest.dag_codec.0
            || stats.car_size != expected_manifest.car_size
            || stats.car_archive_digest.as_bytes() != &expected_manifest.car_digest
        {
            return Err(rejected);
        }
        Ok(GatewayVerifiedPayloadV1 {
            manifest: fetched.manifest,
            plan,
            payload,
        })
    }
}
fn exact_fields(value: &Value, fields: &[&str]) -> Result<(), GatewaySourceErrorV1> {
    let object = value
        .as_object()
        .ok_or(GatewaySourceErrorV1::ContentRejected)?;
    if object.len() != fields.len() || fields.iter().any(|field| !object.contains_key(*field)) {
        return Err(GatewaySourceErrorV1::ContentRejected);
    }
    Ok(())
}
fn number_field(value: &Value, name: &str) -> Result<u64, GatewaySourceErrorV1> {
    value
        .get(name)
        .and_then(Value::as_u64)
        .ok_or(GatewaySourceErrorV1::ContentRejected)
}
fn usize_field(value: &Value, name: &str) -> Result<usize, GatewaySourceErrorV1> {
    usize::try_from(number_field(value, name)?).map_err(|_| GatewaySourceErrorV1::Bounds)
}
fn boolean_field(value: &Value, name: &str) -> Result<bool, GatewaySourceErrorV1> {
    value
        .get(name)
        .and_then(Value::as_bool)
        .ok_or(GatewaySourceErrorV1::ContentRejected)
}
fn array_field<'a>(value: &'a Value, name: &str) -> Result<&'a Vec<Value>, GatewaySourceErrorV1> {
    value
        .get(name)
        .and_then(Value::as_array)
        .ok_or(GatewaySourceErrorV1::ContentRejected)
}
fn digest_field(value: &Value, name: &str) -> Result<[u8; 32], GatewaySourceErrorV1> {
    let hex = value
        .get(name)
        .and_then(Value::as_str)
        .ok_or(GatewaySourceErrorV1::ContentRejected)?;
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(GatewaySourceErrorV1::ContentRejected);
    }
    let mut digest = [0; 32];
    hex::decode_to_slice(hex, &mut digest).map_err(|_| GatewaySourceErrorV1::ContentRejected)?;
    Ok(digest)
}
#[cfg(test)]
mod tests;
