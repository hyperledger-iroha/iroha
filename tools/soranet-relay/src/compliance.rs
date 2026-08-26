//! Compliance logging for relay handshake events.
//!
//! Logs are emitted in JSON Lines format, one event per line. Potential identifiers are hashed with
//! a mandatory private BLAKE3 key so operators can correlate events without exposing raw values or
//! an enumerable unsalted digest.
#[cfg(unix)]
use crate::config::{O_NOFOLLOW_FLAG, effective_uid};
use crate::{
    capability::NegotiatedCapabilities,
    config::{ComplianceConfig, RelayMode},
};
use blake3::Hasher as Blake3Hasher;
use iroha_crypto::soranet::handshake::HandshakeSuite;
use norito::json::{self, Map, Value};
#[cfg(unix)]
use std::fs::OpenOptions;
#[cfg(unix)]
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::{
    fs::{self, File},
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

#[cfg(unix)]
const COMPLIANCE_FILE_MODE: u32 = 0o600;
const HASH_DOMAIN_REMOTE: &[u8] = b"iroha.soranet.compliance.remote.v1";
const HASH_DOMAIN_DESCRIPTOR: &[u8] = b"iroha.soranet.compliance.descriptor.v1";
const HASH_DOMAIN_CIRCUIT: &[u8] = b"iroha.soranet.compliance.circuit.v1";
const HASH_DOMAIN_CHANNEL: &[u8] = b"iroha.soranet.compliance.channel.v1";
const HASH_DOMAIN_ROUTE: &[u8] = b"iroha.soranet.compliance.route.v1";
const HASH_DOMAIN_STREAM: &[u8] = b"iroha.soranet.compliance.stream.v1";
const HASH_DOMAIN_ROOM: &[u8] = b"iroha.soranet.compliance.room.v1";
const HASH_DOMAIN_EXIT_MULTIADDR: &[u8] = b"iroha.soranet.compliance.exit-multiaddr.v1";
const HASH_DOMAIN_ADAPTER_TARGET: &[u8] = b"iroha.soranet.compliance.adapter-target.v1";
const HASH_DOMAIN_MEASUREMENT: &[u8] = b"iroha.soranet.compliance.measurement.v1";
const HASH_DOMAIN_RELAY: &[u8] = b"iroha.soranet.compliance.relay.v1";
const HASH_DOMAIN_VERIFIER: &[u8] = b"iroha.soranet.compliance.verifier.v1";

/// Logger that writes compliance events to a JSON Lines file.
pub struct ComplianceLogger {
    path: PathBuf,
    hash_key: [u8; 32],
    max_bytes: u64,
    max_backups: u8,
    pipeline_spool_dir: Option<PathBuf>,
    writer: Mutex<File>,
}
/// Structured metadata describing a throttling decision for compliance logs.
#[derive(Debug, Clone, Copy)]
pub struct ThrottleAudit {
    /// Throttle scope label (e.g., quota, cooldown).
    pub scope: &'static str,
    /// Optional cooldown duration applied.
    pub cooldown: Option<Duration>,
    /// Optional quota window duration applied.
    pub window: Option<Duration>,
    /// Optional burst limit applied to the throttle.
    pub burst_limit: Option<u32>,
    /// Optional maximum entries enforced by the throttle.
    pub max_entries: Option<usize>,
    /// Optional gap observed between attempts when throttled.
    pub observed_gap: Option<Duration>,
}
impl ComplianceLogger {
    /// Construct a logger from the validated configuration.
    ///
    /// # Errors
    /// Returns an error if the log file cannot be opened or the parent directory cannot be created.
    pub fn from_config(config: &ComplianceConfig) -> Result<Option<Self>, ComplianceError> {
        if !config.enable {
            return Ok(None);
        }
        let configured_path = config
            .log_path()
            .ok_or_else(|| {
                ComplianceError::Config(
                    "compliance logging requires an absolute log path".to_owned(),
                )
            })?
            .to_path_buf();
        if config.max_log_bytes == 0 || config.max_backup_files == 0 {
            return Err(ComplianceError::Config(
                "compliance rotation size and backup count must be non-zero".to_owned(),
            ));
        }
        let path = secure_compliance_log_path(&configured_path)?;
        let pipeline_spool_dir = config
            .pipeline_spool_dir()
            .map(|path| secure_compliance_directory(path, "compliance spool"))
            .transpose()?;
        let mut hash_key = config
            .hash_salt_bytes()
            .map_err(|err| ComplianceError::Config(err.to_string()))?
            .ok_or_else(|| {
                ComplianceError::Config(
                    "compliance logging requires a private anonymisation key".to_owned(),
                )
            })?;
        let file = open_log_file(&path)?;
        let logger = Self {
            path,
            hash_key,
            max_bytes: config.max_log_bytes,
            max_backups: config.max_backup_files,
            pipeline_spool_dir,
            writer: Mutex::new(file),
        };
        zeroize::Zeroize::zeroize(&mut hash_key);
        Ok(Some(logger))
    }
    /// Record a successful handshake.
    #[allow(clippy::too_many_arguments)]
    pub fn log_handshake_success(
        &self,
        remote: SocketAddr,
        mode: RelayMode,
        descriptor_commit: Option<&[u8]>,
        negotiated: &NegotiatedCapabilities,
        warnings: &[String],
        handshake_suite: HandshakeSuite,
        handshake_millis: u64,
        handshake_bytes: u64,
        puzzle_verify_micros: Option<u64>,
    ) -> Result<(), ComplianceError> {
        let timestamp = timestamp_string();
        let signatures = negotiated
            .signatures
            .iter()
            .map(|sig| sig.id.to_string())
            .collect::<Vec<_>>();
        let mut entry = Map::new();
        entry.insert("timestamp".to_owned(), Value::String(timestamp));
        entry.insert(
            "event".to_owned(),
            Value::String("handshake_accepted".to_owned()),
        );
        entry.insert("mode".to_owned(), Value::String(mode.as_label().to_owned()));
        entry.insert(
            "remote_hash".to_owned(),
            Value::String(self.remote_digest(remote)),
        );
        entry.insert(
            "descriptor_commit_hash".to_owned(),
            descriptor_commit
                .map(|bytes| Value::String(self.keyed_hash_hex(HASH_DOMAIN_DESCRIPTOR, bytes)))
                .unwrap_or(Value::Null),
        );
        entry.insert(
            "kem".to_owned(),
            Value::String(negotiated.kem.id.to_string()),
        );
        entry.insert(
            "signatures".to_owned(),
            Value::Array(signatures.into_iter().map(Value::String).collect()),
        );
        entry.insert("padding".to_owned(), Value::from(negotiated.padding));
        entry.insert(
            "handshake_suite".to_owned(),
            Value::String(handshake_suite.label().to_owned()),
        );
        entry.insert(
            "warnings".to_owned(),
            Value::Array(warnings.iter().cloned().map(Value::String).collect()),
        );
        entry.insert("handshake_millis".to_owned(), Value::from(handshake_millis));
        entry.insert("handshake_bytes".to_owned(), Value::from(handshake_bytes));
        if let Some(micros) = puzzle_verify_micros {
            entry.insert("puzzle_verify_micros".to_owned(), Value::from(micros));
        }
        self.append(Value::Object(entry))
    }
    /// Record a rejected handshake with `reason`.
    #[allow(clippy::too_many_arguments)]
    pub fn log_handshake_reject(
        &self,
        remote: SocketAddr,
        mode: RelayMode,
        descriptor_commit: Option<&[u8]>,
        reason: &str,
        throttle: Option<ThrottleAudit>,
        handshake_millis: Option<u64>,
        warnings: &[String],
    ) -> Result<(), ComplianceError> {
        let timestamp = timestamp_string();
        let mut entry = Map::new();
        entry.insert("timestamp".to_owned(), Value::String(timestamp));
        entry.insert(
            "event".to_owned(),
            Value::String("handshake_rejected".to_owned()),
        );
        entry.insert("mode".to_owned(), Value::String(mode.as_label().to_owned()));
        entry.insert(
            "remote_hash".to_owned(),
            Value::String(self.remote_digest(remote)),
        );
        entry.insert(
            "descriptor_commit_hash".to_owned(),
            descriptor_commit
                .map(|bytes| Value::String(self.keyed_hash_hex(HASH_DOMAIN_DESCRIPTOR, bytes)))
                .unwrap_or(Value::Null),
        );
        entry.insert("reason".to_owned(), Value::String(reason.to_owned()));
        if let Some(throttle) = throttle {
            let mut throttle_entry = Map::new();
            throttle_entry.insert("scope".to_owned(), Value::String(throttle.scope.to_owned()));
            if let Some(cooldown) = throttle.cooldown {
                throttle_entry.insert("cooldown_secs".to_owned(), Value::from(cooldown.as_secs()));
                throttle_entry.insert(
                    "cooldown_millis".to_owned(),
                    Value::from(cooldown.as_millis() as u64),
                );
            }
            if let Some(window) = throttle.window {
                throttle_entry.insert("window_secs".to_owned(), Value::from(window.as_secs()));
                throttle_entry.insert(
                    "window_millis".to_owned(),
                    Value::from(window.as_millis() as u64),
                );
            }
            if let Some(burst) = throttle.burst_limit {
                throttle_entry.insert("burst_limit".to_owned(), Value::from(burst));
            }
            if let Some(max_entries) = throttle.max_entries {
                throttle_entry.insert("max_entries".to_owned(), Value::from(max_entries as u64));
            }
            if let Some(observed) = throttle.observed_gap {
                throttle_entry.insert(
                    "observed_gap_millis".to_owned(),
                    Value::from(observed.as_millis() as u64),
                );
            }
            entry.insert("throttle".to_owned(), Value::Object(throttle_entry));
        }
        if let Some(millis) = handshake_millis {
            entry.insert("handshake_millis".to_owned(), Value::from(millis));
        }
        if !warnings.is_empty() {
            entry.insert(
                "warnings".to_owned(),
                Value::Array(warnings.iter().cloned().map(Value::String).collect()),
            );
        }
        self.append(Value::Object(entry))
    }
    /// Record the closure of a circuit along with selected telemetry.
    #[allow(clippy::too_many_arguments)]
    pub fn log_circuit_closed(
        &self,
        remote: SocketAddr,
        mode: RelayMode,
        circuit_id: u64,
        lifetime_millis: Option<u64>,
        kem: Option<&str>,
        signatures: Option<&[(String, bool)]>,
        padding: Option<u16>,
        active_circuits: u64,
        reason: &str,
    ) -> Result<(), ComplianceError> {
        let timestamp = timestamp_string();
        let mut entry = Map::new();
        entry.insert("timestamp".to_owned(), Value::String(timestamp));
        entry.insert(
            "event".to_owned(),
            Value::String("circuit_closed".to_owned()),
        );
        entry.insert("mode".to_owned(), Value::String(mode.as_label().to_owned()));
        entry.insert(
            "remote_hash".to_owned(),
            Value::String(self.remote_digest(remote)),
        );
        entry.insert(
            "circuit_hash".to_owned(),
            Value::String(self.keyed_hash_hex(HASH_DOMAIN_CIRCUIT, &circuit_id.to_be_bytes())),
        );
        entry.insert(
            "lifetime_millis".to_owned(),
            lifetime_millis.map(Value::from).unwrap_or(Value::Null),
        );
        entry.insert(
            "kem".to_owned(),
            kem.map(|value| Value::String(value.to_owned()))
                .unwrap_or(Value::Null),
        );
        let signatures_value = signatures
            .map(|items| {
                items
                    .iter()
                    .map(|(id, required)| {
                        let mut sig = Map::new();
                        sig.insert("id".to_owned(), Value::String(id.clone()));
                        sig.insert("required".to_owned(), Value::from(*required));
                        Value::Object(sig)
                    })
                    .collect::<Vec<_>>()
            })
            .map(Value::Array)
            .unwrap_or(Value::Null);
        entry.insert("signatures".to_owned(), signatures_value);
        entry.insert(
            "padding".to_owned(),
            padding.map(Value::from).unwrap_or(Value::Null),
        );
        entry.insert("active_circuits".to_owned(), Value::from(active_circuits));
        entry.insert("reason".to_owned(), Value::String(reason.to_owned()));
        self.append(Value::Object(entry))
    }
    /// Record successful exit route resolution.
    #[allow(clippy::too_many_arguments)]
    pub fn log_exit_route_open(
        &self,
        remote: SocketAddr,
        mode: RelayMode,
        stream: &'static str,
        authenticated: bool,
        channel_id: &[u8; 32],
        route_id: &[u8; 32],
        stream_id: &[u8; 32],
        room_id: Option<&[u8; 32]>,
        access_kind: &str,
        padding_budget_ms: Option<u16>,
        exit_multiaddr: &str,
        adapter_target: &str,
    ) -> Result<(), ComplianceError> {
        let timestamp = timestamp_string();
        let mut entry = Map::new();
        entry.insert("timestamp".to_owned(), Value::String(timestamp));
        entry.insert(
            "event".to_owned(),
            Value::String("exit_route_opened".to_owned()),
        );
        entry.insert("mode".to_owned(), Value::String(mode.as_label().to_owned()));
        entry.insert(
            "remote_hash".to_owned(),
            Value::String(self.remote_digest(remote)),
        );
        entry.insert("stream".to_owned(), Value::String(stream.to_owned()));
        entry.insert("authenticated".to_owned(), Value::from(authenticated));
        entry.insert(
            "channel_hash".to_owned(),
            Value::String(self.keyed_hash_hex(HASH_DOMAIN_CHANNEL, channel_id)),
        );
        entry.insert(
            "route_hash".to_owned(),
            Value::String(self.keyed_hash_hex(HASH_DOMAIN_ROUTE, route_id)),
        );
        entry.insert(
            "stream_hash".to_owned(),
            Value::String(self.keyed_hash_hex(HASH_DOMAIN_STREAM, stream_id)),
        );
        entry.insert(
            "room_hash".to_owned(),
            room_id
                .map(|room| Value::String(self.keyed_hash_hex(HASH_DOMAIN_ROOM, room)))
                .unwrap_or(Value::Null),
        );
        entry.insert(
            "access_kind".to_owned(),
            Value::String(access_kind.to_owned()),
        );
        entry.insert(
            "padding_budget_ms".to_owned(),
            padding_budget_ms.map(Value::from).unwrap_or(Value::Null),
        );
        entry.insert(
            "exit_multiaddr_hash".to_owned(),
            Value::String(
                self.keyed_hash_hex(HASH_DOMAIN_EXIT_MULTIADDR, exit_multiaddr.as_bytes()),
            ),
        );
        entry.insert(
            "adapter_target_hash".to_owned(),
            Value::String(
                self.keyed_hash_hex(HASH_DOMAIN_ADAPTER_TARGET, adapter_target.as_bytes()),
            ),
        );
        self.append(Value::Object(entry))
    }
    /// Record a rejected exit route attempt.
    pub fn log_exit_route_reject(
        &self,
        remote: SocketAddr,
        mode: RelayMode,
        stream: Option<&str>,
        channel: Option<&str>,
        reason: &str,
    ) -> Result<(), ComplianceError> {
        let timestamp = timestamp_string();
        let mut entry = Map::new();
        entry.insert("timestamp".to_owned(), Value::String(timestamp));
        entry.insert(
            "event".to_owned(),
            Value::String("exit_route_rejected".to_owned()),
        );
        entry.insert("mode".to_owned(), Value::String(mode.as_label().to_owned()));
        entry.insert(
            "remote_hash".to_owned(),
            Value::String(self.remote_digest(remote)),
        );
        entry.insert(
            "stream".to_owned(),
            stream
                .map(|value| Value::String(value.to_owned()))
                .unwrap_or(Value::Null),
        );
        entry.insert(
            "channel_hash".to_owned(),
            channel
                .map(|value| {
                    Value::String(self.keyed_hash_hex(HASH_DOMAIN_CHANNEL, value.as_bytes()))
                })
                .unwrap_or(Value::Null),
        );
        entry.insert("reason".to_owned(), Value::String(reason.to_owned()));
        self.append(Value::Object(entry))
    }
    /// Record the ingestion result of a blinded bandwidth proof.
    #[allow(clippy::too_many_arguments)]
    pub fn log_bandwidth_proof(
        &self,
        remote: SocketAddr,
        mode: RelayMode,
        measurement_id: &[u8; 32],
        relay_id: &[u8; 32],
        epoch: u32,
        verified_bytes: u128,
        sample_count: u16,
        jitter_p95_ms: u16,
        confidence_per_mille: u16,
        issued_at_unix: u64,
        verifier_label: &str,
        accepted: bool,
        reason: Option<&str>,
    ) -> Result<(), ComplianceError> {
        let timestamp = timestamp_string();
        let mut entry = Map::new();
        entry.insert("timestamp".to_owned(), Value::String(timestamp));
        entry.insert(
            "event".to_owned(),
            Value::String("bandwidth_proof".to_owned()),
        );
        entry.insert("mode".to_owned(), Value::String(mode.as_label().to_owned()));
        entry.insert(
            "remote_hash".to_owned(),
            Value::String(self.remote_digest(remote)),
        );
        entry.insert(
            "measurement_hash".to_owned(),
            Value::String(self.keyed_hash_hex(HASH_DOMAIN_MEASUREMENT, measurement_id)),
        );
        entry.insert(
            "relay_hash".to_owned(),
            Value::String(self.keyed_hash_hex(HASH_DOMAIN_RELAY, relay_id)),
        );
        entry.insert("epoch".to_owned(), Value::from(epoch));
        entry.insert(
            "verified_bytes".to_owned(),
            Value::String(verified_bytes.to_string()),
        );
        entry.insert("sample_count".to_owned(), Value::from(sample_count));
        entry.insert("jitter_p95_ms".to_owned(), Value::from(jitter_p95_ms));
        entry.insert(
            "confidence_per_mille".to_owned(),
            Value::from(confidence_per_mille),
        );
        entry.insert("issued_at".to_owned(), Value::from(issued_at_unix));
        entry.insert(
            "verifier_hash".to_owned(),
            Value::String(self.keyed_hash_hex(HASH_DOMAIN_VERIFIER, verifier_label.as_bytes())),
        );
        entry.insert("accepted".to_owned(), Value::from(accepted));
        entry.insert(
            "status".to_owned(),
            Value::String(if accepted {
                "accepted".to_owned()
            } else {
                "discarded".to_owned()
            }),
        );
        entry.insert(
            "reason".to_owned(),
            reason
                .map(|text| Value::String(text.to_owned()))
                .unwrap_or(Value::Null),
        );
        self.append(Value::Object(entry))
    }
    fn append(&self, value: Value) -> Result<(), ComplianceError> {
        let rendered = json::to_string(&value)?;
        {
            let next_len = rendered
                .len()
                .checked_add(1)
                .ok_or(ComplianceError::EntryTooLarge {
                    bytes: usize::MAX,
                    limit: self.max_bytes,
                })?;
            if u64::try_from(next_len).map_or(true, |next_len| next_len > self.max_bytes) {
                return Err(ComplianceError::EntryTooLarge {
                    bytes: next_len,
                    limit: self.max_bytes,
                });
            }
            let mut writer_guard = self.writer.lock().map_err(|_| ComplianceError::Poisoned)?;
            let needs_rotate = self.should_rotate(&writer_guard, next_len)?;
            if needs_rotate {
                self.rotate_logs()?;
                *writer_guard = open_log_file(&self.path)?;
            }
            self.write_entry(&mut writer_guard, &rendered)?;
        }
        if let Some(dir) = &self.pipeline_spool_dir {
            self.write_spool(dir, &rendered)?;
        }
        Ok(())
    }
    fn remote_digest(&self, remote: SocketAddr) -> String {
        self.keyed_hash_hex(HASH_DOMAIN_REMOTE, remote.to_string().as_bytes())
    }
    fn keyed_hash_hex(&self, domain: &[u8], bytes: &[u8]) -> String {
        let mut hasher = Blake3Hasher::new_keyed(&self.hash_key);
        hasher.update(domain);
        hasher.update(&[0]);
        hasher.update(bytes);
        let mut digest = hasher.finalize();
        let encoded = digest.to_hex().to_string();
        zeroize::Zeroize::zeroize(&mut digest);
        zeroize::Zeroize::zeroize(&mut hasher);
        encoded
    }
    fn write_entry(&self, writer: &mut File, rendered: &str) -> Result<(), ComplianceError> {
        writer
            .write_all(rendered.as_bytes())
            .map_err(|source| ComplianceError::Io {
                path: self.path.clone(),
                source,
            })?;
        writer
            .write_all(b"\n")
            .map_err(|source| ComplianceError::Io {
                path: self.path.clone(),
                source,
            })?;
        writer.flush().map_err(|source| ComplianceError::Io {
            path: self.path.clone(),
            source,
        })?;
        writer.sync_data().map_err(|source| ComplianceError::Io {
            path: self.path.clone(),
            source,
        })?;
        Ok(())
    }
    fn should_rotate(&self, writer: &File, next_len: usize) -> Result<bool, ComplianceError> {
        if self.max_bytes == 0 {
            return Ok(false);
        }
        let size = writer
            .metadata()
            .map_err(|source| ComplianceError::Io {
                path: self.path.clone(),
                source,
            })?
            .len();
        Ok(size
            .checked_add(u64::try_from(next_len).unwrap_or(u64::MAX))
            .is_none_or(|size| size > self.max_bytes))
    }
    fn rotate_logs(&self) -> Result<(), ComplianceError> {
        if self.max_backups == 0 {
            return Ok(());
        }
        let oldest = self.backup_path(self.max_backups);
        if secure_existing_compliance_file(&oldest)? {
            fs::remove_file(&oldest).map_err(|source| ComplianceError::Io {
                path: oldest.clone(),
                source,
            })?;
        }
        for idx in (1..self.max_backups).rev() {
            let src = self.backup_path(idx);
            if secure_existing_compliance_file(&src)? {
                let dst = self.backup_path(idx + 1);
                if secure_existing_compliance_file(&dst)? {
                    return Err(ComplianceError::Config(format!(
                        "refusing to overwrite unexpected compliance backup `{}`",
                        dst.display()
                    )));
                }
                if let Err(source) = fs::rename(&src, &dst) {
                    return Err(ComplianceError::Io { path: dst, source });
                }
            }
        }
        let first_backup = self.backup_path(1);
        if secure_existing_compliance_file(&first_backup)? {
            return Err(ComplianceError::Config(format!(
                "refusing to overwrite unexpected compliance backup `{}`",
                first_backup.display()
            )));
        }
        if secure_existing_compliance_file(&self.path)?
            && let Err(source) = fs::rename(&self.path, &first_backup)
        {
            return Err(ComplianceError::Io {
                path: first_backup,
                source,
            });
        }
        if let Some(parent) = self.path.parent() {
            sync_compliance_directory(parent)?;
        }
        Ok(())
    }
    fn backup_path(&self, index: u8) -> PathBuf {
        let mut rotated = self
            .path
            .file_name()
            .expect("validated compliance path has a file name")
            .to_os_string();
        rotated.push(format!(".{index}"));
        self.path.with_file_name(rotated)
    }
    fn write_spool(&self, dir: &Path, payload: &str) -> Result<(), ComplianceError> {
        let mut temporary = tempfile::Builder::new()
            .prefix(".compliance-spool-")
            .tempfile_in(dir)
            .map_err(|source| ComplianceError::Io {
                path: dir.to_path_buf(),
                source,
            })?;
        temporary
            .write_all(payload.as_bytes())
            .map_err(|source| ComplianceError::Io {
                path: temporary.path().to_path_buf(),
                source,
            })?;
        temporary
            .flush()
            .and_then(|()| temporary.as_file().sync_all())
            .map_err(|source| ComplianceError::Io {
                path: temporary.path().to_path_buf(),
                source,
            })?;
        #[cfg(unix)]
        validate_compliance_file_metadata(
            temporary.path(),
            &temporary
                .as_file()
                .metadata()
                .map_err(|source| ComplianceError::Io {
                    path: temporary.path().to_path_buf(),
                    source,
                })?,
            compliance_effective_uid()?,
        )?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| {
                ComplianceError::Config(
                    "system clock precedes the Unix epoch; refusing ambiguous spool name"
                        .to_owned(),
                )
            })?
            .as_nanos();
        const MAX_CREATE_ATTEMPTS: u32 = 1_024;
        for attempt in 0..MAX_CREATE_ATTEMPTS {
            let candidate = if attempt == 0 {
                dir.join(format!("compliance-{timestamp}.json"))
            } else {
                dir.join(format!("compliance-{timestamp}-{attempt}.json"))
            };
            match temporary.persist_noclobber(&candidate) {
                Ok(file) => {
                    #[cfg(unix)]
                    validate_compliance_file_metadata(
                        &candidate,
                        &file.metadata().map_err(|source| ComplianceError::Io {
                            path: candidate.clone(),
                            source,
                        })?,
                        compliance_effective_uid()?,
                    )?;
                    sync_compliance_directory(dir)?;
                    return Ok(());
                }
                Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                    temporary = error.file;
                }
                Err(error) => {
                    return Err(ComplianceError::Io {
                        path: candidate,
                        source: error.error,
                    });
                }
            }
        }
        Err(ComplianceError::Config(format!(
            "failed to allocate a unique compliance spool name after {MAX_CREATE_ATTEMPTS} attempts"
        )))
    }
}

impl std::fmt::Debug for ComplianceLogger {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ComplianceLogger")
            .field("path", &self.path)
            .field("hash_key", &"<redacted>")
            .field("max_bytes", &self.max_bytes)
            .field("max_backups", &self.max_backups)
            .field("pipeline_spool_dir", &self.pipeline_spool_dir)
            .finish_non_exhaustive()
    }
}

impl Drop for ComplianceLogger {
    fn drop(&mut self) {
        zeroize::Zeroize::zeroize(&mut self.hash_key);
    }
}
fn open_log_file(path: &Path) -> Result<File, ComplianceError> {
    #[cfg(not(unix))]
    return Err(ComplianceError::Config(
        "compliance persistence requires Unix filesystem custody guarantees".to_owned(),
    ));

    #[cfg(unix)]
    let effective_uid = compliance_effective_uid()?;
    #[cfg(unix)]
    let before = match fs::symlink_metadata(path) {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(source) => {
            return Err(ComplianceError::Io {
                path: path.to_path_buf(),
                source,
            });
        }
    };
    #[cfg(unix)]
    if let Some(metadata) = &before {
        validate_compliance_file_metadata(path, metadata, effective_uid)?;
    }
    #[cfg(unix)]
    let file = {
        let mut options = OpenOptions::new();
        options
            .create(true)
            .append(true)
            .mode(COMPLIANCE_FILE_MODE)
            .custom_flags(O_NOFOLLOW_FLAG);
        options.open(path).map_err(|err| ComplianceError::Io {
            path: path.to_path_buf(),
            source: err,
        })?
    };
    #[cfg(unix)]
    {
        let opened = file.metadata().map_err(|source| ComplianceError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        validate_compliance_file_metadata(path, &opened, effective_uid)?;
        let named = fs::symlink_metadata(path).map_err(|source| ComplianceError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        validate_compliance_file_metadata(path, &named, effective_uid)?;
        if opened.dev() != named.dev() || opened.ino() != named.ino() {
            return Err(ComplianceError::Config(format!(
                "compliance file `{}` changed while it was opened",
                path.display()
            )));
        }
        if let Some(before) = &before
            && (before.dev() != opened.dev() || before.ino() != opened.ino())
        {
            return Err(ComplianceError::Config(format!(
                "compliance file `{}` changed while it was opened",
                path.display()
            )));
        }
        if before.is_none()
            && let Some(parent) = path.parent()
        {
            sync_compliance_directory(parent)?;
        }
        Ok(file)
    }
}

#[cfg(unix)]
fn compliance_effective_uid() -> Result<u32, ComplianceError> {
    effective_uid().map_err(|error| {
        ComplianceError::Config(format!("failed to determine effective user: {error}"))
    })
}

#[cfg(unix)]
fn secure_compliance_log_path(path: &Path) -> Result<PathBuf, ComplianceError> {
    if !path.is_absolute() {
        return Err(ComplianceError::Config(format!(
            "compliance log path `{}` must be absolute",
            path.display()
        )));
    }
    let file_name = path.file_name().ok_or_else(|| {
        ComplianceError::Config(format!(
            "compliance log path `{}` has no file name",
            path.display()
        ))
    })?;
    let parent = path.parent().ok_or_else(|| {
        ComplianceError::Config(format!(
            "compliance log path `{}` has no parent",
            path.display()
        ))
    })?;
    Ok(secure_compliance_directory(parent, "compliance log")?.join(file_name))
}

#[cfg(not(unix))]
fn secure_compliance_log_path(_path: &Path) -> Result<PathBuf, ComplianceError> {
    Err(ComplianceError::Config(
        "compliance persistence requires Unix filesystem custody guarantees".to_owned(),
    ))
}

#[cfg(unix)]
fn secure_compliance_directory(path: &Path, purpose: &str) -> Result<PathBuf, ComplianceError> {
    if !path.is_absolute() {
        return Err(ComplianceError::Config(format!(
            "{purpose} directory `{}` must be absolute",
            path.display()
        )));
    }
    let canonical = fs::canonicalize(path).map_err(|source| ComplianceError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let effective_uid = compliance_effective_uid()?;
    let mut current = Some(canonical.as_path());
    while let Some(component) = current {
        let metadata = fs::symlink_metadata(component).map_err(|source| ComplianceError::Io {
            path: component.to_path_buf(),
            source,
        })?;
        if !metadata.is_dir() {
            return Err(ComplianceError::Config(format!(
                "{purpose} ancestor `{}` is not a directory",
                component.display()
            )));
        }
        let mode = metadata.permissions().mode();
        let trusted_owner = metadata.uid() == 0 || metadata.uid() == effective_uid;
        let root_sticky = metadata.uid() == 0 && mode & 0o1000 != 0;
        if !trusted_owner || (mode & 0o022 != 0 && !root_sticky) {
            return Err(ComplianceError::Config(format!(
                "{purpose} ancestor `{}` is replaceable by another principal",
                component.display()
            )));
        }
        current = component.parent();
    }
    let leaf = fs::metadata(&canonical).map_err(|source| ComplianceError::Io {
        path: canonical.clone(),
        source,
    })?;
    if leaf.permissions().mode() & 0o777 != 0o700 {
        return Err(ComplianceError::Config(format!(
            "{purpose} directory `{}` must have mode 0700",
            canonical.display()
        )));
    }
    Ok(canonical)
}

#[cfg(not(unix))]
fn secure_compliance_directory(_path: &Path, _purpose: &str) -> Result<PathBuf, ComplianceError> {
    Err(ComplianceError::Config(
        "compliance persistence requires Unix filesystem custody guarantees".to_owned(),
    ))
}

#[cfg(unix)]
fn validate_compliance_file_metadata(
    path: &Path,
    metadata: &fs::Metadata,
    effective_uid: u32,
) -> Result<(), ComplianceError> {
    if !metadata.is_file()
        || metadata.uid() != effective_uid
        || metadata.nlink() != 1
        || metadata.permissions().mode() & 0o777 != COMPLIANCE_FILE_MODE
    {
        return Err(ComplianceError::Config(format!(
            "compliance file `{}` must be an owner-owned, single-link regular file with mode 0600",
            path.display()
        )));
    }
    Ok(())
}

#[cfg(unix)]
fn secure_existing_compliance_file(path: &Path) -> Result<bool, ComplianceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) => {
            validate_compliance_file_metadata(path, &metadata, compliance_effective_uid()?)?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(ComplianceError::Io {
            path: path.to_path_buf(),
            source,
        }),
    }
}

#[cfg(not(unix))]
fn secure_existing_compliance_file(_path: &Path) -> Result<bool, ComplianceError> {
    Err(ComplianceError::Config(
        "compliance persistence requires Unix filesystem custody guarantees".to_owned(),
    ))
}

fn sync_compliance_directory(path: &Path) -> Result<(), ComplianceError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| ComplianceError::Io {
            path: path.to_path_buf(),
            source,
        })
}
fn timestamp_string() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}
#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::capability::{
        GreaseEntry, KemAdvertisement, KemId, NegotiatedCapabilities, SignatureAdvertisement,
        SignatureId,
    };
    use norito::json::{self, Value as JsonValue};
    use std::{
        fs::{self, DirBuilder},
        net::{IpAddr, Ipv4Addr, SocketAddr},
        os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt, symlink},
        sync::Arc,
    };
    use tempfile::tempdir;
    const TEST_HASH_KEY: [u8; 32] = [
        0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F,
        0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B, 0x1C, 0x1D, 0x1E,
        0x1F, 0x20,
    ];

    fn expected_hash(domain: &[u8], value: &[u8]) -> String {
        let mut hasher = Blake3Hasher::new_keyed(&TEST_HASH_KEY);
        hasher.update(domain);
        hasher.update(&[0]);
        hasher.update(value);
        hasher.finalize().to_hex().to_string()
    }

    fn create_private_dir(path: &Path) {
        let mut builder = DirBuilder::new();
        builder.mode(0o700);
        builder.create(path).expect("create private directory");
    }
    fn private_tempdir() -> tempfile::TempDir {
        let temp = tempdir().expect("tempdir");
        fs::set_permissions(temp.path(), fs::Permissions::from_mode(0o700))
            .expect("protect tempdir");
        temp
    }
    fn write_hash_key(directory: &Path) -> PathBuf {
        let path = directory.join("compliance-hash-key.hex");
        let mut options = OpenOptions::new();
        let mut file = options
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&path)
            .expect("create hash key");
        file.write_all(hex::encode(TEST_HASH_KEY).as_bytes())
            .expect("write hash key");
        file.sync_all().expect("sync hash key");
        path
    }
    fn build_logger() -> (ComplianceLogger, std::path::PathBuf, tempfile::TempDir) {
        let temp = private_tempdir();
        let log_path = temp.path().join("compliance.jsonl");
        let hash_salt_path = write_hash_key(temp.path());
        let mut config = ComplianceConfig {
            enable: true,
            log_path: Some(log_path.clone()),
            hash_salt_path: Some(hash_salt_path),
            max_log_bytes: 0,
            max_backup_files: 0,
            pipeline_spool_dir: None,
        };
        config.apply_defaults().expect("defaults");
        let logger = ComplianceLogger::from_config(&config)
            .expect("logger result")
            .expect("logger");
        (logger, log_path, temp)
    }
    fn sample_negotiated() -> NegotiatedCapabilities {
        NegotiatedCapabilities {
            kem: KemAdvertisement {
                id: KemId::MlKem768,
                required: true,
            },
            signatures: vec![SignatureAdvertisement {
                id: SignatureId::Dilithium3,
                required: true,
            }],
            padding: 1024,
            descriptor_commit: None,
            grease: vec![GreaseEntry {
                ty: 0xFFFF,
                value: vec![0xAA, 0xBB, 0xCC],
            }],
            constant_rate: None,
        }
    }
    #[test]
    fn circuit_closed_event_logged() {
        let (logger, log_path, _temp) = build_logger();
        let debug = format!("{logger:?}");
        assert!(debug.contains("<redacted>"));
        assert!(!debug.contains(&hex::encode(TEST_HASH_KEY)));
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 4433);
        let signatures = vec![("dilithium3".to_owned(), true)];
        logger
            .log_circuit_closed(
                remote,
                RelayMode::Entry,
                7,
                Some(1_234),
                Some("mlkem768"),
                Some(signatures.as_slice()),
                Some(512),
                3,
                "application_closed",
            )
            .expect("log circuit closed");
        let contents = std::fs::read_to_string(log_path).expect("read log");
        let line = contents.lines().next().expect("line");
        let value: JsonValue = norito::json::from_str(line).expect("json value");
        let expected_remote_hash = expected_hash(HASH_DOMAIN_REMOTE, remote.to_string().as_bytes());
        assert_eq!(value["event"].as_str().unwrap(), "circuit_closed");
        assert_eq!(value["mode"].as_str().unwrap(), "entry");
        assert_eq!(value["remote_hash"].as_str().unwrap(), expected_remote_hash);
        assert_eq!(
            value["circuit_hash"].as_str().unwrap(),
            expected_hash(HASH_DOMAIN_CIRCUIT, &7_u64.to_be_bytes())
        );
        assert_eq!(value["lifetime_millis"].as_u64().unwrap(), 1_234);
        assert_eq!(value["kem"].as_str().unwrap(), "mlkem768");
        assert_eq!(value["padding"].as_u64().unwrap(), 512);
        assert_eq!(value["active_circuits"].as_u64().unwrap(), 3);
        assert_eq!(value["reason"].as_str().unwrap(), "application_closed");
        let signature = value["signatures"]
            .as_array()
            .expect("signatures array")
            .first()
            .expect("signature entry");
        assert_eq!(signature["id"].as_str().unwrap(), "dilithium3");
        assert!(signature["required"].as_bool().expect("required bool"));
    }
    #[test]
    fn exit_route_events_logged() {
        let (logger, log_path, _temp) = build_logger();
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9000);
        let channel = [0x11u8; 32];
        let route = [0x22u8; 32];
        let stream = [0x33u8; 32];
        let room = [0x44u8; 32];
        logger
            .log_exit_route_open(
                remote,
                RelayMode::Exit,
                "kaigi-stream",
                true,
                &channel,
                &route,
                &stream,
                Some(&room),
                "Authenticated",
                Some(50),
                "/dns/example.com/tcp/443",
                "wss://example.com/socket",
            )
            .expect("log exit open");
        logger
            .log_exit_route_reject(
                remote,
                RelayMode::Exit,
                Some("kaigi-stream"),
                Some("deadbeef"),
                "route missing",
            )
            .expect("log exit reject");
        let contents = std::fs::read_to_string(&log_path).expect("read log");
        let mut lines = contents.lines();
        let open: JsonValue = norito::json::from_str(lines.next().unwrap()).unwrap();
        let reject: JsonValue = norito::json::from_str(lines.next().unwrap()).unwrap();
        assert_eq!(open["event"].as_str().unwrap(), "exit_route_opened");
        assert_eq!(open["mode"].as_str().unwrap(), "exit");
        assert_eq!(open["stream"].as_str().unwrap(), "kaigi-stream");
        assert!(open["authenticated"].as_bool().unwrap());
        assert_eq!(
            open["channel_hash"].as_str().unwrap(),
            expected_hash(HASH_DOMAIN_CHANNEL, &channel)
        );
        assert_eq!(
            open["route_hash"].as_str().unwrap(),
            expected_hash(HASH_DOMAIN_ROUTE, &route)
        );
        assert_eq!(
            open["stream_hash"].as_str().unwrap(),
            expected_hash(HASH_DOMAIN_STREAM, &stream)
        );
        assert_eq!(
            open["room_hash"].as_str().unwrap(),
            expected_hash(HASH_DOMAIN_ROOM, &room)
        );
        assert_eq!(open["access_kind"].as_str().unwrap(), "Authenticated");
        assert_eq!(open["padding_budget_ms"].as_u64().unwrap(), 50);
        let expected_exit_hash =
            expected_hash(HASH_DOMAIN_EXIT_MULTIADDR, b"/dns/example.com/tcp/443");
        assert_eq!(
            open["exit_multiaddr_hash"].as_str().unwrap(),
            expected_exit_hash
        );
        let expected_target_hash =
            expected_hash(HASH_DOMAIN_ADAPTER_TARGET, b"wss://example.com/socket");
        assert_eq!(
            open["adapter_target_hash"].as_str().unwrap(),
            expected_target_hash
        );
        assert_eq!(reject["event"].as_str().unwrap(), "exit_route_rejected");
        assert_eq!(reject["stream"].as_str().unwrap(), "kaigi-stream");
        assert_eq!(
            reject["channel_hash"].as_str().unwrap(),
            expected_hash(HASH_DOMAIN_CHANNEL, b"deadbeef")
        );
        assert_eq!(reject["reason"].as_str().unwrap(), "route missing");
    }
    #[test]
    fn spool_writes_handshake_payload() {
        let temp = private_tempdir();
        let log_path = temp.path().join("compliance.jsonl");
        let spool_dir = temp.path().join("spool");
        create_private_dir(&spool_dir);
        let mut config = ComplianceConfig {
            enable: true,
            log_path: Some(log_path),
            hash_salt_path: Some(write_hash_key(temp.path())),
            max_log_bytes: 0,
            max_backup_files: 0,
            pipeline_spool_dir: Some(spool_dir.clone()),
        };
        config.apply_defaults().expect("defaults");
        let logger = ComplianceLogger::from_config(&config)
            .expect("logger result")
            .expect("logger");
        let negotiated = sample_negotiated();
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9001);
        logger
            .log_handshake_success(
                remote,
                RelayMode::Entry,
                None,
                &negotiated,
                &[],
                HandshakeSuite::Nk2Hybrid,
                12,
                512,
                None,
            )
            .expect("write handshake spool entry");
        let entries: Vec<_> = fs::read_dir(&spool_dir)
            .expect("read spool dir")
            .collect::<Result<_, _>>()
            .expect("spool entries");
        assert_eq!(entries.len(), 1, "expected one spool file");
        let payload = fs::read_to_string(entries[0].path()).expect("read spool payload");
        let value: JsonValue = norito::json::from_str(&payload).expect("parse spool json");
        assert_eq!(value["event"].as_str().unwrap(), "handshake_accepted");
        let expected_hash = expected_hash(HASH_DOMAIN_REMOTE, remote.to_string().as_bytes());
        assert_eq!(value["remote_hash"].as_str().unwrap(), expected_hash);
        let metadata = fs::metadata(entries[0].path()).expect("spool metadata");
        assert_eq!(metadata.permissions().mode() & 0o777, 0o600);
        assert_eq!(metadata.nlink(), 1);
    }

    #[test]
    fn compliance_paths_reject_aliases_links_and_permissive_custody() {
        let temp = private_tempdir();
        let target = temp.path().join("target");
        fs::write(&target, b"do not modify").expect("write target");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("protect target");
        let link = temp.path().join("compliance.jsonl");
        symlink(&target, &link).expect("create log symlink");
        let mut config = ComplianceConfig {
            enable: true,
            log_path: Some(link),
            hash_salt_path: Some(write_hash_key(temp.path())),
            max_log_bytes: 1_024,
            max_backup_files: 1,
            pipeline_spool_dir: None,
        };
        config.apply_defaults().expect("defaults");
        assert!(ComplianceLogger::from_config(&config).is_err());
        assert_eq!(fs::read(&target).expect("read target"), b"do not modify");

        let permissive = temp.path().join("permissive");
        create_private_dir(&permissive);
        fs::set_permissions(&permissive, fs::Permissions::from_mode(0o750))
            .expect("make directory permissive");
        config.log_path = Some(permissive.join("compliance.jsonl"));
        assert!(ComplianceLogger::from_config(&config).is_err());

        config.log_path = Some(PathBuf::from("relative-compliance.jsonl"));
        assert!(ComplianceLogger::from_config(&config).is_err());
    }

    #[test]
    fn compliance_logger_requires_key_even_without_prior_config_validation() {
        let temp = private_tempdir();
        let log_path = temp.path().join("compliance.jsonl");
        let config = ComplianceConfig {
            enable: true,
            log_path: Some(log_path.clone()),
            hash_salt_path: None,
            max_log_bytes: 1_024,
            max_backup_files: 1,
            pipeline_spool_dir: None,
        };
        assert!(matches!(
            ComplianceLogger::from_config(&config),
            Err(ComplianceError::Config(_))
        ));
        assert!(!log_path.exists());
    }

    #[test]
    fn compliance_rotation_is_serialized_and_refuses_backup_symlinks() {
        let temp = private_tempdir();
        let log_path = temp.path().join("compliance.jsonl");
        let target = temp.path().join("target");
        fs::write(&target, b"operator data").expect("write target");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).expect("protect target");
        let mut config = ComplianceConfig {
            enable: true,
            log_path: Some(log_path.clone()),
            hash_salt_path: Some(write_hash_key(temp.path())),
            max_log_bytes: 80,
            max_backup_files: 2,
            pipeline_spool_dir: None,
        };
        config.apply_defaults().expect("defaults");
        let logger = Arc::new(
            ComplianceLogger::from_config(&config)
                .expect("logger result")
                .expect("logger"),
        );
        logger
            .append(Value::String("a".repeat(48)))
            .expect("first entry");
        symlink(&target, log_path.with_file_name("compliance.jsonl.1"))
            .expect("create backup symlink");
        assert!(logger.append(Value::String("b".repeat(48))).is_err());
        assert_eq!(fs::read(&target).expect("read target"), b"operator data");
    }

    #[test]
    fn concurrent_compliance_appends_preserve_complete_json_lines() {
        let temp = private_tempdir();
        let log_path = temp.path().join("compliance.jsonl");
        let mut config = ComplianceConfig {
            enable: true,
            log_path: Some(log_path.clone()),
            hash_salt_path: Some(write_hash_key(temp.path())),
            max_log_bytes: 1_048_576,
            max_backup_files: 2,
            pipeline_spool_dir: None,
        };
        config.apply_defaults().expect("defaults");
        let logger = Arc::new(
            ComplianceLogger::from_config(&config)
                .expect("logger result")
                .expect("logger"),
        );
        let threads = (0..8)
            .map(|worker| {
                let logger = Arc::clone(&logger);
                std::thread::spawn(move || {
                    for sequence in 0..32 {
                        logger
                            .append(norito::json!({"worker": worker, "sequence": sequence}))
                            .expect("append record");
                    }
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().expect("join writer");
        }
        let contents = fs::read_to_string(log_path).expect("read log");
        assert_eq!(contents.lines().count(), 8 * 32);
        for line in contents.lines() {
            let _: JsonValue = json::from_str(line).expect("complete JSON line");
        }
    }
    #[test]
    fn success_log_includes_handshake_metrics() {
        let (logger, log_path, _temp) = build_logger();
        let negotiated = sample_negotiated();
        let warnings = vec!["client preferred NK3 but negotiated NK2".to_string()];
        logger
            .log_handshake_success(
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9_000),
                RelayMode::Entry,
                None,
                &negotiated,
                &warnings,
                HandshakeSuite::Nk2Hybrid,
                87,
                2_048,
                Some(42),
            )
            .expect("write log");
        let contents = fs::read_to_string(&log_path).expect("read log");
        let line = contents.lines().next().expect("line");
        let value: JsonValue = json::from_str(line).expect("json value");
        assert_eq!(value["handshake_millis"].as_u64(), Some(87));
        assert_eq!(value["handshake_bytes"].as_u64(), Some(2_048));
        assert_eq!(value["puzzle_verify_micros"].as_u64(), Some(42));
        let warnings_value = value["warnings"].as_array().expect("warnings array");
        assert_eq!(warnings_value.len(), 1);
    }
    #[test]
    fn reject_log_surfaces_warnings_and_latency() {
        let (logger, log_path, _temp) = build_logger();
        let warnings = vec!["relay omitted suite_list capability".to_string()];
        logger
            .log_handshake_reject(
                SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9_001),
                RelayMode::Entry,
                None,
                "downgrade",
                None,
                Some(123),
                &warnings,
            )
            .expect("write log");
        let contents = fs::read_to_string(&log_path).expect("read log");
        let line = contents.lines().next().expect("line");
        let value: JsonValue = json::from_str(line).expect("json value");
        assert_eq!(value["handshake_millis"].as_u64(), Some(123));
        let warnings_value = value["warnings"].as_array().expect("warnings array");
        assert_eq!(warnings_value.len(), 1);
        assert_eq!(
            warnings_value[0].as_str(),
            Some("relay omitted suite_list capability")
        );
    }
    #[test]
    fn bandwidth_proof_events_include_status() {
        let (logger, log_path, _temp) = build_logger();
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), 9000);
        let measurement = [0xAB; 32];
        let relay_id = [0xCD; 32];
        let verifier = "relay@sora";
        logger
            .log_bandwidth_proof(
                remote,
                RelayMode::Entry,
                &measurement,
                &relay_id,
                42,
                9_765_625_u128,
                32,
                12,
                975,
                1_698_000,
                verifier,
                true,
                None,
            )
            .expect("log proof");
        logger
            .log_bandwidth_proof(
                remote,
                RelayMode::Entry,
                &measurement,
                &relay_id,
                42,
                9_765_625_u128,
                32,
                12,
                975,
                1_698_000,
                verifier,
                false,
                Some("duplicate_measurement"),
            )
            .expect("log duplicate");
        let contents = fs::read_to_string(&log_path).expect("read log");
        let mut lines = contents.lines();
        let accepted: JsonValue = json::from_str(lines.next().unwrap()).unwrap();
        let rejected: JsonValue = json::from_str(lines.next().unwrap()).unwrap();
        let expected_remote_hash = expected_hash(HASH_DOMAIN_REMOTE, remote.to_string().as_bytes());
        assert_eq!(accepted["event"].as_str(), Some("bandwidth_proof"));
        assert_eq!(accepted["mode"].as_str(), Some("entry"));
        assert_eq!(
            accepted["remote_hash"].as_str(),
            Some(expected_remote_hash.as_str())
        );
        assert_eq!(
            accepted["measurement_hash"].as_str(),
            Some(expected_hash(HASH_DOMAIN_MEASUREMENT, &measurement).as_str())
        );
        assert_eq!(
            accepted["relay_hash"].as_str(),
            Some(expected_hash(HASH_DOMAIN_RELAY, &relay_id).as_str())
        );
        assert_eq!(accepted["epoch"].as_u64(), Some(42));
        assert_eq!(accepted["verified_bytes"].as_str(), Some("9765625"));
        assert!(accepted["reason"].is_null());
        assert!(accepted["accepted"].as_bool().unwrap());
        let verifier_hash = expected_hash(HASH_DOMAIN_VERIFIER, verifier.as_bytes());
        assert_eq!(
            accepted["verifier_hash"].as_str(),
            Some(verifier_hash.as_str())
        );
        assert!(!rejected["accepted"].as_bool().unwrap());
        assert_eq!(rejected["reason"].as_str(), Some("duplicate_measurement"));
    }
}
/// Errors that may occur while logging compliance events.
#[derive(Debug, Error)]
pub enum ComplianceError {
    #[error("I/O error while writing compliance log `{path}`: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to render compliance log entry: {0}")]
    Json(#[from] json::Error),
    #[error("compliance log entry is {bytes} bytes; configured limit is {limit} bytes")]
    EntryTooLarge { bytes: usize, limit: u64 },
    #[error("compliance log writer lock is poisoned")]
    Poisoned,
    #[error("invalid compliance configuration: {0}")]
    Config(String),
}
