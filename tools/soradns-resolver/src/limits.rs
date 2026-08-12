//! Fixed resource corridors for resolver inputs and retained state.
//!
//! The resolver accepts local operator files and remote Torii/SoraFS payloads.
//! Keeping their byte, decode, collection, and retained-memory limits together
//! makes the first-release admission contract explicit and auditable.

use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use eyre::{Result, WrapErr, bail, eyre};
use norito::{DecodeLimits, json};

/// Maximum speculative capacity reserved before bytes have been observed (64 KiB).
const MAX_INITIAL_READ_RESERVATION: usize = 64 * 1024;

/// Maximum size of the resolver's Norito JSON configuration (1 MiB).
pub const MAX_CONFIG_BYTES: usize = 1024 * 1024;
/// Maximum encoded size of one proof bundle (1 MiB).
pub const MAX_PROOF_BUNDLE_BYTES: usize = 1024 * 1024;
/// Maximum encoded size of one RAD snapshot (16 MiB).
pub const MAX_RAD_SNAPSHOT_BYTES: usize = 16 * 1024 * 1024;
/// Maximum size of a directory record JSON document (256 KiB).
pub const MAX_DIRECTORY_RECORD_BYTES: usize = 256 * 1024;
/// Maximum size of a canonical directory listing (16 MiB).
pub const MAX_DIRECTORY_JSON_BYTES: usize = 16 * 1024 * 1024;
/// Maximum size of a DoT certificate input (1 MiB).
pub const MAX_TLS_CERT_BYTES: usize = 1024 * 1024;
/// Maximum size of a DoT private-key input (256 KiB).
pub const MAX_TLS_KEY_BYTES: usize = 256 * 1024;

/// Maximum number of sources of either kind in one configuration.
pub const MAX_SOURCES_PER_KIND: usize = 256;
/// Maximum total object references across all configured bundle sources.
pub const MAX_SOURCE_REFERENCES: usize = 4096;
/// Maximum number of request headers attached to one source.
pub const MAX_HEADERS_PER_SOURCE: usize = 64;
/// Maximum number of addresses in any one listener list.
pub const MAX_LISTEN_ADDRESSES: usize = 64;
/// Maximum number of configured static zones.
pub const MAX_STATIC_ZONES: usize = 4096;
/// Maximum total records across all configured static zones.
pub const MAX_STATIC_RECORDS: usize = 16_384;
/// Maximum RAD entries in one snapshot or directory release.
pub const MAX_RAD_ENTRIES: usize = 16_384;
/// Maximum proof bundles retained by the daemon.
pub const MAX_STATE_BUNDLES: usize = 16_384;
/// Maximum resolver adverts retained by the daemon.
pub const MAX_STATE_RAD_ENTRIES: usize = MAX_RAD_ENTRIES;
/// Maximum accounted heap retained by static zones, bundles, and RAD adverts (64 MiB).
pub const MAX_STATE_RETAINED_BYTES: usize = 64 * 1024 * 1024;
/// Maximum accounted heap retained by configured static zones (16 MiB).
pub const MAX_STATIC_ZONE_RETAINED_BYTES: usize = 16 * 1024 * 1024;
/// Maximum decoded heap retained by one proof-bundle source batch (16 MiB).
pub const MAX_SOURCE_BATCH_RETAINED_BYTES: usize = 16 * 1024 * 1024;

/// Maximum length of a general configuration or protocol string (16 KiB).
pub const MAX_FIELD_BYTES: usize = 16 * 1024;
/// Maximum length of a DNS name, CID, hash, path, or other short identifier (4 KiB).
pub const MAX_IDENTIFIER_BYTES: usize = 4 * 1024;
/// Maximum TXT chunks, freeze notes, ALPNs, fingerprints, or similar child strings.
pub const MAX_CHILD_STRINGS: usize = 256;

/// Decode limits for one proof bundle.
#[must_use]
pub const fn proof_bundle_decode_limits() -> DecodeLimits {
    DecodeLimits::new(256, 64 * 1024, 4096, 2 * 1024 * 1024, 32)
}

/// Decode limits for a RAD snapshot.
#[must_use]
pub const fn rad_snapshot_decode_limits() -> DecodeLimits {
    DecodeLimits::new(MAX_RAD_ENTRIES, 64 * 1024, 262_144, 32 * 1024 * 1024, 32)
}

/// Decode limits used while materialising the fixed resolver configuration schema.
#[must_use]
pub const fn config_decode_limits() -> DecodeLimits {
    DecodeLimits::new(
        MAX_STATIC_RECORDS,
        MAX_FIELD_BYTES,
        131_072,
        16 * 1024 * 1024,
        24,
    )
}

/// Decode limits used while materialising a directory JSON artifact.
#[must_use]
pub const fn directory_json_decode_limits() -> DecodeLimits {
    DecodeLimits::new(
        MAX_RAD_ENTRIES,
        MAX_FIELD_BYTES,
        131_072,
        32 * 1024 * 1024,
        16,
    )
}

/// Decode limits used while materialising the fixed directory-record JSON schema.
#[must_use]
pub const fn directory_record_decode_limits() -> DecodeLimits {
    DecodeLimits::new(64, MAX_FIELD_BYTES, 256, 512 * 1024, 16)
}

/// Run allocation-free JSON lexical admission before an owned decode.
pub fn preflight_json(
    bytes: &[u8],
    max_raw_bytes: usize,
    decode_limits: DecodeLimits,
    label: &str,
) -> Result<()> {
    json::preflight_slice(
        bytes,
        json::JsonPreflightLimits::from_decode_limits(max_raw_bytes, decode_limits),
    )
    .map(|_| ())
    .wrap_err_with(|| format!("{label} failed bounded JSON lexical admission"))
}

/// Read one regular local file through a stable descriptor with a hard byte ceiling.
///
/// The named path, opened descriptor, and post-read path must identify the same
/// regular file. The descriptor is read through `max_bytes + 1`, so sparse,
/// growing, and metadata-underreporting files cannot bypass the ceiling.
pub fn read_bounded_file(path: &Path, max_bytes: usize, label: &str) -> Result<Vec<u8>> {
    if max_bytes == 0 {
        bail!("{label} byte limit must be non-zero");
    }
    let named_before = fs::metadata(path)
        .wrap_err_with(|| format!("failed to inspect {label} `{}`", path.display()))?;
    if !named_before.file_type().is_file() {
        bail!("{label} `{}` is not a regular file", path.display());
    }
    admit_metadata_len(&named_before, max_bytes, label, path)?;

    let mut file = File::open(path)
        .wrap_err_with(|| format!("failed to open {label} `{}`", path.display()))?;
    let opened_before = file
        .metadata()
        .wrap_err_with(|| format!("failed to inspect opened {label} `{}`", path.display()))?;
    if !opened_before.file_type().is_file() || !same_file_snapshot(&named_before, &opened_before) {
        bail!("{label} `{}` changed while it was opened", path.display());
    }
    admit_metadata_len(&opened_before, max_bytes, label, path)?;

    let initial_capacity = usize::try_from(opened_before.len())
        .map_err(|_| eyre!("{label} length does not fit this platform"))?
        .min(MAX_INITIAL_READ_RESERVATION);
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(initial_capacity)
        .map_err(|error| eyre!("failed to reserve {label} buffer: {error}"))?;
    let read_limit = u64::try_from(max_bytes)
        .map_err(|_| eyre!("{label} byte limit does not fit u64"))?
        .checked_add(1)
        .ok_or_else(|| eyre!("{label} byte limit overflow"))?;
    let mut bounded = (&mut file).take(read_limit);
    let mut scratch = [0_u8; 8192];
    loop {
        let read = bounded
            .read(&mut scratch)
            .wrap_err_with(|| format!("failed to read {label} `{}`", path.display()))?;
        if read == 0 {
            break;
        }
        let next_len = bytes
            .len()
            .checked_add(read)
            .ok_or_else(|| eyre!("{label} length overflow"))?;
        if next_len > max_bytes {
            bail!(
                "{label} `{}` exceeds the {max_bytes}-byte limit",
                path.display()
            );
        }
        reserve_observed_append(&mut bytes, next_len, max_bytes, label)?;
        bytes.extend_from_slice(&scratch[..read]);
    }
    drop(bounded);

    let opened_after = file
        .metadata()
        .wrap_err_with(|| format!("failed to re-inspect opened {label} `{}`", path.display()))?;
    let named_after = fs::metadata(path)
        .wrap_err_with(|| format!("failed to re-inspect {label} `{}`", path.display()))?;
    let observed_len =
        u64::try_from(bytes.len()).map_err(|_| eyre!("{label} length does not fit u64"))?;
    if observed_len != opened_before.len()
        || !opened_after.file_type().is_file()
        || !named_after.file_type().is_file()
        || !same_file_snapshot(&opened_before, &opened_after)
        || !same_file_snapshot(&opened_after, &named_after)
    {
        bail!("{label} `{}` changed while it was read", path.display());
    }
    Ok(bytes)
}

/// Read one local file on the blocking pool using [`read_bounded_file`].
pub async fn read_bounded_file_async(
    path: PathBuf,
    max_bytes: usize,
    label: String,
) -> Result<Vec<u8>> {
    let task_label = label.clone();
    tokio::task::spawn_blocking(move || read_bounded_file(&path, max_bytes, &task_label))
        .await
        .wrap_err_with(|| format!("{label} reader task failed"))?
}

/// Read a successful HTTP response with Content-Length admission and a streamed hard ceiling.
///
/// Missing lengths are accepted and bounded while streaming. A declared
/// length above the ceiling is rejected before reading a body; a chunked or
/// misreported response is rejected as soon as its observed bytes exceed the
/// same ceiling.
pub async fn read_http_body_bounded(
    mut response: reqwest::Response,
    max_bytes: usize,
    label: &str,
) -> Result<Vec<u8>> {
    if !response.status().is_success() {
        bail!("{label} request failed with status {}", response.status());
    }
    if max_bytes == 0 {
        bail!("{label} byte limit must be non-zero");
    }
    let max_bytes_u64 =
        u64::try_from(max_bytes).map_err(|_| eyre!("{label} byte limit does not fit u64"))?;
    if let Some(declared) = response.content_length()
        && declared > max_bytes_u64
    {
        bail!("{label} declares {declared} bytes, exceeding the {max_bytes}-byte limit");
    }

    let initial_capacity = response
        .content_length()
        .and_then(|length| usize::try_from(length).ok())
        .unwrap_or(0)
        .min(max_bytes)
        .min(MAX_INITIAL_READ_RESERVATION);
    let mut body = Vec::new();
    body.try_reserve_exact(initial_capacity)
        .map_err(|error| eyre!("failed to reserve {label} body: {error}"))?;
    while let Some(chunk) = response
        .chunk()
        .await
        .wrap_err_with(|| format!("failed to stream {label} body"))?
    {
        let next_len = body
            .len()
            .checked_add(chunk.len())
            .ok_or_else(|| eyre!("{label} body length overflow"))?;
        if next_len > max_bytes {
            bail!("{label} exceeded the {max_bytes}-byte limit while streaming");
        }
        reserve_observed_append(&mut body, next_len, max_bytes, label)?;
        body.extend_from_slice(&chunk);
    }
    Ok(body)
}

fn reserve_observed_append(
    buffer: &mut Vec<u8>,
    next_len: usize,
    max_bytes: usize,
    label: &str,
) -> Result<()> {
    if next_len <= buffer.capacity() {
        return Ok(());
    }
    let desired_capacity = next_len
        .saturating_add(MAX_INITIAL_READ_RESERVATION)
        .min(max_bytes);
    let additional = desired_capacity
        .checked_sub(buffer.len())
        .ok_or_else(|| eyre!("{label} buffer capacity accounting underflow"))?;
    buffer
        .try_reserve_exact(additional)
        .map_err(|error| eyre!("failed to grow {label} buffer: {error}"))
}

/// Replace one retained-memory charge without allowing underflow, overflow, or cap escape.
pub(crate) fn replace_retained_bytes(
    current: usize,
    prior: usize,
    replacement: usize,
    maximum: usize,
    label: &str,
) -> Result<usize> {
    current
        .checked_sub(prior)
        .and_then(|bytes| bytes.checked_add(replacement))
        .filter(|bytes| *bytes <= maximum)
        .ok_or_else(|| eyre!("{label} exceeds the {maximum}-byte retained-memory limit"))
}

fn admit_metadata_len(
    metadata: &fs::Metadata,
    max_bytes: usize,
    label: &str,
    path: &Path,
) -> Result<()> {
    let max_bytes =
        u64::try_from(max_bytes).map_err(|_| eyre!("{label} byte limit does not fit u64"))?;
    if metadata.len() > max_bytes {
        bail!(
            "{label} `{}` declares {} bytes, exceeding the {max_bytes}-byte limit",
            path.display(),
            metadata.len()
        );
    }
    Ok(())
}

#[cfg(unix)]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt;

    left.dev() == right.dev()
        && left.ino() == right.ino()
        && left.mode() == right.mode()
        && left.len() == right.len()
        && left.mtime() == right.mtime()
        && left.mtime_nsec() == right.mtime_nsec()
        && left.ctime() == right.ctime()
        && left.ctime_nsec() == right.ctime_nsec()
}

#[cfg(not(unix))]
fn same_file_snapshot(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    left.file_type() == right.file_type()
        && left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use tempfile::NamedTempFile;
    use tokio::{
        io::{AsyncReadExt, AsyncWriteExt},
        net::TcpListener,
    };

    use super::*;

    #[test]
    fn bounded_local_read_accepts_exact_limit_and_rejects_plus_one() {
        let mut exact = NamedTempFile::new().expect("temporary input");
        exact.write_all(&[7; 64]).expect("write exact input");
        exact.flush().expect("flush exact input");
        assert_eq!(
            read_bounded_file(exact.path(), 64, "test input")
                .expect("exact boundary is admitted")
                .len(),
            64
        );

        let mut oversized = NamedTempFile::new().expect("temporary input");
        oversized
            .write_all(&[7; 65])
            .expect("write oversized input");
        oversized.flush().expect("flush oversized input");
        assert!(read_bounded_file(oversized.path(), 64, "test input").is_err());
    }

    #[tokio::test]
    async fn bounded_http_accepts_absent_content_length_at_exact_limit() {
        let url =
            spawn_response(b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n12345678".to_vec()).await;
        let response = reqwest::Client::new()
            .get(url)
            .send()
            .await
            .expect("request succeeds");
        let body = read_http_body_bounded(response, 8, "test response")
            .await
            .expect("exact body without length is admitted");
        assert_eq!(body, b"12345678");
    }

    #[tokio::test]
    async fn bounded_http_rejects_absent_length_plus_one_while_streaming() {
        let url = spawn_response(
            b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\nConnection: close\r\n\r\n9\r\n123456789\r\n0\r\n\r\n"
                .to_vec(),
        )
        .await;
        let response = reqwest::Client::new()
            .get(url)
            .send()
            .await
            .expect("request succeeds");
        assert!(
            read_http_body_bounded(response, 8, "test response")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn bounded_http_rejects_lying_oversized_content_length_before_body() {
        let url = spawn_response(
            b"HTTP/1.1 200 OK\r\nContent-Length: 9\r\nConnection: close\r\n\r\n1".to_vec(),
        )
        .await;
        let response = reqwest::Client::new()
            .get(url)
            .send()
            .await
            .expect("request succeeds");
        let error = read_http_body_bounded(response, 8, "test response")
            .await
            .expect_err("oversized declaration must fail closed");
        assert!(error.to_string().contains("declares 9 bytes"));
    }

    #[test]
    fn replacement_accounting_accepts_exact_and_rejects_plus_one() {
        assert_eq!(
            replace_retained_bytes(100, 10, 10, 100, "test map")
                .expect("exact replacement boundary"),
            100
        );
        assert!(replace_retained_bytes(100, 10, 11, 100, "test map").is_err());
    }

    async fn spawn_response(response: Vec<u8>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test listener");
        let address = listener.local_addr().expect("listener address");
        tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).await.expect("read request");
            stream.write_all(&response).await.expect("write response");
            stream.shutdown().await.expect("close response");
        });
        format!("http://{address}/")
    }
}
