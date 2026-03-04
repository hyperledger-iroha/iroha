//! Compliance logging for relay handshake events.
//!
//! Logs are emitted in JSON Lines format, one event per line. Remote addresses
//! are hashed with an optional salt so operators can correlate events without
//! leaking raw client information.

use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use blake3::Hasher as Blake3Hasher;
use iroha_crypto::soranet::handshake::HandshakeSuite;
use norito::json::{self, Map, Value};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{
    capability::NegotiatedCapabilities,
    config::{ComplianceConfig, RelayMode},
};

/// Logger that writes compliance events to a JSON Lines file.
#[derive(Debug)]
pub struct ComplianceLogger {
    path: PathBuf,
    hash_salt: Option<[u8; 32]>,
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
    /// Returns an error if the log file cannot be opened or the parent
    /// directory cannot be created.
    pub fn from_config(config: &ComplianceConfig) -> Result<Option<Self>, ComplianceError> {
        if !config.enable {
            return Ok(None);
        }
        let path = config
            .log_path()
            .expect("validation ensures log_path when enabled")
            .to_path_buf();
        let hash_salt = config
            .hash_salt_bytes()
            .map_err(|err| ComplianceError::Config(err.to_string()))?;
        let file = open_log_file(&path)?;
        Ok(Some(Self {
            path,
            hash_salt,
            max_bytes: config.max_log_bytes,
            max_backups: config.max_backup_files,
            pipeline_spool_dir: config.pipeline_spool_dir().map(|p| p.to_path_buf()),
            writer: Mutex::new(file),
        }))
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
            "descriptor_commit_hex".to_owned(),
            descriptor_commit
                .map(|bytes| Value::String(hex::encode(bytes)))
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
            "descriptor_commit_hex".to_owned(),
            descriptor_commit
                .map(|bytes| Value::String(hex::encode(bytes)))
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
        entry.insert("circuit_id".to_owned(), Value::from(circuit_id));
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
            "channel_hex".to_owned(),
            Value::String(hex::encode(channel_id)),
        );
        entry.insert("route_hex".to_owned(), Value::String(hex::encode(route_id)));
        entry.insert(
            "stream_hex".to_owned(),
            Value::String(hex::encode(stream_id)),
        );
        entry.insert(
            "room_hex".to_owned(),
            room_id
                .map(|room| Value::String(hex::encode(room)))
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
            Value::String(self.salted_hash_hex(exit_multiaddr.as_bytes())),
        );
        entry.insert(
            "adapter_target_hash".to_owned(),
            Value::String(self.salted_hash_hex(adapter_target.as_bytes())),
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
            "channel".to_owned(),
            channel
                .map(|value| Value::String(value.to_owned()))
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
            "measurement_hex".to_owned(),
            Value::String(hex::encode(measurement_id)),
        );
        entry.insert("relay_hex".to_owned(), Value::String(hex::encode(relay_id)));
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
            Value::String(self.salted_hash_hex(verifier_label.as_bytes())),
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
            let next_len = rendered.len() + 1;
            let mut writer_guard = self.writer.lock().expect("compliance log poisoned");
            let needs_rotate = self.should_rotate(&writer_guard, next_len)?;
            if needs_rotate {
                drop(writer_guard);
                self.rotate_logs()?;
                let mut writer_guard = self.writer.lock().expect("compliance log poisoned");
                *writer_guard = open_log_file(&self.path)?;
                self.write_entry(&mut writer_guard, &rendered)?;
            } else {
                self.write_entry(&mut writer_guard, &rendered)?;
            }
        }

        if let Some(dir) = &self.pipeline_spool_dir {
            self.write_spool(dir, &rendered)?;
        }

        Ok(())
    }

    fn remote_digest(&self, remote: SocketAddr) -> String {
        self.salted_hash_hex(remote.to_string().as_bytes())
    }

    fn salted_hash_hex(&self, bytes: &[u8]) -> String {
        let mut hasher = Blake3Hasher::new();
        if let Some(salt) = &self.hash_salt {
            hasher.update(salt);
        }
        hasher.update(bytes);
        hasher.finalize().to_hex().to_string()
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
        Ok(size + next_len as u64 > self.max_bytes)
    }

    fn rotate_logs(&self) -> Result<(), ComplianceError> {
        if self.max_backups == 0 {
            return Ok(());
        }
        let oldest = self.backup_path(self.max_backups);
        if oldest.exists() {
            fs::remove_file(&oldest).map_err(|source| ComplianceError::Io {
                path: oldest.clone(),
                source,
            })?;
        }
        for idx in (1..self.max_backups).rev() {
            let src = self.backup_path(idx);
            if src.exists() {
                let dst = self.backup_path(idx + 1);
                if let Err(source) = fs::rename(&src, &dst) {
                    return Err(ComplianceError::Io { path: dst, source });
                }
            }
        }
        let first_backup = self.backup_path(1);
        if self.path.exists()
            && let Err(source) = fs::rename(&self.path, &first_backup)
        {
            return Err(ComplianceError::Io {
                path: first_backup,
                source,
            });
        }
        Ok(())
    }

    fn backup_path(&self, index: u8) -> PathBuf {
        let file_name = self
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "relay_compliance.jsonl".to_string());
        let rotated = format!("{file_name}.{index}");
        self.path.with_file_name(rotated)
    }

    fn write_spool(&self, dir: &Path, payload: &str) -> Result<(), ComplianceError> {
        fs::create_dir_all(dir).map_err(|source| ComplianceError::Io {
            path: dir.to_path_buf(),
            source,
        })?;
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let mut attempt = 0u32;
        loop {
            let candidate = if attempt == 0 {
                dir.join(format!("compliance-{timestamp}.json"))
            } else {
                dir.join(format!("compliance-{timestamp}-{attempt}.json"))
            };
            match fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&candidate)
            {
                Ok(mut file) => {
                    file.write_all(payload.as_bytes())
                        .map_err(|source| ComplianceError::Io {
                            path: candidate,
                            source,
                        })?;
                    break;
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    attempt += 1;
                    continue;
                }
                Err(source) => {
                    return Err(ComplianceError::Io {
                        path: candidate,
                        source,
                    });
                }
            }
        }
        Ok(())
    }
}

fn open_log_file(path: &Path) -> Result<File, ComplianceError> {
    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|err| ComplianceError::Io {
            path: parent.to_path_buf(),
            source: err,
        })?;
    }
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)
        .map_err(|err| ComplianceError::Io {
            path: path.to_path_buf(),
            source: err,
        })?;
    Ok(file)
}

fn timestamp_string() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "1970-01-01T00:00:00Z".to_string())
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        net::{IpAddr, Ipv4Addr, SocketAddr},
    };

    use norito::json::{self, Value as JsonValue};
    use tempfile::tempdir;

    use super::*;
    use crate::capability::{
        GreaseEntry, KemAdvertisement, KemId, NegotiatedCapabilities, SignatureAdvertisement,
        SignatureId,
    };

    fn build_logger() -> (ComplianceLogger, std::path::PathBuf, tempfile::TempDir) {
        let temp = tempdir().expect("tempdir");
        let log_path = temp.path().join("compliance.jsonl");
        let mut config = ComplianceConfig {
            enable: true,
            log_path: Some(log_path.clone()),
            hash_salt_hex: None,
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

        let expected_hash = blake3::hash(remote.to_string().as_bytes())
            .to_hex()
            .to_string();
        assert_eq!(value["event"].as_str().unwrap(), "circuit_closed");
        assert_eq!(value["mode"].as_str().unwrap(), "entry");
        assert_eq!(value["remote_hash"].as_str().unwrap(), expected_hash);
        assert_eq!(value["circuit_id"].as_u64().unwrap(), 7);
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
        assert_eq!(open["channel_hex"].as_str().unwrap(), hex::encode(channel));
        assert_eq!(open["route_hex"].as_str().unwrap(), hex::encode(route));
        assert_eq!(open["stream_hex"].as_str().unwrap(), hex::encode(stream));
        assert_eq!(open["room_hex"].as_str().unwrap(), hex::encode(room));
        assert_eq!(open["access_kind"].as_str().unwrap(), "Authenticated");
        assert_eq!(open["padding_budget_ms"].as_u64().unwrap(), 50);
        let expected_exit_hash = blake3::hash(b"/dns/example.com/tcp/443")
            .to_hex()
            .to_string();
        assert_eq!(
            open["exit_multiaddr_hash"].as_str().unwrap(),
            expected_exit_hash
        );
        let expected_target_hash = blake3::hash(b"wss://example.com/socket")
            .to_hex()
            .to_string();
        assert_eq!(
            open["adapter_target_hash"].as_str().unwrap(),
            expected_target_hash
        );

        assert_eq!(reject["event"].as_str().unwrap(), "exit_route_rejected");
        assert_eq!(reject["stream"].as_str().unwrap(), "kaigi-stream");
        assert_eq!(reject["channel"].as_str().unwrap(), "deadbeef");
        assert_eq!(reject["reason"].as_str().unwrap(), "route missing");
    }

    #[test]
    fn spool_writes_handshake_payload() {
        let temp = tempdir().expect("tempdir");
        let log_path = temp.path().join("compliance.jsonl");
        let spool_dir = temp.path().join("spool");
        let mut config = ComplianceConfig {
            enable: true,
            log_path: Some(log_path),
            hash_salt_hex: None,
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
        let expected_hash = blake3::hash(remote.to_string().as_bytes())
            .to_hex()
            .to_string();
        assert_eq!(value["remote_hash"].as_str().unwrap(), expected_hash);
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

        let expected_remote_hash = blake3::hash(remote.to_string().as_bytes())
            .to_hex()
            .to_string();
        assert_eq!(accepted["event"].as_str(), Some("bandwidth_proof"));
        assert_eq!(accepted["mode"].as_str(), Some("entry"));
        assert_eq!(
            accepted["remote_hash"].as_str(),
            Some(expected_remote_hash.as_str())
        );
        let measurement_hex = hex::encode(measurement);
        let relay_hex = hex::encode(relay_id);
        assert_eq!(
            accepted["measurement_hex"].as_str(),
            Some(measurement_hex.as_str())
        );
        assert_eq!(accepted["relay_hex"].as_str(), Some(relay_hex.as_str()));
        assert_eq!(accepted["epoch"].as_u64(), Some(42));
        assert_eq!(accepted["verified_bytes"].as_str(), Some("9765625"));
        assert!(accepted["reason"].is_null());
        assert!(accepted["accepted"].as_bool().unwrap());
        let verifier_hash = blake3::hash(verifier.as_bytes()).to_hex().to_string();
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
    #[error("invalid compliance configuration: {0}")]
    Config(String),
}
